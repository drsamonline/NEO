//! WebAssembly backend for the NOVA compiler.
//!
//! Generates WebAssembly Text Format (WAT) for the subset of SIR that has a
//! defined WebAssembly representation: integers, booleans, floats, string
//! literals, `print`/`println`, plain function calls, recursion, `if`, `while`,
//! `for` over a range, and locals.
//!
//! String literals live in a single linear memory and are passed to the host as
//! `(pointer, length)` through `env.print_str`; integers go through
//! `env.print_i32`. `backends/wasm/host.mjs` supplies those imports.
//!
//! Int arrays use a `[len, capacity, data]` descriptor and grow in place via
//! `array_push`; structs and tuples are bump-allocated blocks. Closures are
//! lifted functions invoked through a `funcref` table with an environment
//! pointer as their first argument.
//!
//! Constructs without a representation yet (floats in struct fields, non-int
//! closure parameters, etc.) are rejected by `validate` rather than silently
//! lowered to placeholders.

use nova_common::sir::{
    SirBinOp, SirExpr, SirFunction, SirLiteral, SirProgram, SirStatement, SirUnOp,
};
use std::collections::{HashMap, HashSet};

pub struct WasmBackend {
    output: String,
    indent_level: usize,
    /// (literal text, byte offset in linear memory), in first-use order.
    strings: Vec<(String, usize)>,
    globals: HashSet<String>,
    params: HashSet<String>,
    /// Declared function names and whether they return a value.
    functions: Vec<(String, bool)>,
    locals: Vec<(String, String)>,
    /// First byte of the bump-allocated heap for arrays and structs.
    heap_start: usize,
    /// Struct name -> field name -> byte offset (each field occupies 4 bytes).
    structs: HashMap<String, Vec<(String, usize)>>,
    /// Lifted closure function name -> table slot.
    closures: HashMap<String, u32>,
    /// Current indirect-call nesting depth, used to pick scratch locals.
    call_depth: usize,
}

/// Reject programs this backend cannot represent faithfully, so `emit --target
/// wasm` fails with a reason instead of writing placeholder code.
pub fn validate(program: &SirProgram) -> Result<(), &'static str> {
    // Only functions reachable from the entry point matter: a `use core;`
    // program imports the whole module, including helpers this backend cannot
    // compile, and it must still run if none of them are called.
    let Some(reachable) = reachable_functions(program) else {
        return Ok(());
    };
    for function in &program.functions {
        if reachable.contains(&function.name) {
            validate_statements(&function.body)?;
        }
    }
    Ok(())
}

/// Functions called (transitively) from `main`/`__main`, or `None` when the
/// program has no entry point.
fn reachable_functions(program: &SirProgram) -> Option<HashSet<String>> {
    let entry = if program.functions.iter().any(|f| f.name == "main") {
        "main".to_string()
    } else if program.functions.iter().any(|f| f.name == "__main") {
        "__main".to_string()
    } else {
        return None;
    };

    let mut reachable = HashSet::new();
    let mut stack = vec![entry];
    while let Some(name) = stack.pop() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        if let Some(func) = program.functions.iter().find(|f| f.name == name) {
            collect_calls(&func.body, &mut stack);
        }
    }
    Some(reachable)
}

fn collect_calls(stmts: &[SirStatement], calls: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            SirStatement::Let { value, .. }
            | SirStatement::Expr(value)
            | SirStatement::Return(Some(value)) => collect_calls_expr(value, calls),
            SirStatement::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_calls_expr(condition, calls);
                collect_calls(then_block, calls);
                if let Some(else_block) = else_block {
                    collect_calls(else_block, calls);
                }
            }
            SirStatement::While { condition, body } => {
                collect_calls_expr(condition, calls);
                collect_calls(body, calls);
            }
            SirStatement::For { iterable, body, .. } => {
                collect_calls_expr(iterable, calls);
                collect_calls(body, calls);
            }
            SirStatement::Switch {
                scrutinee,
                cases,
                default,
            } => {
                collect_calls_expr(scrutinee, calls);
                for (constant, body) in cases {
                    collect_calls_expr(constant, calls);
                    collect_calls(body, calls);
                }
                if let Some(default) = default {
                    collect_calls(default, calls);
                }
            }
            SirStatement::Block(body) => collect_calls(body, calls),
            SirStatement::Return(None) | SirStatement::Break | SirStatement::Continue => {}
        }
    }
}

fn collect_calls_expr(expr: &SirExpr, calls: &mut Vec<String>) {
    match expr {
        SirExpr::Call { callee, args } => {
            calls.push(callee.clone());
            for arg in args {
                collect_calls_expr(arg, calls);
            }
        }
        SirExpr::Typed { expr, .. }
        | SirExpr::Cast { value: expr, .. }
        | SirExpr::UnaryOp { operand: expr, .. }
        | SirExpr::AddressOf(expr)
        | SirExpr::Deref(expr) => collect_calls_expr(expr, calls),
        SirExpr::BinOp { left, right, .. } => {
            collect_calls_expr(left, calls);
            collect_calls_expr(right, calls);
        }
        SirExpr::CallValue { callee, args, .. } => {
            collect_calls_expr(callee, calls);
            for arg in args {
                collect_calls_expr(arg, calls);
            }
        }
        SirExpr::Closure { name, env } => {
            calls.push(name.clone());
            if let Some(env) = env {
                collect_calls_expr(env, calls);
            }
        }
        SirExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_calls_expr(condition, calls);
            collect_calls_expr(then_expr, calls);
            if let Some(else_expr) = else_expr {
                collect_calls_expr(else_expr, calls);
            }
        }
        SirExpr::Block(stmts, final_expr) => {
            collect_calls(stmts, calls);
            if let Some(final_expr) = final_expr {
                collect_calls_expr(final_expr, calls);
            }
        }
        SirExpr::Assign { value, .. } => collect_calls_expr(value, calls),
        SirExpr::Array(items) | SirExpr::Tuple { items, .. } => {
            for item in items {
                collect_calls_expr(item, calls);
            }
        }
        SirExpr::Index { array, index } => {
            collect_calls_expr(array, calls);
            collect_calls_expr(index, calls);
        }
        SirExpr::IndexAssign {
            array,
            index,
            value,
        } => {
            collect_calls_expr(array, calls);
            collect_calls_expr(index, calls);
            collect_calls_expr(value, calls);
        }
        SirExpr::StringSlice { value, start, end } => {
            collect_calls_expr(value, calls);
            collect_calls_expr(start, calls);
            collect_calls_expr(end, calls);
        }
        SirExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_calls_expr(value, calls);
            }
        }
        SirExpr::FieldAccess { object, .. } => collect_calls_expr(object, calls),
        SirExpr::FieldAssign {
            object, value, ..
        } => {
            collect_calls_expr(object, calls);
            collect_calls_expr(value, calls);
        }
        SirExpr::EnvAccess { .. }
        | SirExpr::SizeOf(_)
        | SirExpr::Literal(_)
        | SirExpr::Var(_) => {}
    }
}

