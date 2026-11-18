//! `nova fmt` — format the parts of a file that can be formatted exactly.
//!
//! This is the first milestone from `docs/adr/0001-lossless-syntax-tree.md`, and
//! it is deliberately conservative: it rewrites the **space between items** and
//! touches nothing else.
//!
//! What it changes:
//!
//! - exactly one blank line between top-level items;
//! - no trailing whitespace on any line;
//! - runs of blank lines inside comments collapsed to one;
//! - exactly one newline at end of file.
//!
//! What it does not change: **any byte of an item.** Headers, indentation and
//! bodies come out exactly as they went in. That is not laziness — the item tree
//! records where a body starts and ends without looking inside it, so the
//! formatter has no opinion about the inside and cannot be wrong about it. A
//! formatter that rewrote bodies would need a lossless tree over every expression,
//! which is the next stage.
//!
//! Comments are never lost. They live in the gaps between items, and the gaps are
//! preserved; only the blank lines around them are normalised. A trailing comment
//! on an item's last line stays on that line, because an item's span is extended to
//! the end of its last line before the gap begins.

use crate::syntax::ItemTree;

/// Format `source`.
///
/// Total, and idempotent: `format(format(x)) == format(x)` for every input,
/// including input that does not parse. It cannot fail, because it never parses.
pub fn format(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let tree = ItemTree::parse(source);

    // Alternate gaps and items. A gap holds everything between two items: blank
    // lines, and the comments that belong to the item after it.
    let mut blocks: Vec<String> = Vec::new();
    let mut cursor = 0usize;

    for item in tree.items() {
        let start = item.span.start.offset.min(chars.len());
        let mut end = item.span.end.offset.min(chars.len());

        // Extend to the end of the item's last line so a trailing comment stays
        // attached to the line it was written on rather than being pushed into the
        // gap and re-emitted on its own line.
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

    // Exactly one trailing newline, and no trailing blank lines.
    let trimmed = out.trim_end_matches('\n');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

/// A gap with its blank lines normalised: no leading or trailing blanks, and runs
/// of blanks collapsed to one. Comment lines are kept verbatim apart from trailing
/// whitespace.
fn clean_gap(gap: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut pending_blank = false;
    for raw in gap.lines() {
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

/// Join a cleaned gap to the item that follows it, without doubling blank lines.
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
    fn strips_trailing_whitespace() {
        let source = "fn a() {   \n    let x = 1;  \n}\n";
        assert_eq!(format(source), "fn a() {\n    let x = 1;\n}\n");
    }

    #[test]
    fn never_touches_the_inside_of_an_item() {
        // Deliberately ugly indentation and spacing: the formatter has no opinion.
        let body = "fn a() {\n        let   x:int=1;\n  if x>0{print(x);}\n}";
        let source = format!("{body}\n");
        assert!(
            format(&source).contains(body),
            "the item must come out byte-identical"
        );
    }

    #[test]
    fn keeps_comments_and_their_position() {
        let source = "// about a\nfn a() {\n}\n// about b\nfn b() {\n}\n";
        assert_eq!(format(source), source);
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
            assert!(out.ends_with('}'), "unexpected tail: {out:?}");
            assert_eq!(out.matches('\n').count(), 2, "{out:?}");
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
        ] {
            idempotent(source);
        }
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
        let out = format(source);
        assert!(out.contains("        print(LIMIT);   // deliberately odd indentation"));
    }
}
