//! The shipped standard library must at least be a NOVA program.
//!
//! `stdlib/core`, `stdlib/io`, `stdlib/math` and `stdlib/text` are read by
//! `nova-compiler emit stdlib/*/*.nova`; before milestone 7 none of them
//! lexed, so nothing downstream could ever be tested. This test is the
//! acceptance check for "the standard library parses": every module must lex
//! and parse without diagnostics. It does not claim they compile - lowering
//! structs, enums, `match` and closures is still open.

use std::fs;
use std::path::{Path, PathBuf};

fn nova_files(directory: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory).expect("the stdlib directory should exist");
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            nova_files(&path, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("nova") {
            out.push(path);
        }
    }
}

#[test]
fn every_stdlib_module_parses() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stdlib");
    let mut files = Vec::new();
    nova_files(&root, &mut files);
    files.sort();

    assert_eq!(
        files.len(),
        4,
        "expected the four shipped modules, found {files:?}"
    );

    for file in files {
        let source = fs::read_to_string(&file).expect("readable module");
        let mut lexer = nova_compiler::Lexer::new(source);
        let (tokens, lex_diagnostics) = lexer.tokenize();
        let lex_errors: Vec<String> = lex_diagnostics.iter().map(|d| d.to_string()).collect();
        assert!(
            lex_errors.is_empty(),
            "{} did not lex: {lex_errors:#?}",
            file.display()
        );

        let mut parser = nova_compiler::Parser::new(tokens);
        if let Err(diagnostics) = parser.parse() {
            let errors: Vec<String> = diagnostics.iter().map(|d| d.to_string()).collect();
            panic!("{} did not parse: {errors:#?}", file.display());
        }
    }
}
