//! JavaScript Backend for Nova Compiler
//! 
//! This module converts SIR (Semantic Intermediate Representation) to JavaScript code.

use nova_compiler::sir::{SirProgram, SirFunction, SirGlobal, SirStatement, SirExpr, SirLiteral, SirBinOp, SirUnOp};
use nova_compiler::sir::SirBinOp::*;

pub struct JsBackend {
    output: String,
    indent_level: usize,
}

impl JsBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        
        // Add strict mode
        self.writeln("'use strict';");
        self.writeln("");
        
        // Generate globals
        for global in &program.globals {
            self.generate_global(global);
        }
        
        if !program.globals.is_empty() {
            self.writeln("");
        }
        
        // Generate functions
        for func in &program.functions {
            self.generate_function(func);
            self.writeln("");
        }
        
        // Generate main call if main function exists
        if program.functions.iter().any(|f| f.name == "main") {
            self.writeln("main();");
        }
        
        self.output.clone()
    }

    fn generate_global(&mut self, global: &SirGlobal) {
        let init = if let Some(init_expr) = &global.init_value {
            self.expr_to_js(init_expr)
        } else {
            self.default_value_for_type(&global.typ).to_string()
        };
        self.writeln(&format!("let {} = {};", global.name, init));
    }

    fn generate_function(&mut self, func: &SirFunction) {
        let params: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();
        let params_str = params.join(", ");
        self.writeln(&format!("function {}({}) {{", func.name, params_str));
        
        self.indent_level += 1;
        
        // Generate function body statements
        for stmt in &func.body {
            self.generate_statement(stmt);
        }
        
        self.indent_level -= 1;
        self.writeln("}");
    }

    fn generate_statement(&mut self, stmt: &SirStatement) {
        match stmt {
            SirStatement::Let { name, typ, value } => {
                let init = self.expr_to_js(value);
                self.writeln(&format!("let {} = {};", name, init));
            }
            SirStatement::Expr(expr) => {
                let expr_str = self.expr_to_js(expr);
                self.writeln(&format!("{};", expr_str));
            }
            SirStatement::Return(expr) => {
                let val_str = if let Some(e) = expr {
                    self.expr_to_js(e)
                } else {
                    "undefined".to_string()
                };
                self.writeln(&format!("return {};", val_str));
            }
            SirStatement::If { condition, then_block, else_block } => {
                let cond_str = self.expr_to_js(condition);
                self.writeln(&format!("if ({}) {{", cond_str));
                
                self.indent_level += 1;
                for stmt in then_block {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                
                if let Some(else_stmts) = else_block {
                    self.writeln("} else {");
                    self.indent_level += 1;
                    for stmt in else_stmts {
                        self.generate_statement(stmt);
                    }
                    self.indent_level -= 1;
                }
                
                self.writeln("}");
            }
            SirStatement::While { condition, body } => {
                let cond_str = self.expr_to_js(condition);
                self.writeln(&format!("while ({}) {{", cond_str));
                
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                
                self.writeln("}");
            }
            SirStatement::For { pattern, iterable, body } => {
                // Check if iterable is a range expression
                if let SirExpr::BinOp { left, op: SirBinOp::Range, right } = iterable {
                    let start_str = self.expr_to_js(left);
                    let end_str = self.expr_to_js(right);
                    self.writeln(&format!("for (let {} = {}; {} < {}; {}++) {{", pattern, start_str, pattern, end_str, pattern));
                } else {
                    // Generic for-in loop for arrays/iterables
                    let iter_str = self.expr_to_js(iterable);
                    self.writeln(&format!("for (const {} of {}) {{", pattern, iter_str));
                }
                
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                
                self.writeln("}");
            }
            SirStatement::Block(stmts) => {
                for stmt in stmts {
                    self.generate_statement(stmt);
                }
            }
            SirStatement::Break => {
                self.writeln("break;");
            }
            SirStatement::Continue => {
                self.writeln("continue;");
            }
        }
    }

    fn expr_to_js(&self, expr: &SirExpr) -> String {
        match expr {
            SirExpr::Literal(lit) => {
                match lit {
                    SirLiteral::Int(n) => n.to_string(),
                    SirLiteral::Float(f) => f.to_string(),
                    SirLiteral::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
                    SirLiteral::Bool(b) => b.to_string(),
                    SirLiteral::Char(c) => format!("'{}'", c),
                    SirLiteral::Unit => "undefined".to_string(),
                }
            }
            SirExpr::Var(name) => name.clone(),
            SirExpr::BinOp { left, op, right } => {
                let left_str = self.expr_to_js(left);
                let right_str = self.expr_to_js(right);
                let op_str = match op {
                    SirBinOp::Add => "+",
                    SirBinOp::Sub => "-",
                    SirBinOp::Mul => "*",
                    SirBinOp::Div => "/",
                    SirBinOp::Mod => "%",
                    SirBinOp::Eq => "===",
                    SirBinOp::Ne => "!==",
                    SirBinOp::Lt => "<",
                    SirBinOp::Le => "<=",
                    SirBinOp::Gt => ">",
                    SirBinOp::Ge => ">=",
                    SirBinOp::And => "&&",
                    SirBinOp::Or => "||",
                    SirBinOp::Range => "..", // Should not appear in final JS
                };
                format!("({} {} {})", left_str, op_str, right_str)
            }
            SirExpr::UnaryOp { op, operand } => {
                let operand_str = self.expr_to_js(operand);
                let op_str = match op {
                    SirUnOp::Neg => "-",
                    SirUnOp::Not => "!",
                };
                format!("({}{})", op_str, operand_str)
            }
            SirExpr::Call { callee, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.expr_to_js(a)).collect();
                format!("{}({})", callee, args_str.join(", "))
            }
            SirExpr::Assign { target, value } => {
                format!("{} = {}", target, self.expr_to_js(value))
            }
            SirExpr::If { condition, then_expr, else_expr } => {
                let cond_str = self.expr_to_js(condition);
                let then_str = self.expr_to_js(then_expr);
                if let Some(else_e) = else_expr {
                    let else_str = self.expr_to_js(else_e);
                    format!("({} ? {} : {})", cond_str, then_str, else_str)
                } else {
                    format!("({} ? {} : undefined)", cond_str, then_str)
                }
            }
            SirExpr::Block(stmts, final_expr) => {
                // Blocks in expressions are tricky in JS, we use IIFE
                let mut block_code = "(() => {".to_string();
                for stmt in stmts {
                    // Convert statement to expression form
                    match stmt {
                        SirStatement::Let { name, typ, value } => {
                            block_code.push_str(&format!(" let {} = {};", name, self.expr_to_js(value)));
                        }
                        SirStatement::Expr(e) => {
                            block_code.push_str(&format!(" {};", self.expr_to_js(e)));
                        }
                        _ => {
                            // For other statements, we'd need more complex handling
                            block_code.push_str(" /* statement */");
                        }
                    }
                }
                if let Some(final_e) = final_expr {
                    block_code.push_str(&format!(" return {}; ", self.expr_to_js(final_e)));
                } else {
                    block_code.push_str(" return undefined; ");
                }
                block_code.push_str("})()");
                block_code
            }
        }
    }

    fn type_to_js(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" | "i64" | "u32" | "u64" => "number",
            "float" | "f32" | "f64" => "number",
            "bool" => "boolean",
            "string" | "String" => "string",
            "char" => "string",
            _ => "any",
        }
    }

    fn default_value_for_type(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" | "i64" | "u32" | "u64" => "0",
            "float" | "f32" | "f64" => "0.0",
            "bool" => "false",
            "string" | "String" => "\"\"",
            "char" => "''",
            _ => "null",
        }
    }

    fn writeln(&mut self, line: &str) {
        let indent = "    ".repeat(self.indent_level);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_compiler::sir::*;
    use nova_common::Span;

    #[test]
    fn test_simple_function() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "add".to_string(),
                params: vec![
                    SirParam { name: "a".to_string(), typ: "int".to_string() },
                    SirParam { name: "b".to_string(), typ: "int".to_string() },
                ],
                return_type: Some("int".to_string()),
                body: vec![
                    SirStatement::Return(Some(SirExpr::BinOp {
                        left: Box::new(SirExpr::Var("a".to_string())),
                        op: SirBinOp::Add,
                        right: Box::new(SirExpr::Var("b".to_string())),
                    })),
                ],
                span: Span::default(),
            }],
            globals: vec![],
        };

        let mut backend = JsBackend::new();
        let js_code = backend.generate(&program);
        
        assert!(js_code.contains("function add(a, b)"));
        assert!(js_code.contains("return (a + b);"));
        // No trailing `main();` expected: the test program defines only `add`.
        assert!(!js_code.contains("main();"));
    }

    #[test]
    fn test_for_loop() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "main".to_string(),
                params: vec![],
                return_type: Some("void".to_string()),
                body: vec![
                    SirStatement::For {
                        pattern: "i".to_string(),
                        iterable: SirExpr::BinOp {
                            left: Box::new(SirExpr::Literal(SirLiteral::Int(0))),
                            op: SirBinOp::Range,
                            right: Box::new(SirExpr::Literal(SirLiteral::Int(5))),
                        },
                        body: vec![
                            SirStatement::Expr(SirExpr::Var("i".to_string())),
                        ],
                    },
                ],
                span: Span::default(),
            }],
            globals: vec![],
        };

        let mut backend = JsBackend::new();
        let js_code = backend.generate(&program);
        
        assert!(js_code.contains("for (let i = 0; i < 5; i++)"));
    }
}
