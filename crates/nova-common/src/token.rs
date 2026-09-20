//! Lexer tokens and token kinds

use crate::span::Span;

/// Token kinds for the NOVA language
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Integer(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    
    // Identifiers and keywords
    Identifier(String),
    
    // Keywords
    Fn, Let, Mut, If, Else, While, For, Return,
    True, False, Struct, Enum, Match, Use, Mod,
    Pub, Const, Type, As, In, Loop, Break, Continue, Impl,
    
    // Operators
    Plus, Minus, Star, Slash, Percent,
    Equal, EqualEqual, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    And, Or, Not,
    Assign, PlusAssign, MinusAssign, StarAssign, SlashAssign,
    Ampersand, Pipe, Caret, Tilde,
    LeftShift, RightShift,
    
    // Delimiters
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semicolon, Colon, DoubleColon, Dot, DotDot,
    Arrow, FatArrow,
    
    // Special
    Eof,
    Unknown(char),
}

impl TokenKind {
    pub fn is_keyword(&self) -> bool {
        matches!(self,
            TokenKind::Fn | TokenKind::Let | TokenKind::Mut | TokenKind::If |
            TokenKind::Else | TokenKind::While | TokenKind::For | TokenKind::Return |
            TokenKind::True | TokenKind::False | TokenKind::Struct | TokenKind::Enum |
            TokenKind::Match | TokenKind::Use | TokenKind::Mod | TokenKind::Pub |
            TokenKind::Const | TokenKind::Type | TokenKind::As | TokenKind::In |
            TokenKind::Loop | TokenKind::Break | TokenKind::Continue | TokenKind::Impl
        )
    }
}

/// A token with kind and span
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub lexeme: String,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        let lexeme = match &kind {
            TokenKind::Integer(n) => n.to_string(),
            TokenKind::Float(f) => f.to_string(),
            TokenKind::String(s) => s.clone(),
            TokenKind::Char(c) => c.to_string(),
            TokenKind::Bool(b) => b.to_string(),
            TokenKind::Identifier(s) => s.clone(),
            TokenKind::Fn => "fn".to_string(),
            TokenKind::Let => "let".to_string(),
            TokenKind::Mut => "mut".to_string(),
            TokenKind::If => "if".to_string(),
            TokenKind::Else => "else".to_string(),
            TokenKind::While => "while".to_string(),
            TokenKind::For => "for".to_string(),
            TokenKind::Return => "return".to_string(),
            TokenKind::True => "true".to_string(),
            TokenKind::False => "false".to_string(),
            TokenKind::Struct => "struct".to_string(),
            TokenKind::Enum => "enum".to_string(),
            TokenKind::Match => "match".to_string(),
            TokenKind::Use => "use".to_string(),
            TokenKind::Mod => "mod".to_string(),
            TokenKind::Pub => "pub".to_string(),
            TokenKind::Const => "const".to_string(),
            TokenKind::Type => "type".to_string(),
            TokenKind::As => "as".to_string(),
            TokenKind::In => "in".to_string(),
            TokenKind::Loop => "loop".to_string(),
            TokenKind::Break => "break".to_string(),
            TokenKind::Continue => "continue".to_string(),
            TokenKind::Impl => "impl".to_string(),
            TokenKind::Plus => "+".to_string(),
            TokenKind::Minus => "-".to_string(),
            TokenKind::Star => "*".to_string(),
            TokenKind::Slash => "/".to_string(),
            TokenKind::Percent => "%".to_string(),
            TokenKind::Equal => "=".to_string(),
            TokenKind::EqualEqual => "==".to_string(),
            TokenKind::NotEqual => "!=".to_string(),
            TokenKind::Less => "<".to_string(),
            TokenKind::Greater => ">".to_string(),
            TokenKind::LessEqual => "<=".to_string(),
            TokenKind::GreaterEqual => ">=".to_string(),
            TokenKind::And => "&&".to_string(),
            TokenKind::Or => "||".to_string(),
            TokenKind::Not => "!".to_string(),
            TokenKind::Assign => "=".to_string(),
            TokenKind::PlusAssign => "+=".to_string(),
            TokenKind::MinusAssign => "-=".to_string(),
            TokenKind::StarAssign => "*=".to_string(),
            TokenKind::SlashAssign => "/=".to_string(),
            TokenKind::Ampersand => "&".to_string(),
            TokenKind::Pipe => "|".to_string(),
            TokenKind::Caret => "^".to_string(),
            TokenKind::Tilde => "~".to_string(),
            TokenKind::LeftShift => "<<".to_string(),
            TokenKind::RightShift => ">>".to_string(),
            TokenKind::LParen => "(".to_string(),
            TokenKind::RParen => ")".to_string(),
            TokenKind::LBrace => "{".to_string(),
            TokenKind::RBrace => "}".to_string(),
            TokenKind::LBracket => "[".to_string(),
            TokenKind::RBracket => "]".to_string(),
            TokenKind::Comma => ",".to_string(),
            TokenKind::Semicolon => ";".to_string(),
            TokenKind::Colon => ":".to_string(),
            TokenKind::DoubleColon => "::".to_string(),
            TokenKind::Dot => ".".to_string(),
            TokenKind::DotDot => "..".to_string(),
            TokenKind::Arrow => "->".to_string(),
            TokenKind::FatArrow => "=>".to_string(),
            TokenKind::Eof => "".to_string(),
            TokenKind::Unknown(c) => c.to_string(),
        };
        Self { kind, span, lexeme }
    }

    pub fn eof(span: Span) -> Self {
        Self { kind: TokenKind::Eof, span, lexeme: String::new() }
    }
}
