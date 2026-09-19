//! NOVA Common - Shared types and utilities for the NOVA compiler ecosystem

pub mod span;
pub mod token;
pub mod ast;
pub mod types;
pub mod diagnostics;
pub mod symbol_table;

pub use span::{Span, Position};
pub use token::{Token, TokenKind};
pub use ast::{AstNode, Program, Item, Function, Param, TypeExpr, Block, Statement, LetStmt, Pattern, Expr, BinOp, UnOp, Literal, StructDef, EnumDef, EnumVariant, Field, ConstDef, UseDecl, ModuleDecl, ImplBlock};
pub use types::{Type, TypeKind, PrimitiveType};
pub use diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};
pub use symbol_table::{SymbolTable, Symbol, SymbolKind};
