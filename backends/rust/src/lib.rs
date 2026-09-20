//! Rust Backend for Nova Compiler
//! 
//! This module converts SIR (Semantic Intermediate Representation) to Rust code.

use nova_compiler::sir::{SirProgram, SirFunction, SirGlobal, SirStatement, SirExpr, SirLiteral, SirBinOp, SirUnOp};

pub struct RustBackend {
    output: String,
    indent_level: usize,
}

impl RustBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        
        // Add standard imports
        self.writeln("use std::io;");
        self.writeln("");
        
        // Generate globals as static mut (not ideal but works for simple cases)
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
        
        // Generate main function call if needed
        if program.functions.iter().any(|f| f.name == "main") {
            // main is already generated
        }
        
        self.output.clone()
    }

    fn generate_global(&mut self, global: &SirGlobal) {
        let rust_type = self.type_to_rust(&global.typ);
        if let Some(init_expr) = &global.init_value {
            let init = self.expr_to_rust(init_expr);
            self.writeln(&format!("static mut {}: {} = {};", global.name, rust_type, init));
        } else {
            self.writeln(&format!("static mut {}: {} = {};", global.name, rust_type, self.default_value_for_type(&global.typ)));
        }
    }

    fn generate_function(&mut self, func: &SirFunction) {
        let params: Vec<String> = func.params.iter()
            .map(|p| format!("{}: i32", p)) // Simplified: all params as i32
            .collect();
        
        let return_type = if func.return_type == "void" || func.return_type.is_empty() {
            String::new()
        } else {
            format!(" -> {}", self.type_to_rust(&func.return_type))
        };
        
        self.writeln(&format!("fn {}({}){} {{", func.name, params.join(", "), return_type));
        
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
                let rust_type = self.type_to_rust(typ);
                if let Some(val) = value {
                    let val_str = self.expr_to_rust(val);
                    self.writeln(&format!("let {}: {} = {};", name, rust_type, val_str));
                } else {
                    self.writeln(&format!("let {}: {} = {};", name, rust_type, self.default_value_for_type(typ)));
                }
            }
            SirStatement::Assign { target, value } => {
                let val_str = self.expr_to_rust(value);
                self.writeln(&format!("{} = {};", target, val_str));
            }
            SirStatement::Expr(expr) => {
                let expr_str = self.expr_to_rust(expr);
                self.writeln(&format!("{};", expr_str));
            }
            SirStatement::If { condition, then_body, else_body } => {
                let cond_str = self.expr_to_rust(condition);
                self.writeln(&format!("if {} {{", cond_str));
                
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
                let cond_str = self.expr_to_rust(condition);
                self.writeln(&format!("while {} {{", cond_str));
                
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
                    let start_str = self.expr_to_rust(left);
                    let end_str = self.expr_to_rust(right);
                    self.writeln(&format!("for {} in {}..{} {{", pattern, start_str, end_str));
                } else {
                    // Generic for loop - would need arrays/iterables support
                    let iter_str = self.expr_to_rust(iterable);
                    self.writeln(&format!("for {} in {} {{", pattern, iter_str));
                }
                
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                
                self.writeln("}");
            }
            SirStatement::Return(value) => {
                if let Some(val) = value {
                    let val_str = self.expr_to_rust(val);
                    self.writeln(&format!("return {};", val_str));
                } else {
                    self.writeln("return;");
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

    fn expr_to_rust(&self, expr: &SirExpr) -> String {
        match expr {
            SirExpr::Literal(lit) => {
                match lit {
                    SirLiteral::Int(n) => n.to_string(),
                    SirLiteral::Float(f) => format!("{}f64", f),
                    SirLiteral::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
                    SirLiteral::Bool(b) => b.to_string(),
                    SirLiteral::Char(c) => format!("'{}'", c),
                    SirLiteral::Unit => "()".to_string(),
                }
            }
            SirExpr::Identifier(name) => name.clone(),
            SirExpr::Binary { left, op, right } => {
                let left_str = self.expr_to_rust(left);
                let right_str = self.expr_to_rust(right);
                let op_str = match op {
                    SirBinOp::Add => "+",
                    SirBinOp::Sub => "-",
                    SirBinOp::Mul => "*",
                    SirBinOp::Div => "/",
                    SirBinOp::Mod => "%",
                    SirBinOp::Eq => "==",
                    SirBinOp::Ne => "!=",
                    SirBinOp::Lt => "<",
                    SirBinOp::Le => "<=",
                    SirBinOp::Gt => ">",
                    SirBinOp::Ge => ">=",
                    SirBinOp::And => "&&",
                    SirBinOp::Or => "||",
                    SirBinOp::Range => "..", // Should not appear standalone
                };
                format!("({} {} {})", left_str, op_str, right_str)
            }
            SirExpr::Unary { op, operand } => {
                let operand_str = self.expr_to_rust(operand);
                let op_str = match op {
                    SirUnOp::Neg => "-",
                    SirUnOp::Not => "!",
                };
                format!("({}{})", op_str, operand_str)
            }
            SirExpr::Call { func, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.expr_to_rust(a)).collect();
                format!("{}({})", func, args_str.join(", "))
            }
            SirExpr::If { condition, then_expr, else_expr } => {
                let cond_str = self.expr_to_rust(condition);
                let then_str = self.expr_to_rust(then_expr);
                if let Some(else_e) = else_expr {
                    let else_str = self.expr_to_rust(else_e);
                    format!("if {} {{ {} }} else {{ {} }}", cond_str, then_str, else_str)
                } else {
                    format!("if {} {{ {} }}", cond_str, then_str)
                }
            }
            SirExpr::Block(stmts, final_expr) => {
                // Blocks in Rust are expressions
                let mut block_code = "{ ".to_string();
                for stmt in stmts {
                    match stmt {
                        SirStatement::Let { name, typ, value } => {
                            let rust_type = self.type_to_rust(typ);
                            if let Some(val) = value {
                                block_code.push_str(&format!("let {}: {} = {}; ", name, rust_type, self.expr_to_rust(val)));
                            } else {
                                block_code.push_str(&format!("let {}: {} = {}; ", name, rust_type, self.default_value_for_type(typ)));
                            }
                        }
                        SirStatement::Assign { target, value } => {
                            block_code.push_str(&format!("{} = {}; ", target, self.expr_to_rust(value)));
                        }
                        SirStatement::Expr(e) => {
                            block_code.push_str(&format!("{}; ", self.expr_to_rust(e)));
                        }
                        _ => block_code.push_str("/* stmt */ "),
                    }
                }
                if let Some(final_e) = final_expr {
                    block_code.push_str(&format!("{} }}", self.expr_to_rust(final_e)));
                } else {
                    block_code.push_str("()}");
                }
                block_code
            }
            SirExpr::Break => "break".to_string(),
            SirExpr::Continue => "continue".to_string(),
        }
    }

    fn type_to_rust(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" => "i32",
            "i64" => "i64",
            "u32" => "u32",
            "u64" => "u64",
            "float" | "f32" => "f32",
            "f64" => "f64",
            "bool" => "bool",
            "string" | "String" => "String",
            "char" => "char",
            "void" => "()",
            _ => "i32", // Default to i32 for unknown types
        }
    }

    fn default_value_for_type(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" | "i64" | "u32" | "u64" => "0",
            "float" | "f32" | "f64" => "0.0",
            "bool" => "false",
            "string" | "String" => "\"\"",
            "char" => "'\\0'",
            "void" | "()" => "()",
            _ => "0",
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

        let mut backend = RustBackend::new();
        let rust_code = backend.generate(&program);
        
        assert!(rust_code.contains("fn add("));
        assert!(rust_code.contains("return (a + b);"));
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

        let mut backend = RustBackend::new();
        let rust_code = backend.generate(&program);
        
        assert!(rust_code.contains("for i in 0..5"));
    }
}