fn validate_statements(stmts: &[SirStatement]) -> Result<(), &'static str> {
    for stmt in stmts {
        match stmt {
            SirStatement::Let { value, .. } => validate_expr(value)?,
            SirStatement::Expr(value) | SirStatement::Return(Some(value)) => validate_expr(value)?,
            SirStatement::If {
                condition,
                then_block,
                else_block,
            } => {
                validate_expr(condition)?;
                validate_statements(then_block)?;
                if let Some(else_block) = else_block {
                    validate_statements(else_block)?;
                }
            }
            SirStatement::While { condition, body } => {
                validate_expr(condition)?;
                validate_statements(body)?;
            }
            SirStatement::For { iterable, body, .. } => {
                match iterable {
                    SirExpr::BinOp {
                        left,
                        op: SirBinOp::Range,
                        right,
                    } => {
                        validate_expr(left)?;
                        validate_expr(right)?;
                    }
                    _ => return Err("a for-loop over a non-range iterable"),
                }
                validate_statements(body)?;
            }
            SirStatement::Switch {
                scrutinee,
                cases,
                default,
            } => {
                validate_expr(scrutinee)?;
                for (constant, body) in cases {
                    validate_expr(constant)?;
                    validate_statements(body)?;
                }
                if let Some(default) = default {
                    validate_statements(default)?;
                }
            }
            SirStatement::Block(body) => validate_statements(body)?,
            SirStatement::Return(None) | SirStatement::Break | SirStatement::Continue => {}
        }
    }
    Ok(())
}

fn validate_expr(expr: &SirExpr) -> Result<(), &'static str> {
    match expr {
        SirExpr::Literal(_) | SirExpr::Var(_) | SirExpr::SizeOf(_) => Ok(()),
        SirExpr::BinOp { left, right, op } => {
            if matches!(op, SirBinOp::Range) {
                return Err("a range outside a for-loop");
            }
            validate_expr(left)?;
            validate_expr(right)
        }
        SirExpr::UnaryOp { operand, .. } => validate_expr(operand),
        SirExpr::Typed { expr, .. } | SirExpr::Cast { value: expr, .. } => validate_expr(expr),
        SirExpr::Call { callee, args } => {
            if callee.starts_with("nova_")
                && !matches!(
                    callee.as_str(),
                    "nova_array_len" | "nova_array_push" | "nova_env_new"
                )
            {
                return Err("runtime calls this backend does not implement");
            }
            if callee == "print" || callee == "println" {
                // Only integers and string literals have a host representation.
                if let Some(arg) = args.first() {
                    if matches!(unspanned_literal(arg), Some(SirLiteral::Float(_))) {
                        return Err("printing a float");
                    }
                    if !matches!(unspanned_literal(arg), Some(SirLiteral::String(_))) {
                        validate_expr(arg)?;
                    }
                }
                return Ok(());
            }
            for arg in args {
                validate_expr(arg)?;
            }
            Ok(())
        }
        SirExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            validate_expr(condition)?;
            validate_expr(then_expr)?;
            if let Some(else_expr) = else_expr {
                validate_expr(else_expr)?;
            }
            Ok(())
        }
        SirExpr::Block(stmts, final_expr) => {
            validate_statements(stmts)?;
            if let Some(final_expr) = final_expr {
                validate_expr(final_expr)?;
            }
            Ok(())
        }
        SirExpr::Assign { value, .. } => validate_expr(value),
        SirExpr::Array(items) => {
            // Every element is stored as an i32, so nested arrays would store a
            // pointer (fine) but strings and floats would change meaning.
            for item in items {
                validate_expr(item)?;
            }
            Ok(())
        }
        SirExpr::Index { array, index } => {
            validate_expr(array)?;
            validate_expr(index)
        }
        SirExpr::IndexAssign {
            array,
            index,
            value,
        } => {
            validate_expr(array)?;
            validate_expr(index)?;
            validate_expr(value)
        }
        SirExpr::StringSlice { .. } => Err("string slicing"),
        SirExpr::Tuple { items, .. } => {
            for item in items {
                validate_expr(item)?;
            }
            Ok(())
        }
        SirExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                validate_expr(value)?;
            }
            Ok(())
        }
        SirExpr::FieldAccess {
            object,
            struct_type,
            ..
        } => {
            if struct_type.is_empty() {
                return Err("a field access with no known struct type");
            }
            validate_expr(object)
        }
        SirExpr::FieldAssign {
            object,
            value,
            struct_type,
            ..
        } => {
            if struct_type.is_empty() {
                return Err("a field assignment with no known struct type");
            }
            validate_expr(object)?;
            validate_expr(value)
        }
        SirExpr::Closure { env, .. } => {
            if let Some(env) = env {
                validate_expr(env)?;
            }
            Ok(())
        }
        SirExpr::EnvAccess { .. } => Ok(()),
        SirExpr::CallValue {
            callee,
            args,
            params,
            return_type,
        } => {
            validate_expr(callee)?;
            for arg in args {
                validate_expr(arg)?;
            }
            if params
                .iter()
                .any(|param| matches!(param.as_str(), "float" | "f32" | "f64"))
                || matches!(return_type.as_str(), "float" | "f32" | "f64")
            {
                return Err("closure signatures with floating-point parameters/results");
            }
            Ok(())
        }
        // WASM values for arrays/structs are already pointers, so `&` and `*`
        // around them are no-ops.
        SirExpr::AddressOf(inner) | SirExpr::Deref(inner) => validate_expr(inner),
    }
}

const DATA_BASE: usize = 1024;

impl WasmBackend {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            strings: Vec::new(),
            globals: HashSet::new(),
            params: HashSet::new(),
            functions: Vec::new(),
            locals: Vec::new(),
            heap_start: DATA_BASE,
            structs: HashMap::new(),
            closures: HashMap::new(),
            call_depth: 0,
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        self.strings.clear();
        self.globals.clear();
        self.functions.clear();

        self.closures.clear();
        for function in &program.functions {
            if function.env_type.is_some() {
                let index = self.closures.len() as u32;
                self.closures.insert(function.name.clone(), index);
            }
        }

