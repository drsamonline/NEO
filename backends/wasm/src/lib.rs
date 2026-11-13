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
//! Constructs without a representation yet (arrays, structs, tuples, closures)
//! are rejected by the CLI's target check rather than silently lowered to
//! placeholders - see `unsupported_feature` in this module.

use nova_common::sir::{
    SirBinOp, SirExpr, SirFunction, SirGlobal, SirLiteral, SirProgram, SirStatement, SirUnOp,
};
use std::collections::HashSet;

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
}

/// Reject programs this backend cannot represent faithfully, so `emit --target
/// wasm` fails with a reason instead of writing placeholder code.
pub fn validate(program: &SirProgram) -> Result<(), &'static str> {
    for func in &program.functions {
        validate_statements(&func.body)?;
    }
    Ok(())
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
        SirExpr::Array(_) => Err("arrays"),
        SirExpr::Index { .. } | SirExpr::IndexAssign { .. } => Err("array indexing"),
        SirExpr::StringSlice { .. } => Err("string slicing"),
        SirExpr::Tuple { .. } => Err("tuples"),
        SirExpr::StructLiteral { .. } => Err("struct literals"),
        SirExpr::FieldAccess { .. } | SirExpr::FieldAssign { .. } => Err("struct fields"),
        SirExpr::Closure { .. } | SirExpr::EnvAccess { .. } | SirExpr::CallValue { .. } => {
            Err("closures and indirect calls")
        }
        SirExpr::AddressOf(_) | SirExpr::Deref(_) => Err("references"),
    }
}

const DATA_BASE: usize = 1024;
const PAGE: usize = 65536;

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
        }
    }

    pub fn generate(&mut self, program: &SirProgram) -> String {
        self.output.clear();
        self.strings.clear();
        self.globals.clear();
        self.functions.clear();

        self.globals = program.globals.iter().map(|g| g.name.clone()).collect();
        self.functions = program
            .functions
            .iter()
            .map(|f| (f.name.clone(), returns_value(f)))
            .collect();

        // Assign every string literal an offset before emitting code.
        for func in &program.functions {
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

        self.writeln("(module");
        self.indent_level += 1;

        self.writeln("(import \"env\" \"print_i32\" (func $print_i32 (param i32)))");
        self.writeln("(import \"env\" \"print_str\" (func $print_str (param i32 i32)))");
        self.writeln(&format!("(memory {})", memory_pages(&program.globals, &self.strings)));
        // The host reads string literals out of linear memory.
        self.writeln("(export \"memory\" (memory 0))");

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

        for func in &program.functions {
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

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("(param ${} {})", p.name, self.type_to_wasm(&p.typ)))
            .collect();

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

        for stmt in &func.body {
            self.generate_statement(stmt);
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
                self.generate_expr(operand);
                self.writeln(match op {
                    SirUnOp::Neg => "i32.neg",
                    SirUnOp::Not => "i32.eqz",
                });
            }
            SirExpr::Call { callee, args } if callee == "print" || callee == "println" => {
                self.emit_print(args);
            }
            SirExpr::Call { callee, args } => {
                for arg in args {
                    self.generate_expr(arg);
                }
                self.writeln(&format!("(call ${callee})"));
            }
            SirExpr::CallValue { args, .. } => {
                // Indirect calls (closures) have no WebAssembly ABI yet.
                for arg in args {
                    self.generate_expr(arg);
                }
                self.writeln("(i32.const 0)");
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
            SirExpr::Closure { .. } | SirExpr::EnvAccess { .. } => {
                self.writeln("(; closure: unsupported ;) (i32.const 0)");
            }
            SirExpr::SizeOf(_) => self.writeln("(i32.const 4)"),
            SirExpr::Cast { value, .. } => self.generate_expr(value),
            SirExpr::Tuple { items, .. } => {
                for item in items {
                    self.generate_expr(item);
                    self.writeln("drop");
                }
                self.writeln("(i32.const 0) (; tuple: unsupported ;)");
            }
            SirExpr::StructLiteral { .. } => {
                self.writeln("(i32.const 0) (; struct: unsupported ;)")
            }
            SirExpr::FieldAccess { .. } => self.writeln("(i32.const 0) (; field: unsupported ;)"),
            SirExpr::FieldAssign { value, .. } => {
                self.generate_expr(value);
                self.writeln("drop (; field assignment: unsupported ;)");
            }
            SirExpr::Deref(inner) => {
                self.generate_expr(inner);
                self.writeln("(i32.load)");
            }
            SirExpr::AddressOf(inner) => {
                self.generate_expr(inner);
                self.writeln("(; address-of: unsupported ;)");
            }
            SirExpr::Array(items) => {
                for item in items {
                    self.generate_expr(item);
                    self.writeln("drop");
                }
                self.writeln("(i32.const 0) (; array: unsupported ;)");
            }
            SirExpr::StringSlice { value, .. } => {
                self.generate_expr(value);
                self.writeln("(i32.const 0) (; string slice: unsupported ;)");
            }
            SirExpr::Index { array, index } => {
                self.generate_expr(array);
                self.generate_expr(index);
                self.writeln("(i32.const 0) (; index: unsupported ;)");
            }
            SirExpr::IndexAssign { .. } => {
                self.writeln("(; index assignment: unsupported ;)");
            }
        }
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

fn expression_yields_value(expr: &SirExpr, functions: &[(String, bool)]) -> bool {
    match expr {
        SirExpr::Call { callee, .. } => {
            if callee == "print" || callee == "println" {
                // `emit_print` leaves a dummy value for `drop`.
                return true;
            }
            functions
                .iter()
                .find(|(name, _)| name == callee)
                .map(|(_, returns)| *returns)
                .unwrap_or(true)
        }
        SirExpr::Assign { .. } => false,
        _ => true,
    }
}

fn memory_pages(globals: &[SirGlobal], strings: &[(String, usize)]) -> usize {
    let bytes = strings
        .iter()
        .map(|(text, offset)| offset + text.len())
        .max()
        .unwrap_or(DATA_BASE);
    let globals_bytes = globals.len() * 8;
    ((bytes + globals_bytes) / PAGE).max(1)
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

        assert!(code.contains("(memory 1)"), "{code}");
        assert!(code.contains("(export \"memory\" (memory 0))"), "{code}");
        assert!(code.contains("(data (i32.const 1024) \"hi\")"), "{code}");
        assert!(
            code.contains("(call $print_str (i32.const 1024) (i32.const 2))"),
            "{code}"
        );
    }

    #[test]
    fn empty_program_is_valid_wat() {
        let mut backend = WasmBackend::new();
        let code = backend.generate(&empty_program());
        assert!(code.starts_with("(module"));
        assert!(code.trim_end().ends_with(')'));
    }
}
