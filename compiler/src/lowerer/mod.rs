//! AST to SIR Lowering

use crate::sir::*;
use nova_common::{Program, Function, Statement, Expr, Literal, BinOp as AstBinOp, UnOp as AstUnOp, Pattern, MatchArm, Block};

pub struct Lowerer {
    pub functions: Vec<SirFunction>,
    pub globals: Vec<SirGlobal>,
}

impl Lowerer {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            globals: Vec::new(),
        }
    }

    pub fn lower_program(&mut self, program: &Program) -> SirProgram {
        let mut functions = Vec::new();
        let mut globals = Vec::new();

        for item in &program.items {
            match item {
                nova_common::Item::Function(func) => {
                    functions.push(self.lower_function(func));
                }
                nova_common::Item::Const(const_def) => {
                    let init_value = self.lower_expr(&const_def.value);
                    globals.push(SirGlobal {
                        name: const_def.name.clone(),
                        typ: const_def.typ.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "int".to_string()),
                        init_value: Some(init_value),
                    });
                }
                _ => {}
            }
        }

        SirProgram {
            functions,
            globals,
        }
    }

    fn lower_function(&mut self, func: &Function) -> SirFunction {
        let body_stmts = match &func.body {
            Some(block) => self.lower_block(block),
            None => Vec::new(),
        };

        SirFunction {
            name: func.name.clone(),
            params: func.params.iter().map(|p| SirParam {
                name: p.name.clone(),
                typ: p.typ.name.clone(),
            }).collect(),
            return_type: func.return_type.as_ref().map(|t| t.name.clone()),
            body: body_stmts,
            span: func.span.clone(),
        }
    }

    fn lower_statement(&mut self, stmt: &Statement) -> Vec<SirStatement> {
        match stmt {
            Statement::Let(let_stmt) => {
                let name = match &let_stmt.pattern {
                    Pattern::Identifier(n) => n.clone(),
                    _ => "_".to_string(),
                };
                let typ = let_stmt.typ.as_ref().map(|t| t.name.clone());
                let value = let_stmt.init.as_ref()
                    .map(|e| self.lower_expr(e))
                    .unwrap_or_else(|| SirExpr::Literal(SirLiteral::Int(0)));
                vec![SirStatement::Let { name, typ, value }]
            }
            Statement::Expr(expr) => {
                // Check if the expression is a control flow statement that should be lowered as a statement
                match expr {
                    Expr::If(cond, then_block, else_block) => {
                        return vec![self.lower_if_statement(cond, then_block, else_block)];
                    }
                    Expr::While(cond, body) => {
                        return vec![self.lower_while_statement(cond, body)];
                    }
                    Expr::For(pattern, iterable, body) => {
                        return vec![self.lower_for_statement(pattern, iterable, body)];
                    }
                    Expr::Return(value) => {
                        return vec![SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v)))];
                    }
                    _ => {}
                }
                vec![SirStatement::Expr(self.lower_expr(expr))]
                // Check if the expression is a control flow construct that should be a statement
                match expr {
                    Expr::If(cond, then_block, else_block) => {
                        let then_stmts = self.lower_block(then_block);
                        let else_stmts = else_block.as_ref().map(|b| self.lower_block(b));
                        vec![SirStatement::If {
                            condition: self.lower_expr(cond),
                            then_block: then_stmts,
                            else_block: else_stmts,
                        }]
                    }
                    Expr::While(cond, body) => {
                        vec![SirStatement::While {
                            condition: self.lower_expr(cond),
                            body: self.lower_block(body),
                        }]
                    }
                    Expr::Loop(body) => {
                        // Lower loop as while(true)
                        vec![SirStatement::While {
                            condition: SirExpr::Literal(SirLiteral::Bool(true)),
                            body: self.lower_block(body),
                        }]
                    }
                    Expr::For(pattern, iterable, body) => {
                        let pattern_str = match pattern {
                            Pattern::Identifier(n) => n.clone(),
                            _ => "_".to_string(),
                        };
                        vec![SirStatement::For {
                            pattern: pattern_str,
                            iterable: self.lower_expr(iterable),
                            body: self.lower_block(body),
                        }]
                    }
                    Expr::Block(block) => {
                        vec![SirStatement::Block(self.lower_block(block))]
                    }
                    Expr::Return(value) => {
                        vec![SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v)))]
                    }
                    _ => {
                        vec![SirStatement::Expr(self.lower_expr(expr))]
                    }
                }
            }
            Statement::Item(_item) => {
                // Skip item statements for now (e.g., nested function definitions)
                Vec::new()
            }
        }
    }

    fn lower_if_statement(&mut self, cond: &Expr, then_block: &nova_common::Block, else_block: &Option<nova_common::Block>) -> SirStatement {
        let condition = self.lower_expr(cond);
        
        let then_stmts: Vec<SirStatement> = then_block.statements.iter()
            .flat_map(|s| self.lower_statement(s))
            .collect();
        let then_final = then_block.final_expr.as_ref()
            .map(|e| self.lower_expr(e));
        
        // Add final expression as a statement if present
        let mut all_then_stmts = then_stmts;
        if let Some(final_expr) = then_final {
            all_then_stmts.push(SirStatement::Expr(final_expr));
        }
        
        let else_stmts = else_block.as_ref().map(|else_blk| {
            let mut stmts: Vec<SirStatement> = else_blk.statements.iter()
                .flat_map(|s| self.lower_statement(s))
                .collect();
            if let Some(final_expr) = &else_blk.final_expr {
                stmts.push(SirStatement::Expr(self.lower_expr(final_expr)));
            }
            stmts
        });
        
        SirStatement::If {
            condition,
            then_block: all_then_stmts,
            else_block: else_stmts,
        }
    }

    fn lower_while_statement(&mut self, cond: &Expr, body: &nova_common::Block) -> SirStatement {
        let condition = self.lower_expr(cond);
        let body_stmts: Vec<SirStatement> = body.statements.iter()
            .flat_map(|s| self.lower_statement(s))
            .collect();
        
        SirStatement::While {
            condition,
            body: body_stmts,
        }
    }

    fn lower_for_statement(&mut self, pattern: &Pattern, iterable: &Expr, body: &nova_common::Block) -> SirStatement {
        let pattern_name = match pattern {
            Pattern::Identifier(n) => n.clone(),
            _ => "_".to_string(),
        };
        let iterable_expr = self.lower_expr(iterable);
        let body_stmts: Vec<SirStatement> = body.statements.iter()
            .flat_map(|s| self.lower_statement(s))
            .collect();
        
        SirStatement::For {
            pattern: pattern_name,
            iterable: iterable_expr,
            body: body_stmts,
        }
    fn lower_block(&mut self, block: &nova_common::Block) -> Vec<SirStatement> {
        let mut stmts = Vec::new();
        for stmt in &block.statements {
            stmts.extend(self.lower_statement(stmt));
        }
        if let Some(final_expr) = &block.final_expr {
            // Handle control flow in final expression
            match &**final_expr {
                Expr::Return(value) => {
                    stmts.push(SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v))));
                }
                _ => {
                    stmts.push(SirStatement::Expr(self.lower_expr(final_expr)));
                }
            }
        }
        stmts
    }

    fn lower_expr(&mut self, expr: &Expr) -> SirExpr {
        match expr {
            Expr::Literal(lit) => {
                SirExpr::Literal(match lit {
                    Literal::Integer(i) => SirLiteral::Int(*i),
                    Literal::Float(f) => SirLiteral::Float(*f),
                    Literal::String(s) => SirLiteral::String(s.clone()),
                    Literal::Char(c) => SirLiteral::Char(*c),
                    Literal::Bool(b) => SirLiteral::Bool(*b),
                })
            }
            Expr::Identifier(name) => {
                SirExpr::Var(name.clone())
            }
            Expr::Binary(left, op, right) => {
                SirExpr::BinOp {
                    left: Box::new(self.lower_expr(left)),
                    op: self.lower_bin_op(op),
                    right: Box::new(self.lower_expr(right)),
                }
            }
            Expr::Unary(op, operand) => {
                SirExpr::UnaryOp {
                    op: self.lower_un_op(op),
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            Expr::Call(callee, args) => {
                SirExpr::Call {
                    callee: match callee.as_ref() {
                        Expr::Identifier(name) => name.clone(),
                        _ => "unknown".to_string(),
                    },
                    args: args.iter().map(|a| self.lower_expr(a)).collect(),
                }
            }
            Expr::Assign(target, value) => {
                SirExpr::Assign {
                    target: match target.as_ref() {
                        Expr::Identifier(name) => name.clone(),
                        _ => "unknown".to_string(),
                    },
                    value: Box::new(self.lower_expr(value)),
                }
            }
            Expr::If(cond, then_block, else_block) => {
                // If used in expression context, keep as If expression
                let condition = Box::new(self.lower_expr(cond));
                
                let _then_stmts: Vec<SirStatement> = then_block.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                let then_final = then_block.final_expr.as_ref()
                    .map(|e| Box::new(self.lower_expr(e)));
                
                let else_final = else_block.as_ref().and_then(|else_blk| 
                    else_blk.final_expr.as_ref().map(|e| Box::new(self.lower_expr(e)))
                );
                let _else_stmts: Vec<SirStatement> = else_block.as_ref().map(|else_blk| {
                    else_blk.statements.iter()
                        .flat_map(|s| self.lower_statement(s))
                        .collect()
                }).unwrap_or_default();
                
                // If there's a final expression, use If expression, otherwise use If statement
                if then_final.is_some() || else_final.is_some() {
                    SirExpr::If {
                        condition,
                        then_expr: if let Some(final_expr) = then_final {
                            final_expr
                        } else {
                            Box::new(SirExpr::Literal(SirLiteral::Unit))
                        },
                        else_expr: else_final.or(Some(Box::new(SirExpr::Literal(SirLiteral::Unit)))),
                    }
                } else {
                    // Emit as If statement - requires statement context
                    // This case should be handled by lower_statement now
                    SirExpr::Literal(SirLiteral::Unit)
                }
            }
            Expr::While(cond, body) => {
                // While in expression context returns Unit
                // Statement context is handled by lower_statement
                let _condition = self.lower_expr(cond);
                let _body_stmts: Vec<SirStatement> = body.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::For(pattern, iterable, body) => {
                // For in expression context returns Unit
                // Statement context is handled by lower_statement
                let _pattern_name = match pattern {
                    Pattern::Identifier(n) => n.clone(),
                    _ => "_".to_string(),
                };
                let _iterable_expr = self.lower_expr(iterable);
                let _body_stmts: Vec<SirStatement> = body.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::Return(value) => {
                // Return in expression context returns Unit
                // Statement context is handled by lower_statement
                let _value_expr = value.as_ref().map(|v| self.lower_expr(v));
                
                SirExpr::Literal(SirLiteral::Unit)
                // Lower if expression (returns a value)
                let then_expr = self.lower_block_as_expr(then_block);
                let else_expr = else_block.as_ref().map(|b| Box::new(self.lower_block_as_expr(b)));
                SirExpr::If {
                    condition: Box::new(self.lower_expr(cond)),
                    then_expr: Box::new(then_expr),
                    else_expr,
                }
            }
            Expr::While(cond, body) => {
                // While as expression returns Unit
                SirExpr::Block(
                    vec![SirStatement::While {
                        condition: self.lower_expr(cond),
                        body: self.lower_block(body),
                    }],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::For(pattern, iterable, body) => {
                // For as expression returns Unit
                let pattern_str = match pattern {
                    Pattern::Identifier(n) => n.clone(),
                    _ => "_".to_string(),
                };
                SirExpr::Block(
                    vec![SirStatement::For {
                        pattern: pattern_str,
                        iterable: self.lower_expr(iterable),
                        body: self.lower_block(body),
                    }],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Return(value) => {
                // Return as expression - wraps in block with Unit return
                SirExpr::Block(
                    vec![SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v)))],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Break => {
                // Break as expression - wraps in block
                SirExpr::Block(
                    vec![SirStatement::Break],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Continue => {
                // Continue as expression - wraps in block
                SirExpr::Block(
                    vec![SirStatement::Continue],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Block(block) => {
                let stmts: Vec<SirStatement> = block.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                let final_val = block.final_expr.as_ref()
                    .map(|e| Box::new(self.lower_expr(e)));
                
                SirExpr::Block(stmts, final_val)
                self.lower_block_as_expr(block)
            }
            Expr::Loop(body) => {
                // Loop as expression returns Unit
                SirExpr::Block(
                    vec![SirStatement::While {
                        condition: SirExpr::Literal(SirLiteral::Bool(true)),
                        body: self.lower_block(body),
                    }],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Cast(inner, _type_expr) => {
                // For now, just pass through the inner expression
                self.lower_expr(inner)
            }
            Expr::Path(path_parts) => {
                // For simple paths, return the last part as variable
                if let Some(last) = path_parts.last() {
                    SirExpr::Var(last.clone())
                } else {
                    SirExpr::Literal(SirLiteral::Unit)
                }
            }
            Expr::Field(obj, field) => {
                SirExpr::Call {
                    callee: "field_access".to_string(),
                    args: vec![self.lower_expr(obj), SirExpr::Literal(SirLiteral::String(field.clone()))],
                }
            }
            Expr::Index(obj, index) => {
                SirExpr::Call {
                    callee: "index_access".to_string(),
                    args: vec![self.lower_expr(obj), self.lower_expr(index)],
                }
            }
            Expr::Range(start, end) => {
                let start_expr = match &**start {
                    Expr::Literal(Literal::Integer(0)) => SirExpr::Literal(SirLiteral::Int(0)),
                    _ => self.lower_expr(start),
                };
                SirExpr::BinOp {
                    left: Box::new(start_expr),
                    op: SirBinOp::Range,
                    right: Box::new(self.lower_expr(end)),
                }
            }
            Expr::Match(scrutinee, arms) => {
                // Lower match as a series of if-else expressions
                self.lower_match_as_if(scrutinee, arms)
            }
        }
    }

    fn lower_block_as_expr(&mut self, block: &nova_common::Block) -> SirExpr {
        let mut stmts = Vec::new();
        for stmt in &block.statements {
            stmts.extend(self.lower_statement(stmt));
        }
        let final_expr = block.final_expr.as_ref().map(|e| Box::new(self.lower_expr(e)));
        SirExpr::Block(stmts, final_expr)
    }

    fn lower_match_as_if(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> SirExpr {
        if arms.is_empty() {
            return SirExpr::Literal(SirLiteral::Unit);
        }

        let scrutinee_expr = self.lower_expr(scrutinee);
        
        // Build if-else chain from match arms
        let mut current_if: Option<SirExpr> = None;
        
        for (i, arm) in arms.iter().enumerate().rev() {
            let result_expr = self.lower_expr(&arm.body);
            
            // For now, only handle simple identifier patterns and wildcard
            let is_wildcard = matches!(arm.pattern, Pattern::Wildcard);
            let pattern_name = match &arm.pattern {
                Pattern::Identifier(n) => Some(n.clone()),
                _ => None,
            };
            
            if is_wildcard || i == arms.len() - 1 {
                // Last arm or wildcard - becomes the else branch
                if let Some(prev_if) = current_if.take() {
                    if let SirExpr::If { condition, then_expr, .. } = prev_if {
                        current_if = Some(SirExpr::If {
                            condition,
                            then_expr,
                            else_expr: Some(Box::new(result_expr)),
                        });
                    }
                } else {
                    current_if = Some(result_expr);
                }
            } else if let Some(pattern_name) = pattern_name {
                // Simple equality check
                let condition = SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Var(pattern_name)),
                };
                
                if let Some(prev_if) = current_if.take() {
                    current_if = Some(SirExpr::If {
                        condition: Box::new(condition),
                        then_expr: Box::new(result_expr),
                        else_expr: Some(Box::new(prev_if)),
                    });
                } else {
                    current_if = Some(SirExpr::If {
                        condition: Box::new(condition),
                        then_expr: Box::new(result_expr),
                        else_expr: None,
                    });
                }
            }
        }
        
        current_if.unwrap_or_else(|| SirExpr::Literal(SirLiteral::Unit))
    }

    fn lower_bin_op(&self, op: &AstBinOp) -> SirBinOp {
        match op {
            AstBinOp::Add => SirBinOp::Add,
            AstBinOp::Sub => SirBinOp::Sub,
            AstBinOp::Mul => SirBinOp::Mul,
            AstBinOp::Div => SirBinOp::Div,
            AstBinOp::Rem => SirBinOp::Mod,
            AstBinOp::Eq => SirBinOp::Eq,
            AstBinOp::Ne => SirBinOp::Ne,
            AstBinOp::Lt => SirBinOp::Lt,
            AstBinOp::Le => SirBinOp::Le,
            AstBinOp::Gt => SirBinOp::Gt,
            AstBinOp::Ge => SirBinOp::Ge,
            AstBinOp::And => SirBinOp::And,
            AstBinOp::Or => SirBinOp::Or,
            _ => SirBinOp::Add, // Handle BitAnd, BitOr, BitXor, Shl, Shr
        }
    }

    fn lower_un_op(&self, op: &AstUnOp) -> SirUnOp {
        match op {
            AstUnOp::Neg => SirUnOp::Neg,
            AstUnOp::Not => SirUnOp::Not,
            AstUnOp::Deref => SirUnOp::Neg, // Placeholder for Deref
            AstUnOp::AddrOf => SirUnOp::Not, // Placeholder for AddrOf
        }
    }
}
