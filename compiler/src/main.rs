//! NOVA compiler command line.
//!
//! The compiler lowers NOVA source to C and can hand that C to the system C
//! compiler to produce a runnable executable.

use nova_backend_js::JsBackend;
use nova_backend_rust::RustBackend;
use nova_backend_wasm::WasmBackend;
use nova_compiler::runtime;
use nova_compiler::{CBackend, Lexer, Lowerer, Parser, SemanticAnalyzer};
use nova_common::{escape_json_string, Diagnostic};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

const USAGE: &str = "\
NOVA compiler

USAGE:
    nova-compiler <source.nova>                  print the generated C
    nova-compiler emit <source.nova> [-o FILE]   write the generated C to FILE
    nova-compiler build <source.nova> [-o PATH]  compile and link an executable
    nova-compiler run <source.nova> [-- ARGS...] build, then run the executable
    nova-compiler fmt <source.nova> [-o FILE]    format it; bodies are left untouched
    nova-compiler fmt <source.nova> --check      exit 1 if it is not formatted
    nova-compiler test --parity <source.nova>  run one program on every backend and compare
    nova-compiler doctor [--json]              check toolchains and backend readiness
    nova-compiler explain [Exxxx]              long form of an error code, or list them
    nova-compiler version                      print the compiler version

OPTIONS:
    -o, --output <path>   output path (default: the source file's stem)
        --module-root <dir>  also search <dir> for `use`d modules; repeatable.
                          `nova-pkg` passes one per dependency.
        --target <name>   backend to emit: c (default), js, rust, or wasm
        --error-format <f>  human (default) or json — one JSON object per
                            diagnostic on stdout, nothing on stderr
    -V, --version         print the compiler version
    -h, --help            show this help

Generated C calls the NOVA runtime (`nova_runtime.h` / `nova_runtime.c`).
`emit -o` writes those beside the C; `build` and `run` link them in.
`--target js|rust|wasm` emits source (WebAssembly text format); only the C backend
has a build/run path. `test --parity` runs all four when `node`, `rustc` and
`wat2wasm` are installed (WASM is reported `held` otherwise).
";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None => {
            print!("{USAGE}");
            exit(1);
        }
        Some("-h") | Some("--help") | Some("help") => print!("{USAGE}"),
        Some("-V") | Some("--version") | Some("version") => {
            reject_extra_args(&args[1..], "version");
            println!("nova-compiler {}", env!("CARGO_PKG_VERSION"));
        }
        Some("fmt") | Some("format") => {
            let (input, output, _target, rest) = parse_args(&args[1..]);
            reject_extra_args(&rest, "fmt");
            fmt_command(&input, output.as_deref());
        }
        Some("emit") => {
            let (input, output, target, rest) = parse_args(&args[1..]);
            reject_extra_args(&rest, "emit");
            emit(&input, output.as_deref(), target);
        }
        Some("build") => {
            let (input, output, target, rest) = parse_args(&args[1..]);
            reject_extra_args(&rest, "build");
            require_c(target, "build");
            build(&input, output.as_deref());
        }
        Some("run") => {
            let (input, output, target, rest) = parse_args(&args[1..]);
            require_c(target, "run");
            run(&input, output.as_deref(), &rest);
        }
        Some("explain") => {
            explain(args.get(1).map(String::as_str));
        }
        Some("doctor") => {
            let mut json = false;
            for argument in &args[1..] {
                match argument.as_str() {
                    "--json" => json = true,
                    "-h" | "--help" => {
                        print!("{USAGE}");
                        exit(0);
                    }
                    other => die(&format!("unexpected argument `{other}` for `doctor`")),
                }
            }
            doctor(json);
        }
        Some("test") => {
            let mut parity = false;
            let mut input: Option<PathBuf> = None;
            for argument in &args[1..] {
                match argument.as_str() {
                    "--parity" => parity = true,
                    "-h" | "--help" => {
                        print!("{USAGE}");
                        exit(0);
                    }
                    flag if flag.starts_with('-') => {
                        die(&format!("unknown option `{flag}` for `test`"))
                    }
                    path => {
                        if input.is_some() {
                            die(&format!("unexpected argument `{path}` for `test`"));
                        }
                        input = Some(PathBuf::from(path));
                    }
                }
            }
            let Some(input) = input else {
                die("`test --parity` needs a source file");
            };
            if !parity {
                die("`test` currently requires `--parity`");
            }
            parity_harness(&input);
        }
        Some(flag) if flag.starts_with('-') => die(&format!("unknown option `{flag}`")),
        // Bare path: keep the original behaviour of printing the generated C.
        Some(_) => {
            let (input, output, target, _) = parse_args(&args);
            emit(&input, output.as_deref(), target);
        }
    }
}

