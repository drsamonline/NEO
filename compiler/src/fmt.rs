//! `nova fmt` — normalise whitespace, and never touch a token.
//!
//! This is the first milestone from `docs/adr/0001-lossless-syntax-tree.md`. It is
//! deliberately narrow, and the narrowness is the design: the item tree records
//! where a body starts and ends without looking inside it, so the formatter has no
//! opinion about the inside and cannot be wrong about it.
//!
//! # The invariant
//!
//! **Whitespace may change; a token may not.** The output lexes to the same
//! sequence of tokens, with the same text, as the input. That is a stronger promise
//! than "the bytes are identical", and it is the one a formatter can actually keep:
//! trailing whitespace has to go, and removing it changes bytes.
//!
//! What it does:
//!
//! - exactly one blank line between top-level items;
//! - no trailing whitespace on a line — except a line that ends inside a string
//!   literal, where the spaces are part of the string;
//! - runs of blank lines in a comment block collapsed to one;
//! - exactly one newline at end of file.
//!
//! What it does not do: re-indent or reflow anything inside a body. That needs a
//! lossless tree over every expression, which is the next stage.
//!
//! It never parses, so it is total: it cannot fail, which means it works on a file
//! that is half-typed — the moment you most want to run a formatter.

use crate::lexer::Lexer;
use crate::syntax::ItemTree;
use nova_common::TokenKind;
use std::collections::HashSet;

/// Format `source`.
///
/// Total and idempotent: `format(format(x)) == format(x)` for every input,
/// including input that does not parse.
pub fn format(source: &str) -> String {
    // Trailing whitespace first, on the source's own line numbering, because the
    // string-literal guard is expressed in source lines.
    let source = strip_trailing_whitespace(source);
    let chars: Vec<char> = source.chars().collect();
    let tree = ItemTree::parse(&source);

    let mut blocks: Vec<String> = Vec::new();
    let mut cursor = 0usize;

    for item in tree.items() {
        let start = item.span.start.offset.min(chars.len());
        let mut end = item.span.end.offset.min(chars.len());

        // Extend to the end of the item's last line so a trailing comment stays on
        // the line it was written on instead of being pushed into the gap.
        while end < chars.len() && chars[end] != '\n' {
            end += 1;
        }

        if start < cursor {
            // Items cannot overlap; if they ever do, prefer not to lose text.
            continue;
        }

        let gap: String = chars[cursor..start].iter().collect();
        let body: String = chars[start..end].iter().collect();
        blocks.push(join(clean_gap(&gap), body));
        cursor = end;
    }

    let tail: String = chars[cursor.min(chars.len())..].iter().collect();
    let tail = clean_gap(&tail);

    let mut out = blocks.join("\n\n");
    if !tail.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&tail);
    }

    let trimmed = out.trim_end_matches('\n');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

/// Line numbers (1-based) whose end lies inside a string literal.
///
/// A string that spans lines makes the whitespace at the end of those lines part of
/// the string. NOVA's lexer does not currently produce one — a literal ends at the
/// closing quote or the newline — but the guard costs nothing and the alternative is
/// a formatter that silently edits string contents the day that changes.
fn lines_inside_strings(source: &str) -> HashSet<usize> {
    let mut lexer = Lexer::new(source);
    let (tokens, _diagnostics) = lexer.tokenize();
    let mut protected = HashSet::new();
    for token in &tokens {
        if !matches!(token.kind, TokenKind::String(_)) {
            continue;
        }
        let (start, end) = (token.span.start.line, token.span.end.line);
        if end > start {
            for line in start..=end {
                protected.insert(line);
            }
        }
    }
    protected
}

/// Remove trailing whitespace from every line, except where it is inside a string.
fn strip_trailing_whitespace(source: &str) -> String {
    let protected = lines_inside_strings(source);
    let mut out = String::with_capacity(source.len());
    for (index, line) in source.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        if protected.contains(&(index + 1)) {
            out.push_str(line);
        } else {
            out.push_str(line.trim_end());
        }
    }
    out
}

