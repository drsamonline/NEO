//! NOVA Common - Shared types and utilities for the NOVA compiler ecosystem

pub mod span;
pub mod token;
pub mod ast;
pub mod types;
pub mod diagnostics;
pub mod symbol_table;

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
