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
        "enum Result<T, E> {\n    Ok(T),\n    Err(E),\n}\n\nfn show(r: Result<int, String>) {\n    match r {\n        Result.Ok(v) => { print(v); },\n        Result.Err(e) => { print(e); },\n    }\n}\n\nfn main() {\n    show(Result.Ok(7));\n    show(Result.Err(\"bad\"));\n    let values: Array = [1, 2];\n    __builtin_array_push(values, 3);\n    print(__builtin_array_len(values));\n    print(__builtin_array_pop(values));\n}\n",
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
    let build = Command::new(cc())
        .arg(&c_file)
        .arg(&runtime_c)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("link generated C with the runtime");
    assert!(
        build.status.success(),
        "cc failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run linked binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Hello, NOVA\n");
}
