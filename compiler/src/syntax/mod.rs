//! A lossless map of a file's top-level items.
//!
//! This is the cheap half of the syntax-tree work in `DEV_PLAN.md` Stage 4, and
//! the shape recorded in `docs/adr/0001-lossless-syntax-tree.md`.
//!
//! An item's **header** is recorded exactly — kind, name, the name's span, and the
//! span of the whole item. Its **body** is recorded as a `Span` and nothing else.
//! Nothing inside a body is interpreted until someone asks for it, which is what
//! makes file-level indexing cheap: an outline, folding ranges and a workspace
//! symbol search need the top level, and they get it from one pass over the tokens
//! without parsing a single expression.
//!
//! # The invariant
//!
//! A deferred body holds a `Span` into the source and nothing else. Not a partial
//! tree, not a normalised string, not an interpretation. Anything more would be
//! lossy, and a lossy deferral cannot be expanded correctly later — you would have
//! thrown away the text you needed.
//!
//! `Span`'s `offset` is a **character** offset, because that is what the lexer
//! records: it indexes a `Vec<char>`. Every consumer here wants line and column
//! anyway, which the same `Span` carries.
//!
//! # What this is not
//!
//! Not error recovery. A deferred body is text that has not been looked inside; a
//! recovery node would be text that failed to parse and was skipped. Conflating
//! them is how a formatter ends up reformatting garbage and an editor ends up
//! offering navigation into it. This module never produces a recovery node — it
//! does not parse, so it cannot fail.

use crate::lexer::Lexer;
use nova_common::{Span, Token, TokenKind};

/// What kind of top-level item this is.
///
/// There is no `Trait`, and there is no `Item::Trait` either: `trait` is not a
/// keyword, the grammar has no production for it and the parser cannot produce one.
/// See `docs/adr/0002-traits.md` — the keyword, the grammar production and the
/// parser land together or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Function,
    Struct,
    Enum,
    Const,
    TypeAlias,
    Use,
    Module,
    Impl,
}

impl ItemKind {
    fn from_token(kind: &TokenKind) -> Option<Self> {
        Some(match kind {
            TokenKind::Fn => Self::Function,
            TokenKind::Struct => Self::Struct,
            TokenKind::Enum => Self::Enum,
            TokenKind::Const => Self::Const,
            TokenKind::Type => Self::TypeAlias,
            TokenKind::Use => Self::Use,
            TokenKind::Mod => Self::Module,
            TokenKind::Impl => Self::Impl,
            _ => return None,
        })
    }

    /// The word a person would use for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Const => "const",
            Self::TypeAlias => "type",
            Self::Use => "use",
            Self::Module => "module",
            Self::Impl => "impl",
        }
    }
}

/// One top-level item, with its body left unparsed.
#[derive(Debug, Clone)]
pub struct Item {
    pub kind: ItemKind,
    /// The declared name, when the item has one. `use` has a path, `impl` has a
    /// type — neither is a name in the sense a symbol list means.
    pub name: Option<String>,
    /// The name token's own span, for an editor's `selectionRange`: the identifier,
    /// not the line.
    pub name_span: Option<Span>,
    /// From the item's keyword to the end of its last token.
    pub span: Span,
    /// The outermost `{ … }` of the item, braces included, unparsed. `None` for
    /// items with no body, such as `use` and `const`.
    pub body: Option<Span>,
    /// The token window inside `body`, so expanding it costs no second lex.
    body_tokens: Option<(usize, usize)>,
}

/// Every top-level item of a file, plus the tokens they were found in.
pub struct ItemTree {
    items: Vec<Item>,
    tokens: Vec<Token>,
}

