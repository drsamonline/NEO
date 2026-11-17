//! Cross-crate integration tests: NOVA source in, generated code out.
//!
//! Everything here goes through the same path the CLI uses —
//! `Lexer` → `Parser` → `SemanticAnalyzer` → `Lowerer` → backend — so a break in
//! the handover between two crates fails here and not only in the CLI's
//! end-to-end tests.
//!
//! The unit tests in each crate check one layer. `compiler/tests/end_to_end.rs`
//! checks source to a linked binary. This crate sits between them: source to
//! emitted text, for every backend, with no C compiler and no `node` needed.
//!
//! These tests previously did not compile at all: they imported
//! `nova_compiler::frontend::lexer`, `nova_compiler::middle::sir` and
//! `nova_compiler::frontend::semantic_analyzer`, and none of those modules
//! exist. The crate was also not a workspace member, so nothing noticed.

#![forbid(unsafe_code)]
// This crate has no `unsafe` in it, and this line keeps it that way: a
// future `unsafe` block fails the build instead of arriving in review.
// The runtime crate is the one exception and says why in its own source.

use nova_backend_js::JsBackend;
use nova_backend_rust::RustBackend;
use nova_backend_wasm::WasmBackend;
use nova_compiler::sir::SirProgram;
use nova_compiler::{CBackend, Lexer, Lowerer, Parser, SemanticAnalyzer};

/// Run the front end and return the SIR, or the diagnostics that stopped it.
///
/// This mirrors `compile_to` in `compiler/src/main.rs`. If the two ever disagree
/// about the order of the passes, this is the test that notices.
fn lower(source: &str) -> Result<SirProgram, String> {
    let mut lexer = Lexer::new(source);
    let (tokens, diagnostics) = lexer.tokenize();
    if diagnostics.has_errors() {
        let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        return Err(format!("lexical: {}", messages.join("; ")));
    }

    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => program,
        Err(diagnostics) => {
            let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
            return Err(format!("parse: {}", messages.join("; ")));
        }
    };

    let mut analyzer = SemanticAnalyzer::new();
    if analyzer.analyze(&program).is_err() {
        let messages: Vec<&str> = analyzer
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect();
        return Err(format!("semantic: {}", messages.join("; ")));
    }

    let mut lowerer = Lowerer::new();
    Ok(lowerer.lower_program(&program))
}

const HELLO: &str = r#"
fn main() -> int {
    print("Hello, NOVA");
    return 0;
}
"#;

#[test]
fn hello_world_reaches_c_with_its_runtime_header() {
    let sir = lower(HELLO).expect("hello world must pass the front end");
    let c = CBackend::new().generate(&sir);

    assert!(c.contains("#include \"nova_runtime.h\""), "no runtime header:\n{c}");
    assert!(c.contains("int main(void)"), "no C entry point:\n{c}");
    assert!(c.contains("Hello, NOVA"), "the string literal was lost:\n{c}");
}

#[test]
fn arithmetic_and_bindings_survive_lowering() {
    let source = r#"
fn add(a: int, b: int) -> int {
    return a + b;
}

fn main() -> int {
    let x: int = 40;
    let y: int = add(x, 2);
    print(y);
    return 0;
}
"#;
    let sir = lower(source).expect("arithmetic must pass the front end");
    let c = CBackend::new().generate(&sir);

    assert!(c.contains("int main(void)"), "no C entry point:\n{c}");
    // The helper is a real function, not inlined and not dropped.
    assert!(c.matches("add").count() >= 2, "`add` was not emitted:\n{c}");
}

#[test]
fn a_struct_becomes_a_c_typedef() {
    let source = r#"
struct Counter {
    value: int,
}

fn main() -> int {
    let c = Counter { value: 0 };
    print(c.value);
    return 0;
}
"#;
    let sir = lower(source).expect("a struct literal must pass the front end");
    let c = CBackend::new().generate(&sir);

    assert!(
        c.contains("typedef struct Counter"),
        "the struct did not become a C type:\n{c}"
    );
}

#[test]
fn a_valid_program_produces_no_diagnostics() {
    let mut lexer = Lexer::new(HELLO);
    let (_tokens, diagnostics) = lexer.tokenize();
    assert!(
        diagnostics.is_empty(),
        "a valid program produced diagnostics: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn a_parse_error_is_returned_not_panicked() {
    // `let x: int = ;` has no initialiser. The parser must say so.
    let source = r#"
fn main() -> int {
    let x: int = ;
    return x;
}
"#;
    let mut lexer = Lexer::new(source);
    let (tokens, diagnostics) = lexer.tokenize();
    assert!(
        !diagnostics.has_errors(),
        "this should be a parse error, not a lexical one: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    let mut parser = Parser::new(tokens);
    assert!(parser.parse().is_err(), "a missing initialiser was accepted");
}

#[test]
fn a_wrong_return_type_is_rejected() {
    // Milestone 4 in DEV_PLAN.md: `return 1` from `-> bool` is an error.
    let source = r#"
fn flag() -> bool {
    return 1;
}

fn main() -> int {
    return 0;
}
"#;
    let err = lower(source).expect_err("`-> bool { return 1; }` was accepted");
    assert!(
        err.starts_with("semantic:"),
        "the wrong layer rejected it: {err}"
    );
}

#[test]
fn every_backend_emits_for_the_same_program() {
    let sir = lower(HELLO).expect("hello world must pass the front end");

    let c = CBackend::new().generate(&sir);
    let js = JsBackend::new().generate(&sir);
    let rust = RustBackend::new().generate(&sir);

    for (name, out) in [("c", &c), ("js", &js), ("rust", &rust)] {
        assert!(!out.is_empty(), "the {name} backend emitted nothing");
        assert!(
            out.contains("Hello, NOVA"),
            "the {name} backend lost the string literal"
        );
    }

    // WASM is allowed to reject a program it cannot express, but a hello world
    // is inside the subset it advertises, so it must not reject this one.
    if let Err(unsupported) = nova_backend_wasm::validate(&sir) {
        panic!("the wasm backend rejected hello world as {unsupported}");
    }
    let wasm = WasmBackend::new().generate(&sir);
    assert!(!wasm.is_empty(), "the wasm backend emitted nothing");
}

#[test]
fn lowering_is_deterministic() {
    // Two runs over the same source must produce byte-identical output. A
    // HashMap iteration order leaking into code generation would break the
    // parity harness in a way that looks like a backend bug.
    let first = CBackend::new().generate(&lower(HELLO).expect("front end"));
    let second = CBackend::new().generate(&lower(HELLO).expect("front end"));
    assert_eq!(first, second, "code generation is not deterministic");
}
