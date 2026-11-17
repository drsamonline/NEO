//! NOVA Common - Shared types and utilities for the NOVA compiler ecosystem

#![forbid(unsafe_code)]
// This crate has no `unsafe` in it, and this line keeps it that way: a
// future `unsafe` block fails the build instead of arriving in review.
// The runtime crate is the one exception and says why in its own source.

pub mod span;
pub mod token;
pub mod ast;
pub mod types;
pub mod diagnostics;
pub mod symbol_table;
pub mod sir;

pub use span::{Span, Position};
pub use token::{Token, TokenKind};
pub use ast::{AstNode, Program, Item, Function, Param, TypeExpr, Block, Statement, LetStmt, Pattern, Expr, BinOp, UnOp, Literal, StructDef, EnumDef, EnumVariant, Field, ConstDef, UseDecl, ModuleDecl, ImplBlock, MatchArm};
pub use types::{Type, TypeKind, PrimitiveType};
pub use diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};
pub use symbol_table::{SymbolTable, Symbol, SymbolKind};

/// Escape text for a C, JavaScript or Rust string literal.
///
/// All three backends emit string literals by wrapping the text in quotes, and
/// all three used to escape only `\\` and `"`. A NOVA string written `"a\nb"`
/// reaches the backends as a real newline — the lexer has already turned the
/// escape into the character — so the literal was written across two lines and
/// the output did not compile. Tab and carriage return had the same problem.
///
/// Only these six are rewritten. Any other control character is left alone
/// because a raw one is legal in all three target languages, while a raw
/// newline is not.
pub fn escape_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c => out.push(c),
        }
    }
    out
}

/// Escape text for a JSON string literal.
///
/// JSON is not C. A raw UTF-8 character is legal in a JSON string and is left
/// alone here; a control character must be `\u`-escaped, and the C escaper above
/// emits `\0` where JSON needs `\u0000`. So the two cannot share a function, and
/// reusing the wrong one produces JSON that parses in some readers and not
/// others.
///
/// The caller supplies the surrounding quotes.
pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod json_escape_tests {
    use super::escape_json_string;

    #[test]
    fn the_characters_json_requires_escaping() {
        assert_eq!(escape_json_string("a\"b"), "a\\\"b");
        assert_eq!(escape_json_string("a\\b"), "a\\\\b");
        assert_eq!(escape_json_string("a\nb"), "a\\nb");
        assert_eq!(escape_json_string("a\rb"), "a\\rb");
        assert_eq!(escape_json_string("a\tb"), "a\\tb");
        assert_eq!(escape_json_string("a\u{08}b"), "a\\bb");
        assert_eq!(escape_json_string("a\u{0c}b"), "a\\fb");
    }

    #[test]
    fn a_control_character_becomes_a_u_escape_not_a_c_escape() {
        // The C escaper writes `\0` here. That is not valid JSON.
        assert_eq!(escape_json_string("a\0b"), "a\\u0000b");
        assert_eq!(escape_json_string("\u{1b}"), "\\u001b");
    }

    #[test]
    fn utf8_is_left_raw_because_json_allows_it() {
        assert_eq!(escape_json_string("\u{0928}\u{092e}\u{0938}\u{094d}\u{0924}\u{0947}"),
                   "\u{0928}\u{092e}\u{0938}\u{094d}\u{0924}\u{0947}");
        assert_eq!(escape_json_string("\u{2014}"), "\u{2014}");
        assert_eq!(escape_json_string(""), "");
    }
}

#[cfg(test)]
mod escape_tests {
    use super::escape_string_literal;

    #[test]
    fn the_six_that_break_a_literal() {
        assert_eq!(escape_string_literal("a\nb"), "a\\nb");
        assert_eq!(escape_string_literal("a\tb"), "a\\tb");
        assert_eq!(escape_string_literal("a\rb"), "a\\rb");
        assert_eq!(escape_string_literal("a\0b"), "a\\0b");
        assert_eq!(escape_string_literal("a\\b"), "a\\\\b");
        assert_eq!(escape_string_literal("a\"b"), "a\\\"b");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        assert_eq!(escape_string_literal("Hello, World!"), "Hello, World!");
        assert_eq!(escape_string_literal("नमस्ते"), "नमस्ते");
        assert_eq!(escape_string_literal(""), "");
    }
}
