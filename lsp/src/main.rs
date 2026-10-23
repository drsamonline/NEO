//! NOVA Language Server
//! 
//! Implements the Language Server Protocol (LSP) for NOVA, providing
//! IDE features like autocomplete, go-to-definition, hover info, and diagnostics.

mod compiler_integration;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use crate::compiler_integration::{IncrementalCompiler, NovaSymbolKind};

#[derive(Error, Debug)]
pub enum LspError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
}

/// Document state
#[derive(Debug, Clone)]
struct Document {
    uri: Url,
    content: String,
    version: i32,
    diagnostics: Vec<Diagnostic>,
}

/// Language server backend
struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, Document>>>,
    compile_errors: Arc<RwLock<HashMap<Url, Vec<Diagnostic>>>>,
    compiler: Arc<RwLock<IncrementalCompiler>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Backend {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            compile_errors: Arc::new(RwLock::new(HashMap::new())),
            compiler: Arc::new(RwLock::new(IncrementalCompiler::new())),
        }
    }
    
    /// Analyze a document and return diagnostics using the real compiler
    async fn analyze_document(&self, uri: &Url, content: &str) -> Vec<Diagnostic> {
        let mut compiler = self.compiler.write().await;
        let result = compiler.compile(uri.as_str(), content);
        
        // Update compile errors cache
        let mut errors = self.compile_errors.write().await;
        errors.insert(uri.clone(), result.diagnostics.clone());
        
        result.diagnostics
    }

    /// Find symbol at position
    async fn find_symbol_at(&self, uri: &Url, position: Position) -> Option<String> {
        let documents = self.documents.read().await;
        let doc = documents.get(uri)?;
        
        let lines: Vec<&str> = doc.content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }
        
        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        
        if position.character as usize >= chars.len() {
            return None;
        }
        
        // Simple identifier extraction
        let mut start = position.character as usize;
        let mut end = position.character as usize;
        
        // Move back to start of identifier
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        
        // Move forward to end of identifier
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        
        if start < end {
            Some(chars[start..end].iter().collect())
        } else {
            None
        }
    }
    
    /// Find definition of a symbol
    async fn find_definition(&self, uri: &Url, position: Position) -> Option<Location> {
        // In production, this would use the compiler's symbol table
        // For now, return a placeholder
        let symbol = self.find_symbol_at(uri, position).await?;
        
        // Search for function/variable definition
        let documents = self.documents.read().await;
        
        for (doc_uri, doc) in documents.iter() {
            for (line_num, line) in doc.content.lines().enumerate() {
                if line.contains(&format!("fn {}", symbol)) || 
                   line.contains(&format!("var {}", symbol)) ||
                   line.contains(&format!("let {}", symbol)) {
                    return Some(Location {
                        uri: doc_uri.clone(),
                        range: Range {
                            start: Position {
                                line: line_num as u32,
                                character: 0,
                            },
                            end: Position {
                                line: line_num as u32,
                                character: line.len() as u32,
                            },
                        },
                    });
                }
            }
        }
        
        None
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("nova".to_string()),
                        inter_file_dependencies: true,
                        workspace_diagnostics: false,
                        work_done_progress_options: Default::default(),
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "nova-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "NOVA Language Server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let content = params.text_document.text.clone();
        let version = params.text_document.version;
        
        let mut documents = self.documents.write().await;
        
        let diagnostics = self.analyze_document(&uri, &content).await;
        
        documents.insert(
            uri.clone(),
            Document {
                uri: uri.clone(),
                content,
                version,
                diagnostics: diagnostics.clone(),
            },
        );
        
        drop(documents);
        
        // Publish diagnostics
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        
        let mut documents = self.documents.write().await;
        
        if let Some(doc) = documents.get_mut(&uri) {
            // Apply changes (simplified - just replace content)
            for change in params.content_changes {
                doc.content = change.text;
            }
            doc.version = version;
            
            let content = doc.content.clone();
            drop(documents);
            
            let diagnostics = self.analyze_document(&uri, &content).await;
            
            let mut documents = self.documents.write().await;
            if let Some(doc) = documents.get_mut(&uri) {
                doc.diagnostics = diagnostics.clone();
            }
            drop(documents);
            
            self.client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut documents = self.documents.write().await;
        documents.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        
        let completions = vec![
            CompletionItem {
                label: "fn".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("function keyword".to_string()),
                insert_text: Some("fn ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "let".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("variable declaration".to_string()),
                insert_text: Some("let ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "var".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("mutable variable".to_string()),
                insert_text: Some("var ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "if".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("conditional".to_string()),
                insert_text: Some("if ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "while".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("loop".to_string()),
                insert_text: Some("while ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "for".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("for loop".to_string()),
                insert_text: Some("for ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "return".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("return statement".to_string()),
                insert_text: Some("return ".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "print".to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("built-in print function".to_string()),
                insert_text: Some("print()".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "String".to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("built-in String type".to_string()),
                insert_text: Some("String".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "Int".to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("built-in Int type".to_string()),
                insert_text: Some("Int".to_string()),
                ..Default::default()
            },
        ];
        
        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        
        if let Some(symbol) = self.find_symbol_at(&uri, position).await {
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "Symbol: `{}`",
                    symbol
                ))),
                range: None,
            }));
        }
        
        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        
        if let Some(location) = self.find_definition(&uri, position).await {
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }
        
        Ok(None)
    }

    async fn references(&self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        // TODO: Implement reference finding
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        
        if let Some(doc) = documents.get(&uri) {
            let mut symbols = Vec::new();
            
            // Extract function definitions
            for (line_num, line) in doc.content.lines().enumerate() {
                if let Some(start) = line.find("fn ") {
                    if let Some(end) = line[start..].find('(') {
                        let name = line[start + 3..start + end].trim().to_string();
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some("function".to_string()),
                            kind: SymbolKind::FUNCTION,
                            deprecated: None,
                            range: Range {
                                start: Position {
                                    line: line_num as u32,
                                    character: 0,
                                },
                                end: Position {
                                    line: line_num as u32,
                                    character: line.len() as u32,
                                },
                            },
                            selection_range: Range {
                                start: Position {
                                    line: line_num as u32,
                                    character: start as u32,
                                },
                                end: Position {
                                    line: line_num as u32,
                                    character: (start + end) as u32,
                                },
                            },
                            children: None,
                            tags: None,
                        });
                    }
                }
            }
            
            return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
        }
        
        Ok(None)
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        // Search all open documents' symbol tables for names matching the query.
        let query = params.query.to_lowercase();
        let compiler = self.compiler.read().await;
        let mut results = Vec::new();

        for uri_str in compiler.cached_uris() {
            let Some(cached) = compiler.cached_result(&uri_str) else {
                continue;
            };
            let Ok(uri) = Url::parse(&uri_str) else {
                continue;
            };
            for entries in cached.symbols.symbols.values() {
                for entry in entries {
                    if !query.is_empty() && !entry.name.to_lowercase().contains(&query) {
                        continue;
                    }
                    results.push(SymbolInformation {
                        name: entry.name.clone(),
                        kind: entry.kind.to_lsp(),
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: uri.clone(),
                            range: Range {
                                start: Position {
                                    line: entry.line,
                                    character: entry.character,
                                },
                                end: Position {
                                    line: entry.line,
                                    character: entry.character + entry.name.len() as u32,
                                },
                            },
                        },
                        container_name: Some(entry.scope.clone()),
                    });
                }
            }
        }

        Ok(Some(results))
    }

    async fn diagnostic(&self, params: DocumentDiagnosticParams) -> Result<DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        
        if let Some(doc) = documents.get(&uri) {
            let report = DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    uri: uri.clone(),
                    items: doc.diagnostics.clone(),
                },
            });
            
            return Ok(DocumentDiagnosticReportResult::Report(report));
        }
        
        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    uri,
                    items: vec![],
                },
            }),
        ))
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    eprintln!("Starting NOVA Language Server...");
    
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    
    let (service, socket) = LspService::new(|client| Backend::new(client));
    
    Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
    
    Ok(())
}
