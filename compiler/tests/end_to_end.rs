//! End-to-end tests: NOVA source → C → native binary → stdout.
//!
//! These drive the real `nova-compiler` binary, so they cover lowering, C
//! codegen, the command-line front end and linking in one go. They need a host
//! C compiler; when none is installed they report that and pass, rather than
//! failing for a reason unrelated to the compiler.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn compiler() -> &'static str {
    env!("CARGO_BIN_EXE_nova-compiler")
}

fn cc() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
}

fn c_compiler_available() -> bool {
    Command::new(cc())
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn wat2wasm_available() -> bool {
    let tool = std::env::var("WAT2WASM").unwrap_or_else(|_| "wat2wasm".to_string());
    // The npm wrapper exits non-zero without arguments; being spawnable is what
    // matters here, so an `Ok` output means the tool exists.
    Command::new(tool).output().is_ok()
}

fn rustc_available() -> bool {
    Command::new("rustc")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate has a parent directory")
        .to_path_buf()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nova-e2e-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

#[test]
fn a_compiler_that_writes_nothing_is_not_a_success() {
    // The C compiler's exit status is a claim about itself, not about the file. A wrapper that
    // exits 0 without writing used to produce "✓ built <path>" for a binary that did not exist.
    // `/bin/true` is exactly that wrapper, and it is on every Unix.
    if !Path::new("/bin/true").exists() {
        return;
    }
    let dir = scratch("cc-lies");
    let source = dir.join("ok.nova");
    fs::write(&source, "use core;\nfn main() -> int { print(1); return 0; }\n")
        .expect("write source");
    let binary = dir.join("ok");

    let output = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .env("CC", "/bin/true")
        .output()
        .expect("run nova-compiler build");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a compiler that produced nothing was reported as success: {stderr}"
    );
    assert!(
        stderr.contains("reported success but did not produce"),
        "the failure did not name the missing file: {stderr}"
    );
    assert!(
        !stderr.contains("✓ built"),
        "the success message was printed anyway: {stderr}"
    );
    assert!(!binary.exists(), "no binary should have been produced");
}

#[test]
fn version_flag_and_subcommand_agree() {
    let expected = format!("nova-compiler {}", env!("CARGO_PKG_VERSION"));
    for arguments in [["--version"], ["-V"], ["version"]] {
        let output = Command::new(compiler())
            .args(arguments)
            .output()
            .expect("run nova-compiler version");
        assert!(
            output.status.success(),
            "`{arguments:?}` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn doctor_json_is_machine_readable() {
    let output = Command::new(compiler())
        .args(["doctor", "--json"])
        .output()
        .expect("run nova-compiler doctor --json");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(output.status.success(), c_compiler_available());
    assert!(stdout.trim_start().starts_with('{'), "not a JSON object:\n{stdout}");
    assert!(stdout.trim_end().ends_with('}'));
    assert!(stdout.contains("\"tools\"") && stdout.contains("\"backends\""));
    assert!(stdout.contains("\"ready\": true") || stdout.contains("\"ready\": false"));
    assert!(!stdout.contains("NOVA doctor"), "human text leaked into JSON:\n{stdout}");
}

#[test]
fn doctor_reports_tools_and_backends() {
    let output = Command::new(compiler())
        .arg("doctor")
        .output()
        .expect("run nova-compiler doctor");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.success(),
        c_compiler_available(),
        "doctor exit status should match C compiler availability:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for tool in ["cc", "node", "rustc", "wat2wasm"] {
        assert!(stdout.contains(tool), "doctor did not report `{tool}`:\n{stdout}");
    }
    for backend in ["c", "js", "rust", "wasm"] {
        assert!(
            stdout.lines().any(|line| line.trim_start().starts_with(&format!("{backend} "))),
            "doctor did not report the `{backend}` backend:\n{stdout}"
        );
    }
}

#[test]
fn hello_compiles_links_and_prints() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("hello");
    let source = dir.join("hello.nova");
    let binary = dir.join("hello");
    fs::write(&source, "print(\"Hello, NOVA\");\n").expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(binary.exists(), "no executable at {}", binary.display());

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(run.status.success(), "program exited with {}", run.status);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Hello, NOVA\n");
}

/// Compile a program that imports a standard-library module, run it, return stdout.
///
/// The module is reached through `--module-root` — the flag `nova-pkg` passes for a
/// dependency — because the scratch directory is outside the workspace and
/// `find_module` only walks ancestors for `stdlib/`.
fn run_with_stdlib(name: &str, source: &str) -> String {
    let dir = scratch(name);
    // Not `<name>.nova`: `find_module` looks beside the importing file first, so a
    // program called `math.nova` that says `use math;` imports itself.
    let file = dir.join(format!("{name}_program.nova"));
    let binary = dir.join(name);
    fs::write(&file, source).expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&file)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(
        run.status.success(),
        "program exited with {}:\nstderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

#[test]
fn every_complete_program_in_the_spec_compiles() {
    // A chapter is a test. Seven `nova` blocks in `docs/language/` contain `fn main`, which means
    // they claim to be whole programs — and two of them did not compile: the `panic` example was
    // missing its `use core;`, and the package example needs a dependency it does not have (that
    // one is annotated in the chapter rather than compiled).
    //
    // Fragments are skipped: a block without `fn main` is an excerpt, and compiling it would test
    // the extraction rather than the chapter.
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let spec = workspace_root().join("docs").join("language");
    let dir = scratch("spec-programs");
    let mut compiled = 0;
    let mut skipped = 0;

    for entry in fs::read_dir(&spec).expect("read docs/language") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read chapter");
        for (index, block) in nova_blocks(&text).into_iter().enumerate() {
            if !block.contains("fn main") {
                continue;
            }
            if block.contains("use lib;") {
                // The package chapter's example needs a dependency; it says so in the text.
                skipped += 1;
                continue;
            }
            let name = format!(
                "{}-{index}.nova",
                path.file_stem().and_then(|s| s.to_str()).unwrap_or("chapter")
            );
            let source = dir.join(&name);
            fs::write(&source, &block).expect("write program");
            let build = Command::new(compiler())
                .arg("build")
                .arg(&source)
                .arg("-o")
                .arg(dir.join(format!("{name}.bin")))
                .arg("--module-root")
                .arg(workspace_root().join("stdlib"))
                .output()
                .expect("run nova-compiler build");
            assert!(
                build.status.success(),
                "{}: a complete program in the spec does not compile:\n{}\n{}",
                path.display(),
                block,
                String::from_utf8_lossy(&build.stderr)
            );
            compiled += 1;
        }
    }

    assert!(compiled >= 5, "expected to compile several spec programs, did {compiled}");
    eprintln!("spec programs compiled: {compiled}, skipped: {skipped}");
}

/// Every ```nova fenced block in a Markdown document, in order.
fn nova_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```nova") {
            current = Some(String::new());
        } else if line.trim_start().starts_with("```") {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
        } else if let Some(block) = current.as_mut() {
            block.push_str(line);
            block.push('\n');
        }
    }
    blocks
}

#[test]
fn a_mangled_name_is_valid_in_every_target() {
    // ADR 0019 mangles a colliding name to `module::name`, and `::` is not an identifier
    // character in JavaScript, Rust or WebAssembly. The C backend mapped it; the other three
    // emitted `function alpha::shared(x)` — invalid JavaScript and invalid Rust — because the
    // mapping was written once, in one backend.
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let dir = scratch("mangled-targets");
    fs::write(
        dir.join("alpha.nova"),
        "fn shared(x: int) -> int {\n    x + 1\n}\n",
    )
    .expect("write alpha");
    fs::write(
        dir.join("beta.nova"),
        "fn shared(x: int, y: int) -> int {\n    x + y\n}\n",
    )
    .expect("write beta");
    let program = dir.join("program.nova");
    fs::write(
        &program,
        "use core;\nuse alpha;\nuse beta;\n\nfn main() -> int {\n    print(alpha.shared(1));\n    print(beta.shared(1, 2));\n    return 0;\n}\n",
    )
    .expect("write program");

    for (target, extension) in [("js", "js"), ("rust", "rs"), ("wasm", "wat")] {
        let output = dir.join(format!("out.{extension}"));
        let emit = Command::new(compiler())
            .arg("emit")
            .arg(&program)
            .arg("-o")
            .arg(&output)
            .arg("--target")
            .arg(target)
            .arg("--module-root")
            .arg(workspace_root().join("stdlib"))
            .output()
            .expect("run nova-compiler emit");
        assert!(
            emit.status.success(),
            "emit --target {target} failed:\n{}",
            String::from_utf8_lossy(&emit.stderr)
        );
        let text = fs::read_to_string(&output).expect("read emitted source");
        // `std::io` is a legal Rust path, so the check is on the mangled names rather than on
        // every `::` in the file.
        assert!(
            !text.contains("alpha::shared") && !text.contains("beta::shared"),
            "the {target} output kept `::` in a function name:\n{text}"
        );
        assert!(
            text.contains("alpha__shared") && text.contains("beta__shared"),
            "the {target} output does not mangle both colliding names"
        );
    }
}

