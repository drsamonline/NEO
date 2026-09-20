//! C Backend - Generates C code from SIR

use crate::sir::*;
use std::fmt::Write;

/// C code generator backend
pub struct CBackend {
    output: String,
}

impl CBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    /// Generate C code from a SIR program
    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        
        // Add includes
        let _ = writeln!(self.output, "#include <stdio.h>");
        let _ = writeln!(self.output, "#include <stdlib.h>");
        let _ = writeln!(self.output, "#include <string.h>");
        let _ = writeln!(self.output);
        
        // Generate forward declarations for functions
        for func in &program.functions {
            self.write_function_decl(func);
        }
        let _ = writeln!(self.output);
        
        // Generate global variables
        for global in &program.globals {
            let typ_str = Self::type_to_c_str(&global.typ);
            let _ = write!(self.output, "{} {}", typ_str, global.name);
            if let Some(init) = &global.init_value {
                let _ = write!(self.output, " = {}", self.expr_to_c(init));
            }
            let _ = writeln!(self.output, ";");
        }
        if !program.globals.is_empty() {
            let _ = writeln!(self.output);
        }
        
        // Generate function definitions
        for func in &program.functions {
            self.write_function(func);
        }
        
        self.output.clone()
    }

    fn write_function_decl(&mut self, func: &SirFunction) {
        let return_type = Self::type_to_c_str(func.return_type.as_deref().unwrap_or("void"));
        let _ = write!(self.output, "{} {}(", return_type, func.name);
        
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                let _ = write!(self.output, ", ");
            }
            let _ = write!(self.output, "{} {}", Self::type_to_c_str(&param.typ), param.name);
        }
        
        let _ = writeln!(self.output, ");");
    }

    fn write_function(&mut self, func: &SirFunction) {
        let return_type = Self::type_to_c_str(func.return_type.as_deref().unwrap_or("void"));
        let _ = write!(self.output, "{} {}(", return_type, func.name);
        
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                let _ = write!(self.output, ", ");
            }
            let _ = write!(self.output, "{} {}", Self::type_to_c_str(&param.typ), param.name);
        }
        
        let _ = writeln!(self.output, ") {{");
        
        // Generate function body
        for stmt in &func.body {
            self.write_statement(stmt, 1);
        }
        
        let _ = writeln!(self.output, "}}");
        let _ = writeln!(self.output);
    }

    fn type_to_c_str(typ: &str) -> &'static str {
        match typ {
            "Int" => "int",
            "Float" => "double",
            "Bool" => "int",
            "String" => "char*",
            "Char" => "char",
            "void" => "void",
            _ => "int", // default to int for unknown types
        }
    }

    fn write_statement(&mut self, stmt: &SirStatement, indent: usize) {
        let indent_str = "    ".repeat(indent);
        
        match stmt {
            SirStatement::Let { name, typ, value } => {
                let typ_str = typ.as_ref().map(|t| Self::type_to_c_str(t)).unwrap_or("int");
                let _ = write!(self.output, "{}{} {} = ", indent_str, typ_str, name);
                let _ = writeln!(self.output, "{};", self.expr_to_c(value));
            }
            SirStatement::Expr(expr) => {
                // Check if it's a print call and convert to printf
                if let SirExpr::Call { callee, args } = expr {
                    if callee == "print" && args.len() == 1 {
                        let arg = &args[0];
                        // Determine format specifier based on expression type
                        let fmt_specifier = self.infer_format_specifier(arg);
                        let _ = writeln!(self.output, "{}printf(\"{}\\n\", {});", indent_str, fmt_specifier, self.expr_to_c(arg));
                        return;
                    }
                }
                let _ = writeln!(self.output, "{}{};", indent_str, self.expr_to_c(expr));
            }
            SirStatement::Return(expr) => {
                match expr {
                    Some(e) => {
                        let _ = writeln!(self.output, "{}return {};", indent_str, self.expr_to_c(e));
                    }
                    None => {
                        let _ = writeln!(self.output, "{}return;", indent_str);
                    }
                }
            }
            SirStatement::If { condition, then_block, else_block } => {
                let _ = writeln!(self.output, "{}if ({}) {{", indent_str, self.expr_to_c(condition));
                for s in then_block {
                    self.write_statement(s, indent + 1);
                }
                if let Some(else_stmts) = else_block {
                    let _ = writeln!(self.output, "{}}} else {{", indent_str);
                    for s in else_stmts {
                        self.write_statement(s, indent + 1);
                    }
                }
                let _ = writeln!(self.output, "{}}}", indent_str);
            }
            SirStatement::While { condition, body } => {
                let _ = writeln!(self.output, "{}while ({}) {{", indent_str, self.expr_to_c(condition));
                for s in body {
                    self.write_statement(s, indent + 1);
                }
                let _ = writeln!(self.output, "{}}}", indent_str);
            }
            SirStatement::For { pattern, iterable, body } => {
                // Generate C-style for loop for ranges (e.g., 0..5)
                // Check if iterable is a range expression
                let range_start = "0";
                let range_end = match iterable {
                    SirExpr::BinOp { left: _, op: SirBinOp::Range, right } => {
                        self.expr_to_c(right)
                    }
                    _ => {
                        // For non-range iterables, just use the expression
                        self.expr_to_c(iterable)
                    }
                };
                
                // Check if it's actually a range
                let is_range = matches!(iterable, SirExpr::BinOp { op: SirBinOp::Range, .. });
                
                if is_range {
                    let _ = writeln!(self.output, "{}for (int {} = {}; {} < {}; {}++) {{", 
                        indent_str, pattern, range_start, pattern, range_end, pattern);
                    for s in body {
                        self.write_statement(s, indent + 1);
                    }
                    let _ = writeln!(self.output, "{}}}", indent_str);
                } else {
                    // For arrays or other iterables (placeholder)
                    let _ = writeln!(self.output, "{}// Error: Array/collection iteration not yet implemented", indent_str);
                }
            }
            SirStatement::Block(stmts) => {
                let _ = writeln!(self.output, "{{");
                for s in stmts {
                    self.write_statement(s, indent + 1);
                }
                let _ = writeln!(self.output, "{}}}", indent_str);
            }
        }
    }

    fn expr_to_c(&self, expr: &SirExpr) -> String {
        match expr {
            SirExpr::Literal(lit) => match lit {
                SirLiteral::Int(i) => format!("{}", i),
                SirLiteral::Float(f) => format!("{}", f),
                SirLiteral::Bool(b) => if *b { "1" } else { "0" }.to_string(),
                SirLiteral::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
                SirLiteral::Char(c) => format!("'{}'", c),
                SirLiteral::Unit => "".to_string(),
            },
            SirExpr::Var(name) => name.clone(),
            SirExpr::BinOp { left, op, right } => {
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
                    SirBinOp::Range => "..",  // Range operator (shouldn't appear in final C code)
                };
                format!("({} {} {})", self.expr_to_c(left), op_str, self.expr_to_c(right))
            }
            SirExpr::UnaryOp { op, operand } => {
                let op_str = match op {
                    SirUnOp::Neg => "-",
                    SirUnOp::Not => "!",
                };
                format!("({}{})", op_str, self.expr_to_c(operand))
            }
            SirExpr::Call { callee, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.expr_to_c(a)).collect();
                format!("{}({})", callee, args_str.join(", "))
            }
            SirExpr::Assign { target, value } => {
                format!("{} = {}", target, self.expr_to_c(value))
            }
            SirExpr::If { condition, then_expr, else_expr } => {
                match else_expr {
                    Some(else_e) => format!(
                        "({} ? {} : {})",
                        self.expr_to_c(condition),
                        self.expr_to_c(then_expr),
                        self.expr_to_c(else_e)
                    ),
                    None => format!("({} ? {} : 0)", self.expr_to_c(condition), self.expr_to_c(then_expr)),
                }
            }
            SirExpr::Block(_, _) => {
                "// block expr".to_string()
            }
        }
    }

    fn infer_format_specifier(&self, expr: &SirExpr) -> &'static str {
        match expr {
            SirExpr::Literal(lit) => match lit {
                SirLiteral::Int(_) => "%d",
                SirLiteral::Float(_) => "%f",
                SirLiteral::Bool(_) => "%d",
                SirLiteral::String(_) => "%s",
                SirLiteral::Char(_) => "%c",
                SirLiteral::Unit => "",
            },
            SirExpr::Var(_) => "%d", // Default to int for variables
            SirExpr::BinOp { op, left, right } => {
                match op {
                    SirBinOp::Add | SirBinOp::Sub | SirBinOp::Mul | SirBinOp::Div | SirBinOp::Mod => {
                        // Arithmetic ops: check if either operand is float
                        if matches!(left.as_ref(), SirExpr::Literal(SirLiteral::Float(_))) ||
                           matches!(right.as_ref(), SirExpr::Literal(SirLiteral::Float(_))) {
                            "%f"
                        } else {
                            "%d"
                        }
                    }
                    SirBinOp::Eq | SirBinOp::Ne | SirBinOp::Lt | SirBinOp::Le | SirBinOp::Gt | SirBinOp::Ge => "%d",
                    SirBinOp::And | SirBinOp::Or => "%d",
                    SirBinOp::Range => "%d",
                }
            }
            SirExpr::UnaryOp { op: _, operand } => self.infer_format_specifier(operand),
            SirExpr::Call { callee, args: _ } => {
                // Built-in functions return types
                match callee.as_str() {
                    "len" | "size" => "%d",
                    _ => "%d", // Default
                }
            }
            SirExpr::Assign { value, .. } => self.infer_format_specifier(value),
            SirExpr::If { then_expr, else_expr, .. } => {
                // Use then branch type, or default
                self.infer_format_specifier(then_expr)
            }
            _ => "%d", // Default to int
        }
    }
}
