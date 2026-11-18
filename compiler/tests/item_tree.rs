//! The item tree and the parser must agree about what an item is.
//!
//! They are two independent walks over the same source: `parser` builds a typed AST,
//! `syntax::ItemTree` records where items start and end without looking inside them.
//! Nothing compares them, so nothing stops them drifting — one gains a keyword the
//! other does not know, and a tool built on the item tree silently stops seeing a
//! whole class of declaration.
//!
//! This is the guard. It is the same idea as `tools/grammar-check.py`, which keeps
//! `grammar/grammar.ebnf` describing the parser: two descriptions of one language,
//! compared by a test rather than by hope.
//!
//! One asymmetry is expected and allowed: the parser turns a bare top-level
//! expression into a synthetic `Function` named `__main`, so that `print("hi");` at
//! the top of a file becomes a program. The item tree sees no item keyword there and
//! reports nothing. Those are filtered out.

use nova_compiler::syntax::{ItemKind, ItemTree};
use nova_compiler::{Lexer, Parser};
use nova_common::{Item, Program};
use std::fs;
use std::path::{Path, PathBuf};

fn nova_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
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
    for dir in ["examples", "stdlib", "tests/parity"] {
        walk(&root.join(dir), &mut out);
    }
    out.sort();
    out
}

fn parse(source: &str) -> Option<Program> {
    let mut lexer = Lexer::new(source);
    let (tokens, diagnostics) = lexer.tokenize();
    if diagnostics.has_errors() {
        return None;
    }
    Parser::new(tokens).parse().ok()
}

/// The parser's items as (kind, name), dropping the synthetic `__main` functions that
/// stand in for bare top-level expressions.
fn from_parser(program: &Program) -> Vec<(ItemKind, Option<String>)> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(f) if f.name == "__main" => None,
            Item::Function(f) => Some((ItemKind::Function, Some(f.name.clone()))),
            Item::Struct(s) => Some((ItemKind::Struct, Some(s.name.clone()))),
            Item::Enum(e) => Some((ItemKind::Enum, Some(e.name.clone()))),
            Item::Const(c) => Some((ItemKind::Const, Some(c.name.clone()))),
            Item::TypeAlias(t) => Some((ItemKind::TypeAlias, Some(t.name.clone()))),
            Item::Mod(m) => Some((ItemKind::Module, Some(m.name.clone()))),
            // These carry a path or a type rather than a declared name.
            Item::Use(_) => Some((ItemKind::Use, None)),
            Item::Impl(_) => Some((ItemKind::Impl, None)),
        })
        .collect()
}

fn from_tree(source: &str) -> Vec<(ItemKind, Option<String>)> {
    ItemTree::parse(source)
        .items()
        .iter()
        .map(|item| (item.kind, item.name.clone()))
        .collect()
}

#[test]
fn there_is_something_to_compare() {
    let files = nova_files();
    // Five examples, four stdlib modules, six parity programs.
    assert!(files.len() >= 15, "found only {} programs", files.len());
}

#[test]
fn the_item_tree_and_the_parser_agree_on_every_shipped_program() {
    let mut compared = 0;
    for path in nova_files() {
        let source = fs::read_to_string(&path).expect("read");
        let Some(program) = parse(&source) else {
            // A program that does not parse is a different test's problem.
            continue;
        };
        compared += 1;
        assert_eq!(
            from_tree(&source),
            from_parser(&program),
            "{}: the item tree and the parser disagree",
            path.display()
        );
    }
    assert!(compared >= 15, "only {compared} programs were compared");
}

#[test]
fn a_top_level_expression_is_not_an_item() {
    // The asymmetry this guard filters out, stated so it stays deliberate: the
    // parser invents a `__main` function so that a bare `print(...)` is a program.
    let source = "print(\"hi\");\n";
    assert!(from_tree(source).is_empty(), "no item keyword, so no item");
    let program = parse(source).expect("a bare expression parses");
    assert_eq!(program.items.len(), 1, "the parser synthesises one function");
    assert!(from_parser(&program).is_empty(), "and the guard filters it");
}

#[test]
fn a_trait_is_not_an_item_because_it_is_not_in_the_language() {
    // `trait` is not a keyword, so it lexes as an identifier and the item tree sees
    // no item. The AST has no `Item::Trait` either — see docs/adr/0002-traits.md.
    let source = "trait Show {\n    fn show(&self) -> string;\n}\n";
    assert!(
        from_tree(source).is_empty(),
        "a trait must not appear as an item"
    );
}
