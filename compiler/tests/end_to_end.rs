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
        stderr.contains("not implemented"),
        "unexpected stderr: {stderr}"
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
