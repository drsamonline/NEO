#![forbid(unsafe_code)]
// This crate has no `unsafe` in it, and this line keeps it that way: a
// future `unsafe` block fails the build instead of arriving in review.
// The runtime crate is the one exception and says why in its own source.

pub mod lexer;
pub mod parser;
pub mod ast;
pub mod semantic;

pub mod diagnostics;
pub mod sema;
pub use nova_common::sir;
pub mod backend;
pub mod lowerer;
pub mod syntax;
pub mod fmt;
pub mod modules;
pub mod runtime;

// Re-export main components for easy access
pub use lexer::Lexer;
pub use parser::Parser;
pub use sema::SemanticAnalyzer;
pub use sir::{SirProgram, SirFunction, SirStatement, SirExpr};
pub use backend::CBackend;
pub use lowerer::Lowerer;