impl ItemTree {
    /// Lex a file and record its top-level items. Nothing inside a body is parsed.
    pub fn parse(source: &str) -> Self {
        let mut lexer = Lexer::new(source);
        let (tokens, _diagnostics) = lexer.tokenize();
        let items = scan(&tokens, 0, tokens.len());
        Self { items, tokens }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Expand a deferred body: the items declared directly inside it.
    ///
    /// This is the "on demand" half. An outline asks for the methods of an `impl`
    /// and pays for them; a file-level symbol search never asks and never pays.
    /// The cost is a walk over tokens that were already lexed, so it is linear in
    /// the size of the body and allocates nothing but the result.
    pub fn children_of(&self, item: &Item) -> Vec<Item> {
        match item.body_tokens {
            Some((start, end)) => scan(&self.tokens, start, end),
            None => Vec::new(),
        }
    }

    /// How many tokens the file lexed to. Reported so a test can tell a real
    /// deferral (few items, many tokens) from a scan that gave up.
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

/// Find the items at this nesting level within a token window.
///
/// The window is always a level's own contents, with no enclosing braces in it, so
/// depth starts at zero and "top level of this window" is the same test everywhere.
fn scan(tokens: &[Token], from: usize, to: usize) -> Vec<Item> {
    let mut items = Vec::new();
    let mut open: Option<(usize, ItemKind)> = None;
    let mut depth = 0usize;

    let mut index = from;
    while index < to {
        if depth == 0 {
            if let Some(kind) = ItemKind::from_token(&tokens[index].kind) {
                if let Some((start, kind)) = open.take() {
                    items.push(build(tokens, start, index, kind));
                }
                open = Some((index, kind));
            }
        }
        match tokens[index].kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }

    if let Some((start, kind)) = open {
        items.push(build(tokens, start, to, kind));
    }
    items
}

/// Assemble one item from the token window it occupies.
fn build(tokens: &[Token], start: usize, end: usize, kind: ItemKind) -> Item {
    // `end` is exclusive and may be `tokens.len()`; an empty window would mean a
    // zero-width item, which cannot happen because `start` is inside it.
    let last = end.saturating_sub(1).max(start);
    let span = Span::new(tokens[start].span.start, tokens[last].span.end);

    // The name is the first identifier after the keyword, but not one that belongs
    // to a body: `impl` and `use` carry a path rather than a name.
    let mut name = None;
    let mut name_span = None;
    if matches!(kind, ItemKind::Function | ItemKind::Struct | ItemKind::Enum
                     | ItemKind::Const | ItemKind::TypeAlias | ItemKind::Module) {
        for token in &tokens[start + 1..end] {
            match &token.kind {
                TokenKind::Identifier(text) => {
                    name = Some(text.clone());
                    name_span = Some(token.span);
                    break;
                }
                // Stop at the body: `const X: int = { … }` has no identifier to
                // find beyond the first, and a nested one is not this item's name.
                TokenKind::LBrace => break,
                _ => {}
            }
        }
    }

    let (body, body_tokens) = match outermost_block(tokens, start, end) {
        Some((open, close)) => (
            Some(Span::new(tokens[open].span.start, tokens[close].span.end)),
            Some((open + 1, close)),
        ),
        None => (None, None),
    };

    Item { kind, name, name_span, span, body, body_tokens }
}

/// The outermost `{ … }` of an item, as token indices of the braces.
fn outermost_block(tokens: &[Token], start: usize, end: usize) -> Option<(usize, usize)> {
    let mut open = None;
    let mut depth = 0usize;
    for index in start..end {
        match tokens[index].kind {
            TokenKind::LBrace => {
                if depth == 0 && open.is_none() {
                    open = Some(index);
                }
                depth += 1;
            }
            TokenKind::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    if let Some(open) = open {
                        return Some((open, index));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: &str = "\
use core;

const LIMIT: int = 10;

struct Counter {
    value: int,
}

enum Mode {
    On,
    Off,
}

impl Counter {
    fn bump(&mut self) {
        self.value = self.value + 1;
    }

    fn reset(&mut self) {
        self.value = 0;
    }
}

fn main() -> int {
    print(LIMIT);
    return 0;
}
";

    fn names(tree: &ItemTree) -> Vec<(&'static str, Option<&str>)> {
        tree.items()
            .iter()
            .map(|item| (item.kind.label(), item.name.as_deref()))
            .collect()
    }

    #[test]
    fn finds_every_top_level_item_in_order() {
        let tree = ItemTree::parse(PROGRAM);
        assert_eq!(
            names(&tree),
            vec![
                ("use", None),
                ("const", Some("LIMIT")),
                ("struct", Some("Counter")),
                ("enum", Some("Mode")),
                ("impl", None),
                ("function", Some("main")),
            ]
        );
    }

    #[test]
    fn a_method_is_not_a_top_level_item() {
        // `bump` and `reset` are inside an `impl` body, so the top level must not
        // see them — that is the whole point of deferring the body.
        let tree = ItemTree::parse(PROGRAM);
        let top: Vec<&str> = tree
            .items()
            .iter()
            .filter_map(|item| item.name.as_deref())
            .collect();
        assert!(!top.contains(&"bump"), "top level leaked a method: {top:?}");
        assert!(!top.contains(&"reset"), "top level leaked a method: {top:?}");
    }

    #[test]
    fn a_body_is_expanded_only_when_asked() {
        let tree = ItemTree::parse(PROGRAM);
        let impl_item = tree
            .items()
            .iter()
            .find(|item| item.kind == ItemKind::Impl)
            .expect("impl item");

        let methods: Vec<String> = tree
            .children_of(impl_item)
            .iter()
            .filter_map(|item| item.name.clone())
            .collect();
        assert_eq!(methods, vec!["bump", "reset"]);
    }

    #[test]
    fn a_function_body_is_deferred_not_empty() {
        let tree = ItemTree::parse(PROGRAM);
        let main = tree
            .items()
            .iter()
            .find(|item| item.name.as_deref() == Some("main"))
            .expect("main");
        let body = main.body.expect("main has a body");
        assert!(
            body.end.offset > body.start.offset,
            "the body span must cover the block"
        );
        // The body of `main` is the block, so its braces are not top-level items.
        assert!(tree.children_of(main).is_empty(), "main declares no items");
    }

    #[test]
    fn an_item_with_no_body_has_none() {
        let tree = ItemTree::parse(PROGRAM);
        for item in tree.items() {
            if matches!(item.kind, ItemKind::Use | ItemKind::Const) {
                assert!(item.body.is_none(), "{} should have no body", item.kind.label());
            }
        }
    }

    #[test]
    fn the_name_span_is_the_identifier_not_the_line() {
        let tree = ItemTree::parse(PROGRAM);
        let structure = tree
            .items()
            .iter()
            .find(|item| item.name.as_deref() == Some("Counter") && item.kind == ItemKind::Struct)
            .expect("struct Counter");
        let span = structure.name_span.expect("a struct has a name span");
        // `struct Counter {` — `Counter` starts at column 8.
        assert_eq!(span.start.column, 8);
        assert_eq!(span.end.column, 15);
        assert_eq!(span.start.line, structure.span.start.line);
    }

    #[test]
    fn malformed_source_does_not_panic() {
        // No closing brace, a stray brace, and an item that ends at end-of-file.
        for source in [
            "fn broken() {",
            "} fn also_broken() {",
            "struct {",
            "fn",
            "}}}}",
            "",
            "fn a() { fn b() {} }",
        ] {
            let tree = ItemTree::parse(source);
            // Only the shape matters: it must return, not panic.
            let _ = tree.items().len();
        }
    }
}
