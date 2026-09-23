//! Compiler integration for Language Server
//!
//! This module provides incremental compilation support for the LSP,
//! allowing real-time diagnostics and symbol resolution.

use std::collections::HashMap;
use std::path::PathBuf;
use nova_compiler::{Lexer, Parser, SemanticAnalyzer, Lowerer, SirProgram};
use nova_compiler::diagnostics::{Diagnostic as CompilerDiagnostic, DiagnosticLevel};
use tower_lsp::lsp_types::*;

/// Result of compiling a document
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub diagnostics: Vec<Diagnostic>,
    pub sir: Option<SirProgram>,
    pub symbols: SymbolTable,
}

/// Symbol table entry
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub character: u32,
    pub type_name: Option<String>,
    pub scope: String,
}

/// Symbol kinds supported by NOVA
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Variable,
    Parameter,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
}

/// Symbol table for quick lookups
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    pub symbols: HashMap<String, Vec<SymbolEntry>>,
    pub scopes: HashMap<String, Vec<String>>, // scope -> symbols in that scope
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            scopes: HashMap::new(),
        }
    }

    pub fn add(&mut self, entry: SymbolEntry) {
        self.symbols
            .entry(entry.name.clone())
            .or_insert_with(Vec::new)
            .push(entry);
    }

    pub fn get(&self, name: &str) -> Option<&Vec<SymbolEntry>> {
        self.symbols.get(name)
    }

    pub fn find_in_scope(&self, name: &str, scope: &str) -> Option<&SymbolEntry> {
        if let Some(entries) = self.symbols.get(name) {
            return entries.iter().find(|e| e.scope == scope);
        }
        None
    }
}

/// Incremental compiler for LSP
pub struct IncrementalCompiler {
    cache: HashMap<String, CompileResult>,
}

impl IncrementalCompiler {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Compile a document and return diagnostics + symbols
    pub fn compile(&mut self, uri: &str, content: &str) -> CompileResult {
        // Check cache first
        if let Some(cached) = self.cache.get(uri) {
            return cached.clone();
        }

        let mut diagnostics = Vec::new();
        let mut symbols = SymbolTable::new();
        let mut sir = None;

        // Phase 1: Lexing
        let lexer = Lexer::new(content);
        let tokens = match lexer.tokenize() {
            Ok(tokens) => tokens,
            Err(errs) => {
                for err in errs {
                    diagnostics.push(self.convert_diagnostic(&err, content));
                }
                return CompileResult { diagnostics, sir, symbols };
            }
        };

        // Phase 2: Parsing
        let mut parser = Parser::new(tokens);
        let ast = match parser.parse() {
            Ok(ast) => ast,
            Err(errs) => {
                for err in errs {
                    diagnostics.push(self.convert_diagnostic(&err, content));
                }
                return CompileResult { diagnostics, sir, symbols };
            }
        };

        // Phase 3: Semantic Analysis
        let mut sema = SemanticAnalyzer::new();
        let analyzed = match sema.analyze(&ast) {
            Ok(analyzed) => analyzed,
            Err(errs) => {
                for err in errs {
                    diagnostics.push(self.convert_diagnostic(&err, content));
                }
                // Continue to extract partial symbols even with errors
            }
        };

        // Extract symbols from AST
        self.extract_symbols(&ast, &mut symbols);

        // Phase 4: Lowering to SIR (only if no fatal errors)
        if diagnostics.iter().all(|d| d.severity != Some(DiagnosticSeverity::ERROR)) {
            let mut lowerer = Lowerer::new();
            match lowerer.lower(&ast) {
                Ok(sir_prog) => {
                    sir = Some(sir_prog);
                }
                Err(errs) => {
                    for err in errs {
                        diagnostics.push(self.convert_diagnostic(&err, content));
                    }
                }
            }
        }

        let result = CompileResult { diagnostics, sir, symbols };
        self.cache.insert(uri.to_string(), result.clone());
        result
    }

    /// Convert compiler diagnostic to LSP diagnostic
    fn convert_diagnostic(&self, diag: &CompilerDiagnostic, content: &str) -> Diagnostic {
        let severity = match diag.level {
            DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
            DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
            DiagnosticLevel::Info => DiagnosticSeverity::INFORMATION,
            DiagnosticLevel::Hint => DiagnosticSeverity::HINT,
        };

        let lines: Vec<&str> = content.lines().collect();
        let line_content = if diag.line as usize < lines.len() {
            lines[diag.line as usize]
        } else {
            ""
        };

        Diagnostic {
            range: Range {
                start: Position {
                    line: diag.line,
                    character: diag.column,
                },
                end: Position {
                    line: diag.line,
                    character: (diag.column + diag.length.unwrap_or(0)).max(diag.column),
                },
            },
            severity: Some(severity),
            code: Some(NumberOrString::String(diag.code.clone())),
            code_description: None,
            source: Some("nova".to_string()),
            message: diag.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Extract symbols from AST
    fn extract_symbols(&self, ast: &nova_compiler::ast::Program, symbols: &mut SymbolTable) {
        use nova_compiler::ast::*;

        for item in &ast.items {
            match item {
                Item::Function(func) => {
                    symbols.add(SymbolEntry {
                        name: func.name.clone(),
                        kind: SymbolKind::Function,
                        line: func.span.start.line,
                        character: func.span.start.column,
                        type_name: func.return_type.as_ref().map(|t| t.to_string()),
                        scope: "global".to_string(),
                    });

                    // Add parameters
                    for param in &func.params {
                        symbols.add(SymbolEntry {
                            name: param.name.clone(),
                            kind: SymbolKind::Parameter,
                            line: param.span.start.line,
                            character: param.span.start.column,
                            type_name: Some(param.type_ann.to_string()),
                            scope: format!("fn_{}", func.name),
                        });
                    }
                }
                Item::Var(var) => {
                    symbols.add(SymbolEntry {
                        name: var.name.clone(),
                        kind: SymbolKind::Variable,
                        line: var.span.start.line,
                        character: var.span.start.column,
                        type_name: Some(var.type_ann.to_string()),
                        scope: "global".to_string(),
                    });
                }
                Item::Struct(struct_def) => {
                    symbols.add(SymbolEntry {
                        name: struct_def.name.clone(),
                        kind: SymbolKind::Struct,
                        line: struct_def.span.start.line,
                        character: struct_def.span.start.column,
                        type_name: None,
                        scope: "global".to_string(),
                    });
                }
                Item::Enum(enum_def) => {
                    symbols.add(SymbolEntry {
                        name: enum_def.name.clone(),
                        kind: SymbolKind::Enum,
                        line: enum_def.span.start.line,
                        character: enum_def.span.start.column,
                        type_name: None,
                        scope: "global".to_string(),
                    });
                }
                _ => {}
            }
        }
    }

    /// Invalidate cache for a URI
    pub fn invalidate(&mut self, uri: &str) {
        self.cache.remove(uri);
    }

    /// Clear all cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_compiler() {
        let mut compiler = IncrementalCompiler::new();
        let source = r#"
            fn main() -> i32 {
                var x: i32 = 42;
                return x;
            }
        "#;

        let result = compiler.compile("test.nova", source);
        assert!(result.sir.is_some());
        assert!(result.symbols.get("main").is_some());
        assert!(result.symbols.get("x").is_some());
    }

    #[test]
    fn test_error_detection() {
        let mut compiler = IncrementalCompiler::new();
        let source = r#"
            fn main() -> i32 {
                var x: i32 = "string";  // Type error
                return x;
            }
        "#;

        let result = compiler.compile("test.nova", source);
        assert!(!result.diagnostics.is_empty());
    }
}
