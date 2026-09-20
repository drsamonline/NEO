//! JavaScript Backend for Nova Compiler
//! 
//! This module converts SIR (Semantic Intermediate Representation) to JavaScript code.

use nova_compiler::sir::{SirProgram, SirFunction, SirGlobal, SirStatement, SirExpr, SirLiteral, SirBinOp, SirUnOp};

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
        let js_type = self.type_to_js(&global.typ);
        let init = if let Some(init_expr) = &global.init_value {
            self.expr_to_js(init_expr)
        } else {
            self.default_value_for_type(&global.typ)
        };
        self.writeln(&format!("let {} = {};", global.name, init));
    }

    fn generate_function(&mut self, func: &SirFunction) {
        let params = func.params.join(", ");
        self.writeln(&format!("function {}({}) {{", func.name, params));
        
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
                let js_type = self.type_to_js(typ);
                let init = if let Some(val) = value {
                    self.expr_to_js(val)
                } else {
                    self.default_value_for_type(typ)
                };
                self.writeln(&format!("let {} = {};", name, init));
            }
            SirStatement::Assign { target, value } => {
                let val_str = self.expr_to_js(value);
                self.writeln(&format!("{} = {};", target, val_str));
            }
            SirStatement::Expr(expr) => {
                let expr_str = self.expr_to_js(expr);
                self.writeln(&format!("{};", expr_str));
            }
            SirStatement::If { condition, then_body, else_body } => {
                let cond_str = self.expr_to_js(condition);
                self.writeln(&format!("if ({}) {{", cond_str));
                
                self.indent_level += 1;
                for stmt in then_body {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                
                if let Some(else_stmts) = else_body {
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
            SirStatement::Return(value) => {
                let val_str = if let Some(val) = value {
                    self.expr_to_js(val)
                } else {
                    "undefined".to_string()
                };
                self.writeln(&format!("return {};", val_str));
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
            SirExpr::Identifier(name) => name.clone(),
            SirExpr::Binary { left, op, right } => {
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
            SirExpr::Unary { op, operand } => {
                let operand_str = self.expr_to_js(operand);
                let op_str = match op {
                    SirUnOp::Neg => "-",
                    SirUnOp::Not => "!",
                };
                format!("({}{})", op_str, operand_str)
            }
            SirExpr::Call { func, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.expr_to_js(a)).collect();
                format!("{}({})", func, args_str.join(", "))
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
                        SirStatement::Assign { target, value } => {
                            block_code.push_str(&format!(" {} = {};", target, self.expr_to_js(value)));
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
            SirExpr::Break => "/* break */".to_string(),
            SirExpr::Continue => "/* continue */".to_string(),
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

    #[test]
    fn test_simple_function() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "add".to_string(),
                params: vec!["a".to_string(), "b".to_string()],
                return_type: "int".to_string(),
                body: vec![
                    SirStatement::Return(Some(SirExpr::Binary {
                        left: Box::new(SirExpr::Identifier("a".to_string())),
                        op: SirBinOp::Add,
                        right: Box::new(SirExpr::Identifier("b".to_string())),
                    })),
                ],
            }],
            globals: vec![],
        };

        let mut backend = JsBackend::new();
        let js_code = backend.generate(&program);
        
        assert!(js_code.contains("function add(a, b)"));
        assert!(js_code.contains("return (a + b);"));
        assert!(js_code.contains("add();"));
    }

    #[test]
    fn test_for_loop() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "main".to_string(),
                params: vec![],
                return_type: "void".to_string(),
                body: vec![
                    SirStatement::For {
                        pattern: "i".to_string(),
                        iterable: SirExpr::Binary {
                            left: Box::new(SirExpr::Literal(SirLiteral::Int(0))),
                            op: SirBinOp::Range,
                            right: Box::new(SirExpr::Literal(SirLiteral::Int(5))),
                        },
                        body: vec![
                            SirStatement::Expr(SirExpr::Identifier("i".to_string())),
                        ],
                    },
                ],
            }],
            globals: vec![],
        };

        let mut backend = JsBackend::new();
        let js_code = backend.generate(&program);
        
        assert!(js_code.contains("for (let i = 0; i < 5; i++)"));
    }
}
