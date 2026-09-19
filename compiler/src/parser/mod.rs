//! NOVA Parser - Converts tokens into AST
//! 
//! The parser implements a recursive descent parser for the Nova language.

use nova_common::{Token, TokenKind, Span, AstNode, Program, Item, Function, Param, TypeExpr, Block, Statement, LetStmt, Pattern, Expr, BinOp, UnOp, Literal, StructDef, EnumDef, EnumVariant, Field, ConstDef, UseDecl, ModuleDecl, ImplBlock, Diagnostic, Diagnostics};

/// Parser state
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    diagnostics: Diagnostics,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            diagnostics: Diagnostics::new(),
        }
    }
    
    fn peek(&self) -> &Token {
        self.tokens.get(self.current).unwrap_or_else(|| self.tokens.last().unwrap())
    }
    
    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }
    
    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.tokens[self.current - 1].clone()
    }
    
    fn check(&self, kind: TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&kind)
    }
    
    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }
    
    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, ()> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            self.error_at_current(message);
            Err(())
        }
    }
    
    fn error_at_current(&mut self, message: &str) {
        let token = self.peek();
        self.diagnostics.push(Diagnostic::error(
            format!("{} at '{:?}'", message, token.kind),
        ).with_span(token.span.clone()));
    }
    
    fn synchronize(&mut self) {
        self.advance();
        while !self.is_at_end() {
            if self.check(TokenKind::Semicolon) {
                self.advance();
                return;
            }
            match self.peek().kind {
                TokenKind::Fn | TokenKind::Let | TokenKind::Const | TokenKind::Struct |
                TokenKind::Enum | TokenKind::If | TokenKind::While | TokenKind::For |
                TokenKind::Return => return,
                _ => {}
            }
            self.advance();
        }
    }
    
    pub fn parse(&mut self) -> Result<Program, &Diagnostics> {
        let start_span = self.peek().span.clone();
        let mut program = Program {
            items: Vec::new(),
            span: start_span,
        };
        
        while !self.is_at_end() {
            match self.parse_top_level() {
                Ok(item) => program.items.push(item),
                Err(_) => self.synchronize(),
            }
        }
        
        program.span = Span::new(start_span.start, self.peek().span.end);
        
        if self.diagnostics.has_errors() {
            Err(&self.diagnostics)
        } else {
            Ok(program)
        }
    }
    
    fn parse_top_level(&mut self) -> Result<Item, ()> {
        if self.check(TokenKind::Fn) {
            self.parse_function().map(Item::Function)
        } else if self.check(TokenKind::Struct) {
            self.parse_struct().map(Item::Struct)
        } else if self.check(TokenKind::Enum) {
            self.parse_enum().map(Item::Enum)
        } else if self.check(TokenKind::Use) {
            self.parse_use().map(Item::Use)
        } else if self.check(TokenKind::Mod) {
            self.parse_mod().map(Item::Mod)
        } else if self.check(TokenKind::Const) {
            self.parse_const().map(Item::Const)
        } else if self.check(TokenKind::Impl) {
            self.parse_impl().map(Item::Impl)
        } else if self.check(TokenKind::Let) {
            self.parse_let_stmt().map(|stmt| Item::Function(self.stmt_to_function(stmt)))
        } else {
            // Allow expression statements at top level (e.g., function calls, print statements)
            let expr = self.parse_expr()?;
            if self.match_token(TokenKind::Semicolon) {
                Ok(Item::Function(self.expr_to_function(expr)))
            } else {
                self.expect(TokenKind::Semicolon, "Expected ';' after expression")?;
                Ok(Item::Function(self.expr_to_function(expr)))
            }
        }
    }
    
    // Helper to convert an expression to a dummy function for now
    fn expr_to_function(&self, expr: Expr) -> Function {
        let span = expr.span().clone();
        Function {
            name: "__main".to_string(),
            generics: Vec::new(),
            params: Vec::new(),
            return_type: None,
            body: Some(Block {
                statements: vec![Statement::Expr(expr)],
                final_expr: None,
                span: span.clone(),
            }),
            is_pub: false,
            span,
        }
    }
    
    // Helper to convert a let statement to a dummy function for now
    fn stmt_to_function(&self, stmt: LetStmt) -> Function {
        Function {
            name: "__dummy".to_string(),
            generics: Vec::new(),
            params: Vec::new(),
            return_type: None,
            body: Some(Block {
                statements: vec![Statement::Let(stmt.clone())],
                final_expr: None,
                span: stmt.span.clone(),
            }),
            is_pub: false,
            span: stmt.span.clone(),
        }
    }
    
    fn parse_function(&mut self) -> Result<Function, ()> {
        let fn_token = self.advance();
        
        let name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance()
        } else {
            self.error_at_current("Expected function name");
            return Err(());
        };
        
        self.expect(TokenKind::LParen, "Expected '(' after function name")?;
        
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                let param = self.parse_param()?;
                params.push(param);
                
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        
        self.expect(TokenKind::RParen, "Expected ')' after parameters")?;
        
        let return_type = if self.match_token(TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        
        let body = if self.check(TokenKind::LBrace) {
            Some(self.parse_block()?)
        } else {
            self.expect(TokenKind::Semicolon, "Expected ';' or function body")?;
            None
        };
        
        Ok(Function {
            name: name.lexeme.clone(),
            generics: Vec::new(),
            params,
            return_type,
            body,
            is_pub: false,
            span: Span::new(fn_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_param(&mut self) -> Result<Param, ()> {
        let name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance()
        } else {
            self.error_at_current("Expected parameter name");
            return Err(());
        };
        
        let typ = if self.match_token(TokenKind::Colon) {
            self.parse_type_expr()?
        } else {
            // Default type
            TypeExpr {
                name: "Unknown".to_string(),
                generics: Vec::new(),
                span: name.span.clone(),
            }
        };
        
        Ok(Param {
            name: name.lexeme.clone(),
            typ,
            span: name.span.clone(),
        })
    }
    
    fn parse_type_expr(&mut self) -> Result<TypeExpr, ()> {
        let token = self.advance();
        
        let name = match &token.kind {
            TokenKind::Integer(_) => "Int".to_string(),
            TokenKind::Float(_) => "Float".to_string(),
            TokenKind::String(_) => "String".to_string(),
            TokenKind::Char(_) => "Char".to_string(),
            TokenKind::Bool(_) => "Bool".to_string(),
            TokenKind::Identifier(s) => s.clone(),
            _ => {
                self.error_at_current("Expected type");
                return Err(());
            }
        };
        
        Ok(TypeExpr {
            name,
            generics: Vec::new(),
            span: token.span.clone(),
        })
    }
    
    fn parse_block(&mut self) -> Result<Block, ()> {
        let start = self.expect(TokenKind::LBrace, "Expected '{'")?.span.clone();
        
        let mut statements = Vec::new();
        
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.parse_stmt()?);
        }
        
        let end = self.expect(TokenKind::RBrace, "Expected '}'")?.span.clone();
        
        Ok(Block {
            statements,
            final_expr: None,
            span: Span::new(start.start, end.end),
        })
    }
    
    fn parse_stmt(&mut self) -> Result<Statement, ()> {
        if self.check(TokenKind::Let) {
            Ok(Statement::Let(self.parse_let_stmt()?))
        } else if self.check(TokenKind::If) {
            Ok(self.parse_if_stmt()?)
        } else if self.check(TokenKind::While) {
            Ok(self.parse_while_stmt()?)
        } else if self.check(TokenKind::For) {
            Ok(self.parse_for_stmt()?)
        } else if self.check(TokenKind::Return) {
            Ok(self.parse_return_stmt()?)
        } else if self.check(TokenKind::Break) {
            self.advance();
            self.expect(TokenKind::Semicolon, "Expected ';' after break")?;
            Ok(Statement::Expr(Expr::Break))
        } else if self.check(TokenKind::Continue) {
            self.advance();
            self.expect(TokenKind::Semicolon, "Expected ';' after continue")?;
            Ok(Statement::Expr(Expr::Continue))
        } else if self.check(TokenKind::LBrace) {
            Ok(Statement::Expr(Expr::Block(self.parse_block()?)))
        } else {
            let expr = self.parse_expr()?;
            if self.match_token(TokenKind::Semicolon) {
                Ok(Statement::Expr(expr))
            } else {
                Ok(Statement::Expr(expr))
            }
        }
    }
    
    fn parse_let_stmt(&mut self) -> Result<LetStmt, ()> {
        let let_token = self.advance();
        
        let is_mut = self.match_token(TokenKind::Mut);
        
        let pattern = if self.check(TokenKind::Identifier(String::new())) {
            let name = self.advance();
            Pattern::Identifier(name.lexeme.clone())
        } else {
            self.error_at_current("Expected variable name");
            return Err(());
        };
        
        let typ = if self.match_token(TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        
        let init = if self.match_token(TokenKind::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        
        self.expect(TokenKind::Semicolon, "Expected ';' after variable declaration")?;
        
        Ok(LetStmt {
            pattern,
            typ,
            init,
            is_mut,
            span: Span::new(let_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_const(&mut self) -> Result<ConstDef, ()> {
        let const_token = self.advance();
        
        let name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance()
        } else {
            self.error_at_current("Expected constant name");
            return Err(());
        };
        
        let typ = if self.match_token(TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        
        self.expect(TokenKind::Assign, "Expected '=' after constant name")?;
        
        let value = self.parse_expr()?;
        
        self.expect(TokenKind::Semicolon, "Expected ';' after constant declaration")?;
        
        Ok(ConstDef {
            name: name.lexeme.clone(),
            typ,
            value,
            is_pub: false,
            span: Span::new(const_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_if_stmt(&mut self) -> Result<Statement, ()> {
        let if_token = self.advance();
        
        let condition = self.parse_expr()?;
        
        let then_branch = self.parse_block()?;
        
        let else_branch = if self.match_token(TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        
        Ok(Statement::Expr(Expr::If(
            Box::new(condition),
            then_branch,
            else_branch,
        )))
    }
    
    fn parse_while_stmt(&mut self) -> Result<Statement, ()> {
        self.advance(); // consume 'while'
        
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        
        Ok(Statement::Expr(Expr::While(Box::new(condition), body)))
    }
    
    fn parse_for_stmt(&mut self) -> Result<Statement, ()> {
        self.advance(); // consume 'for'
        
        let pattern = if self.check(TokenKind::Identifier(String::new())) {
            let name = self.advance();
            Pattern::Identifier(name.lexeme.clone())
        } else {
            self.error_at_current("Expected pattern in for loop");
            return Err(());
        };
        
        self.expect(TokenKind::In, "Expected 'in' after for pattern")?;
        
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;
        
        Ok(Statement::Expr(Expr::For(pattern, Box::new(iterable), body)))
    }
    
    fn parse_return_stmt(&mut self) -> Result<Statement, ()> {
        self.advance(); // consume 'return'
        
        let value = if !self.check(TokenKind::Semicolon) && !self.check(TokenKind::RBrace) && !self.is_at_end() {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        
        self.expect(TokenKind::Semicolon, "Expected ';' after return")?;
        
        Ok(Statement::Expr(Expr::Return(value)))
    }
    
    fn parse_struct(&mut self) -> Result<StructDef, ()> {
        let struct_token = self.advance();
        
        let name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance()
        } else {
            self.error_at_current("Expected struct name");
            return Err(());
        };
        
        self.expect(TokenKind::LBrace, "Expected '{' after struct name")?;
        
        let mut fields = Vec::new();
        
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let field_name = if self.check(TokenKind::Identifier(String::new())) {
                self.advance()
            } else {
                self.error_at_current("Expected field name");
                return Err(());
            };
            
            self.expect(TokenKind::Colon, "Expected ':' after field name")?;
            
            let field_type = self.parse_type_expr()?;
            
            fields.push(Field {
                name: field_name.lexeme.clone(),
                typ: field_type,
                is_pub: false,
                span: field_name.span.clone(),
            });
            
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }
        
        self.expect(TokenKind::RBrace, "Expected '}' after struct fields")?;
        
        Ok(StructDef {
            name: name.lexeme.clone(),
            generics: Vec::new(),
            fields,
            is_pub: false,
            span: Span::new(struct_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_enum(&mut self) -> Result<EnumDef, ()> {
        let enum_token = self.advance();
        
        let name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance()
        } else {
            self.error_at_current("Expected enum name");
            return Err(());
        };
        
        self.expect(TokenKind::LBrace, "Expected '{' after enum name")?;
        
        let mut variants = Vec::new();
        
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let variant_name = if self.check(TokenKind::Identifier(String::new())) {
                self.advance()
            } else {
                self.error_at_current("Expected variant name");
                return Err(());
            };
            
            let variant_fields = if self.match_token(TokenKind::LParen) {
                let mut types = Vec::new();
                if !self.check(TokenKind::RParen) {
                    loop {
                        types.push(self.parse_type_expr()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RParen, "Expected ')' after enum variant types")?;
                Some(types)
            } else {
                None
            };
            
            variants.push(EnumVariant {
                name: variant_name.lexeme.clone(),
                fields: variant_fields,
                span: variant_name.span.clone(),
            });
            
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }
        
        self.expect(TokenKind::RBrace, "Expected '}' after enum variants")?;
        
        Ok(EnumDef {
            name: name.lexeme.clone(),
            generics: Vec::new(),
            variants,
            is_pub: false,
            span: Span::new(enum_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_use(&mut self) -> Result<UseDecl, ()> {
        let use_token = self.advance();
        
        let path = self.parse_path()?;
        
        self.expect(TokenKind::Semicolon, "Expected ';' after use statement")?;
        
        Ok(UseDecl {
            path,
            alias: None,
            span: Span::new(use_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_mod(&mut self) -> Result<ModuleDecl, ()> {
        let mod_token = self.advance();
        
        let name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance()
        } else {
            self.error_at_current("Expected module name");
            return Err(());
        };
        
        Ok(ModuleDecl {
            name: name.lexeme.clone(),
            is_pub: false,
            span: Span::new(mod_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_impl(&mut self) -> Result<ImplBlock, ()> {
        let impl_token = self.advance();
        
        let type_name = if self.check(TokenKind::Identifier(String::new())) {
            self.advance().lexeme.clone()
        } else {
            self.error_at_current("Expected type name for impl");
            return Err(());
        };
        
        self.expect(TokenKind::LBrace, "Expected '{' after impl type")?;
        
        let mut methods = Vec::new();
        
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if let Ok(func) = self.parse_function() {
                methods.push(func);
            } else {
                self.synchronize();
            }
        }
        
        self.expect(TokenKind::RBrace, "Expected '}' after impl body")?;
        
        Ok(ImplBlock {
            type_name,
            generics: Vec::new(),
            methods,
            span: Span::new(impl_token.span.start, self.peek().span.end),
        })
    }
    
    fn parse_path(&mut self) -> Result<Vec<String>, ()> {
        let mut segments = Vec::new();
        
        if self.check(TokenKind::Identifier(String::new())) {
            segments.push(self.advance().lexeme.clone());
        }
        
        while self.match_token(TokenKind::DoubleColon) {
            if self.check(TokenKind::Identifier(String::new())) {
                segments.push(self.advance().lexeme.clone());
            } else {
                self.error_at_current("Expected identifier after '::'");
                return Err(());
            }
        }
        
        Ok(segments)
    }
    
    fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_assignment()
    }
    
    fn parse_assignment(&mut self) -> Result<Expr, ()> {
        let left = self.parse_or()?;
        
        if self.match_token(TokenKind::Assign) {
            let right = self.parse_assignment()?;
            return Ok(Expr::Assign(Box::new(left), Box::new(right)));
        }
        
        Ok(left)
    }
    
    fn parse_or(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_and()?;
        
        while self.match_token(TokenKind::Or) || self.match_token(TokenKind::Pipe) {
            let right = self.parse_and()?;
            expr = Expr::Binary(Box::new(expr), BinOp::Or, Box::new(right));
        }
        
        Ok(expr)
    }
    
    fn parse_and(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_equality()?;
        
        while self.match_token(TokenKind::And) || self.match_token(TokenKind::Ampersand) {
            let right = self.parse_equality()?;
            expr = Expr::Binary(Box::new(expr), BinOp::And, Box::new(right));
        }
        
        Ok(expr)
    }
    
    fn parse_equality(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_comparison()?;
        
        while self.match_token(TokenKind::EqualEqual) || self.match_token(TokenKind::NotEqual) {
            let op = if self.tokens[self.current - 1].kind == TokenKind::EqualEqual {
                BinOp::Eq
            } else {
                BinOp::Ne
            };
            let right = self.parse_comparison()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        
        Ok(expr)
    }
    
    fn parse_comparison(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_term()?;
        
        while self.match_token(TokenKind::Less) || self.match_token(TokenKind::LessEqual) ||
              self.match_token(TokenKind::Greater) || self.match_token(TokenKind::GreaterEqual) {
            let op = match self.tokens[self.current - 1].kind {
                TokenKind::Less => BinOp::Lt,
                TokenKind::LessEqual => BinOp::Le,
                TokenKind::Greater => BinOp::Gt,
                TokenKind::GreaterEqual => BinOp::Ge,
                _ => unreachable!(),
            };
            let right = self.parse_term()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        
        Ok(expr)
    }
    
    fn parse_term(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_factor()?;
        
        while self.match_token(TokenKind::Plus) || self.match_token(TokenKind::Minus) {
            let op = if self.tokens[self.current - 1].kind == TokenKind::Plus {
                BinOp::Add
            } else {
                BinOp::Sub
            };
            let right = self.parse_factor()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        
        Ok(expr)
    }
    
    fn parse_factor(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_unary()?;
        
        while self.match_token(TokenKind::Star) || self.match_token(TokenKind::Slash) || self.match_token(TokenKind::Percent) {
            let op = match self.tokens[self.current - 1].kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Rem,
                _ => unreachable!(),
            };
            let right = self.parse_unary()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        
        Ok(expr)
    }
    
    fn parse_unary(&mut self) -> Result<Expr, ()> {
        if self.match_token(TokenKind::Not) {
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary(UnOp::Not, Box::new(operand)));
        }
        if self.match_token(TokenKind::Minus) {
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary(UnOp::Neg, Box::new(operand)));
        }
        
        self.parse_call()
    }
    
    fn parse_call(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_primary()?;
        
        loop {
            if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RParen, "Expected ')' after arguments")?;
                
                expr = Expr::Call(Box::new(expr), args);
            } else if self.match_token(TokenKind::Dot) {
                let name = if self.check(TokenKind::Identifier(String::new())) {
                    self.advance().lexeme.clone()
                } else {
                    self.error_at_current("Expected field or method name after '.'");
                    return Err(());
                };
                
                expr = Expr::Field(Box::new(expr), name);
            } else if self.match_token(TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket, "Expected ']' after index")?;
                
                expr = Expr::Index(Box::new(expr), Box::new(index));
            } else {
                break;
            }
        }
        
        Ok(expr)
    }
    
    fn parse_primary(&mut self) -> Result<Expr, ()> {
        if self.match_token(TokenKind::True) {
            return Ok(Expr::Literal(Literal::Bool(true)));
        }
        if self.match_token(TokenKind::False) {
            return Ok(Expr::Literal(Literal::Bool(false)));
        }
        if let TokenKind::Integer(n) = self.peek().kind.clone() {
            self.advance();
            return Ok(Expr::Literal(Literal::Integer(n)));
        }
        if let TokenKind::Float(f) = self.peek().kind.clone() {
            self.advance();
            return Ok(Expr::Literal(Literal::Float(f)));
        }
        if let TokenKind::String(s) = self.peek().kind.clone() {
            self.advance();
            return Ok(Expr::Literal(Literal::String(s)));
        }
        if let TokenKind::Char(c) = self.peek().kind.clone() {
            self.advance();
            return Ok(Expr::Literal(Literal::Char(c)));
        }
        
        if self.check(TokenKind::Identifier(String::new())) {
            let name = self.advance().lexeme.clone();
            return Ok(Expr::Identifier(name));
        }
        
        if self.match_token(TokenKind::LParen) {
            let expr = self.parse_expr()?;
            self.expect(TokenKind::RParen, "Expected ')' after expression")?;
            return Ok(expr);
        }
        
        if self.match_token(TokenKind::LBracket) {
            let mut elements = Vec::new();
            if !self.check(TokenKind::RBracket) {
                loop {
                    elements.push(self.parse_expr()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RBracket, "Expected ']' after array elements")?;
            // For now, represent arrays as nested binary expressions
            // In a real implementation, we'd have an Array variant
            return Ok(Expr::Identifier("__array".to_string()));
        }
        
        self.error_at_current("Expected expression");
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    
    fn parse_source(source: &str) -> Result<Program, String> {
        let mut lexer = Lexer::new(source);
        let (tokens, diags) = lexer.tokenize();
        
        if diags.has_errors() {
            return Err("Lexer errors".to_string());
        }
        
        let mut parser = Parser::new(tokens);
        match parser.parse() {
            Ok(program) => Ok(program),
            Err(diags) => Err(format!("Parser errors: {:?}", diags)),
        }
    }
    
    #[test]
    fn test_variable_declaration() {
        let result = parse_source("let x = 42;");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_function() {
        let result = parse_source("fn main() { let x = 42; }");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_arithmetic() {
        let result = parse_source("fn test() { let x = 1 + 2 * 3; }");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_struct() {
        let result = parse_source("struct Point { x: Int, y: Int }");
        assert!(result.is_ok());
    }
}
