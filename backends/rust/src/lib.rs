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
    /// Struct name -> fields, used for generated closure environments.
    structs: std::collections::HashMap<String, Vec<(String, String)>>,
    /// Declared return type of the function being written, used when a
    /// unit-valued builtin is returned from an `int`-inferred closure.
    current_return_type: String,
}

impl RustBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            signatures: std::collections::HashMap::new(),
            structs: std::collections::HashMap::new(),
            current_return_type: "void".to_string(),
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
        
        self.structs = program
            .structs
            .iter()
            .map(|structure| {
                (
                    structure.name.clone(),
                    structure
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.typ.clone()))
                        .collect(),
                )
            })
            .collect();

        // Add standard imports
        self.writeln("use std::io;");
        self.writeln("use std::rc::Rc;");
        self.writeln("");
        // `print` writes a value and a newline, matching the C runtime.
        self.writeln("fn print<T: std::fmt::Display>(value: T) {");
        self.writeln("    println!(\"{}\", value);");
        self.writeln("}");
        // NOVA's `println` is the same newline-terminated print at this level.
        self.writeln("fn println<T: std::fmt::Display>(value: T) {");
        self.writeln("    print(value);");
        self.writeln("}");
        // Arrays are `Vec<i32>`; these mirror the C runtime helpers.
        self.writeln("fn nova_array_len(array: &Vec<i32>) -> i32 { array.len() as i32 }");
        self.writeln(
            "fn nova_array_push(array: &mut Vec<i32>, value: i32) { array.push(value); }",
        );
        self.writeln("");
        
        // Closure environments and user structs are plain Rust structs. Values
        // are moved into closures, so no derives are needed.
        for structure in &program.structs {
            let fields: Vec<String> = structure
                .fields
                .iter()
                .map(|field| format!("    {}: {},", field.name, self.rust_type(&field.typ)))
                .collect();
            self.writeln(&format!("struct {} {{", structure.name));
            self.output.push_str(&fields.join("\n"));
            self.output.push('\n');
            self.writeln("}");
            self.writeln("");
        }

        // Generate globals as static mut (not ideal but works for simple cases)
        for global in &program.globals {
            self.generate_global(global);
        }
        
        if !program.globals.is_empty() {
            self.writeln("");
        }
        
        // Emit only functions reachable from the entry point: imported but
        // unused stdlib helpers may use constructs this backend cannot express.
        let reachable = nova_common::sir::reachable_functions(program);
        for func in &program.functions {
            if reachable
                .as_ref()
                .is_some_and(|names| !names.contains(&func.name))
            {
                continue;
            }
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
        // Environment constructors return the struct directly; the C ABI's
        // `void*` copy step has no counterpart here.
        if let Some(struct_name) = func.name.strip_prefix("__env_").map(|id| format!("__Env_{id}")) {
            if let Some(fields) = self.structs.get(&struct_name).cloned() {
                let params: Vec<String> = func
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, self.rust_param_type(&p.typ)))
                    .collect();
                let values: Vec<String> = fields
                    .iter()
                    .map(|(name, _)| format!("{}: {}", name, name))
                    .collect();
                self.writeln(&format!(
                    "fn {}({}) -> {} {{",
                    func.name,
                    params.join(", "),
                    struct_name
                ));
                self.indent_level += 1;
                self.writeln(&format!("{} {{ {} }}", struct_name, values.join(", ")));
                self.indent_level -= 1;
                self.writeln("}");
                return;
            }
        }

        let mut params: Vec<String> = Vec::new();
        if let Some(env_type) = &func.env_type {
            if self.structs.contains_key(env_type) {
                params.push(format!("__env: &{}", env_type));
            }
        }
        params.extend(
            func.params
                .iter()
                .map(|p| format!("{}: {}", p.name, self.rust_param_type(&p.typ))),
        );
        
        self.current_return_type = func
            .return_type
            .clone()
            .unwrap_or_else(|| "void".to_string());
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
                // A tuple literal carries its generated struct name even when
                // the binding's inferred NOVA type is not recorded.
                let literal = match value {
                    SirExpr::Typed { expr, .. } => expr.as_ref(),
                    other => other,
                };
                let rust_type = if let SirExpr::Tuple { name, .. } = literal {
                    name.clone()
                } else if let Some(t) = typ {
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
                // `print` is unit-valued, but an expression-bodied closure with a
                // `print` body is inferred as returning `int`.
                if let Some(SirExpr::Call { callee, args }) = expr.as_ref() {
                    if callee == "print" || callee == "println" {
                        let values: Vec<String> =
                            args.iter().map(|arg| self.expr_to_rust(arg)).collect();
                        self.writeln(&format!("{}({});", callee, values.join(", ")));
                        if self.current_return_type == "void" {
                            self.writeln("return;");
                        } else {
                            let value = self.default_value_for_type(&self.current_return_type);
                            self.writeln(&format!("return {value};"));
                        }
                        return;
                    }
                }
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
            SirExpr::Typed { expr, typ } => {
                if let SirExpr::Closure { name, env } = expr.as_ref() {
                    self.closure_expr(name, env.as_deref(), typ)
                } else {
                    self.expr_to_rust(expr)
                }
            }
            SirExpr::Closure { name, .. } => format!("Rc::new({name})"),
            SirExpr::EnvAccess { field, .. } => format!("__env.{}", field),
            SirExpr::SizeOf(_) => "0".to_string(),
            SirExpr::Cast { value, typ } => {
                format!("({} as {})", self.expr_to_rust(value), self.type_to_rust(typ))
            }
            SirExpr::CallValue { callee, args, .. } => {
                let values: Vec<String> = args.iter().map(|arg| self.expr_to_rust(arg)).collect();
                format!("(*{})({})", self.expr_to_rust(callee), values.join(", "))
            }
            SirExpr::Tuple { name, items } => {
                let fields: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| format!("_{index}: {}", self.expr_to_rust(item)))
                    .collect();
                format!("{} {{ {} }}", name, fields.join(", "))
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
            SirExpr::AddressOf(inner) => format!("&mut ({})", self.expr_to_rust(inner)),
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

    /// Split a closure environment expression into its constructor and the
    /// captured variables it is given.
    fn environment_constructor(&self, env: &SirExpr) -> Option<(String, Vec<String>)> {
        let expr = match env {
            SirExpr::Typed { expr, .. } => expr.as_ref(),
            other => other,
        };
        match expr {
            SirExpr::Call { callee, args } if callee.starts_with("__env_") => Some((
                callee.clone(),
                args.iter().map(|arg| self.expr_to_rust(arg)).collect(),
            )),
            _ => None,
        }
    }

    /// The Rust spelling of a NOVA type; arrays become `Vec<i32>` and function
    /// types become `Rc<dyn Fn(..) -> ..>`.
    fn rust_type(&self, typ: &str) -> String {
        let base = typ.trim();
        if base.contains('[') || base.eq_ignore_ascii_case("array") {
            return "Vec<i32>".to_string();
        }
        if base.starts_with("fn(") {
            let (params, ret) = parse_function_type(base);
            let params: Vec<String> = params
                .iter()
                .map(|param| self.rust_param_type(param))
                .collect();
            if is_void_type(&ret) {
                return format!("Rc<dyn Fn({})>", params.join(", "));
            }
            return format!(
                "Rc<dyn Fn({}) -> {}>",
                params.join(", "),
                self.rust_type(&ret)
            );
        }
        if self.structs.contains_key(base) {
            return base.to_string();
        }
        self.type_to_rust(base).to_string()
    }

    /// Render a closure value as an `Rc<dyn Fn(..) -> ..>`, binding the
    /// environment (if any) into the closure.
    fn closure_expr(&self, name: &str, env: Option<&SirExpr>, signature: &str) -> String {
        let (params, _) = parse_function_type(signature);
        let declarations: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(index, typ)| format!("p{index}: {}", self.rust_param_type(typ)))
            .collect();
        let arguments: Vec<String> = (0..params.len()).map(|index| format!("p{index}")).collect();
        let env_call = env.and_then(|env| self.environment_constructor(env));
        match env_call {
            Some((constructor, captures)) => format!(
                "{{ let __env = {}({}); Rc::new(move |{}| {}(&__env, {})) }}",
                constructor,
                captures.join(", "),
                declarations.join(", "),
                name,
                arguments.join(", ")
            ),
            None if env.is_some() => {
                let environment = self.expr_to_rust(env.expect("environment"));
                format!(
                    "Rc::new(move |{}| {}(&({}), {}))",
                    declarations.join(", "),
                    name,
                    environment,
                    arguments.join(", ")
                )
            }
            None => format!(
                "Rc::new(move |{}| {}({}))",
                declarations.join(", "),
                name,
                arguments.join(", ")
            ),
        }
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

fn is_void_type(typ: &str) -> bool {
    matches!(typ.trim(), "" | "void" | "()" | "unit" | "!")
}

/// Parse `fn(A, B) -> C` into its parameter and result type names.
fn parse_function_type(signature: &str) -> (Vec<String>, String) {
    let signature = signature.trim();
    let Some(rest) = signature.strip_prefix("fn(") else {
        return (Vec::new(), "void".to_string());
    };
    let Some((params, result)) = rest.split_once(')') else {
        return (Vec::new(), "void".to_string());
    };
    let params = params
        .split(',')
        .map(str::trim)
        .filter(|param| !param.is_empty())
        .map(str::to_string)
        .collect();
    let result = result
        .trim()
        .strip_prefix("->")
        .map(str::trim)
        .unwrap_or("void");
    (params, result.to_string())
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
