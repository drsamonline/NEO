//! Rust Backend for Nova Compiler
//! 
//! This module converts SIR (Semantic Intermediate Representation) to Rust code.

use nova_common::sir::{SirProgram, SirFunction, SirGlobal, SirStatement, SirExpr, SirLiteral, SirBinOp, SirUnOp, SirParam};

pub struct RustBackend {
    output: String,
    indent_level: usize,
    /// Declared parameter types per function, so array arguments can be passed
    /// as `&mut Vec<_>` at the call site.
    signatures: std::collections::HashMap<String, Vec<String>>,
}

impl RustBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            signatures: std::collections::HashMap::new(),
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        self.signatures = program
            .functions
            .iter()
            .map(|function| {
                (
                    function.name.clone(),
                    function.params.iter().map(|param| param.typ.clone()).collect(),
                )
            })
            .collect();
        
        // Add standard imports
        self.writeln("use std::io;");
        self.writeln("");
        // `print` writes a value and a newline, matching the C runtime.
        self.writeln("fn print<T: std::fmt::Display>(value: T) {");
        self.writeln("    println!(\"{}\", value);");
        self.writeln("}");
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

        // Rust needs a real `main`; a NOVA program's top-level statements are
        // lowered into the synthetic `__main`.
        let value_main = program.functions.iter().any(|f| {
            f.name == "main"
                && f.return_type
                    .as_deref()
                    .is_some_and(|rt| !matches!(rt, "void" | "()" | "!" | "unit"))
        });
        if value_main {
            self.writeln("fn main() {");
            self.writeln("    std::process::exit(__nova_main());");
            self.writeln("}");
        } else if !program.functions.iter().any(|f| f.name == "main")
            && program.functions.iter().any(|f| f.name == "__main")
        {
            self.writeln("fn main() {");
            self.writeln("    __main();");
            self.writeln("}");
        }
        
        self.output.clone()
    }

    fn generate_global(&mut self, global: &SirGlobal) {
        let rust_type = self.rust_type(&global.typ);
        if let Some(init_expr) = &global.init_value {
            let init = self.expr_to_rust(init_expr);
            self.writeln(&format!("static mut {}: {} = {};", global.name, rust_type, init));
        } else {
            self.writeln(&format!("static mut {}: {} = {};", global.name, rust_type, self.default_value_for_type(&global.typ)));
        }
    }

    fn generate_function(&mut self, func: &SirFunction) {
        let params: Vec<String> = func.params.iter()
            .map(|p| format!("{}: {}", p.name, self.rust_param_type(&p.typ)))
            .collect();
        
        let returns_value = func
            .return_type
            .as_deref()
            .is_some_and(|rt| !matches!(rt, "void" | "()" | "!" | "unit"));
        let return_type = if let Some(ref rt) = func.return_type {
            if rt == "void" || rt.is_empty() || rt == "!" {
                String::new()
            } else {
                format!(" -> {}", self.rust_type(rt))
            }
        } else {
            String::new()
        };

        // Rust's `main` may not return a value, so a value-returning NOVA `main`
        // becomes `__nova_main` with a thin wrapper below.
        let emitted_name = if func.name == "main" && returns_value {
            "__nova_main"
        } else {
            &func.name
        };
        self.writeln(&format!("fn {}({}){} {{", emitted_name, params.join(", "), return_type));
        
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
                let rust_type = if let Some(t) = typ {
                    self.rust_type(t)
                } else {
                    "i32".to_string()
                };
                let val_str = self.expr_to_rust(value);
                // NOVA `let` bindings are reassignable; Rust needs `mut`.
                self.writeln(&format!("let mut {}: {} = {};", name, rust_type, val_str));
            }
            SirStatement::Expr(expr) => {
                let expr_str = self.expr_to_rust(expr);
                self.writeln(&format!("{};", expr_str));
            }
            SirStatement::Return(expr) => {
                let val_str = expr.as_ref().map(|e| self.expr_to_rust(e)).unwrap_or_default();
                self.writeln(&format!("return {};", val_str));
            }
            SirStatement::If { condition, then_block, else_block } => {
                let cond_str = self.expr_to_rust(condition);
                self.writeln(&format!("if {} {{", cond_str));
                
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
            SirStatement::Switch { scrutinee, cases, default } => {
                // Rust has no C-style fallthrough; emit an if/else-if chain.
                let mut fallback: Vec<SirStatement> = default.clone().unwrap_or_default();
                for (constant, body) in cases.iter().rev() {
                    fallback = vec![SirStatement::If {
                        condition: SirExpr::BinOp {
                            left: Box::new(scrutinee.clone()),
                            op: SirBinOp::Eq,
                            right: Box::new(constant.clone()),
                        },
                        then_block: body.clone(),
                        else_block: Some(fallback),
                    }];
                }
                for stmt in &fallback {
                    self.generate_statement(stmt);
                }
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
            SirStatement::Block(stmts) => {
                self.writeln("{");
                self.indent_level += 1;
                for stmt in stmts {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;
                self.writeln("}");
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
            SirExpr::Var(name) => name.clone(),
            SirExpr::BinOp { left, op, right } => {
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
            SirExpr::UnaryOp { op, operand } => {
                let operand_str = self.expr_to_rust(operand);
                let op_str = match op {
                    SirUnOp::Neg => "-",
                    SirUnOp::Not => "!",
                };
                format!("({}{})", op_str, operand_str)
            }
            SirExpr::Call { callee, args } => {
                let params = self.signatures.get(callee).cloned().unwrap_or_default();
                let args_str: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        let value = self.expr_to_rust(arg);
                        let is_array = params
                            .get(index)
                            .is_some_and(|typ| typ.contains('[') || typ.eq_ignore_ascii_case("array"));
                        if is_array {
                            format!("&mut {}", value)
                        } else {
                            value
                        }
                    })
                    .collect();
                format!("{}({})", callee, args_str.join(", "))
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
            SirExpr::Typed { expr, .. } => self.expr_to_rust(expr),
            SirExpr::Closure { name, .. } => name.clone(),
            SirExpr::EnvAccess { field, .. } => format!("__env.{}", field),
            SirExpr::SizeOf(_) => "0".to_string(),
            SirExpr::Cast { value, typ } => {
                format!("({} as {})", self.expr_to_rust(value), self.type_to_rust(typ))
            }
            SirExpr::CallValue { callee, args, .. } => {
                let values: Vec<String> = args.iter().map(|arg| self.expr_to_rust(arg)).collect();
                format!("{}({})", self.expr_to_rust(callee), values.join(", "))
            }
            SirExpr::Tuple { items, .. } => {
                let values: Vec<String> = items.iter().map(|item| self.expr_to_rust(item)).collect();
                format!("({})", values.join(", "))
            }
            SirExpr::StructLiteral { name, fields } => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(field, value)| format!("{}: {}", field, self.expr_to_rust(value)))
                    .collect();
                format!("{} {{ {} }}", name, fields.join(", "))
            }
            SirExpr::FieldAccess { object, field, .. } => {
                format!("({}).{}", self.expr_to_rust(object), field)
            }
            SirExpr::Block(stmts, final_expr) => {
                // Blocks in Rust are expressions
                let mut block_code = "{ ".to_string();
                for stmt in stmts {
                    match stmt {
                        SirStatement::Let { name, typ, value } => {
                            let rust_type = typ.as_ref().map(|t| self.type_to_rust(t)).unwrap_or("i32");
                            block_code.push_str(&format!("let {}: {} = {}; ", name, rust_type, self.expr_to_rust(value)));
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
            SirExpr::Assign { target, value } => {
                format!("{} = {}", target, self.expr_to_rust(value))
            }
            SirExpr::FieldAssign { object, field, value, .. } => {
                format!(
                    "({}).{} = {}",
                    self.expr_to_rust(object),
                    field,
                    self.expr_to_rust(value)
                )
            }
            SirExpr::Deref(inner) => format!("(*{})", self.expr_to_rust(inner)),
            SirExpr::AddressOf(inner) => format!("&({})", self.expr_to_rust(inner)),
            SirExpr::Array(items) => {
                let values: Vec<String> = items.iter().map(|item| self.expr_to_rust(item)).collect();
                format!("vec![{}]", values.join(", "))
            }
            SirExpr::StringSlice { value, start, end } => format!(
                "{}[{}..{}].to_string()",
                self.expr_to_rust(value),
                self.expr_to_rust(start),
                self.expr_to_rust(end)
            ),
            SirExpr::Index { array, index } => format!(
                "({})[({}) as usize]",
                self.expr_to_rust(array),
                self.expr_to_rust(index)
            ),
            SirExpr::IndexAssign { array, index, value } => format!(
                "({})[({}) as usize] = {}",
                self.expr_to_rust(array),
                self.expr_to_rust(index),
                self.expr_to_rust(value)
            ),
        }
    }

    /// An array parameter is borrowed so callers keep ownership.
    fn rust_param_type(&self, typ: &str) -> String {
        if typ.trim().contains('[') || typ.trim().eq_ignore_ascii_case("array") {
            return "&mut Vec<i32>".to_string();
        }
        self.rust_type(typ)
    }

    /// The Rust spelling of a NOVA type; arrays become `Vec<i32>`.
    fn rust_type(&self, typ: &str) -> String {
        let base = typ.trim();
        if base.contains('[') || base.eq_ignore_ascii_case("array") {
            return "Vec<i32>".to_string();
        }
        self.type_to_rust(base).to_string()
    }

    fn type_to_rust(&self, typ: &str) -> &'static str {
        match typ {
            "int" | "i32" => "i32",
            "i64" => "i64",
            "u32" => "u32",
            "u64" => "u64",
            "float" | "f32" | "f64" => "f64",
            "bool" => "bool",
            // Literals are `&str`; a `String` binding would reject them.
            "string" | "String" => "&'static str",
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
    use nova_common::sir::*;
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
                env_type: None,
            }],
            globals: vec![],
            structs: vec![],
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
                env_type: None,
            }],
            globals: vec![],
            structs: vec![],
        };

        let mut backend = RustBackend::new();
        let rust_code = backend.generate(&program);

        assert!(rust_code.contains("for i in 0..5"));
    }
}