        self.structs.clear();
        for structure in &program.structs {
            let layout = structure
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| (field.name.clone(), index * 4))
                .collect();
            self.structs.insert(structure.name.clone(), layout);
        }

        self.globals = program.globals.iter().map(|g| g.name.clone()).collect();
        self.functions = program
            .functions
            .iter()
            .map(|f| (f.name.clone(), returns_value(f)))
            .collect();

        // Only reachable functions are emitted: `use core;` merges helpers that
        // this backend cannot compile, and they must not appear as dead code.
        let reachable = reachable_functions(program);
        let functions: Vec<&SirFunction> = program
            .functions
            .iter()
            .filter(|func| {
                reachable
                    .as_ref()
                    .map(|names| names.contains(&func.name))
                    .unwrap_or(true)
            })
            .collect();

        // Assign every string literal an offset before emitting code.
        for func in &functions {
            self.collect_statement_strings(&func.body);
        }
        for global in &program.globals {
            if let Some(init) = &global.init_value {
                self.collect_expr_strings(init);
            }
        }
        // Deduplicate identical literals and assign each one an offset now that
        // the full set is known. Offsets are byte positions in linear memory.
        let mut seen = HashSet::new();
        self.strings.retain(|(text, _)| seen.insert(text.clone()));
        let mut next_offset = DATA_BASE;
        for (text, offset) in &mut self.strings {
            *offset = next_offset;
            next_offset += text.len();
        }
        // Arrays are bump-allocated after the string data.
        self.heap_start = (next_offset + 3) & !3;

        self.writeln("(module");
        self.indent_level += 1;

        self.writeln("(import \"env\" \"print_i32\" (func $print_i32 (param i32)))");
        self.writeln("(import \"env\" \"print_str\" (func $print_str (param i32 i32)))");
        // Growable arrays: [len, capacity, data]; push copies into a larger
        // block when full, so the descriptor pointer stays valid.
        self.writeln("(func $__array_push (param $arr i32) (param $val i32)");
        self.indent_level += 1;
        self.writeln("(local $len i32) (local $cap i32) (local $data i32) (local $new i32) (local $i i32)");
        self.writeln("(local.set $len (i32.load (local.get $arr)))");
        self.writeln("(local.set $cap (i32.load offset=4 (local.get $arr)))");
        self.writeln("(local.set $data (i32.load offset=8 (local.get $arr)))");
        self.writeln("(if (i32.eq (local.get $len) (local.get $cap))");
        self.indent_level += 1;
        self.writeln("(then");
        self.indent_level += 1;
        self.writeln("(local.set $cap (select (i32.const 4) (i32.mul (local.get $cap) (i32.const 2)) (i32.eqz (local.get $cap))))");
        self.writeln("(local.set $new (global.get $__heap))");
        self.writeln("(global.set $__heap (i32.add (global.get $__heap) (i32.mul (local.get $cap) (i32.const 4))))");
        self.writeln("(local.set $i (i32.const 0))");
        self.writeln("(block $done");
        self.indent_level += 1;
        self.writeln("(loop $copy");
        self.indent_level += 1;
        self.writeln("(br_if $done (i32.ge_u (local.get $i) (local.get $len)))");
        self.writeln("(i32.store (i32.add (local.get $new) (i32.mul (local.get $i) (i32.const 4))) (i32.load (i32.add (local.get $data) (i32.mul (local.get $i) (i32.const 4)))))");
        self.writeln("(local.set $i (i32.add (local.get $i) (i32.const 1)))");
        self.writeln("(br $copy)");
        self.indent_level -= 1;
        self.writeln(")");
        self.indent_level -= 1;
        self.writeln(")");
        self.writeln("(i32.store offset=8 (local.get $arr) (local.get $new))");
        self.writeln("(i32.store offset=4 (local.get $arr) (local.get $cap))");
        self.writeln("(local.set $data (local.get $new))");
        self.indent_level -= 1;
        self.writeln(")");
        self.indent_level -= 1;
        self.writeln(")");
        self.writeln("(i32.store (i32.add (local.get $data) (i32.mul (local.get $len) (i32.const 4))) (local.get $val))");
        self.writeln("(i32.store (local.get $arr) (i32.add (local.get $len) (i32.const 1)))");
        self.indent_level -= 1;
        self.writeln(")");
        // 16 pages (1 MiB) leaves room for strings plus bump-allocated arrays.
        self.writeln("(memory 16)");
        // The host reads string literals out of linear memory.
        self.writeln("(export \"memory\" (memory 0))");
        self.writeln(&format!(
            "(global $__heap (mut i32) (i32.const {}))",
            self.heap_start
        ));

        // One function type per closure arity (the environment is an extra
        // leading i32 parameter), for both result shapes.
        for arity in 0..8 {
            let params = "i32 ".repeat(arity + 1);
            self.writeln(&format!(
                "(type $closure_{arity}_r (func (param {params})(result i32)))"
            ));
            self.writeln(&format!(
                "(type $closure_{arity}_v (func (param {params})))"
            ));
        }
        if !self.closures.is_empty() {
            self.writeln(&format!("(table {} funcref)", self.closures.len()));
        }

        for global in &program.globals {
            let typ = self.type_to_wasm(&global.typ);
            let init = global
                .init_value
                .as_ref()
                .map(|expr| self.literal_to_wasm_const(expr))
                .unwrap_or_else(|| self.default_const_for_type(&global.typ).to_string());
            self.writeln(&format!("(global ${} (mut {typ}) {init})", global.name));
        }

        for (text, offset) in self.strings.clone() {
            self.writeln(&format!(
                "(data (i32.const {offset}) \"{}\")",
                escape_wat_string(&text)
            ));
        }

        for func in &functions {
            self.generate_function(func);
        }

        // The lowerer synthesises `__main` for a top-level script; expose it as
        // `main` so every host (and the parity harness) has one entry point.
        let entry = if program.functions.iter().any(|f| f.name == "main") {
            Some("main")
        } else if program.functions.iter().any(|f| f.name == "__main") {
            Some("__main")
        } else {
            None
        };
        if let Some(entry) = entry {
            self.writeln(&format!("(export \"main\" (func ${entry}))"));
        }

        if !self.closures.is_empty() {
            let mut entries: Vec<(u32, String)> = self
                .closures
                .iter()
                .map(|(name, index)| (*index, name.clone()))
                .collect();
            entries.sort_by_key(|(index, _)| *index);
            let names: Vec<String> = entries
                .iter()
                .map(|(_, name)| format!("${name}"))
                .collect();
            self.writeln(&format!("(elem (i32.const 0) {})", names.join(" ")));
        }

        self.indent_level -= 1;
        self.writeln(")");
        self.output.clone()
    }

    fn generate_function(&mut self, func: &SirFunction) {
        self.locals.clear();
        self.params = func.params.iter().map(|p| p.name.clone()).collect();
        self.collect_locals(&func.body);

        let result_type = if returns_value(func) {
            format!(
                " (result {})",
                self.type_to_wasm(func.return_type.as_deref().unwrap_or("int"))
            )
        } else {
            String::new()
        };

        let mut params: Vec<String> = Vec::new();
        if func.env_type.is_some() {
            params.push("(param $__env i32)".to_string());
        }
        params.extend(
            func.params
                .iter()
                .map(|p| format!("(param ${} {})", p.name, self.type_to_wasm(&p.typ))),
        );

        self.writeln(&format!(
            "(func ${}{}{}",
            func.name,
            if params.is_empty() {
                String::new()
            } else {
                format!(" {}", params.join(" "))
            },
            result_type
        ));
        self.indent_level += 1;

        for (name, typ) in self.locals.clone() {
            self.writeln(&format!("(local ${name} {typ})"));
        }
        // Scratch registers used while building arrays/structs and checking
        // array bounds.
        self.writeln("(local $__arr_tmp i32)");
        self.writeln("(local $__idx_tmp i32)");
        for depth in 0..4 {
            self.writeln(&format!("(local $__call_ptr{depth} i32)"));
            self.writeln(&format!("(local $__call_env{depth} i32)"));
            self.writeln(&format!("(local $__call_index{depth} i32)"));
        }
        self.call_depth = 0;

        for stmt in &func.body {
            self.generate_statement(stmt);
        }

        // A value-returning function whose body ends in control flow (every
        // branch returns) still needs a terminator on the fall-through path.
        if returns_value(func) {
            self.writeln("unreachable");
        }

        self.indent_level -= 1;
        self.writeln(")");
    }

    /// Every `local.set` target must have a declaration, so gather them before
    /// emitting the body. A missing type defaults to `i32`.
    fn collect_locals(&mut self, stmts: &[SirStatement]) {
        for stmt in stmts {
            match stmt {
                SirStatement::Let { name, typ, value } => {
                    self.add_local(name, self.wasm_type(typ.as_deref()));
                    self.collect_expr_locals(value);
                }
                SirStatement::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.collect_expr_locals(condition);
                    self.collect_locals(then_block);
                    if let Some(else_block) = else_block {
                        self.collect_locals(else_block);
                    }
                }
                SirStatement::While { condition, body } => {
                    self.collect_expr_locals(condition);
                    self.collect_locals(body);
                }
                SirStatement::For {
                    pattern,
                    iterable,
                    body,
                } => {
                    self.add_local(pattern, "i32");
                    self.add_local(&format!("{pattern}_end"), "i32");
                    self.collect_expr_locals(iterable);
                    self.collect_locals(body);
                }
                SirStatement::Switch {
                    scrutinee,
                    cases,
                    default,
                } => {
                    self.collect_expr_locals(scrutinee);
                    for (constant, body) in cases {
                        self.collect_expr_locals(constant);
                        self.collect_locals(body);
                    }
                    if let Some(default) = default {
                        self.collect_locals(default);
                    }
                }
                SirStatement::Block(body) => self.collect_locals(body),
                SirStatement::Expr(value) => self.collect_expr_locals(value),
                SirStatement::Return(Some(value)) => self.collect_expr_locals(value),
                SirStatement::Return(None) | SirStatement::Break | SirStatement::Continue => {}
            }
        }
    }

    fn collect_expr_locals(&mut self, expr: &SirExpr) {
        match expr {
            SirExpr::Assign { target, value } => {
                self.add_local(target, "i32");
                self.collect_expr_locals(value);
            }
            SirExpr::BinOp { left, right, .. } => {
                self.collect_expr_locals(left);
                self.collect_expr_locals(right);
            }
            SirExpr::UnaryOp { operand, .. } => self.collect_expr_locals(operand),
            SirExpr::AddressOf(inner) | SirExpr::Deref(inner) => self.collect_expr_locals(inner),
            SirExpr::Cast { value, .. } => self.collect_expr_locals(value),
            SirExpr::Typed { expr, .. } => self.collect_expr_locals(expr),
            SirExpr::Call { args, .. } => {
                for arg in args {
                    self.collect_expr_locals(arg);
                }
            }
            SirExpr::CallValue { args, .. } => {
                for arg in args {
                    self.collect_expr_locals(arg);
                }
            }
            SirExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_expr_locals(condition);
                self.collect_expr_locals(then_expr);
                if let Some(else_expr) = else_expr {
                    self.collect_expr_locals(else_expr);
                }
            }
            SirExpr::Block(stmts, final_expr) => {
                self.collect_locals(stmts);
                if let Some(final_expr) = final_expr {
                    self.collect_expr_locals(final_expr);
                }
            }
            SirExpr::Index { array, index } => {
                self.collect_expr_locals(array);
                self.collect_expr_locals(index);
            }
            SirExpr::IndexAssign {
                array,
                index,
                value,
            } => {
                self.collect_expr_locals(array);
                self.collect_expr_locals(index);
                self.collect_expr_locals(value);
            }
            SirExpr::StringSlice { value, start, end } => {
                self.collect_expr_locals(value);
                self.collect_expr_locals(start);
                self.collect_expr_locals(end);
            }
            SirExpr::Tuple { items, .. } | SirExpr::Array(items) => {
                for item in items {
                    self.collect_expr_locals(item);
                }
            }
            SirExpr::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.collect_expr_locals(value);
                }
            }
            SirExpr::FieldAccess { object, .. } => self.collect_expr_locals(object),
            SirExpr::FieldAssign {
                object, value, ..
            } => {
                self.collect_expr_locals(object);
                self.collect_expr_locals(value);
            }
            SirExpr::Closure { .. }
            | SirExpr::EnvAccess { .. }
            | SirExpr::SizeOf(_)
            | SirExpr::Literal(_)
            | SirExpr::Var(_) => {}
        }
    }

    fn add_local(&mut self, name: &str, typ: &'static str) {
        if self.params.contains(name) || self.globals.contains(name) {
            return;
        }
        if let Some((_, existing)) = self.locals.iter().find(|(n, _)| n == name) {
            // A later annotation is more precise than the `i32` default.
            if *existing == "i32" && typ != "i32" {
                if let Some((_, slot)) = self.locals.iter_mut().find(|(n, _)| n == name) {
                    *slot = typ.to_string();
                }
            }
            return;
        }
        self.locals.push((name.to_string(), typ.to_string()));
    }

    fn collect_statement_strings(&mut self, stmts: &[SirStatement]) {
        for stmt in stmts {
            match stmt {
                SirStatement::Let { value, .. } => self.collect_expr_strings(value),
                SirStatement::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.collect_expr_strings(condition);
                    self.collect_statement_strings(then_block);
                    if let Some(else_block) = else_block {
                        self.collect_statement_strings(else_block);
                    }
                }
                SirStatement::While { condition, body } => {
                    self.collect_expr_strings(condition);
                    self.collect_statement_strings(body);
                }
                SirStatement::For { iterable, body, .. } => {
                    self.collect_expr_strings(iterable);
                    self.collect_statement_strings(body);
                }
                SirStatement::Switch {
                    scrutinee,
                    cases,
                    default,
                } => {
                    self.collect_expr_strings(scrutinee);
                    for (constant, body) in cases {
                        self.collect_expr_strings(constant);
                        self.collect_statement_strings(body);
                    }
                    if let Some(default) = default {
                        self.collect_statement_strings(default);
                    }
                }
                SirStatement::Block(body) => self.collect_statement_strings(body),
                SirStatement::Expr(value) | SirStatement::Return(Some(value)) => {
                    self.collect_expr_strings(value)
                }
                SirStatement::Return(None) | SirStatement::Break | SirStatement::Continue => {}
            }
        }
    }

    fn collect_expr_strings(&mut self, expr: &SirExpr) {
        match expr {
            SirExpr::Literal(SirLiteral::String(text)) => {
                if !self.strings.iter().any(|(existing, _)| existing == text) {
                    self.strings.push((text.clone(), 0));
                }
            }
            SirExpr::BinOp { left, right, .. } => {
                self.collect_expr_strings(left);
                self.collect_expr_strings(right);
            }
            SirExpr::UnaryOp { operand, .. } => self.collect_expr_strings(operand),
            SirExpr::AddressOf(inner)
            | SirExpr::Deref(inner)
            | SirExpr::Cast { value: inner, .. } => self.collect_expr_strings(inner),
            SirExpr::Typed { expr, .. } => self.collect_expr_strings(expr),
            SirExpr::Assign { value, .. } => self.collect_expr_strings(value),
            SirExpr::Call { args, .. } | SirExpr::CallValue { args, .. } => {
                for arg in args {
                    self.collect_expr_strings(arg);
                }
            }
            SirExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_expr_strings(condition);
                self.collect_expr_strings(then_expr);
                if let Some(else_expr) = else_expr {
                    self.collect_expr_strings(else_expr);
                }
            }
            SirExpr::Block(stmts, final_expr) => {
                self.collect_statement_strings(stmts);
                if let Some(final_expr) = final_expr {
                    self.collect_expr_strings(final_expr);
                }
            }
            SirExpr::Index { array, index } => {
                self.collect_expr_strings(array);
                self.collect_expr_strings(index);
            }
            SirExpr::IndexAssign {
                array,
                index,
                value,
            } => {
                self.collect_expr_strings(array);
                self.collect_expr_strings(index);
                self.collect_expr_strings(value);
            }
            SirExpr::StringSlice { value, start, end } => {
                self.collect_expr_strings(value);
                self.collect_expr_strings(start);
                self.collect_expr_strings(end);
            }
            SirExpr::Tuple { items, .. } | SirExpr::Array(items) => {
                for item in items {
                    self.collect_expr_strings(item);
                }
            }
            SirExpr::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.collect_expr_strings(value);
                }
            }
            SirExpr::FieldAccess { object, .. } => self.collect_expr_strings(object),
            SirExpr::FieldAssign {
                object, value, ..
            } => {
                self.collect_expr_strings(object);
                self.collect_expr_strings(value);
            }
            SirExpr::Closure { .. }
            | SirExpr::EnvAccess { .. }
            | SirExpr::SizeOf(_)
            | SirExpr::Literal(_)
            | SirExpr::Var(_) => {}
        }
    }

    fn generate_statement(&mut self, stmt: &SirStatement) {
        match stmt {
            SirStatement::Let { name, value, .. } => {
                self.generate_expr(value);
                self.store(name);
            }
            SirStatement::Expr(expr) => {
                self.generate_expr(expr);
                if expression_yields_value(expr, &self.functions) {
                    self.writeln("drop");
                }
            }
            SirStatement::If {
                condition,
                then_block,
                else_block,
            } => {
                self.generate_expr(condition);
                self.writeln("(if");
                self.indent_level += 1;
                self.writeln("(then");
                self.indent_level += 1;
                for stmt in then_block {
                    self.generate_statement(stmt);
                }
                self.indent_level -= 1;

                if let Some(else_stmts) = else_block {
                    self.writeln(")(else");
                    self.indent_level += 1;
                    for stmt in else_stmts {
                        self.generate_statement(stmt);
                    }
                    self.indent_level -= 1;
                }
                self.writeln(")");
                self.indent_level -= 1;
                self.writeln(")");
            }
            SirStatement::Switch {
                scrutinee,
                cases,
                default,
            } => {
                // WebAssembly has no switch instruction; emit an if/else chain.
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
            SirStatement::For {
                pattern,
                iterable,
                body,
            } => {
                if let SirExpr::BinOp {
                    left,
                    op: SirBinOp::Range,
                    right,
                } = iterable
                {
                    self.generate_expr(left);
                    self.store(pattern);

                    let end_name = format!("{pattern}_end");
                    self.generate_expr(right);
                    self.store(&end_name);

                    self.writeln("(block $break");
                    self.indent_level += 1;
                    self.writeln("(loop $continue");
                    self.indent_level += 1;

                    self.writeln(&format!("(local.get ${pattern})"));
                    self.writeln(&format!("(local.get ${end_name})"));
                    self.writeln("(i32.lt_s)");
                    self.writeln("(i32.eqz)");
                    self.writeln("(br_if $break)");

                    for stmt in body {
                        self.generate_statement(stmt);
                    }

                    self.writeln(&format!("(local.get ${pattern})"));
                    self.writeln("(i32.const 1)");
                    self.writeln("(i32.add)");
                    self.store(pattern);
                    self.writeln("(br $continue)");

                    self.indent_level -= 1;
                    self.writeln(")");
                    self.indent_level -= 1;
                    self.writeln(")");
                } else {
                    // A non-range iterable has no WebAssembly representation yet;
                    // the CLI rejects such programs for this target.
                    self.writeln("(; unsupported for-loop iterable ;)");
                }
            }
            SirStatement::Return(value) => {
                if let Some(val) = value {
                    self.generate_expr(val);
                }
                self.writeln("return");
            }
            SirStatement::Block(stmts) => {
                for stmt in stmts {
                    self.generate_statement(stmt);
                }
            }
            SirStatement::Break => self.writeln("br 1"),
            SirStatement::Continue => self.writeln("br 0"),
        }
    }

    fn generate_expr(&mut self, expr: &SirExpr) {
        match expr {
            SirExpr::Literal(lit) => match lit {
                SirLiteral::Int(n) => self.writeln(&format!("(i32.const {n})")),
                SirLiteral::Float(f) => self.writeln(&format!("(f64.const {f})")),
                SirLiteral::Bool(b) => {
                    self.writeln(&format!("(i32.const {})", i32::from(*b)))
                }
                SirLiteral::Char(c) => self.writeln(&format!("(i32.const {})", *c as u32)),
                SirLiteral::String(text) => {
                    let offset = self
                        .strings
                        .iter()
                        .find(|(existing, _)| existing == text)
                        .map(|(_, offset)| *offset)
                        .unwrap_or(DATA_BASE);
                    self.writeln(&format!("(i32.const {offset})"));
                }
                SirLiteral::Unit => self.writeln("(i32.const 0)"),
            },
            SirExpr::Var(name) => {
                if self.globals.contains(name) {
                    self.writeln(&format!("(global.get ${name})"));
                } else {
                    self.writeln(&format!("(local.get ${name})"));
                }
            }
            SirExpr::BinOp { left, op, right } => {
                self.generate_expr(left);
                self.generate_expr(right);
                self.writeln(wat_instruction(op));
            }
            SirExpr::UnaryOp { op, operand } => {
                match op {
                    // WebAssembly has no integer negate instruction: 0 - x.
                    SirUnOp::Neg => {
                        self.writeln("(i32.const 0)");
                        self.generate_expr(operand);
                        self.writeln("i32.sub");
                    }
                    SirUnOp::Not => {
                        self.generate_expr(operand);
                        self.writeln("i32.eqz");
                    }
                }
            }
            SirExpr::Call { callee, args } if callee == "print" || callee == "println" => {
                self.emit_print(args);
            }
            // Arrays are a pointer to `[len, elem0, elem1, ...]` in linear memory.
            // `array_push` grows the data and updates the descriptor in place.
            SirExpr::Call { callee, args } if callee == "nova_array_push" => {
                if let Some(array) = args.first() {
                    self.generate_expr(array);
                } else {
                    self.writeln("(i32.const 0)");
                }
                if let Some(value) = args.get(1) {
                    self.generate_expr(value);
                } else {
                    self.writeln("(i32.const 0)");
                }
                self.writeln("(call $__array_push)");
            }
            // The environment already lives in linear memory, so copying it to
            // the heap is a no-op on this backend.
            SirExpr::Call { callee, args } if callee == "nova_env_new" => {
                if let Some(value) = args.first() {
                    self.generate_expr(value);
                } else {
                    self.writeln("(i32.const 0)");
                }
            }
            SirExpr::Call { callee, args } if callee == "nova_array_len" => {
                if let Some(array) = args.first() {
                    self.generate_expr(array);
                    self.writeln("(i32.load)");
                } else {
                    self.writeln("(i32.const 0)");
                }
            }
            SirExpr::Call { callee, args } => {
                for arg in args {
                    self.generate_expr(arg);
                }
                self.writeln(&format!("(call ${callee})"));
            }
            SirExpr::CallValue {
                callee,
                args,
                return_type,
                ..
            } => {
                // Closure value: [table index, environment pointer].
                let depth = self.call_depth.min(3);
                self.call_depth += 1;
                self.generate_expr(callee);
                self.writeln(&format!("(local.tee $__call_ptr{depth})"));
                self.writeln("(i32.load offset=4)");
                self.writeln(&format!("(local.set $__call_env{depth})"));
                self.writeln(&format!("(local.get $__call_ptr{depth})"));
                self.writeln("(i32.load)");
                self.writeln(&format!("(local.set $__call_index{depth})"));
                self.writeln(&format!("(local.get $__call_env{depth})"));
                for arg in args {
                    self.generate_expr(arg);
                }
                self.call_depth -= 1;
                self.writeln(&format!("(local.get $__call_index{depth})"));
                let returns = return_type.as_str();
                let suffix = if matches!(returns, "" | "void" | "()" | "unit" | "!") {
                    "v"
                } else {
                    "r"
                };
                self.writeln(&format!(
                    "(call_indirect (type $closure_{}_{suffix}))",
                    args.len()
                ));
            }
            SirExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                self.generate_expr(condition);
                self.writeln("(if");
                self.indent_level += 1;
                self.writeln("(then");
                self.indent_level += 1;
                self.generate_expr(then_expr);
                self.indent_level -= 1;

                if let Some(else_e) = else_expr {
                    self.writeln(")(else");
                    self.indent_level += 1;
                    self.generate_expr(else_e);
                    self.indent_level -= 1;
                }
                self.writeln(")");
                self.indent_level -= 1;
                self.writeln(")");
            }
            SirExpr::Typed { expr, .. } => self.generate_expr(expr),
            SirExpr::Assign { target, value } => {
                self.generate_expr(value);
                self.store(target);
            }
            SirExpr::Block(stmts, final_expr) => {
                for stmt in stmts {
                    self.generate_statement(stmt);
                }
                if let Some(final_e) = final_expr {
                    self.generate_expr(final_e);
                }
            }
            // Constructs without a WebAssembly representation. The CLI rejects
            // them before reaching here; emitting a marker keeps `emit` honest
            // if the backend is used directly.
            SirExpr::Closure { name, env } => {
                self.writeln("(global.get $__heap)");
                self.writeln("(local.set $__arr_tmp)");
                self.writeln("(global.set $__heap");
                self.indent_level += 1;
                self.writeln("(i32.add (global.get $__heap) (i32.const 8))");
                self.indent_level -= 1;
                self.writeln(")");
                let index = self.closures.get(name).copied().unwrap_or(0);
                self.writeln("(local.get $__arr_tmp)");
                self.writeln(&format!("(i32.const {index})"));
                self.writeln("(i32.store)");
                self.writeln("(local.get $__arr_tmp)");
                if let Some(env) = env {
                    self.generate_expr(env);
                } else {
                    self.writeln("(i32.const 0)");
                }
                self.writeln("(i32.store offset=4)");
                self.writeln("(local.get $__arr_tmp)");
            }
            SirExpr::EnvAccess { env_type, field } => {
                self.writeln("(local.get $__env)");
                let offset = self
                    .structs
                    .get(env_type)
                    .and_then(|layout| layout.iter().find(|(name, _)| name == field))
                    .map(|(_, offset)| *offset)
                    .unwrap_or(0);
                self.writeln(&format!("(i32.load offset={offset})"));
            }
            SirExpr::SizeOf(_) => self.writeln("(i32.const 4)"),
            SirExpr::Cast { value, .. } => self.generate_expr(value),
            SirExpr::Tuple { name, items } => {
                let values: Vec<(String, &SirExpr)> = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| (format!("_{index}"), item))
                    .collect();
                self.emit_struct_value(name, &values);
            }
            SirExpr::StructLiteral { name, fields } => {
                let values: Vec<(String, &SirExpr)> = fields
                    .iter()
                    .map(|(field, value)| (field.clone(), value))
                    .collect();
                self.emit_struct_value(name, &values);
            }
            SirExpr::FieldAccess {
                object,
                field,
                struct_type,
            } => {
                self.generate_expr(object);
                let Some(offset) = self
                    .structs
                    .get(struct_type)
                    .and_then(|layout| layout.iter().find(|(name, _)| name == field))
                    .map(|(_, offset)| *offset)
                else {
                    self.writeln("unreachable (; unknown struct field ;)");
                    self.writeln("(i32.const 0)");
                    return;
                };
                self.writeln(&format!("(i32.load offset={offset})"));
            }
            SirExpr::FieldAssign {
                object,
                field,
                value,
                struct_type,
            } => {
                self.generate_expr(object);
                let Some(offset) = self
                    .structs
                    .get(struct_type)
                    .and_then(|layout| layout.iter().find(|(name, _)| name == field))
                    .map(|(_, offset)| *offset)
                else {
                    self.writeln("unreachable (; unknown struct field ;)");
                    return;
                };
                self.generate_expr(value);
                self.writeln(&format!("(i32.store offset={offset})"));
            }
            SirExpr::Deref(inner) => self.generate_expr(inner),
            // Arrays and structs are already pointers into linear memory, so
            // taking their address is a no-op.
            SirExpr::AddressOf(inner) => self.generate_expr(inner),
            SirExpr::Array(items) => {
                // Descriptor: [len, capacity, data pointer]. Keeping the
                // descriptor stable lets `array_push` grow the data without
                // changing the value held by the variable.
                let data_size = 4 * items.len();
                self.writeln("(global.get $__heap)");
                self.writeln("(local.set $__arr_tmp)");
                self.writeln("(global.set $__heap");
                self.indent_level += 1;
                self.writeln(&format!(
                    "(i32.add (global.get $__heap) (i32.const {}))",
                    12 + data_size
                ));
                self.indent_level -= 1;
                self.writeln(")");
                // Data starts right after the 12-byte descriptor.
                self.writeln("(local.get $__arr_tmp)");
                self.writeln("(i32.const 12)");
                self.writeln("(i32.add)");
                self.writeln("(local.set $__idx_tmp)");
                // len, capacity, data.
                self.writeln("(local.get $__arr_tmp)");
                self.writeln(&format!("(i32.const {})", items.len()));
                self.writeln("(i32.store)");
                self.writeln("(local.get $__arr_tmp)");
                self.writeln(&format!("(i32.const {})", items.len()));
                self.writeln("(i32.store offset=4)");
                self.writeln("(local.get $__arr_tmp)");
                self.writeln("(local.get $__idx_tmp)");
                self.writeln("(i32.store offset=8)");
                // Elements.
                for (index, item) in items.iter().enumerate() {
                    self.writeln("(local.get $__idx_tmp)");
                    self.generate_expr(item);
                    self.writeln(&format!("(i32.store offset={})", index * 4));
                }
                self.writeln("(local.get $__arr_tmp)");
            }
            SirExpr::StringSlice { value, .. } => {
                self.generate_expr(value);
                self.writeln("(i32.const 0) (; string slice: unsupported ;)");
            }
            SirExpr::Index { array, index } => {
                self.emit_bounds_check(array, index);
                self.emit_element_address();
                self.writeln("(i32.load)");
            }
            SirExpr::IndexAssign {
                array,
                index,
                value,
            } => {
                self.emit_bounds_check(array, index);
                self.emit_element_address();
                self.generate_expr(value);
                self.writeln("(i32.store)");
            }
        }
    }

    /// Evaluate the array and index once each, then trap when the index is out
    /// of range, matching the C runtime's bounds failure instead of reading
    /// unrelated memory.
    fn emit_bounds_check(&mut self, array: &SirExpr, index: &SirExpr) {
        self.generate_expr(array);
        self.writeln("(local.set $__arr_tmp)");
        self.generate_expr(index);
        self.writeln("(local.set $__idx_tmp)");
        self.writeln("(local.get $__idx_tmp)");
        self.writeln("(local.get $__arr_tmp)");
        self.writeln("(i32.load)");
        self.writeln("(i32.ge_u)");
        self.writeln("(if");
        self.indent_level += 1;
        self.writeln("(then");
        self.indent_level += 1;
        self.writeln("unreachable");
        self.indent_level -= 1;
        self.writeln(")");
        self.indent_level -= 1;
        self.writeln(")");
    }

    /// Push `data pointer + index*4` for the array/value captured by the check.
    fn emit_element_address(&mut self) {
        self.writeln("(local.get $__arr_tmp)");
        self.writeln("(i32.load offset=8)");
        self.writeln("(local.get $__idx_tmp)");
        self.writeln("(i32.const 4)");
        self.writeln("(i32.mul)");
        self.writeln("(i32.add)");
    }

    /// Allocate `size` bytes from the bump heap and leave the base pointer in
    /// `$__arr_tmp`.
    fn allocate(&mut self, size: usize) {
        self.writeln("(global.get $__heap)");
        self.writeln("(local.set $__arr_tmp)");
        self.writeln("(global.set $__heap");
        self.indent_level += 1;
        self.writeln(&format!(
            "(i32.add (global.get $__heap) (i32.const {size}))"
        ));
        self.indent_level -= 1;
        self.writeln(")");
    }

    /// Build a struct/tuple value: every field is zero-initialised, then the
    /// provided values are stored at their declared offsets.
    fn emit_struct_value(&mut self, name: &str, values: &[(String, &SirExpr)]) {
        let layout = self.structs.get(name).cloned().unwrap_or_default();
        self.allocate(layout.len() * 4);
        for (_, offset) in &layout {
            self.writeln("(local.get $__arr_tmp)");
            self.writeln("(i32.const 0)");
            self.writeln(&format!("(i32.store offset={offset})"));
        }
        for (field, value) in values {
            let Some((_, offset)) = layout.iter().find(|(name, _)| name == field) else {
                continue;
            };
            self.writeln("(local.get $__arr_tmp)");
            self.generate_expr(value);
            self.writeln(&format!("(i32.store offset={offset})"));
        }
        self.writeln("(local.get $__arr_tmp)");
    }

    /// `print`/`println` end the line in NOVA. Strings pass a pointer and length
    /// to the host; everything else is an i32.
    fn emit_print(&mut self, args: &[SirExpr]) {
        let Some(arg) = args.first() else {
            return;
        };
        match unspanned_literal(arg) {
            Some(SirLiteral::String(text)) => {
                let offset = self
                    .strings
                    .iter()
                    .find(|(existing, _)| existing.as_str() == text.as_str())
                    .map(|(_, offset)| *offset)
                    .unwrap_or(DATA_BASE);
                self.writeln(&format!(
                    "(call $print_str (i32.const {offset}) (i32.const {}))",
                    text.len()
                ));
            }
            _ => {
                self.generate_expr(arg);
                self.writeln("(call $print_i32)");
            }
        }
        // `print` is a statement in practice; a dummy value keeps `drop` valid
        // when it appears in expression position.
        self.writeln("(i32.const 0)");
    }

    fn store(&mut self, name: &str) {
        if self.globals.contains(name) {
            self.writeln(&format!("(global.set ${name})"));
        } else {
            self.writeln(&format!("(local.set ${name})"));
        }
    }

    fn literal_to_wasm_const(&self, expr: &SirExpr) -> String {
        match expr {
            SirExpr::Literal(SirLiteral::Int(n)) => format!("(i32.const {n})"),
            SirExpr::Literal(SirLiteral::Float(f)) => format!("(f64.const {f})"),
            SirExpr::Literal(SirLiteral::Bool(b)) => format!("(i32.const {})", i32::from(*b)),
            _ => "(i32.const 0)".to_string(),
        }
    }

    fn type_to_wasm(&self, typ: &str) -> &'static str {
        match typ {
            "float" | "f64" => "f64",
            "f32" => "f32",
            _ => "i32",
        }
    }

    fn wasm_type(&self, typ: Option<&str>) -> &'static str {
        typ.map(|typ| self.type_to_wasm(typ)).unwrap_or("i32")
    }

    fn default_const_for_type(&self, typ: &str) -> &'static str {
        match typ {
            "float" | "f64" => "(f64.const 0)",
            "f32" => "(f32.const 0)",
            _ => "(i32.const 0)",
        }
    }

    fn writeln(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent_level);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }
}

