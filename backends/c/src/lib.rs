//! C Backend Code Generator
//! 
//! Translates SIR (Semantic Intermediate Representation) to C code

use nova_compiler::sir::*;

pub struct CGenerator {
    output: String,
    indent_level: usize,
}

impl CGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        self.indent_level = 0;

        // Generate includes
        self.writeln("#include <stdio.h>");
        self.writeln("#include <stdlib.h>");
        self.writeln("#include <string.h>");
        self.writeln("");

        // Generate forward declarations for functions (skip main, declare it last as int)
        for func in &program.functions {
            if func.name != "__main" && func.name != "main" {
                self.generate_function_declaration(func);
            }
        }
        
        // Declare main function with proper signature
        if program.functions.iter().any(|f| f.name == "__main" || f.name == "main") {
            self.writeln("int main(void);");
        }

        if !program.functions.is_empty() {
            self.writeln("");
        }

        // Generate global variables
        for global in &program.globals {
            self.generate_global(global);
        }

        if (!program.globals.is_empty() || !program.functions.is_empty()) && program.functions.iter().any(|f| f.name == "__main" || f.name == "main") {
            self.writeln("");
        }

        // Generate function definitions (non-main first)
        for func in &program.functions {
            if func.name != "__main" && func.name != "main" {
                self.generate_function_definition(func);
                self.writeln("");
            }
        }
        
        // Generate main function last
        for func in &program.functions {
            if func.name == "__main" || func.name == "main" {
                self.generate_function_definition(func);
                self.writeln("");
            }
        }

        self.output.clone()
    }

    fn generate_function_declaration(&mut self, func: &SirFunction) {
        let return_type = self.get_c_return_type(&func.return_type);
        
        self.write(&format!("{} {}(", return_type, func.name));
        
        if func.params.is_empty() {
            self.write("void");
        } else {
            for (i, param) in func.params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                let param_type = self.get_c_type(&param.typ);
                self.write(&format!("{} {}", param_type, param.name));
            }
        }
        
        self.writeln(");");
    }

    fn generate_global(&mut self, global: &SirGlobal) {
        let c_type = self.get_c_type(&global.typ);
        self.write(&format!("{} {}", c_type, global.name));
        
        if let Some(init_value) = &global.init_value {
            self.write(" = ");
            self.generate_expr(init_value);
        }
        
        self.writeln(";");
    }

    fn generate_function_definition(&mut self, func: &SirFunction) {
        let return_type = self.get_c_return_type(&func.return_type);
        
        // Use proper main signature for main function
        if func.name == "__main" || func.name == "main" {
            self.writeln("int main(void) {");
        } else {
            self.write(&format!("{} {}(", return_type, func.name));
            
            if func.params.is_empty() {
                self.write("void");
            } else {
                for (i, param) in func.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let param_type = self.get_c_type(&param.typ);
                    self.write(&format!("{} {}", param_type, param.name));
                }
            }
            
            self.writeln(") {");
        }
        self.indent_level += 1;

        for stmt in &func.body {
            self.generate_statement(stmt);
        }

        // Add implicit return for main function
        if func.name == "__main" || func.name == "main" {
            if func.body.last().map_or(true, |s| !matches!(s, SirStatement::Return(_))) {
                self.writeln("return 0;");
            }
        } else if func.return_type.is_none() {
            // void function, no return needed
        } else {
            // Check if last statement is a return, if not add one
            if func.body.last().map_or(true, |s| !matches!(s, SirStatement::Return(_))) {
                self.writeln("return 0;");
            }
        }

        self.indent_level -= 1;
        self.writeln("}");
    }

    fn generate_statement(&mut self, stmt: &SirStatement) {
        match stmt {
            SirStatement::Let { name, typ, value } => {
                let c_type = match typ {
                    Some(t) => self.get_c_type(t),
                    None => "int".to_string(),
                };
                self.write(&format!("{} {} = ", c_type, name));
                self.generate_expr(value);
                self.writeln(";");
            }
            SirStatement::Expr(expr) => {
                // Check if it's a call to print/println and convert to printf
                if let SirExpr::Call { callee, args } = expr {
                    if callee == "print" || callee == "println" {
                        self.write("printf(");
                        let mut format_string = String::new();
                        let mut has_args = false;
                        
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                format_string.push(' ');
                            }
                            // Simple heuristic: if it's a string literal, use %s
                            // otherwise determine format based on type
                            if let SirExpr::Literal(SirLiteral::String(_)) = arg {
                                format_string.push_str("%s");
                            } else {
                                // For non-string args, need format specifier
                                // For now, assume int and use %d
                                format_string.push_str("%d");
                            }
                            has_args = true;
                        }
                        
                        // Add newline for println
                        if callee == "println" {
                            format_string.push_str("\\n");
                        }
                        
                        self.write(&format!("\"{}\"", format_string));
                        
                        if has_args {
                            self.write(", ");
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    self.write(", ");
                                }
                                self.generate_expr(arg);
                            }
                        }
                        
                        self.write(")");
                        self.writeln(";");
                        return;
                    }
                }
                self.generate_expr(expr);
                self.writeln(";");
            }
            SirStatement::Return(expr) => {
                self.write("return");
                if let Some(e) = expr {
                    self.write(" ");
                    self.generate_expr(e);
                }
                self.writeln(";");
            }
            SirStatement::If { condition, then_block, else_block } => {
                self.write("if (");
                self.generate_expr(condition);
                self.writeln(") {");
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
                self.write("while (");
                self.generate_expr(condition);
                self.writeln(") {");
                self.indent_level += 1;
                for stmt in body {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                self.writeln("}");
            }
            SirStatement::For { pattern, iterable, body } => {
                // Generate a for loop assuming the iterable is a range call
                // Pattern: for i in range(start, end) -> for (int i = start; i < end; i++)
                if let SirExpr::Call { callee, args } = iterable {
                    if callee == "range" && args.len() == 2 {
                        let c_type = "int";
                        self.write(&format!("for ({} {} = ", c_type, pattern));
                        self.generate_expr(&args[0]);
                        self.write("; ");
                        self.write(&format!("{} < ", pattern));
                        self.generate_expr(&args[1]);
                        self.write("; ");
                        self.write(&format!("{}++)", pattern));
                        self.writeln(" {");
                        self.indent_level += 1;
                        for stmt in body {
                            self.generate_statement(stmt);
                        }
                        self.indent_level -= 1;
                        self.writeln("}");
                        return;
                    }
                }
                // Fallback: simple iteration if not a range call
                self.writeln("// TODO: Implement generic for loop");
            }
            SirStatement::Block(stmts) => {
                self.writeln("{");
                self.indent_level += 1;
                for stmt in stmts {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                self.writeln("}");
            }
        }
    }

    fn generate_expr(&mut self, expr: &SirExpr) {
        match expr {
            SirExpr::Literal(lit) => {
                match lit {
                    SirLiteral::Int(i) => self.write(&format!("{}", i)),
                    SirLiteral::Float(f) => self.write(&format!("{}", f)),
                    SirLiteral::Bool(b) => self.write(if *b { "1" } else { "0" }),
                    SirLiteral::String(s) => self.write(&format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))),
                    SirLiteral::Char(c) => self.write(&format!("'{}'", c)),
                    SirLiteral::Unit => {},
                }
            }
            SirExpr::Var(name) => {
                self.write(name);
            }
            SirExpr::BinOp { left, op, right } => {
                self.write("(");
                self.generate_expr(left);
                self.write(&format!(" {} ", self.get_c_bin_op(op)));
                self.generate_expr(right);
                self.write(")");
            }
            SirExpr::UnaryOp { op, operand } => {
                self.write(&self.get_c_un_op(op));
                self.generate_expr(operand);
            }
            SirExpr::Call { callee, args } => {
                self.write(&format!("{}(", callee));
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expr(arg);
                }
                self.write(")");
            }
            SirExpr::Assign { target, value } => {
                self.write(&format!("{} = ", target));
                self.generate_expr(value);
            }
            SirExpr::If { condition, then_expr, else_expr } => {
                self.write("(");
                self.generate_expr(condition);
                self.write(" ? ");
                self.generate_expr(then_expr);
                if let Some(else_e) = else_expr {
                    self.write(" : ");
                    self.generate_expr(else_e);
                } else {
                    self.write(" : 0");
                }
                self.write(")");
            }
            SirExpr::Block(stmts, final_expr) => {
                self.writeln("{");
                self.indent_level += 1;
                for stmt in stmts {
                    self.generate_statement(stmt);
                }
                if let Some(expr) = final_expr {
                    self.generate_expr(expr);
                }
                self.indent_level -= 1;
                self.write("}");
            }
        }
    }

    fn get_c_type(&self, nova_type: &str) -> String {
        match nova_type.to_lowercase().as_str() {
            "int" | "i32" | "i64" => "int".to_string(),
            "float" | "f32" | "f64" => "double".to_string(),
            "bool" => "int".to_string(),
            "char" => "char".to_string(),
            "string" | "str" => "char*".to_string(),
            _ => "int".to_string(), // Default to int for unknown types
        }
    }

    fn get_c_return_type(&self, return_type: &Option<String>) -> String {
        match return_type {
            Some(t) => self.get_c_type(t),
            None => "void".to_string(),
        }
    }

    fn get_c_bin_op(&self, op: &SirBinOp) -> &'static str {
        match op {
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
        }
    }

    fn get_c_un_op(&self, op: &SirUnOp) -> &'static str {
        match op {
            SirUnOp::Neg => "-",
            SirUnOp::Not => "!",
        }
    }

    fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn writeln(&mut self, text: &str) {
        for _ in 0..self.indent_level {
            self.output.push_str("    ");
        }
        self.output.push_str(text);
        self.output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_common::span::Span;

    #[test]
    fn test_simple_function() {
        let mut generator = CGenerator::new();
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "main".to_string(),
                params: vec![],
                return_type: Some("int".to_string()),
                body: vec![
                    SirStatement::Expr(SirExpr::Call {
                        callee: "printf".to_string(),
                        args: vec![
                            SirExpr::Literal(SirLiteral::String("Hello, World!\n".to_string())),
                        ],
                    }),
                    SirStatement::Return(Some(SirExpr::Literal(SirLiteral::Int(0)))),
                ],
                span: Span::default(),
            }],
            globals: vec![],
        };

        let c_code = generator.generate(&program);
        println!("Generated C code:\n{}", c_code);
        assert!(c_code.contains("int main(void)"));
        assert!(c_code.contains("printf"));
        assert!(c_code.contains("Hello, World!"));
        // The return is already in the body, so we just check it exists
        assert!(c_code.contains("return"));
    }
}
