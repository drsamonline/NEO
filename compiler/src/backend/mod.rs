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
            self.write_global(global);
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
        let return_type = func.return_type.as_deref().unwrap_or("void");
        let _ = write!(self.output, "{} {}(", return_type, func.name);
        
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                let _ = write!(self.output, ", ");
            }
            let _ = write!(self.output, "{} {}", param.typ, param.name);
        }
        
        let _ = writeln!(self.output, ");");
    }

    fn write_global(&mut self, global: &SirGlobal) {
        let _ = write!(self.output, "{} {}", global.typ, global.name);
        if let Some(init) = &global.init_value {
            let _ = write!(self.output, " = {}", self.expr_to_c(init));
        }
        let _ = writeln!(self.output, ";");
    }

    fn write_function(&mut self, func: &SirFunction) {
        let return_type = func.return_type.as_deref().unwrap_or("void");
        let _ = write!(self.output, "{} {}(", return_type, func.name);
        
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                let _ = write!(self.output, ", ");
            }
            let _ = write!(self.output, "{} {}", param.typ, param.name);
        }
        
        let _ = writeln!(self.output, ") {{");
        
        // Generate function body
        for stmt in &func.body {
            self.write_statement(stmt, 1);
        }
        
        let _ = writeln!(self.output, "}}");
        let _ = writeln!(self.output);
    }

    fn write_statement(&mut self, stmt: &SirStatement, indent: usize) {
        let indent_str = "    ".repeat(indent);
        
        match stmt {
            SirStatement::Let { name, typ, value } => {
                let typ_str = typ.as_deref().unwrap_or("int");
                let _ = write!(self.output, "{}{} {} = ", indent_str, typ_str, name);
                let _ = writeln!(self.output, "{};", self.expr_to_c(value));
            }
            SirStatement::Expr(expr) => {
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
            SirStatement::For { pattern, iterable: _, body: _ } => {
                // Simple for loop over range or array
                let _ = writeln!(self.output, "{}// TODO: for loop over {}", indent_str, pattern);
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
}