#[test]
fn two_modules_may_declare_one_name() {
    // ADR 0019. Two modules declaring `shared` used to make the import fatal; the second was
    // once dropped silently, then refused by name. Both are now kept: the definition is mangled
    // and the call must say which one it means.
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let dir = scratch("collision");
    fs::write(
        dir.join("alpha.nova"),
        "fn shared(x: int) -> int {\n    x + 1\n}\n",
    )
    .expect("write alpha");
    fs::write(
        dir.join("beta.nova"),
        "fn shared(x: int, y: int) -> int {\n    x + y\n}\n",
    )
    .expect("write beta");

    let program = dir.join("program.nova");
    fs::write(
        &program,
        "use core;\nuse alpha;\nuse beta;\n\nfn main() -> int {\n    print(alpha.shared(1));\n    print(beta.shared(1, 2));\n    return 0;\n}\n",
    )
    .expect("write program");

    let binary = dir.join("collision");
    let build = Command::new(compiler())
        .arg("build")
        .arg(&program)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "2\n3\n",
        "each module's `shared` must be called, not one of them twice"
    );

    // And the unqualified form must be refused, naming both modules.
    let ambiguous = dir.join("ambiguous.nova");
    fs::write(
        &ambiguous,
        "use core;\nuse alpha;\nuse beta;\n\nfn main() -> int {\n    print(shared(1));\n    return 0;\n}\n",
    )
    .expect("write ambiguous program");
    let refused = Command::new(compiler())
        .arg("build")
        .arg(&ambiguous)
        .arg("-o")
        .arg(dir.join("ambiguous"))
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success(), "an ambiguous use was accepted");
    assert!(
        stderr.contains("declared by alpha and beta"),
        "the refusal did not name both modules: {stderr}"
    );
    assert!(
        stderr.contains("alpha.shared"),
        "the refusal did not offer the qualified form: {stderr}"
    );
}

#[test]
fn a_qualified_call_answers_the_same_as_an_unqualified_one() {
    // ADR 0018. `math.log(2.0)` used to be `E0002: undefined name math`; then it built but
    // answered 0, because the call's return type was lost and `print` fell back to `int`.
    // Both spellings must agree, and the value must be the real one.
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "qualified",
        "use core;\nuse math;\n\nfn main() -> int {\n    print(log(2.0));\n    print(math.log(2.0));\n    return 0;\n}\n",
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected two lines, got {stdout:?}");
    assert_eq!(
        lines[0], lines[1],
        "qualified and unqualified disagree: {stdout:?}"
    );
    assert!(
        lines[0].starts_with("0.693"),
        "`log(2.0)` should be about 0.693, got {:?}",
        lines[0]
    );
}

/// Does a `[float]` array survive being passed to `sum_float`?
///
/// It did not: `sum_float([1.5, 2.5])` returned **3**, because `NovaArray` held `int`
/// elements and `nova_array_get` returned `int`, so the literals were truncated to `1`
/// and `2` before anything summed them. `stdlib/math` still compiled and passed every
/// gate, and computed the wrong answer for `mean`, `variance`, `stddev` and `median`.
///
/// This test was ignored for that defect, deliberately kept runnable rather than
/// remembered, and un-ignores with [ADR 0006](../../docs/adr/0006-array-element-types.md)
/// implemented. It is now the guard against the truncation returning.
#[test]
fn a_float_array_sum_is_not_truncated() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "float_array",
        "\
use math;

fn main() -> int {
    print(sum_float([1.5, 2.5]));
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "4",
        "1.5 + 2.5 must be 4; `NovaArray` holds `int`, so a `[float]` array is \
         truncated on the way in — see ADR 0006"
    );
}

/// The four functions that were built on the truncated sum, each asserted.
///
/// `sum_float` alone is not enough: `mean`, `variance`, `stddev` and `median` all read
/// the array through `array_len` and `arr[i]`, so a fix that repaired only the sum would
/// leave four functions computing quietly wrong answers — which is how the defect
/// survived its first gate. These five numbers come from one array, `[1.5, 2.5]`.
#[test]
fn float_array_statistics_are_not_truncated() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "float_stats",
        "\
use math;

fn main() -> int {
    print(sum_float([1.5, 2.5]));
    print(mean([1.5, 2.5]));
    print(median([1.5, 2.5]));
    print(variance([1.5, 2.5]));
    print(stddev([1.5, 2.5]));
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "4\n2\n2\n0.25\n0.5",
        "sum 4, mean 2, median 2, variance 0.25, stddev 0.5 — an `int` array gives \
         3, 1, 1, 0.25, 0.5 here"
    );
}

/// Every `math` call that used to crash, now that the name collision is fixed.
///
/// Four of them crash, and the emitted C says why:
///
///     int round(double x)  { return (int)nova_math_round(x); }
///     int floor(double x)  { return (int)nova_math_floor(x); }
///     int ceil(double x)   { return (int)nova_math_ceil(x); }
///     double powf(double base, double exp) { return nova_math_powf(base, exp); }
///
/// Those were **definitions of libm symbols**. `nova_math_floor` lives in the runtime
/// translation unit and calls `floor`; at link time that resolved to the program's own
/// `floor`, which called `nova_math_floor` again — unbounded recursion, so SIGSEGV.
///
/// The backend now emits a NOVA function whose name belongs to the C library as
/// `nova_fn_<name>`, so the collision is gone. This test existed as a characterisation
/// of the defect and is now the guard against it returning.
/// A loop whose body is only an expression keeps that body.
///
/// A block whose sole content is an expression parses it as `final_expr`, and the loop
/// lowering read only `statements`. So this emitted `while (i < 3) { }` — an infinite
/// loop, inside a module that compiled and passed every gate. It is asserted with a bound
/// so a regression hangs the suite rather than passing quietly.
#[test]
fn a_loop_body_that_is_only_an_if_is_not_dropped() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "loop_body",
        "\
use core;

fn main() -> int {
    let mut i = 0;
    let mut n = 0;
    while i < 3 {
        if i == 1 {
            n = n + 10;
        } else {
            n = n + 1;
        }
        i = i + 1;
    }
    print(n);
    return 0;
}
",
    );
    assert_eq!(stdout.trim(), "12", "the loop body must not vanish");
}

/// `==` on two `String`s compares their contents, not their addresses.
///
/// Two equal strings need not share an address, so pointer comparison made every string
/// equality false. `stdlib/text`'s `split` compared `substr(...) == delimiter` and never
/// found one, which is how a compiling module returned the wrong answer.
#[test]
fn comparing_two_strings_compares_their_contents() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "string_eq",
        "\
use core;

