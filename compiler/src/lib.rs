pub mod lexer;
pub mod parser;
pub mod ast;
pub mod semantic;
pub mod sir;
pub mod diagnostics;
pub mod sema;

// Re-export main components for easy access
pub use lexer::Lexer;
pub use parser::Parser;
pub use sema::SemanticAnalyzer;
