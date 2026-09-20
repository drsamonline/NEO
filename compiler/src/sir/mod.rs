//! Semantic IR (Intermediate Representation)
//! 
//! SIR is a lowered representation of the AST that's closer to machine code
//! and easier to translate to backend languages like C.

use nova_common::{Span, TypeExpr};

/// SIR Program - the root of the SIR tree
pub struct SirProgram {
    pub functions: Vec<SirFunction>,
    pub globals: Vec<SirGlobal>,
}

/// SIR Function
pub struct SirFunction {
    pub name: String,
    pub params: Vec<SirParam>,
    pub return_type: Option<String>,
    pub body: Vec<SirStatement>,
    pub span: Span,
}

/// SIR Function Parameter
pub struct SirParam {
    pub name: String,
    pub typ: String,
}

/// SIR Global Variable
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
    /// If expression (returns a value)
    If {
        condition: Box<SirExpr>,
        then_expr: Box<SirExpr>,
        else_expr: Option<Box<SirExpr>>,
    },
    /// Block expression (returns final value)
    Block(Vec<SirStatement>, Option<Box<SirExpr>>),
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
}

/// SIR Unary Operators
#[derive(Debug, Clone, Copy)]
pub enum SirUnOp {
    Neg,
    Not,
}