/// A backend the CLI can emit from SIR.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    C,
    JavaScript,
    Rust,
    WebAssembly,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Target::C => "c",
            Target::JavaScript => "js",
            Target::Rust => "rust",
            Target::WebAssembly => "wasm",
        }
    }
}

/// `build` and `run` compile C; other targets only emit source.
fn require_c(target: Target, command: &str) {
    if target != Target::C {
        die(&format!(
            "`{command}` only supports `--target c`; use `emit --target {}` for {} source",
            target.name(),
            target.name()
        ));
    }
}

/// Parse `<source> [-o PATH] [--target c|js|rust] [-- args...]`.
// ============================================================================
// Diagnostics as a contract
// ============================================================================

/// Set by `--error-format=json`.
///
/// An `AtomicBool` rather than a parameter threaded through every call site: the
/// CLI is single-threaded and the flag is global by nature, so a parameter would
/// touch six functions to carry one bit. This is the same reasoning that got
/// `static mut` removed from the runtime — an atomic is the sound version of the
/// thing that was wanted.
static JSON_DIAGNOSTICS: AtomicBool = AtomicBool::new(false);

/// Set by `fmt --check`: report whether the file is formatted, and change nothing.
static CHECK_ONLY: AtomicBool = AtomicBool::new(false);

/// Set by `--module-root`, repeatable. A dependency's source root arrives this way:
/// `nova-pkg` reads `nova.toml` and passes one flag per dependency, so the compiler
/// never reads a manifest and never fetches anything.
static MODULE_ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn module_roots() -> Vec<PathBuf> {
    MODULE_ROOTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn json_diagnostics() -> bool {
    JSON_DIAGNOSTICS.load(Ordering::SeqCst)
}

fn json_quote(s: &str) -> String {
    format!("\"{}\"", escape_json_string(s))
}

fn json_string_array(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| json_quote(item)).collect();
    format!("[{}]", quoted.join(","))
}

/// One diagnostic, as one JSON object on one line.
///
/// A line-per-object shape rather than one big array: an editor can parse it
/// incrementally, and a truncated stream still leaves the diagnostics that
/// arrived. `code` and `span` are `null` rather than absent, so a consumer does
/// not have to distinguish "missing" from "no value".
fn diagnostic_json(file: &str, diag: &Diagnostic) -> String {
    let span = match &diag.span {
        Some(span) => format!(
            "{{\"line_start\":{},\"column_start\":{},\"line_end\":{},\"column_end\":{}}}",
            span.start.line, span.start.column, span.end.line, span.end.column
        ),
        None => "null".to_string(),
    };
    let code = match &diag.code {
        Some(code) => json_quote(code),
        None => "null".to_string(),
    };
    format!(
        "{{\"level\":{},\"code\":{},\"message\":{},\"file\":{},\"span\":{},\"notes\":{},\"helps\":{}}}",
        json_quote(&diag.level.to_string()),
        code,
        json_quote(&diag.message),
        json_quote(file),
        span,
        json_string_array(&diag.notes),
        json_string_array(&diag.helps),
    )
}

/// Print one diagnostic, in whichever format was asked for.
fn emit_diagnostic(file: &str, diag: &Diagnostic) {
    if json_diagnostics() {
        println!("{}", diagnostic_json(file, diag));
        return;
    }
    let code = match &diag.code {
        Some(code) => format!("[{code}] "),
        None => String::new(),
    };
    match &diag.span {
        Some(span) => eprintln!(
            "  {file}:{}:{}: {code}{}",
            span.start.line, span.start.column, diag.message
        ),
        None => eprintln!("  {file}: {code}{}", diag.message),
    }
    for note in &diag.notes {
        eprintln!("    note: {note}");
    }
    for help in &diag.helps {
        eprintln!("    help: {help}");
    }
}

