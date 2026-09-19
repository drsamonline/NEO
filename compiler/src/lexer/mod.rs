//! NOVA Lexer - Converts source code into tokens

use nova_common::{Token, TokenKind, Span, Position, Diagnostics};

/// Lexer state
pub struct Lexer {
    source: String,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    diagnostics: Diagnostics,
}

impl Lexer {
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let chars: Vec<char> = source.chars().collect();
        Self {
            source,
            chars,
            pos: 0,
            line: 1,
            column: 1,
            diagnostics: Diagnostics::new(),
        }
    }

    fn current_char(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.current_char();
        if let Some(ch) = ch {
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        ch
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if !self.skip_comment() {
                break;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char() {
            match ch {
                ' ' | '\t' | '\r' | '\n' => { self.advance(); }
                _ => break,
            }
        }
    }

    fn skip_comment(&mut self) -> bool {
        if let Some('/') = self.current_char() {
            if let Some(next) = self.peek_char() {
                if next == '/' {
                    while let Some(ch) = self.current_char() {
                        if ch == '\n' { break; }
                        self.advance();
                    }
                    return true;
                } else if next == '*' {
                    self.advance();
                    self.advance();
                    let mut prev = '/';
                    while let Some(ch) = self.current_char() {
                        if prev == '*' && ch == '/' {
                            self.advance();
                            return true;
                        }
                        prev = ch;
                        self.advance();
                    }
                    return true;
                }
            }
        }
        false
    }

    fn read_identifier(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(ch) = self.current_char() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let ident: String = self.source[start..self.pos].to_string();
        Self::keyword_or_ident(&ident)
    }

    fn read_number(&mut self) -> TokenKind {
        let start = self.pos;
        let mut is_float = false;
        
        if let Some('0') = self.current_char() {
            if let Some(next) = self.peek_char() {
                match next {
                    'x' | 'X' => return self.read_radix(16, "hex"),
                    'b' | 'B' => return self.read_radix(2, "binary"),
                    'o' | 'O' => return self.read_radix(8, "octal"),
                    _ => {}
                }
            }
        }

        while let Some(ch) = self.current_char() {
            if ch.is_ascii_digit() || ch == '_' {
                self.advance();
            } else if ch == '.' && !is_float {
                if let Some(next) = self.peek_char() {
                    if next.is_ascii_digit() {
                        is_float = true;
                        self.advance();
                    } else { break; }
                } else { break; }
            } else if ch == 'e' || ch == 'E' {
                is_float = true;
                self.advance();
                if let Some('+' | '-') = self.current_char() {
                    self.advance();
                }
            } else { break; }
        }

        let num_str: String = self.source[start..self.pos].chars().filter(|c| *c != '_').collect();
        if is_float {
            TokenKind::Float(num_str.parse::<f64>().unwrap_or_else(|_| {
                self.diagnostics.error(format!("Invalid float: {}", num_str));
                0.0
            }))
        } else {
            TokenKind::Integer(num_str.parse::<i64>().unwrap_or_else(|_| {
                self.diagnostics.error(format!("Invalid integer: {}", num_str));
                0
            }))
        }
    }

    fn read_radix(&mut self, radix: u32, name: &str) -> TokenKind {
        self.advance(); self.advance();
        let start = self.pos;
        let valid = match radix {
            16 => |c: char| c.is_ascii_hexdigit(),
            2 => |c: char| c == '0' || c == '1',
            8 => |c: char| ('0'..='7').contains(&c),
            _ => |c: char| c.is_ascii_digit(),
        };
        while let Some(ch) = self.current_char() {
            if valid(ch) || ch == '_' { self.advance(); } else { break; }
        }
        let num_str: String = self.source[start..self.pos].chars().filter(|c| *c != '_').collect();
        TokenKind::Integer(i64::from_str_radix(&num_str, radix).unwrap_or_else(|_| {
            self.diagnostics.error(format!("Invalid {} literal", name));
            0
        }))
    }

    fn read_string(&mut self) -> TokenKind {
        self.advance();
        let mut content = String::new();
        while let Some(ch) = self.current_char() {
            match ch {
                '"' => { self.advance(); return TokenKind::String(content); }
                '\\' => {
                    self.advance();
                    if let Some(e) = self.current_char() {
                        content.push(match e {
                            'n' => '\n', 'r' => '\r', 't' => '\t',
                            '\\' => '\\', '"' => '"', '\'' => '\'', '0' => '\0',
                            _ => { self.diagnostics.warning("Unknown escape"); '\\'; e }
                        });
                        self.advance();
                    }
                }
                '\n' => { self.diagnostics.error("Unterminated string"); return TokenKind::String(content); }
                _ => { content.push(ch); self.advance(); }
            }
        }
        self.diagnostics.error("Unterminated string");
        TokenKind::String(content)
    }

    fn read_char(&mut self) -> TokenKind {
        self.advance();
        let mut result = ' ';
        if let Some(c) = self.current_char() {
            if c == '\\' {
                self.advance();
                if let Some(e) = self.current_char() {
                    result = match e { 'n' => '\n', 'r' => '\r', 't' => '\t', '\\' => '\\', '\'' => '\'', _ => e };
                    self.advance();
                }
            } else { result = c; self.advance(); }
        }
        if let Some('\'') = self.current_char() { self.advance(); }
        else { self.diagnostics.error("Unterminated char"); }
        TokenKind::Char(result)
    }

    fn make_token(&self, kind: TokenKind, start: Position, end: Position) -> Token {
        Token::new(kind, Span::new(start, end))
    }

    fn position(&self) -> Position {
        Position::new(self.line, self.column, self.pos)
    }

    pub fn tokenize(&mut self) -> (Vec<Token>, &Diagnostics) {
        let mut tokens = Vec::new();
        loop {
            let start_pos = self.position();
            self.skip_whitespace_and_comments();
            let ch = match self.current_char() { Some(c) => c, None => break };
            let kind = match ch {
                'a'..='z'|'A'..='Z'|'_' => self.read_identifier(),
                '0'..='9' => self.read_number(),
                '"' => self.read_string(),
                '\'' => self.read_char(),
                '+' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::PlusAssign } else { TokenKind::Plus } }
                '-' => { self.advance(); if self.current_char() == Some('>') { self.advance(); TokenKind::Arrow } else if self.current_char() == Some('=') { self.advance(); TokenKind::MinusAssign } else { TokenKind::Minus } }
                '*' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::StarAssign } else { TokenKind::Star } }
                '/' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::SlashAssign } else { TokenKind::Slash } }
                '%' => { self.advance(); TokenKind::Percent }
                '=' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::EqualEqual } else { TokenKind::Assign } }
                '!' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::NotEqual } else { TokenKind::Not } }
                '<' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::LessEqual } else if self.current_char() == Some('<') { self.advance(); TokenKind::LeftShift } else { TokenKind::Less } }
                '>' => { self.advance(); if self.current_char() == Some('=') { self.advance(); TokenKind::GreaterEqual } else if self.current_char() == Some('>') { self.advance(); TokenKind::RightShift } else { TokenKind::Greater } }
                '&' => { self.advance(); if self.current_char() == Some('&') { self.advance(); TokenKind::And } else { TokenKind::Ampersand } }
                '|' => { self.advance(); if self.current_char() == Some('|') { self.advance(); TokenKind::Or } else { TokenKind::Pipe } }
                '^' => { self.advance(); TokenKind::Caret }
                '~' => { self.advance(); TokenKind::Tilde }
                '(' => { self.advance(); TokenKind::LParen }
                ')' => { self.advance(); TokenKind::RParen }
                '{' => { self.advance(); TokenKind::LBrace }
                '}' => { self.advance(); TokenKind::RBrace }
                '[' => { self.advance(); TokenKind::LBracket }
                ']' => { self.advance(); TokenKind::RBracket }
                ',' => { self.advance(); TokenKind::Comma }
                ';' => { self.advance(); TokenKind::Semicolon }
                ':' => { self.advance(); if self.current_char() == Some(':') { self.advance(); TokenKind::DoubleColon } else { TokenKind::Colon } }
                '.' => { self.advance(); TokenKind::Dot }
                _ => { self.advance(); TokenKind::Unknown(ch) }
            };
            tokens.push(self.make_token(kind, start_pos, self.position()));
        }
        tokens.push(Token::eof(Span::at(self.position())));
        (tokens, &self.diagnostics)
    }

    fn keyword_or_ident(s: &str) -> TokenKind {
        match s {
            "fn" => TokenKind::Fn, "let" => TokenKind::Let, "mut" => TokenKind::Mut,
            "if" => TokenKind::If, "else" => TokenKind::Else, "while" => TokenKind::While,
            "for" => TokenKind::For, "return" => TokenKind::Return,
            "true" => TokenKind::True, "false" => TokenKind::False,
            "struct" => TokenKind::Struct, "enum" => TokenKind::Enum, "match" => TokenKind::Match,
            "use" => TokenKind::Use, "mod" => TokenKind::Mod, "pub" => TokenKind::Pub,
            "const" => TokenKind::Const, "type" => TokenKind::Type, "as" => TokenKind::As,
            "in" => TokenKind::In, "loop" => TokenKind::Loop, "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            _ => TokenKind::Identifier(s.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_basic() {
        let mut lex = Lexer::new("let x = 42;");
        let (tokens, diags) = lex.tokenize();
        assert!(!diags.has_errors());
        assert!(tokens.len() > 4);
    }
    #[test]
    fn test_keywords() {
        let mut lex = Lexer::new("fn main() {}");
        let (tokens, _) = lex.tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Fn);
    }
}
