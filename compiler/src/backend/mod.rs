//! C Backend - Generates C code from SIR

use crate::sir::*;
use std::fmt::Write;

/// C code generator backend
pub struct CBackend {
    output: String,
    /// Names of the program's struct types, so a struct-typed value is emitted
    /// with the struct's own C type instead of the unknown-type fallback.
    struct_names: Vec<String>,
    /// Structs that are really tagged enums (they have a `tag` field). Generic
    /// enums share one C type; generic structs are monomorphised per arguments.
    tagged: Vec<String>,
}

impl CBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            struct_names: Vec::new(),
            tagged: Vec::new(),
        }
    }

    /// Generate C code from a SIR program
    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        self.struct_names = program.structs.iter().map(|s| s.name.clone()).collect();
        self.tagged = program
            .structs
            .iter()
            .filter(|s| s.fields.iter().any(|field| field.name == "tag"))
            .map(|s| s.name.clone())
            .collect();
        
        // Generated code talks to the NOVA runtime shim, not to libc directly.
        let _ = writeln!(self.output, "#include \"nova_runtime.h\"");
        let _ = writeln!(self.output);

        // Struct types first: functions refer to them by name.
        for struct_def in &program.structs {
            let _ = writeln!(self.output, "typedef struct {} {{", struct_def.name);
            for field in &struct_def.fields {
                let _ = writeln!(self.output, "    {} {};", self.c_type(&field.typ), field.name);
            }
            let _ = writeln!(self.output, "}} {};", struct_def.name);
            let _ = writeln!(self.output);
        }
        
        // Generate forward declarations for functions
        for func in &program.functions {
            self.write_function_decl(func);
        }
        let _ = writeln!(self.output);
        
        // Generate global variables
        for global in &program.globals {
            let typ_str = self.c_type(&global.typ);
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
        // The entry function is emitted with the C entry-point signature below,
        // so it gets no separate prototype.
        if Self::is_entry(&func.name) {
            return;
        }
        let return_type = self.c_type(func.return_type.as_deref().unwrap_or("void"));
        let _ = write!(self.output, "{} {}(", return_type, func.name);

        // A lifted closure receives its environment before its own parameters.
        let mut written = 0;
        if func.env_type.is_some() {
            let _ = write!(self.output, "void *__env");
            written += 1;
        }
        for param in func.params.iter() {
            if written > 0 {
                let _ = write!(self.output, ", ");
            }
            let _ = write!(self.output, "{} {}", self.c_type(&param.typ), param.name);
            written += 1;
        }
        
        let _ = writeln!(self.output, ");");
    }

    fn write_function(&mut self, func: &SirFunction) {
        if Self::is_entry(&func.name) {
            // C requires `int main`, whatever NOVA declared. The value the NOVA
            // program returns (if any) is returned from here.
            let _ = writeln!(self.output, "int main(void) {{");
        } else {
            let return_type = self.c_type(func.return_type.as_deref().unwrap_or("void"));
            let _ = write!(self.output, "{} {}(", return_type, func.name);

            let mut written = 0;
            if func.env_type.is_some() {
                let _ = write!(self.output, "void *__env");
                written += 1;
            }
            for param in func.params.iter() {
                if written > 0 {
                    let _ = write!(self.output, ", ");
                }
                let _ = write!(self.output, "{} {}", self.c_type(&param.typ), param.name);
                written += 1;
            }

            let _ = writeln!(self.output, ") {{");
        }
        
        // Generate function body
        for stmt in &func.body {
            self.write_statement(stmt, 1);
        }

        // A `main` that falls off the end must still return an exit status.
        if Self::is_entry(&func.name)
            && !matches!(func.body.last(), Some(SirStatement::Return(_)))
        {
            let _ = writeln!(self.output, "    return 0;");
        }
        
        let _ = writeln!(self.output, "}}");
        let _ = writeln!(self.output);
    }

    /// True for the function that becomes C's `main`: an explicit `fn main`, or
    /// the synthetic `__main` the parser builds from top-level statements.
    fn is_entry(name: &str) -> bool {
        name == "main" || name == "__main"
    }

    /// The C spelling of a NOVA type: a struct's own name when it is one of the
    /// program's structs, otherwise a scalar C type (unknown names become `int`).
    fn c_type(&self, typ: &str) -> String {
        // Function-typed values are closure pointers, not C function pointers.
        if typ.trim_start().starts_with("fn(") {
            return "NovaClosure*".to_string();
        }
        if typ.trim() == "void*" {
            return "void*".to_string();
        }
        // A tuple type is stored as a generated struct named after its elements.
        if let Some(elements) = tuple_element_types(typ) {
            if !elements.is_empty() {
                return tuple_struct_name(&elements);
            }
        }
        // A generic struct is monomorphised: `Box<int>` -> `Box_int`. A generic
        // enum uses one tagged struct: `Result<int, String>` -> `Result`.
        if let Some((base, arguments)) = typ.split_once('<') {
            if self.struct_names.iter().any(|name| name == base) {
                if self.tagged.iter().any(|name| name == base) {
                    return base.to_string();
                }
                let mangled: Vec<String> = arguments
                    .trim_end_matches('>')
                    .split(',')
                    .map(|argument| Self::mangle_type(argument.trim()))
                    .collect();
                return format!("{}_{}", base, mangled.join("_"));
            }
        }
        // Arrays carry their length, so indexing can be bounds-checked wherever
        // the array is passed. `Array` is the example type; `[T]` is the slice
        // spelling the parser accepts. A `&`/`&mut` array is a pointer to the
        // header, so a push inside the callee is visible to the caller.
        let by_reference = typ.trim_start().starts_with('&');
        let base = typ
            .trim_start_matches('&')
            .trim_start_matches("mut ")
            .trim_start();
        if base.contains('[') || base.eq_ignore_ascii_case("array") {
            return if by_reference {
                "NovaArray*".to_string()
            } else {
                "NovaArray".to_string()
            };
        }
        // A method's `self` is a pointer to the struct.
        if let Some(base) = typ.strip_suffix('*') {
            let base = base.trim();
            if self.struct_names.iter().any(|name| name == base) {
                return format!("{}*", base);
            }
        }
        if self.struct_names.iter().any(|name| name == typ) {
            return typ.to_string();
        }
        Self::type_to_c_str(typ).to_string()
    }

    /// True when the expression is the `self` of a lowered method, which C sees
    /// as a pointer.
    fn is_self(expr: &SirExpr) -> bool {
        match expr {
            SirExpr::Var(name) => name == "self",
            SirExpr::Typed { expr, .. } => Self::is_self(expr),
            _ => false,
        }
    }

    fn mangle_type(typ: &str) -> String {
        typ.chars()
            .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }

    fn type_to_c_str(typ: &str) -> &'static str {
        match typ.to_lowercase().as_str() {
            "int" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => "int",
            "float" | "f32" | "f64" => "double",
            "bool" => "int",
            "string" | "str" => "char*",
            "char" => "char",
            "void" => "void",
            _ => "int", // default to int for unknown types
        }
    }

    /// The `nova_print_*` variant for a NOVA type name, when it is one the
    /// runtime knows. Unknown types (arrays, structs) return `None` so the
    /// caller can fall back to inferring from the expression.
    fn print_kind_for_type(typ: &str) -> Option<&'static str> {
        match typ.to_lowercase().as_str() {
            "bool" => Some("bool"),
            "float" | "f32" | "f64" => Some("float"),
            "char" => Some("char"),
            "string" | "str" => Some("str"),
            "int" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                Some("int")
            }
            _ => None,
        }
    }

    fn write_statement(&mut self, stmt: &SirStatement, indent: usize) {
        let indent_str = "    ".repeat(indent);
        
        match stmt {
            SirStatement::Let { name, typ, value } => {
                let typ_str = typ
                    .as_ref()
                    .map(|t| self.c_type(t))
                    .unwrap_or_else(|| "int".to_string());
                let _ = write!(self.output, "{}{} {} = ", indent_str, typ_str, name);
                let _ = writeln!(self.output, "{};", self.expr_to_c(value));
            }
            SirStatement::Expr(expr) => {
                // `print` and `println` are the language's only I/O; both write a line
                // through the runtime, so generated C needs no stdio of its own.
                if let SirExpr::Call { callee, args } = expr {
                    if (callee == "print" || callee == "println") && !args.is_empty() {
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                let _ =
                                    writeln!(self.output, "{}nova_print_str(\" \");", indent_str);
                            }
                            let _ = writeln!(
                                self.output,
                                "{}nova_print_{}({});",
                                indent_str,
                                self.print_kind(arg),
                                self.expr_to_c(arg)
                            );
                        }
                        let _ = writeln!(self.output, "{}nova_print_newline();", indent_str);
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
            SirStatement::Switch { scrutinee, cases, default } => {
                let _ = writeln!(
                    self.output,
                    "{}switch ({}) {{",
                    indent_str,
                    self.expr_to_c(scrutinee)
                );
                for (constant, body) in cases {
                    let _ = writeln!(
                        self.output,
                        "{}case {}:",
                        indent_str,
                        self.expr_to_c(constant)
                    );
                    for statement in body {
                        self.write_statement(statement, indent + 1);
                    }
                    let _ = writeln!(self.output, "{}    break;", indent_str);
                }
                if let Some(body) = default {
                    let _ = writeln!(self.output, "{}default:", indent_str);
                    for statement in body {
                        self.write_statement(statement, indent + 1);
                    }
                    let _ = writeln!(self.output, "{}    break;", indent_str);
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
                // `for i in a..b` (a SIR range) and `for i in range(a, b)` both
                // become one C loop; the start bound is the real one, not 0.
                match self.range_bounds(iterable) {
                    Some((start, end)) => {
                        let _ = writeln!(
                            self.output,
                            "{}for (int {} = {}; {} < {}; {}++) {{",
                            indent_str, pattern, start, pattern, end, pattern
                        );
                        for s in body {
                            self.write_statement(s, indent + 1);
                        }
                        let _ = writeln!(self.output, "{}}}", indent_str);
                    }
                    None => {
                        let _ = writeln!(
                            self.output,
                            "{}// unsupported for-loop iterable: only ranges are implemented",
                            indent_str
                        );
                    }
                }
            }
            SirStatement::Block(stmts) => {
                let _ = writeln!(self.output, "{{");
                for s in stmts {
                    self.write_statement(s, indent + 1);
                }
                let _ = writeln!(self.output, "{}}}", indent_str);
            }
            SirStatement::Break => {
                let _ = writeln!(self.output, "{}break;", indent_str);
            }
            SirStatement::Continue => {
                let _ = writeln!(self.output, "{}continue;", indent_str);
            }
        }
    }

    /// Lower a range iterable to its C `(start, end)` bounds, or `None` when the
    /// iterable is not a range this backend can emit.
    fn range_bounds(&self, iterable: &SirExpr) -> Option<(String, String)> {
        match iterable {
            SirExpr::BinOp { left, op: SirBinOp::Range, right } => {
                Some((self.expr_to_c(left), self.expr_to_c(right)))
            }
            SirExpr::Call { callee, args } if callee == "range" => match args.len() {
                1 => Some(("0".to_string(), self.expr_to_c(&args[0]))),
                2.. => Some((self.expr_to_c(&args[0]), self.expr_to_c(&args[1]))),
                _ => None,
            },
            _ => None,
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
            SirExpr::FieldAssign { object, field, value } => {
                if Self::is_self(object) {
                    format!("(self)->{} = {}", field, self.expr_to_c(value))
                } else {
                    format!(
                        "({}).{} = {}",
                        self.expr_to_c(object),
                        field,
                        self.expr_to_c(value)
                    )
                }
            }
            SirExpr::AddressOf(inner) => format!("&({})", self.expr_to_c(inner)),
            SirExpr::Deref(inner) => format!("(*{})", self.expr_to_c(inner)),
            SirExpr::Array(items) => {
                if items.is_empty() {
                    return "nova_array_from(0, 0)".to_string();
                }
                let values: Vec<String> = items.iter().map(|item| self.expr_to_c(item)).collect();
                // Heap-allocated, so a later push can grow it safely.
                format!(
                    "nova_array_from((int[]){{ {} }}, {})",
                    values.join(", "),
                    items.len()
                )
            }
            SirExpr::StringSlice { value, start, end } => format!(
                "nova_substr({}, {}, ({} - {}))",
                self.expr_to_c(value),
                self.expr_to_c(start),
                self.expr_to_c(end),
                self.expr_to_c(start)
            ),
            SirExpr::Index { array, index } => format!(
                "nova_array_get({}, {})",
                self.expr_to_c(array),
                self.expr_to_c(index)
            ),
            SirExpr::IndexAssign { array, index, value } => format!(
                "nova_array_set({}, {}, {})",
                self.expr_to_c(array),
                self.expr_to_c(index),
                self.expr_to_c(value)
            ),
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
            // The type annotation is codegen metadata, not a value.
            SirExpr::Typed { expr, .. } => self.expr_to_c(expr),
            SirExpr::Closure { name, env } => {
                let env = env
                    .as_ref()
                    .map(|env| self.expr_to_c(env))
                    .unwrap_or_else(|| "(void*)0".to_string());
                format!("nova_closure_new((void*)&{}, {})", name, env)
            }
            SirExpr::EnvAccess { env_type, field } => {
                format!("(({}*)__env)->{}", env_type, field)
            }
            SirExpr::Cast { value, typ } => {
                format!("(({})({}))", self.c_type(typ), self.expr_to_c(value))
            }
            SirExpr::SizeOf(typ) => format!("sizeof({})", self.c_type(typ)),
            SirExpr::CallValue { callee, args, return_type, params } => {
                let mut signature = String::from("void*");
                for param in params {
                    signature.push_str(", ");
                    signature.push_str(&self.c_type(param));
                }
                let mut values = vec![format!(
                    "(void*)nova_closure_env({})",
                    self.expr_to_c(callee)
                )];
                values.extend(args.iter().map(|arg| self.expr_to_c(arg)));
                format!(
                    "(({} (*)({}))nova_closure_fn({}))({})",
                    self.c_type(return_type),
                    signature,
                    self.expr_to_c(callee),
                    values.join(", ")
                )
            }
            SirExpr::Tuple { name, items } => {
                let initialisers: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| format!("._{} = {}", index, self.expr_to_c(item)))
                    .collect();
                format!("({}){{ {} }}", name, initialisers.join(", "))
            }
            SirExpr::StructLiteral { name, fields } => {
                let initialisers: Vec<String> = fields
                    .iter()
                    .map(|(field, value)| format!(".{} = {}", field, self.expr_to_c(value)))
                    .collect();
                format!("({}){{ {} }}", name, initialisers.join(", "))
            }
            SirExpr::FieldAccess { object, field } => {
                if Self::is_self(object) {
                    format!("(self)->{}", field)
                } else {
                    format!("({}).{}", self.expr_to_c(object), field)
                }
            }
            SirExpr::Block(_, _) => {
                "// block expr".to_string()
            }
        }
    }

    /// Which `nova_print_*` runtime function takes this value.
    ///
    /// SIR does not carry static types, so this infers the way the old format
    /// specifiers did: literals are exact, arithmetic follows its operands, and
    /// comparisons and `!` are booleans.
    fn print_kind(&self, expr: &SirExpr) -> &'static str {
        match expr {
            SirExpr::Literal(lit) => match lit {
                SirLiteral::Int(_) => "int",
                SirLiteral::Float(_) => "float",
                SirLiteral::Bool(_) => "bool",
                SirLiteral::String(_) => "str",
                SirLiteral::Char(_) => "char",
                SirLiteral::Unit => "int",
            },
            // No type information on variables yet; int is the only usable guess.
            SirExpr::Var(_) => "int",
            SirExpr::BinOp { op, left, right } => match op {
                SirBinOp::Add | SirBinOp::Sub | SirBinOp::Mul | SirBinOp::Div | SirBinOp::Mod => {
                    if self.print_kind(left) == "float" || self.print_kind(right) == "float" {
                        "float"
                    } else {
                        "int"
                    }
                }
                SirBinOp::Eq
                | SirBinOp::Ne
                | SirBinOp::Lt
                | SirBinOp::Le
                | SirBinOp::Gt
                | SirBinOp::Ge
                | SirBinOp::And
                | SirBinOp::Or => "bool",
                SirBinOp::Range => "int",
            },
            SirExpr::UnaryOp { op, operand } => match op {
                SirUnOp::Not => "bool",
                SirUnOp::Neg => self.print_kind(operand),
            },
            SirExpr::Call { callee, .. } => match callee.as_str() {
                "len" | "size" => "int",
                _ => "int",
            },
            SirExpr::Assign { value, .. } => self.print_kind(value),
            SirExpr::FieldAssign { value, .. } => self.print_kind(value),
            SirExpr::AddressOf(_) | SirExpr::Deref(_) => "int",
            SirExpr::Array(_) | SirExpr::Index { .. } | SirExpr::Tuple { .. } => "int",
            SirExpr::Closure { .. } | SirExpr::EnvAccess { .. } | SirExpr::CallValue { .. }
            | SirExpr::Cast { .. } | SirExpr::SizeOf(_) => "int",
            SirExpr::StringSlice { .. } => "str",
            SirExpr::IndexAssign { value, .. } => self.print_kind(value),
            SirExpr::If { then_expr, .. } => self.print_kind(then_expr),
            SirExpr::Typed { expr, typ } => Self::print_kind_for_type(typ)
                .unwrap_or_else(|| self.print_kind(expr)),
            // Printing a whole struct or an untyped field is not supported yet.
            SirExpr::StructLiteral { .. } => "int",
            SirExpr::FieldAccess { .. } => "int",
            SirExpr::Block(_, _) => "int",
        }
    }
}