/// The error codes the compiler can emit, with the long form `explain` prints.
///
/// This table is the source of truth. `docs/errors.md` is checked against it by
/// `tools/error-code-check.py`, so the page cannot list a code the compiler does
/// not emit, or omit one it does.
const ERROR_CODES: &[(&str, &str, &str)] = &[
    (
        "E0010",
        "value used after being moved",
        "A value that is not `Copy` was passed to a function and then used again.\n\n\
         `int`, `float`, `bool`, `char` and references are copied when they are passed;\n\
         `String`, arrays, structs and closures are not — they own a buffer, and passing\n\
         one gives it away. This is a warning while the rule is new (ADR 0017): the\n\
         number of places it fires is being counted before it is enforced.",
    ),
    (
        "E0002",
        "undefined name",
        "A name was used that nothing has declared in scope.\n\n\
         Usually a typo, or a `use`/`mod` that did not pull in the module holding\n\
         the name. NOVA resolves across a whole file, so a function declared later\n\
         in the same file is visible.",
    ),
    (
        "E0061",
        "wrong number of arguments",
        "A call passes a different number of arguments than the callee declares.\n\n\
         The `help:` line prints the full parameter list, which is the fastest way\n\
         to see whether an argument is missing or the wrong function is being\n\
         called. Generic calls are checked against the instantiation at the call\n\
         site.",
    ),
    (
        "E0308",
        "mismatched types",
        "One code covers four situations, distinguished by the message:\n\n\
         - `argument N of f is X, but the parameter is Y` — an argument's type\n\
         - `function f returns X, but this return value is Y` — a `return expr`\n\
         - `function f returns X, but this return has no value` — a bare `return`\n\
         - `function f returns X, but its final expression is Y` — a block value\n\n\
         `int` -> `float` is the only implicit conversion. Everything else is\n\
         written out with `as`.",
    ),
];

fn explain(code: Option<&str>) -> ! {
    match code {
        None => {
            println!("Error codes this compiler can emit:\n");
            for (code, title, _) in ERROR_CODES {
                println!("  {code}  {title}");
            }
            println!("\n`nova-compiler explain Exxxx` prints the long form.");
            exit(0);
        }
        Some(wanted) => {
            let wanted = wanted.trim();
            for (code, title, body) in ERROR_CODES {
                if code.eq_ignore_ascii_case(wanted) {
                    println!("{code} — {title}\n");
                    println!("{body}");
                    exit(0);
                }
            }
            let known: Vec<&str> = ERROR_CODES.iter().map(|(code, _, _)| *code).collect();
            die(&format!(
                "unknown error code `{wanted}`: known codes are {}",
                known.join(", ")
            ));
        }
    }
}

fn parse_args(args: &[String]) -> (PathBuf, Option<PathBuf>, Target, Vec<String>) {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut target = Target::C;
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(path) => output = Some(PathBuf::from(path)),
                    None => die("`-o` needs a path"),
                }
            }
            "--target" => {
                i += 1;
                match args.get(i).map(String::as_str) {
                    Some("c") => target = Target::C,
                    Some("js") | Some("javascript") => target = Target::JavaScript,
                    Some("rust") => target = Target::Rust,
                    Some("wasm") | Some("wat") => target = Target::WebAssembly,
                    Some(other) => die(&format!(
                        "unknown target `{other}`: expected `c`, `js`, `rust`, or `wasm`"
                    )),
                    None => die("`--target` needs a name"),
                }
            }
            "--error-format" => {
                i += 1;
                match args.get(i).map(String::as_str) {
                    Some("json") => JSON_DIAGNOSTICS.store(true, Ordering::SeqCst),
                    Some("human") => JSON_DIAGNOSTICS.store(false, Ordering::SeqCst),
                    Some(other) => die(&format!(
                        "unknown error format `{other}`: expected `human` or `json`"
                    )),
                    None => die("`--error-format` needs a value"),
                }
            }
            flag if flag.starts_with("--error-format=") => {
                match &flag["--error-format=".len()..] {
                    "json" => JSON_DIAGNOSTICS.store(true, Ordering::SeqCst),
                    "human" => JSON_DIAGNOSTICS.store(false, Ordering::SeqCst),
                    other => die(&format!(
                        "unknown error format `{other}`: expected `human` or `json`"
                    )),
                }
            }
            "--check" => CHECK_ONLY.store(true, Ordering::SeqCst),
            "--module-root" => {
                i += 1;
                match args.get(i) {
                    Some(path) => MODULE_ROOTS
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .push(PathBuf::from(path)),
                    None => die("`--module-root` needs a directory"),
                }
            }
            "--" => {
                rest.extend(args[i + 1..].iter().cloned());
                break;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                exit(0);
            }
            flag if flag.starts_with('-') && flag.len() > 1 => {
                die(&format!("unknown option `{flag}`"));
            }
            path => {
                if input.is_some() {
                    rest.push(path.to_string());
                } else {
                    input = Some(PathBuf::from(path));
                }
            }
        }
        i += 1;
    }

    match input {
        Some(input) => (input, output, target, rest),
        None => {
            eprintln!("error: no source file given\n");
            print!("{USAGE}");
            exit(1);
        }
    }
}

