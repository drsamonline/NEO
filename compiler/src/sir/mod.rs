//! Semantic IR (Intermediate Representation)
//! 
//! SIR is a lowered representation of the AST that's closer to machine code
//! and easier to translate to backend languages like C.

use nova_common::Span;

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
    /// Address of a value, used to pass a `&self` receiver to a method.
    AddressOf(Box<SirExpr>),
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
    },
    /// Array literal: `[a, b, c]`.
    Array(Vec<SirExpr>),
    /// Array element read: `array[index]`.
    Index {
        array: Box<SirExpr>,
        index: Box<SirExpr>,
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
    /// Struct construction: `Name { field: value, ... }`.
    StructLiteral {
        name: String,
        fields: Vec<(String, SirExpr)>,
    },
    /// Field access: `object.field`.
    FieldAccess {
        object: Box<SirExpr>,
        field: String,
    },
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
