//! AST node definitions

use crate::span::Span;
use crate::types::Type;

/// Trait for all AST nodes
pub trait AstNode {
    fn span(&self) -> Span;
}

/// A complete program (compilation unit)
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

impl AstNode for Program {
    fn span(&self) -> Span {
        self.span
    }
}

/// Top-level item in a program
#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Enum(EnumDef),
    Const(ConstDef),
    TypeAlias(TypeAlias),
    Use(UseDecl),
    Mod(ModuleDecl),
    Impl(ImplBlock),
    Trait(TraitDef),
}

impl AstNode for Item {
    fn span(&self) -> Span {
        match self {
            Item::Function(f) => f.span,
            Item::Struct(s) => s.span,
            Item::Enum(e) => e.span,
            Item::Const(c) => c.span,
            Item::TypeAlias(t) => t.span,
            Item::Use(u) => u.span,
            Item::Mod(m) => m.span,
            Item::Impl(i) => i.span,
            Item::Trait(t) => t.span,
        }
    }
}

/// Function definition
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Option<Block>,
    pub is_pub: bool,
    pub span: Span,
}

impl AstNode for Function {
    fn span(&self) -> Span {
        self.span
    }
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub typ: TypeExpr,
    pub span: Span,
}

impl AstNode for Param {
    fn span(&self) -> Span {
        self.span
    }
}

/// Block of statements
#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub final_expr: Option<Box<Expr>>,
    pub span: Span,
}

impl AstNode for Block {
    fn span(&self) -> Span {
        self.span
    }
}

/// Statement in a block
#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStmt),
    Expr(Expr),
    Item(Item),
}

impl AstNode for Statement {
    fn span(&self) -> Span {
        match self {
            Statement::Let(l) => l.span,
            Statement::Expr(e) => e.span(),
            Statement::Item(i) => i.span(),
        }
    }
}

/// Let statement
#[derive(Debug, Clone)]
pub struct LetStmt {
    pub pattern: Pattern,
    pub typ: Option<TypeExpr>,
    pub init: Option<Expr>,
    pub is_mut: bool,
    pub span: Span,
}

impl AstNode for LetStmt {
    fn span(&self) -> Span {
        self.span
    }
}

/// Pattern for matching
#[derive(Debug, Clone)]
pub enum Pattern {
    Identifier(String),
    Tuple(Vec<Pattern>),
    Struct(String, Vec<(String, Pattern)>),
    EnumVariant(String, String, Option<Box<Pattern>>),
    Wildcard,
    Literal(Literal),
}

/// Type expression
#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub name: String,
    pub generics: Vec<TypeExpr>,
    pub span: Span,
}

impl AstNode for TypeExpr {
    fn span(&self) -> Span {
        self.span
    }
}

/// Expression
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Identifier(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Block, Option<Block>),
    Match(Box<Expr>, Vec<MatchArm>),
    While(Box<Expr>, Block),
    For(Pattern, Box<Expr>, Block),
    Loop(Block),
    Block(Block),
    Return(Option<Box<Expr>>),
    Break,
    Continue,
    Assign(Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, TypeExpr),
    Path(Vec<String>),
}

impl AstNode for Expr {
    fn span(&self) -> Span {
        // Simplified - would need proper span tracking in production
        Span::zero()
    }
}

/// Match arm
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

/// Literal value
#[derive(Debug, Clone)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
}

/// Binary operators
#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add, Sub, Mul, Div, Rem,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

/// Unary operators
#[derive(Debug, Clone, Copy)]
pub enum UnOp {
    Neg, Not, Deref, AddrOf,
}

/// Struct definition
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<Field>,
    pub is_pub: bool,
    pub span: Span,
}

impl AstNode for StructDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// Struct field
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub typ: TypeExpr,
    pub is_pub: bool,
    pub span: Span,
}

/// Enum definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub is_pub: bool,
    pub span: Span,
}

impl AstNode for EnumDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Option<Vec<TypeExpr>>,
    pub span: Span,
}

/// Constant definition
#[derive(Debug, Clone)]
pub struct ConstDef {
    pub name: String,
    pub typ: Option<TypeExpr>,
    pub value: Expr,
    pub is_pub: bool,
    pub span: Span,
}

/// Type alias
#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: String,
    pub generics: Vec<String>,
    pub typ: TypeExpr,
    pub is_pub: bool,
    pub span: Span,
}

/// Use declaration
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
}

/// Module declaration
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub is_pub: bool,
    pub span: Span,
}

/// Impl block
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub type_name: String,
    pub generics: Vec<String>,
    pub methods: Vec<Function>,
    pub span: Span,
}

/// Trait definition
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: String,
    pub generics: Vec<String>,
    pub methods: Vec<Function>,
    pub is_pub: bool,
    pub span: Span,
}