fn reject_extra_args(rest: &[String], command: &str) {
    if let Some(extra) = rest.first() {
        die(&format!("unexpected argument `{extra}` for `{command}`"));
    }
}

/// Run the whole pipeline and return the generated source for `target`.
fn compile_to_c(input: &Path) -> String {
    compile_to(input, Target::C)
}

fn compile_to(input: &Path, target: Target) -> String {
    let source = fs::read_to_string(input)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", input.display())));
    let name = input.display().to_string();

    let mut lexer = Lexer::new(&source);
    let (tokens, diagnostics) = lexer.tokenize();
    if diagnostics.has_errors() {
        if !json_diagnostics() {
            eprintln!("{name}: lexical errors:");
        }
        for diag in diagnostics.iter() {
            emit_diagnostic(&name, diag);
        }
        die("lexical analysis failed");
    }

    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => program,
        Err(diagnostics) => {
            if !json_diagnostics() {
                eprintln!("{name}: parse errors:");
            }
            for diag in diagnostics.iter() {
                emit_diagnostic(&name, diag);
            }
            die("parsing failed");
        }
    };

    // `use name;` pulls the module's declarations into this program.
    let program = match nova_compiler::modules::load_modules(program, input, &module_roots()) {
        Ok(program) => program,
        Err(message) => die(&message),
    };

    let mut analyzer = SemanticAnalyzer::new();
    if analyzer.analyze(&program).is_err() {
        if !json_diagnostics() {
            eprintln!("{name}: semantic errors:");
        }
        for diag in analyzer.diagnostics.iter() {
            emit_diagnostic(&name, diag);
        }
        die("semantic analysis failed");
    }

    // A successful analysis can still have warnings, and they used to be dropped here: only a
    // failed analysis printed anything, so a warning was invisible on exactly the programs it is
    // about. ADR 0017 step 3 counts these, and a count needs them printed.
    //
    // **Only the program's own.** An imported module is merged into the same tree, so its spans
    // run past the end of the file the user named — measured: a 12-line program produced a
    // warning at line 543, which is `text.nova`. A user cannot act on a warning about code they
    // did not write, and 26 of the corpus's 28 move warnings were exactly that. The bound is the
    // same one the C backend uses for `#line`: a line past the end of the named file is not from
    // that file.
    let own_lines = std::fs::read_to_string(&name)
        .map(|text| text.lines().count())
        .unwrap_or(0);
    let mut suppressed = 0usize;
    for diag in analyzer.diagnostics.iter() {
        if !matches!(diag.level, nova_common::DiagnosticLevel::Warning) {
            continue;
        }
        let from_own_file = match &diag.span {
            Some(span) => own_lines == 0 || span.start.line <= own_lines,
            None => true,
        };
        if from_own_file {
            emit_diagnostic(&name, diag);
        } else {
            suppressed += 1;
        }
    }
    if suppressed > 0 && !json_diagnostics() {
        eprintln!(
            "{suppressed} warning(s) in imported modules were not shown; they are not your code"
        );
    }

    let mut lowerer = Lowerer::new();
    let sir = lowerer.lower_program(&program);
    match target {
        // The C backend is told the source file so it can emit `#line`, which the host
        // compiler turns into a line table. The other backends have no line mapping yet.
        Target::C => {
            let lines = std::fs::read_to_string(&name)
                .map(|text| text.lines().count())
                .unwrap_or(0);
            CBackend::new().generate_with_source(&sir, Some((&name, lines)))
        }
        Target::JavaScript => JsBackend::new().generate(&sir),
        Target::Rust => RustBackend::new().generate(&sir),
        Target::WebAssembly => {
            if let Err(feature) = nova_backend_wasm::validate(&sir) {
                die(&format!(
                    "the wasm target does not support {feature} yet; use --target c, js or rust"
                ));
            }
            WasmBackend::new().generate(&sir)
        }
    }
}