fn main() -> int {
    let a = \"abc\";
    let b = \"ab\";
    print(a == \"abc\");
    print(a == \"abd\");
    print(a != b);
    print(b + \"c\" == a);
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "true\nfalse\ntrue\ntrue",
        "string comparison must be by content, including a concatenation on one side"
    );
}

/// A `[[int]]` holds `[int]` elements, and a write through a row reaches the matrix.
///
/// The row is copied by value into the matrix, and the copy shares its `int *data` buffer
/// with the original — that sharing is what makes `matrix[0][1] = 9` visible in
/// `matrix[0][1]` afterwards. [ADR 0009](../../docs/adr/0009-nested-arrays.md).
#[test]
fn a_nested_array_can_be_written_through() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "nested_array",
        "\
use core;

fn main() -> int {
    let mut matrix: [[int]] = [];
    let mut row: [int] = [];
    array_push(row, 1);
    array_push(row, 2);
    array_push(matrix, row);
    let mut row2: [int] = [];
    array_push(row2, 3);
    array_push(row2, 4);
    array_push(matrix, row2);
    matrix[0][1] = 9;
    print(array_len(matrix));
    print(matrix[0][0]);
    print(matrix[0][1]);
    print(matrix[1][1]);
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "2\n1\n9\n4",
        "a write through a row must land in the matrix, not in a copy that is thrown away"
    );
}

/// `stdlib/text` works, not merely compiles.
///
/// The compile gate asks whether `cc` accepts the emitted C. It accepted `text` while
/// `split` was an infinite loop and while string equality compared pointers, so this
/// asserts the functions themselves — the two that were wrong, plus `levenshtein`, which
/// is what needed nested arrays.
#[test]
fn the_text_module_works_end_to_end() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "text_module",
        "\
use core;
use text;

fn main() -> int {
    print(to_upper(\"hello\"));
    print(trim(\"  padded  \"));
    let parts = split(\"a,b,c\", \",\");
    print(array_len(parts));
    print(join(parts, \"-\"));
    print(levenshtein(\"kitten\", \"sitting\"));
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "HELLO\npadded\n3\na-b-c\n3",
        "`text` must compute, not just compile"
    );
}

/// An `if` used as a value keeps its `else if` chain, and takes its branches' type.
///
/// Two defects, both only in value position — as a statement the same chain lowered
/// correctly, which is why this survived. `else if` has no AST slot: the parser puts the
/// nested `if` in an else block as its single statement with no `final_expr`, so reading
/// only `final_expr` saw no else at all and emitted a `Unit` literal, which the C backend
/// renders as nothing:
///
///     int v = ((x == 1) ? 10 : );          /* the chain's tail was dropped */
///     int s = ((x > 0) ? "pos" : "neg");   /* and a String was typed `int` */
///
/// Both branches are exercised, because a ternary that works one way and not the other is
/// exactly what a single-case test misses.
#[test]
fn an_if_used_as_a_value_keeps_its_else_chain() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "if_value",
        "\
use core;

fn main() -> int {
    let x = 3;
    let v = if x == 1 { 10 } else if x == 2 { 20 } else { 30 };
    print(v);
    let s = if x > 0 { \"pos\" } else { \"neg\" };
    print(s);
    let y = 0 - 2;
    let t = if y > 0 { \"pos\" } else { \"neg\" };
    print(t);
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "30\npos\nneg",
        "the chain's final else must survive, and a String branch must stay a String"
    );
}

/// `as` converts, and the converted value has the target's type.
///
/// The cast used to be dropped by the lowerer, which made `as` a comment: C converted
/// implicitly at every assignment and call, so nothing noticed — but the NOVA type was
/// never the one the source asked for. That is why `tostring((c as int - 32) as char)`
/// produced `"69"` rather than `"E"`.
///
/// `2.9 as int + 1` is here for the precedence: `as` binds tighter than `+`, so it must
/// be `(int)2.9 + 1` = 3 and not `(int)(2.9 + 1)` = 3 — the same answer by luck, so the
/// `tostring` line above is the one that actually pins it down.
#[test]
fn a_cast_converts() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "casts",
        "\
use core;

fn main() -> int {
    let c = 'e';
    print(tostring((c as int - 32) as char));
    print(3.7 as int);
    print('A' as int);
    print(7 as float);
    print(65 as char);
    print(2.9 as int + 1);
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "E\n3\n65\n7\nA\n3",
        "each cast must produce the target's type and value"
    );
}

/// `s[i]` is the byte at `i`, typed `char`.
///
/// It used to lower to `nova_array_get(s, i)` with `s` a `char *`, which is 35 of the
/// errors that kept `stdlib/text` uncompilable. [ADR 0007](../../docs/adr/0007-string-indexing.md)
/// decided it is a byte rather than a codepoint, because a `char` is one byte.
///
/// `tostring` on the result is asserted too: a `char` used to fall through to the `int`
/// case, so `tostring(s[0])` gave `"72"` and `text`'s `to_upper` concatenated codes
/// instead of letters.
#[test]
fn a_string_index_is_a_byte() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "string_index",
        "\
use core;

fn main() -> int {
    let s = \"Hello\";
    print(strlen(s));
    print(s[0]);
    print(s[4]);
    print(s[0] == 'H');
    print(tostring(s[0]));
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "5\nH\no\ntrue\nH",
        "`s[i]` must be the character at that byte, not an array element and not a code"
    );
}

/// The emitted C carries `#line` directives naming the NOVA source.
///
/// The host C compiler turns those into a `.debug_line` section, which is what lets a debugger
/// show NOVA line numbers without this repository writing a DWARF emitter. Two things are
/// asserted beyond "a directive exists": every line it names **is a line of the file** — a
/// 12-line program once emitted `#line 387`, attributing a standard-library statement to the
/// user's file — and the program still runs.
#[test]
fn emitted_c_carries_line_directives() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let dir = scratch("line_directives");
    let file = dir.join("line_program.nova");
    let source = "use core;\n\nfn add(a: int, b: int) -> int {\n    let total = a + b;\n    total\n}\n\nfn main() -> int {\n    let x = add(2, 3);\n    print(x);\n    return 0;\n}\n";
    fs::write(&file, source).expect("write source");
    let total_lines = source.lines().count();

    let emitted = Command::new(compiler())
        .arg("emit")
        .arg(&file)
        .arg("-o")
        .arg(dir.join("out.c"))
        .output()
        .expect("run nova-compiler emit");
    assert!(emitted.status.success(), "{}", String::from_utf8_lossy(&emitted.stderr));
    let c = fs::read_to_string(dir.join("out.c")).expect("read emitted C");

    let mut named = 0;
    for line in c.lines() {
        let Some(rest) = line.strip_prefix("#line ") else {
            continue;
        };
        named += 1;
        let number: usize = rest
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("unparseable directive: {line}"));
        assert!(
            number >= 1 && number <= total_lines,
            "`#line {number}` is outside the {total_lines}-line source: {line}"
        );
        assert!(
            rest.contains("line_program.nova"),
            "the directive must name the source file: {line}"
        );
    }
    assert!(named > 0, "no `#line` directives were emitted:\n{c}");
}

/// A compiled binary carries a `.debug_line` table naming the NOVA source.
///
/// This is the other half of `#line`: the directive is inert unless the build passes `-g`, so
/// the two together are what give a debugger NOVA line numbers without this repository writing
/// a DWARF emitter. Asserted by reading the binary, because the claim is about the binary.
#[test]
fn a_compiled_binary_names_the_nova_source_in_its_line_table() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    if Command::new("readelf").arg("--version").output().is_err() {
        eprintln!("skipping: no readelf to read the line table with");
        return;
    }
    let dir = scratch("debug_line");
    let file = dir.join("debug_program.nova");
    let binary = dir.join("debug_program");
    fs::write(
        &file,
        "use core;\n\nfn main() -> int {\n    print(41 + 1);\n    return 0;\n}\n",
    )
    .expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&file)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));

    let table = Command::new("readelf")
        .arg("--debug-dump=decodedline")
        .arg(&binary)
        .output()
        .expect("run readelf");
    let dumped = String::from_utf8_lossy(&table.stdout);
    assert!(
        dumped.contains("debug_program.nova"),
        "the binary's line table must name the NOVA source, not only the generated C"
    );

    // And the program still behaves.
    let run = Command::new(&binary).output().expect("run built binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "42");
}

/// `break` and `continue` inside an `if` inside a loop.
///
/// `break;` parses as `Statement::Expr(Expr::Break)`, and the *expression* form wraps it in
/// a block whose value is `Unit`. `Unit` renders as an empty string, so an `if` branch
/// holding only `break` emitted:
///
/// ```c
/// if ((i == 2)) { int __block_0; { break; __block_0 = ; } __block_0; }
/// ```
///
/// which is not C. A search loop — the most ordinary shape there is — did not compile.
/// `break` and `continue` in statement position now lower as statements, and the backend
/// refuses to emit an empty assignment if a `Unit` block reaches it another way.
#[test]
fn break_and_continue_inside_a_branch_compile() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "break_continue",
        "\
use core;

fn first_index_of(values: [int], target: int) -> int {
    let mut i = 0;
    while i < array_len(values) {
        if values[i] == target {
            break;
        } else if values[i] > target {
            return 0 - 1;
        } else {
            i = i + 1;
        }
    }
    i
}

fn sum_of_evens(values: [int]) -> int {
    let mut total = 0;
    let mut i = 0;
    while i < array_len(values) {
        if is_odd(values[i]) {
            i = i + 1;
            continue;
        }
        total = total + values[i];
        i = i + 1;
    }
    total
}

fn main() -> int {
    print(first_index_of([1, 2, 3], 2));
    print(first_index_of([1, 3, 5], 2));
    print(sum_of_evens([1, 2, 3, 4]));
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "1\n-1\n6",
        "a loop that breaks or continues inside a branch must compile and behave"
    );
}

/// `int` holds 64 bits in a variable and in an array element, not only in a literal.
///
/// The language reference says every integer spelling is "a 64-bit signed value", and the
/// runtime had always taken `long long`. The backend emitted C `int`, so a literal reached
/// `print` as 64 bits and the same value assigned to a variable truncated:
///
/// ```nova
/// print(3000000000);        // 3000000000
/// let big = 3000000000;
/// print(big);               // -1294967296
/// ```
///
/// Widening the scalar alone would have moved the truncation into array elements, so the
/// two moved together. See ADR 0011.
#[test]
fn int_is_64_bit_in_variables_and_arrays() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "int_width",
        "\
use core;

fn main() -> int {
    let big = 3000000000;
    print(big);
    let mut a: [int] = [];
    array_push(a, 3000000000);
    print(a[0]);
    let literal = [4000000000];
    print(literal[0]);
    print(9223372036854775807);
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "3000000000\n3000000000\n4000000000\n9223372036854775807",
        "a 64-bit int must survive a variable and an array element, not only a literal"
    );
}

/// A struct field keeps its generic type, and the struct order follows its fields.
///
/// `Holder { maybe: Option<int> }` emitted `Option_int maybe;` because the field's type was
/// read through `.name`. Fixing that exposed an ordering problem: a struct *field* needs a
/// complete type, and the option structs were emitted after the user's, so `Holder` came
/// before `Option_int`. Structs are emitted in dependency order now.
#[test]
fn a_struct_field_keeps_its_generic_type() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "struct_option_field",
        "\
struct Holder {
    label: String,
    maybe: Option<int>
}