/// A gap with its blank lines normalised: no leading or trailing blanks, and runs
/// of blanks collapsed to one. Comment lines are kept as they are.
fn clean_gap(gap: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut pending_blank = false;
    for raw in gap.split('\n') {
        let line = raw.trim_end();
        if line.is_empty() {
            pending_blank = !lines.is_empty();
            continue;
        }
        if pending_blank {
            lines.push("");
            pending_blank = false;
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Join a cleaned gap to the item that follows it.
fn join(gap: String, body: String) -> String {
    if gap.is_empty() {
        body
    } else {
        format!("{gap}\n{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    /// The invariant: the same tokens, in the same order, with the same text.
    fn tokens(source: &str) -> Vec<(String, String)> {
        let mut lexer = crate::lexer::Lexer::new(source);
        let (tokens, _diagnostics) = lexer.tokenize();
        tokens
            .iter()
            .map(|token| (format!("{:?}", token.kind), token.lexeme.clone()))
            .collect()
    }

    fn assert_tokens_preserved(source: &str) {
        let out = format(source);
        assert_eq!(tokens(source), tokens(&out), "tokens changed for {source:?}");
    }

    fn idempotent(source: &str) {
        let once = format(source);
        let twice = format(&once);
        assert_eq!(once, twice, "formatting is not idempotent for {source:?}");
    }

    #[test]
    fn collapses_blank_lines_between_items() {
        let source = "fn a() {\n}\n\n\n\nfn b() {\n}\n";
        assert_eq!(format(source), "fn a() {\n}\n\nfn b() {\n}\n");
    }

    #[test]
    fn inserts_a_blank_line_between_adjacent_items() {
        let source = "fn a() {\n}\nfn b() {\n}\n";
        assert_eq!(format(source), "fn a() {\n}\n\nfn b() {\n}\n");
    }

    #[test]
    fn strips_trailing_whitespace_inside_a_body_too() {
        let source = "fn a() {   \n    let x = 1;  \n}\n";
        assert_eq!(format(source), "fn a() {\n    let x = 1;\n}\n");
        assert_tokens_preserved(source);
    }

    #[test]
    fn never_reindents_inside_an_item() {
        // Deliberately ugly indentation and spacing: the formatter has no opinion.
        let body = "fn a() {\n        let   x:int=1;\n  if x>0{print(x);}\n}";
        let out = format(&format!("{body}\n"));
        assert!(out.contains(body), "indentation must be preserved: {out}");
        assert_tokens_preserved(body);
    }

    #[test]
    fn a_blank_line_goes_before_a_comment_that_documents_the_next_item() {
        // The comment belongs to `b`, so it separates from `a` — that is the rule,
        // and it matches what a reader expects above a declaration.
        let source = "// about a\nfn a() {\n}\n// about b\nfn b() {\n}\n";
        assert_eq!(format(source), "// about a\nfn a() {\n}\n\n// about b\nfn b() {\n}\n");
    }

    #[test]
    fn keeps_a_trailing_comment_on_its_own_line() {
        let source = "fn a() {\n} // done\n\nfn b() {\n}\n";
        let out = format(source);
        assert!(out.contains("} // done"), "trailing comment moved: {out}");
    }

    #[test]
    fn keeps_a_header_comment_block_attached() {
        let source = "// header\n\n\n// second line\nfn a() {\n}\n";
        assert_eq!(format(source), "// header\n\n// second line\nfn a() {\n}\n");
    }

    #[test]
    fn ends_with_exactly_one_newline() {
        for source in ["fn a() {\n}\n\n\n", "fn a() {\n}"] {
            let out = format(source);
            assert_eq!(out, "fn a() {\n}\n", "unexpected output for {source:?}");
        }
    }

    #[test]
    fn an_empty_file_stays_empty() {
        assert_eq!(format(""), "");
        assert_eq!(format("\n\n\n"), "");
        assert_eq!(format("   \n  \n"), "");
    }

    #[test]
    fn malformed_input_does_not_panic_and_is_idempotent() {
        for source in [
            "fn broken() {",
            "}}}}",
            "struct {",
            "fn a() { fn b() {} }",
            "// only a comment\n",
            "fn a() {\n}\n} stray",
            "fn a() -> string { return \"unterminated\n}\n",
        ] {
            idempotent(source);
            assert_tokens_preserved(source);
        }
    }

    #[test]
    fn a_string_literal_is_never_edited() {
        let source = "fn a() -> string {\n    return \"keep  my   spacing\";\n}\n";
        let out = format(source);
        assert!(out.contains("\"keep  my   spacing\""), "{out}");
        assert_tokens_preserved(source);
    }

    #[test]
    fn is_idempotent_on_a_real_program() {
        let source = "\
// A program with mixed spacing.

use core;


const LIMIT: int = 10;


fn main() -> int {
        print(LIMIT);   // deliberately odd indentation
    return 0;
}


";
        idempotent(source);
        assert_tokens_preserved(source);
        let out = format(source);
        assert!(out.contains("        print(LIMIT);   // deliberately odd indentation"));
    }
}