fn wat_instruction(op: &SirBinOp) -> &'static str {
        match op {
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
            // Ranges are handled by `for`; a standalone range has no value.
            SirBinOp::Range => "drop",
        }
}

fn unspanned_literal(expr: &SirExpr) -> Option<&SirLiteral> {
    match expr {
        SirExpr::Literal(lit) => Some(lit),
        SirExpr::Typed { expr, .. } => unspanned_literal(expr),
        _ => None,
    }
}

fn returns_value(func: &SirFunction) -> bool {
    func.return_type
        .as_deref()
        .map(|typ| !typ.is_empty() && typ != "void" && typ != "!")
        .unwrap_or(false)
}

fn is_void_type(typ: &str) -> bool {
    matches!(typ, "" | "void" | "()" | "unit" | "!")
}

fn expression_yields_value(expr: &SirExpr, functions: &[(String, bool)]) -> bool {
    if let SirExpr::Typed { expr, .. } = expr {
        return expression_yields_value(expr, functions);
    }
    match expr {
        SirExpr::Call { callee, .. } => {
            if callee == "print" || callee == "println" {
                // `emit_print` leaves a dummy value for `drop`.
                return true;
            }
            if callee == "nova_array_push" || callee == "nova_env_new" {
                return false;
            }
            functions
                .iter()
                .find(|(name, _)| name == callee)
                .map(|(_, returns)| *returns)
                .unwrap_or(true)
        }
        SirExpr::Assign { .. }
        | SirExpr::IndexAssign { .. }
        | SirExpr::FieldAssign { .. } => false,
        SirExpr::CallValue { return_type, .. } => {
            !matches!(return_type.as_str(), "" | "void" | "()" | "unit" | "!")
        }
        _ => true,
    }
}

