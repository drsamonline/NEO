//! AST to SIR Lowering

use crate::sir::*;
use nova_common::{Program, Function, Statement, Expr, Literal, BinOp as AstBinOp, UnOp as AstUnOp, Pattern};

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
            Some(block) => {
                let mut stmts = Vec::new();
                for stmt in &block.statements {
                    stmts.extend(self.lower_statement(stmt));
                }
                if let Some(final_expr) = &block.final_expr {
                    stmts.push(SirStatement::Expr(self.lower_expr(final_expr)));
                }
                stmts
            }
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
                vec![SirStatement::Expr(self.lower_expr(expr))]
            }
            Statement::Item(_) => {
                // Skip item statements for now (e.g., nested function definitions)
                Vec::new()
            }
        }
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
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::While(cond, body) => {
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::For(pattern, iterable, body) => {
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::Return(value) => {
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::Break => SirExpr::Literal(SirLiteral::Unit),
            Expr::Continue => SirExpr::Literal(SirLiteral::Unit),
            Expr::Block(block) => {
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::Cast(inner, type_expr) => {
                self.lower_expr(inner)
            }
            Expr::Path(path_parts) => {
                SirExpr::Var("path".to_string())
            }
            Expr::Field(_, _) => {
                SirExpr::Literal(SirLiteral::Unit) // Placeholder for field access
            }
            Expr::Index(_, _) => {
                SirExpr::Literal(SirLiteral::Unit) // Placeholder for index access
            }
            Expr::Range(_, _) => {
                SirExpr::Literal(SirLiteral::Unit) // Placeholder for range
            }
            Expr::Match(_, _) => {
                SirExpr::Literal(SirLiteral::Unit) // Placeholder for match
            }
            Expr::Loop(_) => {
                SirExpr::Literal(SirLiteral::Unit) // Placeholder for loop
            }
        }
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
