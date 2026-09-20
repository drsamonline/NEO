//! WebAssembly Backend for Nova Compiler
//! 
//! This module converts SIR (Semantic Intermediate Representation) to WebAssembly Text Format (WAT).

use nova_compiler::sir::{SirProgram, SirFunction, SirGlobal, SirStatement, SirExpr, SirLiteral, SirBinOp, SirUnOp};

pub struct WasmBackend {
    output: String,
    indent_level: usize,
    local_count: usize,
}

impl WasmBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            local_count: 0,
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        
        // Module header
        self.writeln("(module");
        self.indent_level += 1;
        
        // Generate globals
        for global in &program.globals {
            self.generate_global(global);
        }
        
        // Generate functions
        for func in &program.functions {
            self.generate_function(func);
        }
        
        // Export main function if it exists
        if program.functions.iter().any(|f| f.name == "main") {
            self.writeln("(export \"main\" (func $main))");
        }
        
        self.indent_level -= 1;
        self.writeln(")");
        
        self.output.clone()
    }

    fn generate_global(&mut self, global: &SirGlobal) {
        let wasm_type = self.type_to_wasm(&global.typ);
        let init_value = if let Some(init_expr) = &global.init_value {
            self.literal_to_wasm_const(init_expr)
        } else {
            self.default_const_for_type(&global.typ)
        };
        
        self.writeln(&format!("(global ${} (mut {}) {})", global.name, wasm_type, init_value));
    }

    fn generate_function(&mut self, func: &SirFunction) {
        self.local_count = 0;
        
        // Determine result type
        let result_type = if func.return_type == "void" || func.return_type.is_empty() {
            String::new()
        } else {
            format!(" (result {})", self.type_to_wasm(&func.return_type))
        };
        
        // Build parameter list
        let params: Vec<String> = func.params.iter()
            .map(|p| format!("(param ${} i32)", p))
            .collect();
        
        self.writeln(&format!("(func ${}{}{}", func.name, if params.is_empty() { String::new() } else { format!(" {}", params.join(" ")) }, result_type));
        self.indent_level += 1;
        
        // Generate function body as a block
        self.writeln("(block");
        self.indent_level += 1;
        
        for stmt in &func.body {
            self.generate_statement(stmt);
        }
        
        self.indent_level -= 1;
        self.writeln(")");
        
        self.indent_level -= 1;
        self.writeln(")");
    }

    fn generate_statement(&mut self, stmt: &SirStatement) {
        match stmt {
            SirStatement::Let { name, typ, value } => {
                // In Wasm, locals are declared at function start, we'll use a simplified approach
                if let Some(val) = value {
                    self.generate_expr(val);
                    self.writeln(&format!("(local.set ${})", name));
                } else {
                    self.writeln(&format!("({} (local.get ${}))", self.default_for_type(typ), name));
                }
            }
            SirStatement::Assign { target, value } => {
                self.generate_expr(value);
                self.writeln(&format!("(local.set ${})", target));
            }
            SirStatement::Expr(expr) => {
                self.generate_expr(expr);
                self.writeln("drop");
            }
            SirStatement::If { condition, then_body, else_body } => {
                self.generate_expr(condition);
                self.writeln("(then");
                self.indent_level += 1;
                for stmt in then_body {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                
                if let Some(else_stmts) = else_body {
                    self.writeln(")(else");
                    self.indent_level += 1;
                    for stmt in else_stmts {
                        self.generate_statement(stmt);
                    }
                    self.indent_level -= 1;
                    self.writeln(")");
                } else {
                    self.writeln(")");
                }
            }
            SirStatement::While { condition, body } => {
                // While loop using block and br
                self.writeln("(block $break");
                self.indent_level += 1;
                self.writeln("(loop $continue");
                self.indent_level += 1;
                
                self.generate_expr(condition);
                self.writeln("(i32.eqz)");
                self.writeln("(br_if $break)");
                
                for stmt in body {
                    self.generate_statement(stmt);
                }
                
                self.writeln("(br $continue)");
                
                self.indent_level -= 1;
                self.writeln(")");
                self.indent_level -= 1;
                self.writeln(")");
            }
            SirStatement::For { pattern, iterable, body } => {
                // For loop with range: convert to while loop
                if let SirExpr::BinOp { left, op: SirBinOp::Range, right } = iterable {
                    // Initialize counter
                    self.generate_expr(left);
                    self.writeln(&format!("(local.set ${})", pattern));
                    
                    let end_name = format!("{}_end", pattern);
                    self.generate_expr(right);
                    self.writeln(&format!("(local.set ${})", end_name));
                    
                    self.writeln("(block $break");
                    self.indent_level += 1;
                    self.writeln("(loop $continue");
                    self.indent_level += 1;
                    
                    // Check condition: counter < end
                    self.writeln(&format!("(local.get ${})", pattern));
                    self.writeln(&format!("(local.get ${})", end_name));
                    self.writeln("(i32.lt_s)");
                    self.writeln("(i32.eqz)");
                    self.writeln("(br_if $break)");
                    
                    for stmt in body {
                        self.generate_statement(stmt);
                    }
                    
                    // Increment counter
                    self.writeln(&format!("(local.get ${})", pattern));
                    self.writeln("(i32.const 1)");
                    self.writeln("(i32.add)");
                    self.writeln(&format!("(local.set ${})", pattern));
                    
                    self.writeln("(br $continue)");
                    
                    self.indent_level -= 1;
                    self.writeln(")");
                    self.indent_level -= 1;
                    self.writeln(")");
                } else {
                    // Generic for loop - simplified
                    self.generate_expr(iterable);
                    self.writeln(&format!("(local.set ${})", pattern));
                    for stmt in body {
                        self.generate_statement(stmt);
                    }
                }
            }
            SirStatement::Return(value) => {
                if let Some(val) = value {
                    self.generate_expr(val);
                    self.writeln("return");
                } else {
                    self.writeln("return");
                }
            }
            SirStatement::Break => {
                self.writeln("br 1");
            }
            SirStatement::Continue => {
                self.writeln("br 0");
            }
        }
    }

    fn generate_expr(&mut self, expr: &SirExpr) {
        match expr {
            SirExpr::Literal(lit) => {
                match lit {
                    SirLiteral::Int(n) => self.writeln(&format!("(i32.const {})", n)),
                    SirLiteral::Float(f) => self.writeln(&format!("(f64.const {})", f)),
                    SirLiteral::String(_) => self.writeln("(i32.const 0)"), // String placeholder
                    SirLiteral::Bool(b) => self.writeln(&format!("(i32.const {})", if *b { 1 } else { 0 })),
                    SirLiteral::Char(c) => self.writeln(&format!("(i32.const {})", *c as u8)),
                    SirLiteral::Unit => {}, // No operation needed
                }
            }
            SirExpr::Identifier(name) => {
                self.writeln(&format!("(local.get ${})", name));
            }
            SirExpr::Binary { left, op, right } => {
                self.generate_expr(left);
                self.generate_expr(right);
                
                let op_str = match op {
                    SirBinOp::Add => "i32.add",
                    SirBinOp::Sub => "i32.sub",
                    SirBinOp::Mul => "i32.mul",
                    SirBinOp::Div => "i32.div_s",
                    SirBinOp::Mod => "i32.rem_s",
                    SirBinOp::Eq => "i32.eq",
                    SirBinOp::Ne => "i32.ne",
                    SirBinOp::Lt => "i32.lt_s",
                    SirBinOp::Le => "i32.le_s",
                    SirBinOp::Gt => "i32.gt_s",
                    SirBinOp::Ge => "i32.ge_s",
                    SirBinOp::And => "i32.and",
                    SirBinOp::Or => "i32.or",
                    SirBinOp::Range => ";; range", // Should not appear
                };
                self.writeln(op_str);
            }
            SirExpr::Unary { op, operand } => {
                self.generate_expr(operand);
                
                let op_str = match op {
                    SirUnOp::Neg => "i32.neg",
                    SirUnOp::Not => "i32.eqz",
                };
                self.writeln(op_str);
            }
            SirExpr::Call { func, args } => {
                for arg in args {
                    self.generate_expr(arg);
                }
                self.writeln(&format!("(call ${})", func));
            }
            SirExpr::If { condition, then_expr, else_expr } => {
                self.generate_expr(condition);
                self.writeln("(then");
                self.indent_level += 1;
                self.generate_expr(then_expr);
                self.indent_level -= 1;
                
                if let Some(else_e) = else_expr {
                    self.writeln(")(else");
                    self.indent_level += 1;
                    self.generate_expr(else_e);
                    self.indent_level -= 1;
                    self.writeln(")");
                } else {
                    self.writeln(")");
                }
            }
            SirExpr::Break | SirExpr::Continue => {
                // These are handled in statements, not expressions
            }
        }
    }

    fn literal_to_wasm_const(&self, expr: &SirExpr) -> String {
        if let SirExpr::Literal(lit) = expr {
            match lit {
                SirLiteral::Int(n) => format!("(i32.const {})", n),
                SirLiteral::Float(f) => format!("(f64.const {})", f),
                SirLiteral::Bool(b) => format!("(i32.const {})", if *b { 1 } else { 0 }),
                _ => "(i32.const 0)".to_string(),
            }
        } else {
            "(i32.const 0)".to_string()
        }
    }

    fn type_to_wasm(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" | "i64" | "u32" | "u64" | "bool" | "char" => "i32",
            "float" | "f32" => "f32",
            "f64" => "f64",
            _ => "i32",
        }
    }

    fn default_const_for_type(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" | "i64" | "u32" | "u64" | "bool" | "char" => "(i32.const 0)",
            "float" | "f32" => "(f32.const 0.0)",
            "f64" => "(f64.const 0.0)",
            _ => "(i32.const 0)",
        }
    }

    fn default_for_type(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" | "i64" | "u32" | "u64" | "bool" | "char" => "i32.const 0",
            "float" | "f32" => "f32.const 0.0",
            "f64" => "f64.const 0.0",
            _ => "i32.const 0",
        }
    }

    fn writeln(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent_level);
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

        let mut backend = WasmBackend::new();
        let wasm_code = backend.generate(&program);
        
        assert!(wasm_code.contains("(func $add"));
        assert!(wasm_code.contains("(export \"main\""));
    }
}