fn main() -> int {
    let h = Holder { label: \"x\", maybe: Some(9) };
    print(h.label);
    match h.maybe {
        Some(v) => print(v),
        None => print(0 - 1),
    }
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "x\n9",
        "a struct field must keep `Option<int>`, and the struct must follow the type it embeds"
    );
}

/// A method and a local `const` keep a generic return type.
///
/// Three places computed a type from `TypeExpr::name` instead of `type_display`, and
/// `Option<int>`'s name is `Option`, which `c_type` maps to `int`. So a method returning
/// `Option<int>` was declared to return an `int` while returning an `Option_int`, and a
/// `const LIMIT: Option<int>` declared an `int`.
///
/// This also covers the receiver: `self` was emitted by value unless the source wrote `&`,
/// while the call site passed an address and the body read `self->field` — three
/// disagreements about one parameter, which is why a method returning an `Option` had
/// never compiled.
#[test]
fn methods_and_consts_keep_their_generic_return_type() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "method_option",
        "\
struct Holder {
    v: int
}

impl Holder {
    fn maybe(self) -> Option<int> {
        Some(self.v)
    }
}

fn main() -> int {
    let h = Holder { v: 5 };
    match h.maybe() {
        Some(x) => print(x),
        None => print(0 - 1),
    }
    const LIMIT: Option<int> = Some(7);
    match LIMIT {
        Some(x) => print(x),
        None => print(0 - 1),
    }
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "5\n7",
        "a method and a const must keep `Option<int>`, not collapse it to `int`"
    );
}

/// A `[String]` array holds `char *`, not `int`.
///
/// `NovaArray` holds `int` elements, so before `NovaArray_string` existed a `[String]`
/// array put eight-byte pointers into four-byte slots. The same monomorphisation that
/// fixed `[float]` fixes this: the element type is known at every construction and use
/// site, so the header and the runtime functions follow from it.
///
/// Every operation is asserted, not just the read: `array_len`, `arr[i]`, `push`, and
/// `arr[i] = value`. The last one is the one that had no `_string` arm at all — the
/// backend emitted `nova_array_set`, which takes and stores an `int`.
#[test]
fn a_string_array_keeps_its_elements() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "string_array",
        "\
use core;

fn main() -> int {
    let names: [String] = [\"ada\", \"grace\", \"alan\"];
    print(array_len(names));
    print(names[0]);
    print(names[2]);
    let mut more: [String] = [\"x\"];
    array_push(more, \"y\");
    print(array_len(more));
    print(more[1]);
    more[0] = \"z\";
    print(more[0]);
    return 0;
}
",
    );
    assert_eq!(
        stdout.trim(),
        "3\nada\nalan\n2\ny\nz",
        "a `[String]` array must hold the strings themselves; an `int` element gives \
         truncated pointers here, or does not compile"
    );
}

/// An empty array literal takes its element type from the declaration.
///
/// `[]` says nothing about what it holds, so `let r: [String] = []` is the only place
/// the element type exists. Emitting `nova_array_from(0, 0)` there produced a
/// `NovaArray` assigned to a `NovaArray_string`, which is what stopped `stdlib/text`
/// at its first line.
#[test]
fn an_empty_array_literal_takes_the_declared_element_type() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "empty_array",
        "\
use core;

fn main() -> int {
    let mut names: [String] = [];
    print(array_len(names));
    array_push(names, \"only\");
    print(names[0]);
    let empty_floats: [float] = [];
    print(array_len(empty_floats));
    return 0;
}
",
    );
    assert_eq!(stdout.trim(), "0\nonly\n0");
}

#[test]
fn every_math_call_works() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    // Every one of these works now. They were four-of-seven before the collision was
    // fixed: `floor`, `ceil`, `round` and `powf` were emitted as definitions of libm
    // symbols, so the runtime's call to `floor` resolved back to the program's own.
    let cases: &[(&str, &str, bool)] = &[
        ("print(sqrt(16.0));", "4\n", true),
        ("print(cbrt(27.0));", "3\n", true),
        ("print(PI > 3.14);", "true\n", true),
        ("print(floor(3.7));", "3\n", true),
        ("print(ceil(3.2));", "4\n", true),
        ("print(round(2.5));", "3\n", true),
        ("print(powf(2.0, 10.0));", "1024\n", true),
    ];

    let mut surprises = Vec::new();
    for (call, expected, works) in cases {
        let source = format!("use math;\n\nfn main() -> int {{\n    {call}\n    return 0;\n}}\n");
        let dir = scratch(&format!("math-{}", call.len()));
        let file = dir.join("program.nova");
        let binary = dir.join("program");
        fs::write(&file, &source).expect("write source");

        let build = Command::new(compiler())
            .arg("build")
            .arg(&file)
            .arg("-o")
            .arg(&binary)
            .arg("--module-root")
            .arg(workspace_root().join("stdlib"))
            .output()
            .expect("run nova-compiler build");
        assert!(
            build.status.success(),
            "{call} did not build: {}",
            String::from_utf8_lossy(&build.stderr).trim()
        );

        let run = Command::new(&binary).output().expect("run built binary");
        let ok = run.status.success() && String::from_utf8_lossy(&run.stdout) == *expected;

        if ok != *works {
            surprises.push(format!(
                "{call}: {} (the table says it {})",
                if ok { "works now" } else { "does not work" },
                if *works { "works" } else { "crashes" }
            ));
        }
    }

    assert!(
        surprises.is_empty(),
        "the characterisation is out of date:\n  {}",
        surprises.join("\n  ")
    );
}

#[test]
fn the_floating_point_builtins_run() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    // The builtins are called directly rather than through `use math;` on purpose.
    // The module emits C but does not *compile* — `normalize2` builds an
    // `NovaTuple_int_int` where a `NovaTuple_float_float` is declared, and `mod_int`
    // applies `%` to two doubles — so importing it fails for reasons that have
    // nothing to do with the builtins. Those are recorded in `DEV_PLAN.md`.
    //
    // Floats print through `%g`, which is why `4.0` comes out as `4`.
    let dir = scratch("builtins");
    let file = dir.join("builtins.nova");
    let binary = dir.join("builtins");
    fs::write(
        &file,
        "\
fn main() -> int {
    print(__builtin_sqrt(16.0));
    print(__builtin_cbrt(27.0));
    print(__builtin_powf(2.0, 10.0));
    print(__builtin_sin(0.0));
    print(__builtin_exp(0.0));
    print(__builtin_log(1.0));
    print(__builtin_floor(3.7) as int);
    print(__builtin_round(2.5) as int);
    print(__builtin_is_nan(__builtin_nan()));
    print(__builtin_is_finite(1.0));
    print(__builtin_parse_int(\"42\"));
    print(__builtin_parse_float(\"2.5\"));
    return 0;
}
",
    )
    .expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&file)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(
        run.status.success(),
        "program exited with {}:\nstderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "4\n3\n1024\n0\n1\n0\n3\n3\ntrue\ntrue\n42\n2.5\n"
    );
}

#[test]
fn fibonacci_example_runs() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("fibonacci");
    let binary = dir.join("fibonacci");
    let source = workspace_root().join("examples").join("fibonacci.nova");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(run.status.success(), "program exited with {}", run.status);

    let expected = "First\n10\nFibonacci numbers:\n0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n";
    assert_eq!(String::from_utf8_lossy(&run.stdout), expected);
}

#[test]
fn run_subcommand_forwards_exit_status() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("run");
    let source = dir.join("status.nova");
    fs::write(
        &source,
        "fn main() -> int {\n    print(\"ran\");\n    return 7;\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ran\n");
    assert_eq!(run.status.code(), Some(7));
}

#[test]
fn structs_compile_and_run() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("structs");
    let source = dir.join("structs.nova");
    let binary = dir.join("structs");
    fs::write(
        &source,
        "struct Point {\n    x: int,\n    y: int,\n}\n\nfn main() {\n    let p = Point { x: 3, y: 4 };\n    print(p.x + p.y);\n}\n",
    )
    .expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(run.status.success(), "program exited with {}", run.status);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\n");
}

