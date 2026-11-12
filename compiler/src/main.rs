//! NOVA compiler command line.
//!
//! The compiler lowers NOVA source to C and can hand that C to the system C
//! compiler to produce a runnable executable.

use nova_backend_js::JsBackend;
use nova_backend_rust::RustBackend;
use nova_compiler::runtime;
use nova_compiler::{CBackend, Lexer, Lowerer, Parser, SemanticAnalyzer};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "\
NOVA compiler

USAGE:
    nova-compiler <source.nova>                  print the generated C
    nova-compiler emit <source.nova> [-o FILE]   write the generated C to FILE
    nova-compiler build <source.nova> [-o PATH]  compile and link an executable
    nova-compiler run <source.nova> [-- ARGS...] build, then run the executable

OPTIONS:
    -o, --output <path>   output path (default: the source file's stem)
        --target <name>   backend to emit: c (default), js, or rust
    -h, --help            show this help

Generated C calls the NOVA runtime (`nova_runtime.h` / `nova_runtime.c`).
`emit -o` writes those beside the C; `build` and `run` link them in.
`--target js` and `--target rust` emit source for those languages; only the C
backend has a build/run path.
";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None => {
            print!("{USAGE}");
            exit(1);
        }
        Some("-h") | Some("--help") | Some("help") => print!("{USAGE}"),
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
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Target::C => "c",
            Target::JavaScript => "js",
            Target::Rust => "rust",
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
                    Some(other) => die(&format!(
                        "unknown target `{other}`: expected `c`, `js`, or `rust`"
                    )),
                    None => die("`--target` needs a name"),
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
    let name = input.display();

    let mut lexer = Lexer::new(&source);
    let (tokens, diagnostics) = lexer.tokenize();
    if diagnostics.has_errors() {
        eprintln!("{name}: lexical errors:");
        for diag in diagnostics.iter() {
            eprintln!("  {}", diag.message);
        }
        die("lexical analysis failed");
    }

    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => program,
        Err(diagnostics) => {
            eprintln!("{name}: parse errors:");
            for diag in diagnostics.iter() {
                eprintln!("  {}", diag.message);
            }
            die("parsing failed");
        }
    };

    // `use name;` pulls the module's declarations into this program.
    let program = match nova_compiler::modules::load_modules(program, input) {
        Ok(program) => program,
        Err(message) => die(&message),
    };

    let mut analyzer = SemanticAnalyzer::new();
    if analyzer.analyze(&program).is_err() {
        eprintln!("{name}: semantic errors:");
        for diag in analyzer.diagnostics.iter() {
            eprintln!("  {}", diag.message);
        }
        die("semantic analysis failed");
    }

    let mut lowerer = Lowerer::new();
    let sir = lowerer.lower_program(&program);
    match target {
        Target::C => CBackend::new().generate(&sir),
        Target::JavaScript => JsBackend::new().generate(&sir),
        Target::Rust => RustBackend::new().generate(&sir),
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
            eprintln!("✓ wrote {}", path.display());
        }
        None => {
            print!("{c_code}");
            if target == Target::C {
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
    let result = Command::new(&cc)
        .arg(&c_path)
        .arg(&runtime_c)
        .arg("-o")
        .arg(&binary)
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

fn default_stem(input: &Path) -> String {
    input
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a.out".to_string())
}

fn die(message: &str) -> ! {
    eprintln!("error: {message}");
    exit(1);
}
