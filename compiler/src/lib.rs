pub mod lexer;
pub mod parser;
pub mod ast;
pub mod semantic;
pub mod sir;
pub mod diagnostics;
pub mod sema;
pub mod backend;
pub mod lowerer;

// Re-export main components for easy access
pub use lexer::Lexer;
pub use parser::Parser;
pub use sema::SemanticAnalyzer;
pub use sir::{SirProgram, SirFunction, SirStatement, SirExpr};
pub use backend::CBackend;
pub use lowerer::Lowerer;