#[test]
fn struct_fields_can_be_assigned() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("struct-field-assign");
    let source = dir.join("structs.nova");
    let binary = dir.join("structs");
    fs::write(
        &source,
        "struct Point {\n    x: int,\n    y: int,\n}\n\nfn main() {\n    let mut p = Point { x: 1, y: 2 };\n    p.x = 5;\n    print(p.x);\n    print(p.y);\n}\n",
    )
    .expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(run.status.success(), "program exited with {}", run.status);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "5\n2\n");
}

#[test]
fn methods_read_and_mutate_the_receiver() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("methods");
    let source = dir.join("counter.nova");
    let binary = dir.join("counter");
    fs::write(
        &source,
        "struct Counter {\n    value: int,\n}\n\nimpl Counter {\n    fn bump(&mut self) {\n        self.value = self.value + 1;\n    }\n\n    fn get(&self) -> int {\n        self.value\n    }\n}\n\nfn main() {\n    let mut c = Counter { value: 0 };\n    c.bump();\n    c.bump();\n    print(c.get());\n}\n",
    )
    .expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(run.status.success(), "program exited with {}", run.status);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "2\n");
}

#[test]
fn enums_and_match_compile_and_run() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("enums");
    let source = dir.join("colors.nova");
    fs::write(
        &source,
        "enum Color {\n    Red,\n    Green,\n    Blue,\n}\n\nfn code(c: Color) -> int {\n    match c {\n        Color.Red => 1,\n        Color.Green => 2,\n        _ => 3,\n    }\n}\n\nfn main() {\n    print(code(Color.Red));\n    print(code(Color.Blue));\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1\n3\n");
}

#[test]
fn array_example_runs() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let source = workspace_root().join("examples").join("array_ops.nova");
    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let expected = "Original array:\n5\n2\n8\n1\n9\nSum:\n25\nMax:\n9\nReversed array:\n9\n1\n8\n2\n5\n";
    assert_eq!(String::from_utf8_lossy(&run.stdout), expected);
}

#[test]
fn mod_declaration_loads_a_module_file() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("mod-declaration");
    let source = dir.join("main.nova");
    fs::write(&source, "mod helper;\n\nfn main() {\n    print(triple(14));\n}\n")
        .expect("write main");
    fs::write(
        dir.join("helper.nova"),
        "fn triple(value: int) -> int {\n    value * 3\n}\n",
    )
    .expect("write helper");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
}

#[test]
fn reference_array_parameters_share_the_header() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("array-reference");
    let source = dir.join("array_ref.nova");
    fs::write(
        &source,
        "fn append(values: &mut [int], value: int) {\n    __builtin_array_push(values, value);\n}\n\nfn main() {\n    let numbers: Array = [1, 2];\n    append(numbers, 3);\n    print(__builtin_array_len(numbers));\n    numbers[0] = 9;\n    print(numbers[0]);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "3\n9\n");
}

#[test]
fn string_slices_and_match_patterns_run() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("strings");
    let source = dir.join("strings.nova");
    fs::write(
        &source,
        "fn main() {\n    let text: String = \"hello world\";\n    print(text[0..5]);\n    print(text[6..11]);\n    match text {\n        \"hello world\" => { print(\"matched\"); },\n        _ => { print(\"no\"); },\n    }\n    match text {\n        \"other\" => { print(\"wrong\"); },\n        _ => { print(\"fallback\"); },\n    }\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "hello\nworld\nmatched\nfallback\n"
    );
}

#[test]
fn generic_methods_are_monomorphised() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("generic-method");
    let source = dir.join("generic_method.nova");
    fs::write(
        &source,
        "struct Box<T> {\n    value: T,\n}\n\nimpl Box<T> {\n    fn get(&self) -> T {\n        self.value\n    }\n}\n\nfn main() {\n    let boxed: Box<int> = Box { value: 7 };\n    print(boxed.get());\n    let real: Box<float> = Box { value: 2.5 };\n    print(real.get());\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\n2.5\n");
}

#[test]
fn generic_structs_are_monomorphised() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("generic-struct");
    let source = dir.join("generic_struct.nova");
    fs::write(
        &source,
        "struct Box<T> {\n    value: T,\n}\n\nstruct Pair<A, B> {\n    first: A,\n    second: B,\n}\n\nfn main() {\n    let boxed: Box<int> = Box { value: 7 };\n    print(boxed.value);\n    let pair: Pair<int, float> = Pair { first: 3, second: 2.5 };\n    print(pair.first);\n    print(pair.second);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\n3\n2.5\n");
}

#[test]
fn use_pulls_in_a_sibling_module() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("modules");
    let source = dir.join("main.nova");
    fs::write(&source, "use helper.*;\n\nfn main() {\n    print(twice(21));\n}\n")
        .expect("write main");
    fs::write(
        dir.join("helper.nova"),
        "fn twice(value: int) -> int {\n    value * 2\n}\n",
    )
    .expect("write helper");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
}

#[test]
fn generic_enum_payloads_and_growable_arrays_run() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("generic-enum");
    let source = dir.join("generic_enum.nova");
    fs::write(
        &source,
        "enum Result<T, E> {\n    Ok(T),\n    Err(E),\n}\n\nfn show(r: Result<int, String>) {\n    match r {\n        Result.Ok(v) => { print(v); },\n        Result.Err(e) => { print(e); },\n    }\n}\n\nfn main() {\n    show(Result.Ok(7));\n    show(Result.Err(\"bad\"));\n    let values: Array = [1, 2];\n    __builtin_array_push(values, 3);\n    print(__builtin_array_len(values));\n    match __builtin_array_pop(values) {\n        Some(v) => { print(v); },\n        None => { print(0 - 1); },\n    }\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\nbad\n3\n3\n");
}

#[test]
fn option_and_payload_enums_run() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("payload");
    let source = dir.join("payload.nova");
    fs::write(
        &source,
        "enum Shape {\n    Circle(int),\n    Square(int),\n}\n\nfn describe(value: Option<int>) {\n    match value {\n        Some(v) => { print(v); },\n        None => { print(\"none\"); },\n    }\n}\n\nfn main() {\n    describe(Some(42));\n    describe(None);\n    let shape: Shape = Shape.Circle(7);\n    match shape {\n        Shape.Circle(r) => { print(r); },\n        Shape.Square(w) => { print(w); },\n    }\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\nnone\n7\n");
}

#[test]
fn bounds_are_checked_on_writes_too() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("bounds-write");
    let source = dir.join("bounds_write.nova");
    fs::write(
        &source,
        "fn main() {\n    let values: Array = [1, 2];\n    values[2] = 9;\n    print(\"unreachable\");\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(!run.status.success(), "out-of-bounds write must fail");
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("array index out of bounds"),
        "expected a bounds diagnostic, got: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn builtin_functions_use_the_runtime() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("builtins");
    let source = dir.join("builtins.nova");
    fs::write(
        &source,
        "fn main() {\n    let text: String = \"nova\";\n    print(__builtin_strlen(text));\n    print(__builtin_strcat(text, \"!\"));\n    print(__builtin_strcmp(text, \"nova\"));\n    print(__builtin_typeof(text));\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "4\nnova!\n0\nString\n"
    );
}

#[test]
fn out_of_bounds_array_access_fails_at_runtime() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("bounds");
    let source = dir.join("bounds.nova");
    fs::write(
        &source,
        "fn main() {\n    let values: Array = [1, 2, 3];\n    print(values[7]);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(!run.status.success(), "out-of-bounds access must fail");
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("array index out of bounds"),
        "expected a bounds diagnostic, got: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn match_statement_emits_a_c_switch_and_runs() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("switch");
    let source = dir.join("switch.nova");
    fs::write(
        &source,
        "enum Color {\n    Red,\n    Green,\n    Blue,\n}\n\nfn main() {\n    let c = Color.Green;\n    match c {\n        Color.Red => { print(\"red\"); },\n        Color.Green => { print(\"green\"); },\n        _ => { print(\"other\"); },\n\n    }\n}\n",
    )
    .expect("write source");

    let c_file = dir.join("switch.c");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("-o")
        .arg(&c_file)
        .output()
        .expect("run nova-compiler emit");
    assert!(
        emit.status.success(),
        "emit failed: {}",
        String::from_utf8_lossy(&emit.stderr)
    );

    let c_code = fs::read_to_string(&c_file).expect("read generated C");
    assert!(
        c_code.contains("switch ("),
        "statement match should emit a C switch:\n{c_code}"
    );
    assert!(c_code.contains("case 0:") && c_code.contains("case 1:"));
    assert!(c_code.contains("default:"));

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "green\n");
}

#[test]
fn generic_functions_compile_for_each_type() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("generics");
    let source = dir.join("generics.nova");
    fs::write(
        &source,
        "fn id<T>(x: T) -> T {\n    x\n}\n\nfn show<T>(value: T) {\n    print(value);\n}\n\nfn main() {\n    print(id(7));\n    print(id(\"hi\"));\n    show(42);\n    show(\"hey\");\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\nhi\n42\nhey\n");
}

