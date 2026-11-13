//! Semantic IR (Intermediate Representation)
//! 
//! SIR is a lowered representation of the AST that's closer to machine code
//! and easier to translate to backend languages like C.

use crate::Span;

/// SIR Program - the root of the SIR tree
#[derive(Debug, Clone)]
pub struct SirProgram {
    pub functions: Vec<SirFunction>,
    pub globals: Vec<SirGlobal>,
    /// User-defined struct types, emitted before the functions that use them.
    pub structs: Vec<SirStruct>,
}

/// SIR struct type
#[derive(Debug, Clone)]
pub struct SirStruct {
    pub name: String,
    pub fields: Vec<SirStructField>,
}

/// SIR struct field
#[derive(Debug, Clone)]
pub struct SirStructField {
    pub name: String,
    pub typ: String,
}

/// SIR Function
#[derive(Debug, Clone)]
pub struct SirFunction {
    pub name: String,
    pub params: Vec<SirParam>,
    pub return_type: Option<String>,
    pub body: Vec<SirStatement>,
    pub span: Span,
    /// Set for a lifted closure: the generated environment struct. The C
    /// function then takes an extra leading `void *__env` parameter.
    pub env_type: Option<String>,
}

/// SIR Function Parameter
#[derive(Debug, Clone)]
pub struct SirParam {
    pub name: String,
    pub typ: String,
}

/// SIR Global Variable
#[derive(Debug, Clone)]
pub struct SirGlobal {
    pub name: String,
    pub typ: String,
    pub init_value: Option<SirExpr>,
}

/// SIR Statement
#[derive(Debug, Clone)]
pub enum SirStatement {
    /// Variable declaration: let x = expr
    Let {
        name: String,
        typ: Option<String>,
        value: SirExpr,
    },
    /// Expression statement
    Expr(SirExpr),
    /// Return statement
    Return(Option<SirExpr>),
    /// If statement
    If {
        condition: SirExpr,
        then_block: Vec<SirStatement>,
        else_block: Option<Vec<SirStatement>>,
    },
    /// While loop
    While {
        condition: SirExpr,
        body: Vec<SirStatement>,
    },
    /// For loop
    For {
        pattern: String,
        iterable: SirExpr,
        body: Vec<SirStatement>,
    },
    /// Block statement
    Block(Vec<SirStatement>),
    /// Statement-position `match`: a C `switch` over integer case constants,
    /// with the wildcard arm (if any) as `default`.
    Switch {
        scrutinee: SirExpr,
        cases: Vec<(SirExpr, Vec<SirStatement>)>,
        default: Option<Vec<SirStatement>>,
    },
    /// Break statement
    Break,
    /// Continue statement
    Continue,
}