/// `fmt`: rewrite the space between items, and nothing else.
///
/// The formatter is total — it never parses, so it cannot fail — which means it
/// works on a file that is half-typed. That is the point: a formatter you cannot
/// run on broken code is a formatter you cannot run while editing.
fn fmt_command(input: &Path, output: Option<&Path>) {
    let source = fs::read_to_string(input)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", input.display())));
    let formatted = nova_compiler::fmt::format(&source);

    if CHECK_ONLY.load(Ordering::SeqCst) {
        if formatted == source {
            exit(0);
        }
        eprintln!("{}: not formatted", input.display());
        exit(1);
    }

    if formatted == source {
        eprintln!("{} is already formatted", input.display());
        return;
    }

    match output {
        Some(path) => {
            fs::write(path, &formatted)
                .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
            eprintln!("✓ wrote {}", path.display());
        }
        None => {
            fs::write(input, &formatted)
                .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", input.display())));
            eprintln!("✓ formatted {}", input.display());
        }
    }
}

/// `emit`: print the generated source, or write it to `-o`.
fn emit(input: &Path, output: Option<&Path>, target: Target) {
    let c_code = compile_to(input, target);
    match output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)
                        .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", parent.display())));
                }
            }
            fs::write(path, &c_code)
                .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
            if target == Target::C {
                write_runtime_for(path);
            }
            // Suppressed in JSON mode: there, stdout is the channel a consumer
            // parses and stderr should be empty, so nothing but JSON reaches a
            // parser.
            if !json_diagnostics() {
                eprintln!("✓ wrote {}", path.display());
            }
        }
        None => {
            print!("{c_code}");
            if target == Target::C && !json_diagnostics() {
                eprintln!(
                    "note: the generated C calls the NOVA runtime; \
                     `emit -o <file.c>` writes {} and {} beside it, and `build` links them",
                    runtime::HEADER_FILE,
                    runtime::SOURCE_FILE
                );
            }
        }
    }
}

/// Write the runtime header and source beside the generated C, returning the
/// path of the runtime source to link.
fn write_runtime_for(c_path: &Path) -> PathBuf {
    let dir = match c_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let header = dir.join(runtime::HEADER_FILE);
    let source = dir.join(runtime::SOURCE_FILE);
    fs::write(&header, runtime::HEADER)
        .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", header.display())));
    fs::write(&source, runtime::SOURCE)
        .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", source.display())));
    source
}

/// `build`: write the C beside the executable and link it, returning the binary path.
fn build(input: &Path, output: Option<&Path>) -> PathBuf {
    let c_code = compile_to_c(input);
    let binary = match output {
        Some(path) => path.to_path_buf(),
        None => PathBuf::from(default_stem(input)),
    };
    let c_path = binary.with_extension("c");

    if let Some(parent) = binary.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", parent.display())));
        }
    }
    fs::write(&c_path, &c_code)
        .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", c_path.display())));
    let runtime_c = write_runtime_for(&c_path);

    let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let mut link = Command::new(&cc);
    // `-g` is what turns the `#line` directives in the generated C into a `.debug_line`
    // section. Without it those directives are inert and the debugger has nothing to read,
    // so a NOVA build produces debug info by default — the cost is binary size, and the
    // alternative is a debugger that cannot work on any program the compiler produced.
    link.arg("-g");
    // A frame pointer in every frame, so a debugger can walk the stack at all. Without it the
    // compiler is free to use `rbp` as a general register and a backtrace has nothing to follow
    // — the same reason `gdb` recommends this flag. The cost is one register and a few
    // instructions per call; the alternative is a `backtrace` that refuses on every program.
    link.arg("-fno-omit-frame-pointer");
    link.arg(&c_path).arg(&runtime_c).arg("-o").arg(&binary);
    // The runtime shim calls `sqrt`, `sin` and friends, which live in libm on every
    // Unix. macOS folds them into libSystem and ignores `-lm`; MSVC has no `-lm` and
    // does not need one, so the flag is added only where it is both needed and valid.
    if !cfg!(windows) {
        link.arg("-lm");
    }
    let result = link
        .output()
        .unwrap_or_else(|e| {
            die(&format!(
                "could not run the C compiler `{cc}`: {e}\n\
                 install a C toolchain (gcc or clang) or set CC to its path"
            ))
        });

    if !result.status.success() {
        eprint!("{}", String::from_utf8_lossy(&result.stderr));
        die(&format!("`{cc}` failed to compile {}", c_path.display()));
    }

    eprintln!("✓ built {} (C: {})", binary.display(), c_path.display());
    binary
}