#[test]
fn build_reports_a_missing_source_file() {
    let missing = std::env::temp_dir().join("nova-does-not-exist-12345.nova");
    let _ = fs::remove_file(&missing);

    let build = Command::new(compiler())
        .arg("build")
        .arg(&missing)
        .output()
        .expect("run nova-compiler build");

    assert!(!build.status.success());
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("cannot read") || stderr.contains("No such file"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn unsupported_target_is_refused() {
    let source = std::env::temp_dir().join("nova-target-check.nova");
    fs::write(&source, "print(\"x\");\n").expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("--target")
        .arg("wasm")
        .output()
        .expect("run nova-compiler build");

    assert!(!build.status.success());
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("only supports `--target c`"),
        "unexpected stderr: {stderr}"
    );

    // A name that is not a backend stays an error.
    let unknown = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("--target")
        .arg("cobol")
        .output()
        .expect("emit unknown target");
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown target"));

    // A WAT module is emitted rather than refused.
    let wasm = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("--target")
        .arg("wasm")
        .output()
        .expect("emit wasm");
    assert!(wasm.status.success());
    assert!(String::from_utf8_lossy(&wasm.stdout).contains("(module"));

    // `build` only links C, even when a target is a known emitter.
    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("--target")
        .arg("js")
        .output()
        .expect("run nova-compiler build");
    assert!(!build.status.success());
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("only supports `--target c`"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn parity_harness_compares_c_js_rust_and_wasm() {
    if !node_available() || !rustc_available() {
        eprintln!("skipping: node and rustc are required for the parity harness");
        return;
    }

    let dir = scratch("parity");
    let source = dir.join("cli.nova");
    fs::write(
        &source,
        "fn main() -> int {\n    let n = 6 * 7;\n    print(\"answer:\");\n    print(n);\n    return 3;\n}\n",
    )
    .expect("write source");

    let parity = Command::new(compiler())
        .arg("test")
        .arg("--parity")
        .arg(&source)
        .output()
        .expect("run nova-compiler test");

    let stdout = String::from_utf8_lossy(&parity.stdout);
    for backend in ["c", "js", "rust"] {
        assert!(
            stdout.contains(&format!("{backend:<5} pass")),
            "{backend} should pass parity: {stdout}"
        );
    }
    if wat2wasm_available() {
        assert!(parity.status.success(), "all four backends should pass: {stdout}");
        assert!(stdout.contains("wasm  pass"), "{stdout}");
    } else {
        // Without an assembler the backend is explicitly held, and that is a
        // failure of the run rather than a silent skip.
        assert!(!parity.status.success(), "held backend must fail the run");
        assert!(stdout.contains("wasm  held"), "wasm should be held: {stdout}");
    }
}

#[test]
fn core_stdlib_math_functions_return_their_values() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    // `use core;` resolves through the source file's ancestors, so the program
    // must live inside the workspace (not in the system temp directory).
    let dir = scratch("core-math");
    let source = workspace_root().join("target").join("core_math.nova");
    fs::create_dir_all(source.parent().expect("target directory")).expect("create target");
    fs::write(
        &source,
        "use core;\n\nfn main() -> int {\n    print(factorial(6));\n    print(fibonacci(10));\n    print(sign(-5));\n    print(clamp(12, 0, 10));\n    print(gcd(48, 18));\n    return 0;\n}\n",
    )
    .expect("write source");

    let binary = dir.join("core_math");
    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("build core_math");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run core_math");
    let _ = fs::remove_file(&source);
    // An `if`/`else if` chain used as the function's result must return each
    // branch's value; before this test these printed 1 and garbage.
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "720\n55\n-1\n10\n6\n"
    );
    assert_eq!(run.status.code(), Some(0));
}

#[test]
fn wasm_output_assembles_and_runs_on_the_host() {
    if !node_available() || !wat2wasm_available() {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    }

    let dir = scratch("wasm-run");
    let source = dir.join("sum.nova");
    fs::write(
        &source,
        "fn main() -> int {\n    let total = 0;\n    for i in 0..10 {\n        total = total + i * i;\n    }\n    print(\"sum:\");\n    print(total);\n    return 4;\n}\n",
    )
    .expect("write source");

    let wat = dir.join("sum.wat");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("--target")
        .arg("wasm")
        .arg("-o")
        .arg(&wat)
        .output()
        .expect("emit wasm");
    assert!(emit.status.success(), "{}", String::from_utf8_lossy(&emit.stderr));

    let wasm = dir.join("sum.wasm");
    let assembler = std::env::var("WAT2WASM").unwrap_or_else(|_| "wat2wasm".to_string());
    let assembled = Command::new(assembler)
        .arg(&wat)
        .arg("-o")
        .arg(&wasm)
        .output()
        .expect("run wat2wasm");
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stderr)
    );

    let host = workspace_root().join("backends/wasm/host.mjs");
    let run = Command::new("node")
        .arg(host)
        .arg(&wasm)
        .output()
        .expect("run the WASM host");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "sum:\n285\n");
    assert_eq!(run.status.code(), Some(4));
}

/// Emit, assemble and run a WASM program on the committed host.
/// Returns `None` when the toolchain needed to run WebAssembly is missing.
fn run_wasm_source(name: &str, source: &str) -> Option<(String, Option<i32>)> {
    if !node_available() || !wat2wasm_available() {
        return None;
    }
    // `use core;` resolves through the source file's ancestors, so the program
    // has to live inside the workspace for the corpus tests.
    let dir = workspace_root()
        .join("target")
        .join(format!("e2e-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch directory");
    let path = dir.join(format!("{name}.nova"));
    fs::write(&path, source).expect("write wasm source");
    let wat = dir.join(format!("{name}.wat"));
    let wasm = dir.join(format!("{name}.wasm"));

    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&path)
        .arg("--target")
        .arg("wasm")
        .arg("-o")
        .arg(&wat)
        .output()
        .expect("emit wasm");
    assert!(emit.status.success(), "{}", String::from_utf8_lossy(&emit.stderr));

    let assembler = std::env::var("WAT2WASM").unwrap_or_else(|_| "wat2wasm".to_string());
    let assembled = Command::new(assembler)
        .arg(&wat)
        .arg("-o")
        .arg(&wasm)
        .output()
        .expect("run wat2wasm");
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stderr)
    );

    let run = Command::new("node")
        .arg(workspace_root().join("backends/wasm/host.mjs"))
        .arg(&wasm)
        .output()
        .expect("run the WASM host");
    Some((
        String::from_utf8_lossy(&run.stdout).into_owned(),
        run.status.code(),
    ))
}