#[derive(Debug, Clone)]
pub enum SirExpr {
    /// Literal value
    Literal(SirLiteral),
    /// Variable reference
    Var(String),
    /// Binary operation
    BinOp {
        left: Box<SirExpr>,
        op: SirBinOp,
        right: Box<SirExpr>,
    },
    /// Unary operation
    UnaryOp {
        op: SirUnOp,
        operand: Box<SirExpr>,
    },
    /// Address of a value, used to pass a `&self` receiver to a method or a
    /// `&mut` parameter to a callee.
    AddressOf(Box<SirExpr>),
    /// Dereference a pointer, used when a `&mut` array parameter is indexed.
    Deref(Box<SirExpr>),
    /// Function call
    Call {
        callee: String,
        args: Vec<SirExpr>,
    },
    /// Assignment
    Assign {
        target: String,
        value: Box<SirExpr>,
    },
    /// Field assignment: `object.field = value`.
    FieldAssign {
        object: Box<SirExpr>,
        field: String,
        value: Box<SirExpr>,
        /// Declared struct type of `object`, so codegen can find the field's
        /// offset without re-deriving types. Empty when the lowerer did not know
        /// it (the WASM backend rejects that case rather than guessing).
        struct_type: String,
    },
    /// Array literal: `[a, b, c]`.
    Array(Vec<SirExpr>),
    /// Array element read: `array[index]`.
    Index {
        array: Box<SirExpr>,
        index: Box<SirExpr>,
    },
    /// String slice: `text[start..end]`.
    StringSlice {
        value: Box<SirExpr>,
        start: Box<SirExpr>,
        end: Box<SirExpr>,
    },
    /// Array element write: `array[index] = value`.
    IndexAssign {
        array: Box<SirExpr>,
        index: Box<SirExpr>,
        value: Box<SirExpr>,
    },
    /// If expression (returns a value)
    If {
        condition: Box<SirExpr>,
        then_expr: Box<SirExpr>,
        else_expr: Option<Box<SirExpr>>,
    },
    /// Block expression (returns final value)
    Block(Vec<SirStatement>, Option<Box<SirExpr>>),
    /// An expression whose static type the lowerer knows.
    ///
    /// Codegen needs this to choose the right runtime function for `print`, and
    /// Bools in particular cannot be recovered from their C representation.
    Typed {
        expr: Box<SirExpr>,
        typ: String,
    },
    /// Tuple construction: `(a, b)`. `name` is the generated struct type.
    Tuple {
        name: String,
        items: Vec<SirExpr>,
    },
    /// A closure value: the lifted function plus its heap environment
    /// (`None` when it captures nothing).
    Closure {
        name: String,
        env: Option<Box<SirExpr>>,
    },
    /// Read a captured variable out of a closure's environment.
    EnvAccess {
        env_type: String,
        field: String,
    },
    /// Call a closure value.
    CallValue {
        callee: Box<SirExpr>,
        args: Vec<SirExpr>,
        return_type: String,
        params: Vec<String>,
    },
    /// `sizeof(Type)`, needed to copy a closure environment onto the heap.
    SizeOf(String),
    /// Explicit conversion, needed to pass an environment struct as `void *`.
    Cast {
        value: Box<SirExpr>,
        typ: String,
    },
    /// Struct construction: `Name { field: value, ... }`.
    StructLiteral {
        name: String,
        fields: Vec<(String, SirExpr)>,
    },
    /// Field access: `object.field`.
    FieldAccess {
        object: Box<SirExpr>,
        field: String,
        /// Declared struct type of `object`; see `FieldAssign::struct_type`.
        struct_type: String,
    },
}

/// Function names reachable from `main`/`__main`, or `None` when the program
/// has no entry point. Backends use this to avoid emitting imported-but-unused
/// helpers that they cannot translate.
pub fn reachable_functions(program: &SirProgram) -> Option<std::collections::HashSet<String>> {
    let entry = if program.functions.iter().any(|f| f.name == "main") {
        "main".to_string()
    } else if program.functions.iter().any(|f| f.name == "__main") {
        "__main".to_string()
    } else {
        return None;
    };

    let mut reachable = std::collections::HashSet::new();
    let mut stack = vec![entry];
    while let Some(name) = stack.pop() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        if let Some(func) = program.functions.iter().find(|f| f.name == name) {
            collect_callees(&func.body, &mut stack);
        }
    }
    Some(reachable)
}

fn collect_callees(stmts: &[SirStatement], calls: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            SirStatement::Let { value, .. }
            | SirStatement::Expr(value)
            | SirStatement::Return(Some(value)) => collect_callees_expr(value, calls),
            SirStatement::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_callees_expr(condition, calls);
                collect_callees(then_block, calls);
                if let Some(else_block) = else_block {
                    collect_callees(else_block, calls);
                }
            }
            SirStatement::While { condition, body } => {
                collect_callees_expr(condition, calls);
                collect_callees(body, calls);
            }
            SirStatement::For { iterable, body, .. } => {
                collect_callees_expr(iterable, calls);
                collect_callees(body, calls);
            }
            SirStatement::Switch {
                scrutinee,
                cases,
                default,
            } => {
                collect_callees_expr(scrutinee, calls);
                for (constant, body) in cases {
                    collect_callees_expr(constant, calls);
                    collect_callees(body, calls);
                }
                if let Some(default) = default {
                    collect_callees(default, calls);
                }
            }
            SirStatement::Block(body) => collect_callees(body, calls),
            SirStatement::Return(None) | SirStatement::Break | SirStatement::Continue => {}
        }
    }
}