/// How one backend fared in the parity harness.
struct ParityOutcome {
    backend: &'static str,
    result: std::result::Result<BackendRun, String>,
}

struct BackendRun {
    stdout: String,
    status: i32,
}

/// `test --parity`: run one program on every backend and compare stdout/status.
///
/// C is the reference unless `<stem>.expected` sits beside the source. A backend
/// whose toolchain is missing, or WebAssembly (no executor is installed), is
/// reported as `held` rather than silently skipped.
fn parity_harness(input: &Path) -> ! {
    let dir = env::temp_dir().join(format!("nova-parity-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|e| die(&format!("cannot create {}: {e}", dir.display())));

    let expected = fs::read_to_string(input.with_extension("expected")).ok();
    let mut outcomes = Vec::new();

    // C reference.
    let c_result = (|| -> std::result::Result<BackendRun, String> {
        let binary = dir.join("parity-c");
        let binary = build(input, Some(&binary));
        let output = Command::new(&binary)
            .output()
            .map_err(|e| format!("cannot run the C binary: {e}"))?;
        Ok(BackendRun {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            status: output.status.code().unwrap_or(1),
        })
    })();
    outcomes.push(ParityOutcome { backend: "c", result: c_result });

    let (reference_stdout, reference_status) = match &outcomes[0].result {
        Ok(run) => (run.stdout.clone(), run.status),
        Err(message) => die(&format!("parity: C reference failed: {message}")),
    };
    let expected_stdout = expected.as_deref().unwrap_or(&reference_stdout);

    // JavaScript through Node.
    outcomes.push(ParityOutcome {
        backend: "js",
        result: run_source_backend(input, &dir, Target::JavaScript, "node"),
    });
    // Rust through rustc.
    outcomes.push(ParityOutcome {
        backend: "rust",
        result: run_source_backend(input, &dir, Target::Rust, "rustc"),
    });
    // WebAssembly text needs an assembler and a host; neither is guaranteed.
    outcomes.push(ParityOutcome {
        backend: "wasm",
        result: run_wasm_backend(input, &dir),
    });

    println!("parity: {}", input.display());
    let mut held = 0;
    for outcome in &outcomes {
        match &outcome.result {
            Ok(run) => {
                let stdout_ok = run.stdout == expected_stdout;
                let status_ok = run.status == reference_status;
                let state = if stdout_ok && status_ok { "pass" } else { "FAIL" };
                if state == "FAIL" {
                    held += 1;
                }
                let mut notes = Vec::new();
                if !stdout_ok {
                    notes.push(format!(
                        "stdout differs (expected {:?}, got {:?})",
                        expected_stdout, run.stdout
                    ));
                }
                if !status_ok {
                    notes.push(format!(
                        "exit {} (C exited {})",
                        run.status,                         reference_status
                    ));
                }
                if notes.is_empty() {
                    println!("  {:<5} {:<4} exit {}", outcome.backend, state, run.status);
                } else {
                    println!("  {:<5} {:<4} {}", outcome.backend, state, notes.join("; "));
                }
            }
            Err(message) => {
                held += 1;
                println!("  {:<5} held {}", outcome.backend, message);
            }
        }
    }

    if held > 0 {
        eprintln!("parity: FAILED ({held} backend(s) not passing)");
        exit(1);
    }
    eprintln!("parity: ok (all 4 backends)");
    exit(0);
}

/// Emit `target` source, compile it with `tool` (Node runs source directly;
/// rustc compiles first), and run the program.
fn run_source_backend(
    input: &Path,
    dir: &Path,
    target: Target,
    tool: &str,
) -> std::result::Result<BackendRun, String> {
    let available = Command::new(tool)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !available {
        return Err(format!("`{tool}` is not installed"));
    }

    let extension = if target == Target::JavaScript { "js" } else { "rs" };
    let source_path = dir.join(format!("parity.{extension}"));
    let source = compile_to(input, target);
    fs::write(&source_path, source)
        .map_err(|e| format!("cannot write {}: {e}", source_path.display()))?;

    let output = if target == Target::JavaScript {
        Command::new("node")
            .arg(&source_path)
            .output()
            .map_err(|e| format!("cannot run node: {e}"))?
    } else {
        let binary = dir.join("parity-rust");
        let compiled = Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(&binary)
            .output()
            .map_err(|e| format!("cannot run rustc: {e}"))?;
        if !compiled.status.success() {
            return Err(format!(
                "rustc failed: {}",
                String::from_utf8_lossy(&compiled.stderr).trim()
            ));
        }
        Command::new(&binary)
            .output()
            .map_err(|e| format!("cannot run the Rust binary: {e}"))?
    };

    Ok(BackendRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        status: output.status.code().unwrap_or(1),
    })
}

/// Assemble the WAT with `wat2wasm` and run it on the committed Node host.
///
/// Both tools are optional; when either is missing the backend is reported as
/// `held` rather than silently skipped or counted as passing.
fn run_wasm_backend(
    input: &Path,
    dir: &Path,
) -> std::result::Result<BackendRun, String> {
    let assembler = env::var("WAT2WASM").unwrap_or_else(|_| "wat2wasm".to_string());
    if Command::new(&assembler).output().is_err() {
        return Err(format!(
            "`{assembler}` is not installed (set WAT2WASM to its path)"
        ));
    }
    let node_available = Command::new("node")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !node_available {
        return Err("`node` is not installed".to_string());
    }

    let wat_path = dir.join("parity.wat");
    let wasm_path = dir.join("parity.wasm");
    fs::write(&wat_path, compile_to(input, Target::WebAssembly))
        .map_err(|e| format!("cannot write {}: {e}", wat_path.display()))?;

    let assembled = Command::new(&assembler)
        .arg(&wat_path)
        .arg("-o")
        .arg(&wasm_path)
        .output()
        .map_err(|e| format!("cannot run `{assembler}`: {e}"))?;
    if !assembled.status.success() {
        return Err(format!(
            "wat2wasm failed: {}",
            String::from_utf8_lossy(&assembled.stderr).trim()
        ));
    }

    let host = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("backends")
        .join("wasm")
        .join("host.mjs");
    let output = Command::new("node")
        .arg(&host)
        .arg(&wasm_path)
        .output()
        .map_err(|e| format!("cannot run the WASM host: {e}"))?;

    Ok(BackendRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        status: output.status.code().unwrap_or(1),
    })
}