#[test]
fn wasm_structs_and_tuples_run_on_the_host() {
    let Some((stdout, code)) = run_wasm_source(
        "structs",
        "struct Point {\n    x: int,\n    y: int,\n}\n\nfn main() -> int {\n    let p = Point { x: 3, y: 4 };\n    p.y = p.y + 1;\n    let pair = (p.x, p.y);\n    let (a, b) = pair;\n    print(a);\n    print(b);\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert_eq!(stdout, "3\n5\n");
    assert_eq!(code, Some(0));
}

#[test]
fn wasm_growable_arrays_run_on_the_host() {
    let Some((stdout, code)) = run_wasm_source(
        "wasm_push",
        "use core;\n\nfn main() -> int {\n    let a = [1, 2];\n    array_push(a, 3);\n    array_push(a, 4);\n    print(array_len(a));\n    print(a[3]);\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert_eq!(stdout, "4\n4\n");
    assert_eq!(code, Some(0));
}

#[test]
fn wasm_closures_run_on_the_host() {
    let Some((stdout, code)) = run_wasm_source(
        "wasm_closures",
        "use core;\n\nfn main() -> int {\n    let inc = |x| x + 1;\n    let dbl = |x| x * 2;\n    let both = compose(inc, dbl);\n    print(both(3));\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert_eq!(stdout, "7\n");
    assert_eq!(code, Some(0));
}

#[test]
fn wasm_closure_prints_string_parameter() {
    let Some((stdout, code)) = run_wasm_source(
        "wasm_print_param",
        "fn main() -> int {\n    let show = |s: string| print(s);\n    show(\"ok\");\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert_eq!(stdout, "ok\n");
    assert_eq!(code, Some(0));
}

#[test]
fn wasm_string_returning_closures_run_on_the_host() {
    let Some((stdout, code)) = run_wasm_source(
        "wasm_string_closure",
        "fn main() -> int {\n    let greeting = \"hi\";\n    let say = || greeting;\n    print(say());\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert_eq!(stdout, "hi\n");
    assert_eq!(code, Some(0));
}

#[test]
fn wasm_capturing_closures_run_on_the_host() {
    let Some((stdout, code)) = run_wasm_source(
        "wasm_capture",
        "fn main() -> int {\n    let y = 10;\n    let add = |x| x + y;\n    print(add(5));\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert_eq!(stdout, "15\n");
    assert_eq!(code, Some(0));
}

#[test]
fn wasm_out_of_bounds_index_traps() {
    let Some((stdout, code)) = run_wasm_source(
        "oob",
        "fn main() -> int {\n    let a = [1, 2];\n    print(a[5]);\n    return 0;\n}\n",
    ) else {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    };
    assert!(stdout.is_empty(), "must not read past the array: {stdout}");
    assert_ne!(code, Some(0), "out-of-bounds access must trap");
}

#[test]
fn wasm_arrays_run_on_the_host() {
    if !node_available() || !wat2wasm_available() {
        eprintln!("skipping: node and wat2wasm are required to run WebAssembly");
        return;
    }

    let dir = scratch("wasm-array");
    let source = workspace_root().join("target").join("wasm_array.nova");
    fs::create_dir_all(source.parent().expect("target directory")).expect("create target");
    fs::write(
        &source,
        "use core;\n\nfn total_of(a: [int]) -> int {\n    let total = 0;\n    let i = 0;\n    while i < array_len(a) {\n        total = total + a[i];\n        i = i + 1;\n    }\n    return total;\n}\n\nfn main() -> int {\n    let a = [1, 2, 3, 4, 5];\n    a[1] = 10;\n    print(total_of(a));\n    return 0;\n}\n",
    )
    .expect("write source");

    let wat = dir.join("array.wat");
    let wasm = dir.join("array.wasm");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("--target")
        .arg("wasm")
        .arg("-o")
        .arg(&wat)
        .output()
        .expect("emit wasm");
    let _ = fs::remove_file(&source);
    assert!(emit.status.success(), "{}", String::from_utf8_lossy(&emit.stderr));

    let assembler = std::env::var("WAT2WASM").unwrap_or_else(|_| "wat2wasm".to_string());
    let assembled = Command::new(assembler)
        .arg(&wat)
        .arg("-o")
        .arg(&wasm)
        .output()
        .expect("run wat2wasm");
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stderr)
    );

    let run = Command::new("node")
        .arg(workspace_root().join("backends/wasm/host.mjs"))
        .arg(&wasm)
        .output()
        .expect("run the WASM host");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "23\n");
    assert_eq!(run.status.code(), Some(0));
}

#[test]
fn javascript_and_rust_targets_emit_source() {
    let source = std::env::temp_dir().join("nova-target-emit.nova");
    fs::write(&source, "fn main() {\n    let x = 1;\n    println(x);\n}\n").expect("write source");

    let js = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("--target")
        .arg("js")
        .output()
        .expect("emit js");
    assert!(js.status.success());
    let js_source = String::from_utf8_lossy(&js.stdout);
    assert!(js_source.contains("'use strict'"), "unexpected JS: {js_source}");

    let rust = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("--target")
        .arg("rust")
        .output()
        .expect("emit rust");
    assert!(rust.status.success());
    let rust_source = String::from_utf8_lossy(&rust.stdout);
    assert!(
        rust_source.contains("fn main"),
        "unexpected Rust: {rust_source}"
    );
}

#[test]
fn c_closures_print_their_parameters() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("closure-print");
    let source = dir.join("closure_print.nova");
    fs::write(
        &source,
        "fn main() -> int {\n    let show = |s: string| print(s);\n    show(\"ok\");\n    let block = |s: string| {\n        print(s);\n        s\n    };\n    print(block(\"block\"));\n    return 0;\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\nblock\nblock\n");
}

#[test]
fn stdlib_compose_runs_with_closures() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("stdlib-compose");
    // A module file beside the program exercises the loader without needing the
    // full standard library in the scratch directory.
    fs::write(
        dir.join("core.nova"),
        "fn print<T>(value: T) { __builtin_print(value); }\n\nfn compose<A, B, C>(f: fn(B) -> C, g: fn(A) -> B) -> fn(A) -> C {\n    |x| f(g(x))\n}\n",
    )
    .expect("write module");
    let source = dir.join("compose.nova");
    fs::write(
        &source,
        "use core;\n\nfn main() {\n    let inc = |x| x + 1;\n    let dbl = |x| x * 2;\n    let both = compose(inc, dbl);\n    println(both(3));\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\n");
}

#[test]
fn tuples_destructure_into_bindings() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("tuples");
    let source = dir.join("tuples.nova");
    fs::write(
        &source,
        "fn main() {\n    let pair = (3, 4);\n    let (a, b) = pair;\n    println(a + b);\n\n    let (name, score) = (\"nova\", 9);\n    println(name);\n    println(score);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler");
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "7\nnova\n9");
}

#[test]
fn closures_capture_and_pass_as_values() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("closures");
    let source = dir.join("closures.nova");
    fs::write(
        &source,
        "fn apply(f: fn(int) -> int, x: int) -> int {\n    f(x)\n}\n\nfn apply_twice(f: fn(int) -> int, x: int) -> int {\n    let g = |y| f(y) + 1;\n    g(x)\n}\n\nfn make_adder(base: int) -> fn(int) -> int {\n    |x| x + base\n}\n\nfn main() {\n    let add = |x| x + 1;\n    println(add(4));\n\n    let base = 10;\n    let add_base = |x| x + base;\n    println(add_base(5));\n\n    println(apply(|n| n * 2, 21));\n\n    let add5 = make_adder(5);\n    println(add5(3));\n\n    let greeting = \"hi\";\n    let say = || greeting;\n    println(say());\n\n    println(apply_twice(|n| n * 3, 2));\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler");
    assert!(run.status.success());
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "5\n15\n42\n8\nhi\n7"
    );
}

#[test]
fn values_print_through_the_runtime() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("values");
    let source = dir.join("values.nova");
    fs::write(
        &source,
        "fn main() {\n    print(true);\n    print(false);\n    print(3.5);\n    print('x');\n    print(7);\n    print(2 + 3);\n    print(1 < 2);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "true\nfalse\n3.5\nx\n7\n5\ntrue\n"
    );
}

#[test]
fn declared_types_drive_printing() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("typed");
    let source = dir.join("typed.nova");
    fs::write(
        &source,
        "fn main() {\n    let flag: bool = true;\n    let off: bool = false;\n    let n: int = 7;\n    let name = \"nova\";\n    let f = 1.5;\n    print(flag);\n    print(off);\n    print(n);\n    print(name);\n    print(f);\n    print(flag == off);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "true\nfalse\n7\nnova\n1.5\nfalse\n"
    );
}

#[test]
fn call_results_print_with_their_return_type() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("typed-calls");
    let source = dir.join("calls.nova");
    fs::write(
        &source,
        "fn is_ready() -> bool {\n    return 1 < 2;\n}\n\nfn half() -> float {\n    return 1.5;\n}\n\nfn greeting() -> string {\n    return \"hi\";\n}\n\nfn main() {\n    print(is_ready());\n    print(half());\n    print(greeting());\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler run");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n1.5\nhi\n");
}

#[test]
fn match_guards_bind_and_filter_arms() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("match-guards");
    let source = dir.join("guards.nova");
    fs::write(
        &source,
        "fn classify(n: int) -> string {\n    match n {\n        x if x > 10 => {\n            return \"big\";\n        },\n        0 => {\n            return \"zero\";\n        },\n        _ => {\n            return \"small\";\n        },\n    }\n    return \"other\";\n}\n\nfn main() {\n    println(classify(12));\n    println(classify(0));\n    println(classify(3));\n\n    let n = 5;\n    let label: string = match n {\n        _ if n > 3 => \"high\",\n        _ => \"low\",\n    };\n    println(label);\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "big\nzero\nsmall\nhigh\n");
}

#[test]
fn rust_array_example_matches_c_output() {
    if !rustc_available() {
        eprintln!("skipping: rustc is not installed");
        return;
    }

    let example = workspace_root().join("examples/array_ops.nova");
    let dir = scratch("rust-array");
    let rust = dir.join("array_ops.rs");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&example)
        .arg("-o")
        .arg(&rust)
        .arg("--target")
        .arg("rust")
        .output()
        .expect("emit rust");
    assert!(emit.status.success(), "emit failed: {}", String::from_utf8_lossy(&emit.stderr));

    let binary = dir.join("array_ops-rust");
    let compile = Command::new("rustc")
        .arg(&rust)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run rustc");
    assert!(
        compile.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let rust_run = Command::new(&binary).output().expect("run rust binary");
    assert!(rust_run.status.success());

    let c_run = Command::new(compiler())
        .arg("run")
        .arg(&example)
        .output()
        .expect("run C example");
    assert!(c_run.status.success());
    assert_eq!(
        String::from_utf8_lossy(&rust_run.stdout),
        String::from_utf8_lossy(&c_run.stdout)
    );
}

