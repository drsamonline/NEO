//! Compiler integration for Language Server
//!
//! This module provides incremental compilation support for the LSP,
//! allowing real-time diagnostics and symbol resolution. It drives the
//! actual NOVA compiler pipeline (lexer → parser → semantic analysis →
//! lowering to SIR) and converts compiler diagnostics into LSP diagnostics.

use std::collections::HashMap;
use nova_compiler::diagnostics::{Diagnostic as CompilerDiagnostic, DiagnosticLevel};
use nova_compiler::{Lexer, Lowerer, Parser, SemanticAnalyzer, SirProgram};
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
    pub kind: NovaSymbolKind,
    pub line: u32,
    pub character: u32,
    pub type_name: Option<String>,
    pub scope: String,
}

/// Symbol kinds supported by NOVA
#[derive(Debug, Clone, PartialEq)]
pub enum NovaSymbolKind {
    Function,
    Variable,
    Parameter,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
}

impl NovaSymbolKind {
    /// Map to the corresponding LSP symbol kind for document symbol responses.
    pub fn to_lsp(&self) -> tower_lsp::lsp_types::SymbolKind {
        match self {
            NovaSymbolKind::Function => tower_lsp::lsp_types::SymbolKind::FUNCTION,
            NovaSymbolKind::Variable => tower_lsp::lsp_types::SymbolKind::VARIABLE,
            NovaSymbolKind::Parameter => tower_lsp::lsp_types::SymbolKind::TYPE_PARAMETER,
            NovaSymbolKind::Struct => tower_lsp::lsp_types::SymbolKind::STRUCT,
            NovaSymbolKind::Enum => tower_lsp::lsp_types::SymbolKind::ENUM,
            NovaSymbolKind::Trait => tower_lsp::lsp_types::SymbolKind::INTERFACE,
            NovaSymbolKind::Module => tower_lsp::lsp_types::SymbolKind::MODULE,
            NovaSymbolKind::Constant => tower_lsp::lsp_types::SymbolKind::CONSTANT,
        }
    }
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
        self.scopes
            .entry(entry.scope.clone())
            .or_insert_with(Vec::new)
            .push(entry.name.clone());
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

        // Phase 1: Lexing — the lexer collects errors into its Diagnostics
        // container and returns (tokens, &diagnostics).
        let mut lexer = Lexer::new(content);
        let (tokens, lex_diags) = lexer.tokenize();
        for diag in lex_diags.iter() {
            diagnostics.push(self.convert_diagnostic(diag, content));
        }
        if lex_diags.has_errors() {
            let result = CompileResult {
                diagnostics,
                sir,
                symbols,
            };
            self.cache.insert(uri.to_string(), result.clone());
            return result;
        }

        // Phase 2: Parsing
        let mut parser = Parser::new(tokens);
        let ast = match parser.parse() {
            Ok(ast) => ast,
            Err(errs) => {
                for diag in errs.iter() {
                    diagnostics.push(self.convert_diagnostic(diag, content));
                }
                let result = CompileResult {
                    diagnostics,
                    sir,
                    symbols,
                };
                self.cache.insert(uri.to_string(), result.clone());
                return result;
            }
        };

        // Extract symbols from AST (works even if later phases fail)
        self.extract_symbols(&ast, &mut symbols);