/// `run`: build into a scratch directory (unless `-o` was given), then run the
/// program with inherited stdio and the program's own exit status.
fn run(input: &Path, output: Option<&Path>, args: &[String]) -> ! {
    let binary = match output {
        Some(path) => build(input, Some(path)),
        None => build(input, Some(&scratch_binary(input))),
    };

    let status = Command::new(&binary).args(args).status().unwrap_or_else(|e| {
        die(&format!("could not run {}: {e}", binary.display()));
    });
    exit(status.code().unwrap_or(1));
}

/// Build artifacts for `run` go to a per-process temp directory, so a plain
/// `nova-compiler run file.nova` leaves nothing behind in the source tree.
fn scratch_binary(input: &Path) -> PathBuf {
    let dir = env::temp_dir().join(format!("nova-run-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|e| {
        die(&format!("cannot create {}: {e}", dir.display()));
    });
    dir.join(default_stem(input))
}

/// One external command the compiler can use.
struct ToolStatus {
    name: &'static str,
    command: String,
    available: bool,
    version: Option<String>,
    hint: &'static str,
}

impl ToolStatus {
    fn detect(name: &'static str, command: String, version_args: &[&str], hint: &'static str) -> Self {
        let output = Command::new(&command).args(version_args).output();
        let (available, version) = match output {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                let line = text.lines().next().unwrap_or("").trim();
                (true, (!line.is_empty()).then(|| line.chars().take(72).collect()))
            }
            // The npm `wat2wasm` wrapper exits non-zero without arguments but
            // still proves the command can be spawned.
            Ok(_) if name == "wat2wasm" => (true, None),
            Ok(_) => (false, None),
            Err(_) => (false, None),
        };
        Self { name, command, available, version, hint }
    }
}

