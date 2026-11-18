//! The formatter's promise, checked against every program this repository ships.
//!
//! `nova fmt` says it changes whitespace and never a token. That is checkable, and
//! it is the right claim: "the bytes are identical" cannot be true of a formatter
//! whose job includes removing trailing whitespace, but "the tokens are identical"
//! can.
//!
//! Note what is *not* asserted: that the repository is formatted. Requiring that
//! would mean committing a large reformat, and the claim worth testing is that the
//! formatter is safe — not that it has been run.

use nova_compiler::fmt::format;
use nova_compiler::syntax::ItemTree;
use nova_compiler::Lexer;
use std::fs;
use std::path::{Path, PathBuf};

/// Every `.nova` file under the repository, found by walking rather than by a glob
/// crate, because the compiler has no dependencies and its tests should not need
/// one either.
fn nova_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip build output and the fuzz crate's own tree.
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "nova") {
                out.push(path);
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the compiler crate has a parent")
        .to_path_buf();
    let mut out = Vec::new();
    for dir in ["examples", "stdlib", "tests/parity", "tests/diagnostics"] {
        walk(&root.join(dir), &mut out);
    }
    out.sort();
    out
}

/// Every token's kind and text, in order. Spans are deliberately excluded: blank
/// lines move, so positions change and positions are not the promise.
fn tokens(source: &str) -> Vec<(String, String)> {
    let mut lexer = Lexer::new(source);
    let (tokens, _diagnostics) = lexer.tokenize();
    tokens
        .iter()
        .map(|token| (format!("{:?}", token.kind), token.lexeme.clone()))
        .collect()
}

#[test]
fn there_is_something_to_check() {
    let files = nova_files();
    assert!(
        files.len() >= 20,
        "expected to find the shipped programs, found {}",
        files.len()
    );
}

#[test]
fn formatting_changes_no_token() {
    for path in nova_files() {
        let source = fs::read_to_string(&path).expect("read");
        let formatted = format(&source);
        let before = tokens(&source);
        let after = tokens(&formatted);
        assert_eq!(
            before.len(),
            after.len(),
            "{}: the token count changed",
            path.display()
        );
        for (index, (a, b)) in before.iter().zip(after.iter()).enumerate() {
            assert_eq!(a, b, "{}: token {index} changed", path.display());
        }
    }
}

#[test]
fn formatting_is_idempotent_on_every_shipped_program() {
    for path in nova_files() {
        let source = fs::read_to_string(&path).expect("read");
        let once = format(&source);
        let twice = format(&once);
        assert_eq!(once, twice, "{}: not idempotent", path.display());
    }
}

#[test]
fn formatting_does_not_lose_comments() {
    for path in nova_files() {
        let source = fs::read_to_string(&path).expect("read");
        let formatted = format(&source);
        let comments = |text: &str| {
            text.lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    trimmed.starts_with("//") || trimmed.starts_with('#')
                })
                .map(|line| line.trim_end().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            comments(&source),
            comments(&formatted),
            "{}: a comment was moved or lost",
            path.display()
        );
    }
}

#[test]
fn formatting_does_not_change_the_program() {
    // The strongest statement available without running the compiler twice: the
    // formatted text still parses to the same item structure.
    for path in nova_files() {
        let source = fs::read_to_string(&path).expect("read");
        let formatted = format(&source);
        let before: Vec<_> = ItemTree::parse(&source)
            .items()
            .iter()
            .map(|item| (item.kind, item.name.clone()))
            .collect();
        let after: Vec<_> = ItemTree::parse(&formatted)
            .items()
            .iter()
            .map(|item| (item.kind, item.name.clone()))
            .collect();
        assert_eq!(before, after, "{}: the item structure changed", path.display());
    }
}