fn collect_callees_expr(expr: &SirExpr, calls: &mut Vec<String>) {
    match expr {
        SirExpr::Call { callee, args } => {
            calls.push(callee.clone());
            for arg in args {
                collect_callees_expr(arg, calls);
            }
        }
        SirExpr::Typed { expr, .. }
        | SirExpr::Cast { value: expr, .. }
        | SirExpr::UnaryOp { operand: expr, .. }
        | SirExpr::AddressOf(expr)
        | SirExpr::Deref(expr) => collect_callees_expr(expr, calls),
        SirExpr::BinOp { left, right, .. } => {
            collect_callees_expr(left, calls);
            collect_callees_expr(right, calls);
        }
        SirExpr::CallValue { args, .. } => {
            for arg in args {
                collect_callees_expr(arg, calls);
            }
        }
        SirExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_callees_expr(condition, calls);
            collect_callees_expr(then_expr, calls);
            if let Some(else_expr) = else_expr {
                collect_callees_expr(else_expr, calls);
            }
        }
        SirExpr::Block(stmts, final_expr) => {
            collect_callees(stmts, calls);
            if let Some(final_expr) = final_expr {
                collect_callees_expr(final_expr, calls);
            }
        }
        SirExpr::Assign { value, .. } => collect_callees_expr(value, calls),
        SirExpr::Array(items) | SirExpr::Tuple { items, .. } => {
            for item in items {
                collect_callees_expr(item, calls);
            }
        }
        SirExpr::Index { array, index } => {
            collect_callees_expr(array, calls);
            collect_callees_expr(index, calls);
        }
        SirExpr::IndexAssign {
            array,
            index,
            value,
        } => {
            collect_callees_expr(array, calls);
            collect_callees_expr(index, calls);
            collect_callees_expr(value, calls);
        }
        SirExpr::StringSlice { value, start, end } => {
            collect_callees_expr(value, calls);
            collect_callees_expr(start, calls);
            collect_callees_expr(end, calls);
        }
        SirExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_callees_expr(value, calls);
            }
        }
        SirExpr::FieldAccess { object, .. } => collect_callees_expr(object, calls),
        SirExpr::FieldAssign {
            object, value, ..
        } => {
            collect_callees_expr(object, calls);
            collect_callees_expr(value, calls);
        }
        SirExpr::Closure { name, env } => {
            calls.push(name.clone());
            if let Some(env) = env {
                collect_callees_expr(env, calls);
            }
        }
        SirExpr::CallValue { callee, args, .. } => {
            collect_callees_expr(callee, calls);
            for arg in args {
                collect_callees_expr(arg, calls);
            }
        }
        SirExpr::EnvAccess { .. }
        | SirExpr::SizeOf(_)
        | SirExpr::Literal(_)
        | SirExpr::Var(_) => {}
    }
}

/// The C struct name for a tuple of `types`, e.g. `NovaTuple_int_string`.
pub fn tuple_struct_name(types: &[String]) -> String {
    let mangled: Vec<String> = types
        .iter()
        .map(|typ| {
            typ.chars()
                .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
                .collect::<String>()
                .trim_matches('_')
                .to_string()
        })
        .collect();
    format!("NovaTuple_{}", mangled.join("_"))
}

/// Split a tuple type `(A, B)` into its top-level element types.
pub fn tuple_element_types(typ: &str) -> Option<Vec<String>> {
    let inner = typ.trim().strip_prefix('(')?.strip_suffix(')')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut elements = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in inner.chars() {
        match ch {
            '(' | '<' | '[' => depth += 1,
            ')' | '>' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                elements.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    elements.push(current.trim().to_string());
    Some(elements)
}

/// SIR Literal
#[derive(Debug, Clone)]
pub enum SirLiteral {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Char(char),
    Unit,
}

/// SIR Binary Operators
#[derive(Debug, Clone, Copy)]
pub enum SirBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Range,
}

/// SIR Unary Operators
#[derive(Debug, Clone, Copy)]
pub enum SirUnOp {
    Neg,
    Not,
}