/// `doctor`: report the tools the compiler can use and which backends are ready.
fn doctor(json: bool) -> ! {
    let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let tools = [
        ToolStatus::detect("cc", cc, &["--version"], "install gcc or clang, or set CC"),
        ToolStatus::detect("node", "node".to_string(), &["--version"], "install Node.js for JS and WASM parity"),
        ToolStatus::detect("rustc", "rustc".to_string(), &["--version"], "install Rust for the Rust backend"),
        ToolStatus::detect("wat2wasm", "wat2wasm".to_string(), &[], "npm install --global wat2wasm"),
    ];

    let available = |name: &str| tools.iter().any(|tool| tool.name == name && tool.available);

    if json {
        print_doctor_json(&tools);
        exit(if available("cc") { 0 } else { 1 });
    }

    println!("NOVA doctor\n\ntools");
    for tool in &tools {
        let state = if tool.available { "ok" } else { "missing" };
        println!("  {:<9} {:<8} {}", tool.name, state, tool.command);
        if let Some(version) = &tool.version {
            println!("            {version}");
        }
        if !tool.available {
            println!("            {}", tool.hint);
        }
    }

    let backends = [
        ("c", available("cc"), "build and run"),
        ("js", available("node"), "emit and parity"),
        ("rust", available("rustc"), "emit and parity"),
        ("wasm", available("node") && available("wat2wasm"), "emit and parity"),
    ];

    println!("\nbackends");
    for (name, ready, use_case) in backends {
        let state = if ready { "ready" } else { "unavailable" };
        println!("  {:<5} {:<12} {}", name, state, use_case);
    }

    if available("cc") {
        println!("\nrequired toolchain: ok");
        exit(0);
    }
    eprintln!("\nrequired toolchain: missing a C compiler; build and run cannot work");
    exit(1);
}

fn json_string(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for ch in text.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

/// A stable machine-readable form for editors and CI. Kept hand-written so the
/// compiler CLI gains no JSON dependency just for this report.
fn print_doctor_json(tools: &[ToolStatus]) {
    let available = |name: &str| tools.iter().any(|tool| tool.name == name && tool.available);
    println!("{{");
    println!("  \"tools\": [");
    for (index, tool) in tools.iter().enumerate() {
        let version = tool
            .version
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string());
        let comma = if index + 1 == tools.len() { "" } else { "," };
        println!(
            "    {{\"name\": {}, \"command\": {}, \"available\": {}, \"version\": {}}}{comma}",
            json_string(tool.name),
            json_string(&tool.command),
            tool.available,
            version
        );
    }
    println!("  ],");
    let backends = [
        ("c", available("cc")),
        ("js", available("node")),
        ("rust", available("rustc")),
        ("wasm", available("node") && available("wat2wasm")),
    ];
    println!("  \"backends\": [");
    for (index, (name, ready)) in backends.iter().enumerate() {
        let comma = if index + 1 == backends.len() { "" } else { "," };
        println!(
            "    {{\"name\": {}, \"ready\": {}}}{comma}",
            json_string(name),
            ready
        );
    }
    println!("  ],");
    println!("  \"ready\": {}", available("cc"));
    println!("}}");
}

fn default_stem(input: &Path) -> String {
    input
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a.out".to_string())
}

fn die(message: &str) -> ! {
    if json_diagnostics() {
        // A failure that is not a diagnostic still has to reach a machine reader:
        // in JSON mode stdout is the only channel, so nothing goes to stderr and
        // the exit status stays the signal for "this run failed".
        println!(
            "{{\"level\":\"error\",\"code\":null,\"message\":{},\"file\":null,\"span\":null,\"notes\":[],\"helps\":[]}}",
            json_quote(message)
        );
    } else {
        eprintln!("error: {message}");
    }
    exit(1);
}