        // Phase 3: Semantic Analysis
        let mut sema = SemanticAnalyzer::new();
        if sema.analyze(&ast).is_err() {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E0001".to_string())),
                code_description: None,
                source: Some("nova".to_string()),
                message: "semantic analysis failed".to_string(),
                related_information: None,
                tags: None,
                data: None,
            });
        }

        // Phase 4: Lowering to SIR (only if no fatal errors so far)
        if diagnostics
            .iter()
            .all(|d| d.severity != Some(DiagnosticSeverity::ERROR))
        {
            let mut lowerer = Lowerer::new();
            sir = Some(lowerer.lower_program(&ast));
        }

        let result = CompileResult {
            diagnostics,
            sir,
            symbols,
        };
        self.cache.insert(uri.to_string(), result.clone());
        result
    }

    /// Convert compiler diagnostic to LSP diagnostic
    fn convert_diagnostic(&self, diag: &CompilerDiagnostic, content: &str) -> Diagnostic {
        let severity = match diag.level {
            DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
            DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
            DiagnosticLevel::Info => DiagnosticSeverity::INFORMATION,
            DiagnosticLevel::Note => DiagnosticSeverity::INFORMATION,
            DiagnosticLevel::Help => DiagnosticSeverity::HINT,
        };

        // The compiler uses 1-based line/column positions; LSP is 0-based.
        let (start_line, start_char, end_line, end_char) = match &diag.span {
            Some(span) => (
                span.start.line.saturating_sub(1) as u32,
                span.start.column.saturating_sub(1) as u32,
                span.end.line.saturating_sub(1) as u32,
                span.end.column.saturating_sub(1) as u32,
            ),
            None => (0, 0, 0, 0),
        };

        // Clamp the end of the range to the actual line length when available.
        let lines: Vec<&str> = content.lines().collect();
        let end_char = if (start_line as usize) < lines.len() {
            let line_len = lines[start_line as usize].chars().count() as u32;
            end_char.min(line_len.max(start_char))
        } else {
            end_char
        };

        Diagnostic {
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_char,
                },
                end: Position {
                    line: end_line,
                    character: end_char.max(start_char),
                },
            },
            severity: Some(severity),
            code: diag.code.clone().map(NumberOrString::String),
            code_description: None,
            source: Some("nova".to_string()),
            message: diag.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Extract symbols from AST
    fn extract_symbols(&self, ast: &nova_common::Program, symbols: &mut SymbolTable) {
        use nova_common::ast::Item;

        for item in &ast.items {
            match item {
                Item::Function(func) => {
                    symbols.add(SymbolEntry {
                        name: func.name.clone(),
                        kind: NovaSymbolKind::Function,
                        line: func.span.start.line.saturating_sub(1) as u32,
                        character: func.span.start.column.saturating_sub(1) as u32,
                        type_name: func.return_type.as_ref().map(|t| t.name.clone()),
                        scope: "global".to_string(),
                    });

                    // Add parameters
                    for param in &func.params {
                        symbols.add(SymbolEntry {
                            name: param.name.clone(),
                            kind: NovaSymbolKind::Parameter,
                            line: param.span.start.line.saturating_sub(1) as u32,
                            character: param.span.start.column.saturating_sub(1) as u32,
                            type_name: Some(param.typ.name.clone()),
                            scope: format!("fn_{}", func.name),
                        });
                    }
                }
                Item::Const(const_def) => {
                    symbols.add(SymbolEntry {
                        name: const_def.name.clone(),
                        kind: NovaSymbolKind::Constant,
                        line: const_def.span.start.line.saturating_sub(1) as u32,
                        character: const_def.span.start.column.saturating_sub(1) as u32,
                        type_name: const_def.typ.as_ref().map(|t| t.name.clone()),
                        scope: "global".to_string(),
                    });
                }
                Item::Struct(struct_def) => {
                    symbols.add(SymbolEntry {
                        name: struct_def.name.clone(),
                        kind: NovaSymbolKind::Struct,
                        line: struct_def.span.start.line.saturating_sub(1) as u32,
                        character: struct_def.span.start.column.saturating_sub(1) as u32,
                        type_name: None,
                        scope: "global".to_string(),
                    });
                }
                Item::Enum(enum_def) => {
                    symbols.add(SymbolEntry {
                        name: enum_def.name.clone(),
                        kind: NovaSymbolKind::Enum,
                        line: enum_def.span.start.line.saturating_sub(1) as u32,
                        character: enum_def.span.start.column.saturating_sub(1) as u32,
                        type_name: None,
                        scope: "global".to_string(),
                    });
                }
                Item::Trait(trait_def) => {
                    symbols.add(SymbolEntry {
                        name: trait_def.name.clone(),
                        kind: NovaSymbolKind::Trait,
                        line: trait_def.span.start.line.saturating_sub(1) as u32,
                        character: trait_def.span.start.column.saturating_sub(1) as u32,
                        type_name: None,
                        scope: "global".to_string(),
                    });
                }
                Item::Mod(module_decl) => {
                    symbols.add(SymbolEntry {
                        name: module_decl.name.clone(),
                        kind: NovaSymbolKind::Module,
                        line: module_decl.span.start.line.saturating_sub(1) as u32,
                        character: module_decl.span.start.column.saturating_sub(1) as u32,
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

    /// List all cached document URIs
    pub fn cached_uris(&self) -> Vec<String> {
        self.cache.keys().cloned().collect()
    }

    /// Get a cached compile result for a URI, if present
    pub fn cached_result(&self, uri: &str) -> Option<&CompileResult> {
        self.cache.get(uri)
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
    fn test_incremental_compiler_valid_program() {
        let mut compiler = IncrementalCompiler::new();
        let source = r#"
fn main() -> i32 {
    let x: i32 = 42;
    return x;
}
"#;

        let result = compiler.compile("test.nova", source);
        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.sir.is_some());
        assert!(result.symbols.get("main").is_some());
    }

    #[test]
    fn test_error_detection() {
        let mut compiler = IncrementalCompiler::new();
        // Unterminated string should produce a lexer diagnostic.
        let source = r#"
fn main() {
    let s = "unterminated;
}
"#;

        let result = compiler.compile("bad.nova", source);
        assert!(
            !result.diagnostics.is_empty(),
            "expected at least one diagnostic"
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)));
    }

    #[test]
    fn test_cache_invalidation() {
        let mut compiler = IncrementalCompiler::new();
        let source = "fn foo() { }\n";
        let r1 = compiler.compile("cached.nova", source);
        assert!(r1.symbols.get("foo").is_some());

        compiler.invalidate("cached.nova");
        let r2 = compiler.compile("cached.nova", "fn bar() { }\n");
        assert!(r2.symbols.get("bar").is_some());
        assert!(r2.symbols.get("foo").is_none());
    }
}