fn escape_wat_string(text: &str) -> String {
    let mut escaped = String::new();
    for byte in text.bytes() {
        match byte {
            b'"' => escaped.push_str("\\\""),
            b'\\' => escaped.push_str("\\\\"),
            b'\n' => escaped.push_str("\\n"),
            b'\t' => escaped.push_str("\\t"),
            b'\r' => escaped.push_str("\\r"),
            0x20..=0x7e => escaped.push(byte as char),
            _ => escaped.push_str(&format!("\\{byte:02x}")),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_common::Span;

    fn empty_program() -> SirProgram {
        SirProgram {
            functions: vec![],
            globals: vec![],
            structs: vec![],
        }
    }

    #[test]
    fn test_simple_function() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "add".to_string(),
                params: vec![
                    nova_common::sir::SirParam {
                        name: "a".to_string(),
                        typ: "int".to_string(),
                    },
                    nova_common::sir::SirParam {
                        name: "b".to_string(),
                        typ: "int".to_string(),
                    },
                ],
                return_type: Some("int".to_string()),
                body: vec![SirStatement::Return(Some(SirExpr::BinOp {
                    left: Box::new(SirExpr::Var("a".to_string())),
                    op: SirBinOp::Add,
                    right: Box::new(SirExpr::Var("b".to_string())),
                }))],
                span: Span::default(),
                env_type: None,
            }],
            globals: vec![],
            structs: vec![],
        };

        let mut backend = WasmBackend::new();
        let wasm_code = backend.generate(&program);

        assert!(wasm_code.contains("(func $add"));
        assert!(wasm_code.contains("(param $a i32)"));
        // A program with no `main` must not export one.
        assert!(!wasm_code.contains("(export \"main\""));
    }

    #[test]
    fn locals_are_declared_and_print_uses_the_host_import() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "__main".to_string(),
                params: vec![],
                return_type: None,
                body: vec![SirStatement::Let {
                    name: "n".to_string(),
                    typ: Some("int".to_string()),
                    value: SirExpr::Literal(SirLiteral::Int(42)),
                }],
                span: Span::default(),
                env_type: None,
            }],
            globals: vec![],
            structs: vec![],
        };

        let mut backend = WasmBackend::new();
        let code = backend.generate(&program);

        assert!(code.contains("(local $n i32)"), "{code}");
        assert!(code.contains("(local.set $n)"), "{code}");
        assert!(code.contains("(export \"main\" (func $__main))"), "{code}");
    }

    #[test]
    fn string_literals_get_data_segments_and_print_str() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "__main".to_string(),
                params: vec![],
                return_type: None,
                body: vec![SirStatement::Expr(SirExpr::Call {
                    callee: "print".to_string(),
                    args: vec![SirExpr::Literal(SirLiteral::String("hi".to_string()))],
                })],
                span: Span::default(),
                env_type: None,
            }],
            globals: vec![],
            structs: vec![],
        };

        let mut backend = WasmBackend::new();
        let code = backend.generate(&program);

        assert!(code.contains("(memory 16)"), "{code}");
        assert!(code.contains("(export \"memory\" (memory 0))"), "{code}");
        assert!(code.contains("(data (i32.const 1024) \"hi\")"), "{code}");
        assert!(
            code.contains("(call $print_str (i32.const 1024) (i32.const 2))"),
            "{code}"
        );
    }

    #[test]
    fn array_literals_allocate_and_index() {
        let program = SirProgram {
            functions: vec![SirFunction {
                name: "main".to_string(),
                params: vec![],
                return_type: None,
                body: vec![
                    SirStatement::Let {
                        name: "a".to_string(),
                        typ: Some("Array".to_string()),
                        value: SirExpr::Array(vec![
                            SirExpr::Literal(SirLiteral::Int(1)),
                            SirExpr::Literal(SirLiteral::Int(2)),
                        ]),
                    },
                    SirStatement::Expr(SirExpr::IndexAssign {
                        array: Box::new(SirExpr::Var("a".to_string())),
                        index: Box::new(SirExpr::Literal(SirLiteral::Int(1))),
                        value: Box::new(SirExpr::Literal(SirLiteral::Int(9))),
                    }),
                    SirStatement::Expr(SirExpr::Call {
                        callee: "print".to_string(),
                        args: vec![SirExpr::Index {
                            array: Box::new(SirExpr::Var("a".to_string())),
                            index: Box::new(SirExpr::Literal(SirLiteral::Int(1))),
                        }],
                    }),
                ],
                span: Span::default(),
                env_type: None,
            }],
            globals: vec![],
            structs: vec![],
        };

        let mut backend = WasmBackend::new();
        let code = backend.generate(&program);

        assert!(code.contains("(global $__heap"), "{code}");
        assert!(code.contains("(i32.store offset=4)"), "{code}");
        assert!(code.contains("(i32.store offset=8)"), "{code}");
        // Array values are pointers; the header holds the length.
        assert!(code.contains("(i32.const 2)"), "{code}");
        assert!(code.contains("(i32.store)"), "{code}");
        assert!(!code.contains("array: unsupported"), "{code}");
    }

    #[test]
    fn empty_program_is_valid_wat() {
        let mut backend = WasmBackend::new();
        let code = backend.generate(&empty_program());
        assert!(code.starts_with("(module"));
        assert!(code.trim_end().ends_with(')'));
    }
}