#[test]
fn closure_parameters_accept_annotations_and_context() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("typed-closures");
    let source = dir.join("typed_closures.nova");
    fs::write(
        &source,
        "fn main() {\n    let f: fn(int) -> int = |x| x + 2;\n    println(f(3));\n\n    let g = |x: float| x * 2.0;\n    println(g(1.5));\n\n    let h = |x: string| x;\n    println(h(\"ok\"));\n}\n",
    )
    .expect("write source");

    let run = Command::new(compiler())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run nova-compiler");
    assert!(
        run.status.success(),
        "program failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "5\n3\nok\n");
}

#[test]
fn javascript_output_runs_under_node() {
    if !node_available() {
        eprintln!("skipping: node is not installed");
        return;
    }

    let dir = scratch("js-run");
    let source = dir.join("typed.nova");
    fs::write(
        &source,
        "fn main() {\n    let flag: bool = true;\n    let n: int = 7;\n    let name = \"nova\";\n    let f = 1.5;\n    print(flag);\n    print(n);\n    print(name);\n    print(f);\n}\n",
    )
    .expect("write source");
    let js = dir.join("typed.js");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("-o")
        .arg(&js)
        .arg("--target")
        .arg("js")
        .output()
        .expect("emit js");
    assert!(emit.status.success(), "emit failed: {}", String::from_utf8_lossy(&emit.stderr));

    let run = Command::new("node")
        .arg(&js)
        .output()
        .expect("run node");
    assert!(
        run.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n7\nnova\n1.5\n");
}

#[test]
fn rust_closures_run_and_match_c() {
    if !rustc_available() {
        eprintln!("skipping: rustc is not installed");
        return;
    }

    let dir = scratch("rust-closures");
    let source = dir.join("closures.nova");
    fs::write(
        &source,
        "fn apply(f: fn(int) -> int, x: int) -> int {\n    f(x)\n}\n\nfn make_adder(base: int) -> fn(int) -> int {\n    |x| x + base\n}\n\nfn compose(f: fn(int) -> int, g: fn(int) -> int) -> fn(int) -> int {\n    |x| f(g(x))\n}\n\nfn main() -> int {\n    let inc = |x| x + 1;\n    let dbl = |x| x * 2;\n    let both = compose(inc, dbl);\n    print(both(3));\n    let add5 = make_adder(5);\n    print(add5(3));\n    print(apply(|n| n * 2, 21));\n    return 0;\n}\n",
    )
    .expect("write source");

    let rust = dir.join("closures.rs");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("-o")
        .arg(&rust)
        .arg("--target")
        .arg("rust")
        .output()
        .expect("emit rust");
    assert!(emit.status.success(), "emit failed: {}", String::from_utf8_lossy(&emit.stderr));

    let binary = dir.join("closures-rust");
    let compile = Command::new("rustc")
        .arg(&rust)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run rustc");
    assert!(
        compile.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let rust_run = Command::new(&binary).output().expect("run rust binary");
    assert_eq!(String::from_utf8_lossy(&rust_run.stdout), "7\n8\n42\n");

    if c_compiler_available() {
        let c_run = Command::new(compiler())
            .arg("run")
            .arg(&source)
            .output()
            .expect("run C program");
        assert_eq!(
            String::from_utf8_lossy(&rust_run.stdout),
            String::from_utf8_lossy(&c_run.stdout)
        );
    }
}

#[test]
fn rust_output_compiles_and_runs() {
    if !rustc_available() {
        eprintln!("skipping: rustc is not installed");
        return;
    }

    let dir = scratch("rust-run");
    let source = dir.join("typed.nova");
    fs::write(
        &source,
        "fn main() {\n    let flag: bool = true;\n    let n: int = 7;\n    let name = \"nova\";\n    let f = 1.5;\n    print(flag);\n    print(n);\n    print(name);\n    print(f);\n}\n",
    )
    .expect("write source");
    let rust = dir.join("typed.rs");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("-o")
        .arg(&rust)
        .arg("--target")
        .arg("rust")
        .output()
        .expect("emit rust");
    assert!(emit.status.success(), "emit failed: {}", String::from_utf8_lossy(&emit.stderr));

    let binary = dir.join("typed-rust");
    let compile = Command::new("rustc")
        .arg(&rust)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("run rustc");
    assert!(
        compile.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&binary).output().expect("run rust binary");
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n7\nnova\n1.5\n");
}

#[test]
fn emitted_c_links_against_the_generated_runtime() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }

    let dir = scratch("emit-runtime");
    let source = dir.join("hello.nova");
    fs::write(&source, "print(\"Hello, NOVA\");\n").expect("write source");

    let c_file = dir.join("hello.c");
    let emit = Command::new(compiler())
        .arg("emit")
        .arg(&source)
        .arg("-o")
        .arg(&c_file)
        .output()
        .expect("run nova-compiler emit");
    assert!(
        emit.status.success(),
        "emit failed: {}",
        String::from_utf8_lossy(&emit.stderr)
    );

    let header = dir.join("nova_runtime.h");
    let runtime_c = dir.join("nova_runtime.c");
    assert!(c_file.exists(), "no generated C");
    assert!(header.exists(), "emit did not write nova_runtime.h");
    assert!(runtime_c.exists(), "emit did not write nova_runtime.c");

    let c_code = fs::read_to_string(&c_file).expect("read generated C");
    assert!(
        c_code.contains("#include \"nova_runtime.h\""),
        "generated C must include the runtime: {c_code}"
    );
    assert!(
        !c_code.contains("printf"),
        "generated C should go through the runtime, not libc: {c_code}"
    );
    assert!(
        c_code.contains("nova_print_str") && c_code.contains("nova_print_newline"),
        "generated C should call the runtime print functions: {c_code}"
    );

    let binary = dir.join("hello");
    let mut link = Command::new(cc());
    link.arg(&c_file).arg(&runtime_c).arg("-o").arg(&binary);
    // The runtime shim calls libm (`sqrt`, `sin`, …), so a Linux link needs `-lm`.
    if !cfg!(windows) {
        link.arg("-lm");
    }
    let build = link.output().expect("link generated C with the runtime");
    assert!(
        build.status.success(),
        "cc failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run linked binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Hello, NOVA\n");
}

#[test]
fn matching_on_an_option_prints_both_branches() {
    // ADR 0022, step 0. `docs/language/12_errors.md` shows this pattern as the one to copy, and
    // nothing compiled it: the chapter's block has no `fn main`, so
    // `every_complete_program_in_the_spec_compiles` skips it. Whether `match` on an `Option` works
    // end to end was an assumption until this test existed.
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let dir = scratch("option-match");
    let source = dir.join("option_match.nova");
    fs::write(
        &source,
        "use core;\n\n\
fn divide(a: int, b: int) -> Option<int> {\n    \
    if b == 0 {\n        \
        return None;\n    \
    }\n    \
    Some(a / b)\n\
}\n\n\
fn main() -> int {\n    \
    match divide(10, 0) {\n        \
        Some(value) => print(value),\n        \
        None => print(-1),\n    \
    }\n    \
    match divide(10, 2) {\n        \
        Some(value) => print(value),\n        \
        None => print(-1),\n    \
    }\n    \
    return 0;\n\
}\n",
    )
    .expect("write source");
    let binary = dir.join("option_match");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "the chapter's Option/match pattern does not compile:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run the program");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(
        stdout, "-1\n5\n",
        "expected the None branch then the Some branch; got {stdout:?}"
    );
}

#[test]
fn a_result_needs_no_enum_declaration_and_no_prefix() {
    // ADR 0022, corrected 2026-11-27. `Result<T, E>` already works as a **user enum** —
    // `generic_enum_payloads_and_growable_arrays_run` proves a match binds an `int` in one arm and
    // a `String` in the other. What was missing is smaller: the declaration and the `Result.`
    // prefix. This program is written the way a user would write it.
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let dir = scratch("result-sugar");
    let source = dir.join("result_sugar.nova");
    fs::write(
        &source,
        "use core;\n\n\
fn parse_positive(text: String) -> Result<int, String> {\n    \
    if text == \"\" {\n        \
        return Err(\"empty\");\n    \
    }\n    \
    Ok(1)\n\
}\n\n\
fn main() -> int {\n    \
    match parse_positive(\"\") {\n        \
        Ok(n) => print(n),\n        \
        Err(reason) => print(reason),\n    \
    }\n    \
    match parse_positive(\"x\") {\n        \
        Ok(n) => print(n),\n        \
        Err(reason) => print(reason),\n    \
    }\n    \
    return 0;\n\
}\n",
    )
    .expect("write source");
    let binary = dir.join("result_sugar");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "`Result` sugar does not compile:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run the program");
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "empty\n1\n",
        "expected the Err branch then the Ok branch"
    );
}
