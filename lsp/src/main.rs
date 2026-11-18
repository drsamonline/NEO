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
use nova_common::{Span, Token, TokenKind};
use nova_compiler::syntax::{Item, ItemKind, ItemTree};
use nova_compiler::Lexer;

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

// ============================================================================
// Navigation
// ============================================================================
//
// Definition and references work on the compiler's own tokens, so every range is
// the exact span of an identifier rather than a whole line.
//
// What this is not: a resolver. Two identifiers with the same text are treated as
// the same symbol, so shadowing in nested scopes is not distinguished, and a field
// name that happens to match a local is not told apart from it. Doing that needs a
// lossless syntax tree with a scope map — the Stage 4 work in `DEV_PLAN.md`. This
// is the honest token-level version, and it is exact about *where*, which the
// previous implementation was not: it returned the whole line, starting at column
// zero, for any line containing the text `fn name`.

/// Every token of a document, with the compiler's spans.
fn tokens_of(source: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(source);
    let (tokens, _diagnostics) = lexer.tokenize();
    tokens
}

/// NOVA counts lines and columns from 1; the protocol counts from 0.
fn to_lsp(token: &Token) -> Option<Range> {
    if token.span.start.line == 0 || token.span.start.column == 0 {
        return None;
    }
    let line = (token.span.start.line - 1) as u32;
    let start = (token.span.start.column - 1) as u32;
    // The lexeme is the token's own text, so its length is the range's width. It
    // is counted in characters, which matches the protocol for the ASCII
    // identifiers that make up almost all of this language; a non-ASCII
    // identifier is off by the difference between chars and UTF-16 units.
    let width = token.lexeme.chars().count() as u32;
    Some(Range {
        start: Position { line, character: start },
        end: Position { line, character: start + width },
    })
}

/// A `Span` as a protocol range.
///
/// The lexer records `end` as the position just past the last character, which is
/// exactly what a range's exclusive end wants, so this needs no width arithmetic.
fn span_to_lsp(span: &Span) -> Option<Range> {
    if span.start.line == 0 || span.start.column == 0 {
        return None;
    }
    Some(Range {
        start: Position {
            line: (span.start.line - 1) as u32,
            character: (span.start.column - 1) as u32,
        },
        end: Position {
            line: (span.end.line - 1) as u32,
            character: (span.end.column - 1) as u32,
        },
    })
}

/// The outline kind for an item.
///
/// `use` is deliberately absent: a list of imports is not an outline, and showing
/// one buries the declarations a reader is looking for. It stays in the item tree
/// for whatever wants it.
fn symbol_kind_for(kind: ItemKind) -> Option<SymbolKind> {
    Some(match kind {
        ItemKind::Function => SymbolKind::FUNCTION,
        ItemKind::Struct => SymbolKind::STRUCT,
        ItemKind::Enum => SymbolKind::ENUM,
        ItemKind::Const => SymbolKind::CONSTANT,
        ItemKind::TypeAlias => SymbolKind::TYPE_PARAMETER,
        ItemKind::Module => SymbolKind::MODULE,
        ItemKind::Impl => SymbolKind::CLASS,
        ItemKind::Use => return None,
    })
}

/// One item as an outline entry. An `impl`'s methods are expanded here, on demand,
/// which is the whole point of deferring bodies: a file-level search never pays
/// for them.
fn symbol_for(tree: &ItemTree, item: &Item) -> Option<DocumentSymbol> {
    let kind = symbol_kind_for(item.kind)?;
    let range = span_to_lsp(&item.span)?;
    let selection_range = item.name_span.as_ref().and_then(span_to_lsp).unwrap_or(range);
    let name = item
        .name
        .clone()
        .unwrap_or_else(|| item.kind.label().to_string());

    let children = if item.kind == ItemKind::Impl {
        let nested: Vec<DocumentSymbol> = tree
            .children_of(item)
            .iter()
            .filter_map(|child| symbol_for(tree, child))
            .collect();
        if nested.is_empty() { None } else { Some(nested) }
    } else {
        None
    };

    Some(DocumentSymbol {
        name,
        detail: Some(item.kind.label().to_string()),
        kind,
        deprecated: None,
        range,
        selection_range,
        children,
        tags: None,
    })
}

/// The identifier the cursor is inside, if any.
fn identifier_at<'a>(tokens: &'a [Token], position: Position) -> Option<&'a Token> {
    tokens.iter().find(|token| {
        if !matches!(token.kind, TokenKind::Identifier(_)) {
            return false;
        }
        let Some(range) = to_lsp(token) else {
            return false;
        };
        if position.line != range.start.line {
            return false;
        }
        // Inclusive of the start, exclusive of the end, so a cursor just past the
        // last character is not "inside" the name.
        position.character >= range.start.character && position.character < range.end.character
    })
}

/// Whether a token kind introduces a declaration.
fn introduces_declaration(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Fn
            | TokenKind::Let
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Const
            | TokenKind::Type
            | TokenKind::Mod
            // `let mut x` puts `mut` between the keyword and the name.
            | TokenKind::Mut
    )
}

/// The first declaration of `name` in the token stream.
fn declaration_of<'a>(tokens: &'a [Token], name: &str) -> Option<&'a Token> {
    tokens
        .windows(2)
        .find(|pair| {
            matches!(&pair[1].kind, TokenKind::Identifier(text) if text.as_str() == name)
                && introduces_declaration(&pair[0].kind)
        })
        .map(|pair| &pair[1])
}

/// Every identifier token with this text.
fn occurrences_of<'a>(tokens: &'a [Token], name: &str) -> Vec<&'a Token> {
    tokens
        .iter()
        .filter(|token| matches!(&token.kind, TokenKind::Identifier(text) if text.as_str() == name))
        .collect()
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
        let documents = self.documents.read().await;

        // The document the cursor is in first: a declaration is usually there.
        let current = documents.get(uri)?;
        let tokens = tokens_of(&current.content);
        let name = identifier_at(&tokens, position)?.lexeme.clone();

        if let Some(declaration) = declaration_of(&tokens, &name) {
            return Some(Location {
                uri: uri.clone(),
                range: to_lsp(declaration)?,
            });
        }

        // Then every other open document, so a name imported from a sibling file
        // still resolves. This is the same cross-file behaviour as before, now
        // returning the identifier's span instead of the whole line.
        for (other_uri, document) in documents.iter() {
            if other_uri == uri {
                continue;
            }
            let other_tokens = tokens_of(&document.content);
            if let Some(declaration) = declaration_of(&other_tokens, &name) {
                return Some(Location {
                    uri: other_uri.clone(),
                    range: to_lsp(declaration)?,
                });
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
                // `did_change` applies each change as the document's new full text,
                // so advertise FULL sync; INCREMENTAL would make editors send partial
                // ranges that this server would treat as whole documents.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
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
                // Token-level, and honest about it: two identifiers with the
                // same text are treated as one symbol, so shadowing is not
                // resolved. See the Navigation section for what that costs and
                // which Stage 4 work would fix it.
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

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        let documents = self.documents.read().await;
        let Some(document) = documents.get(&uri) else {
            return Ok(None);
        };

        let tokens = tokens_of(&document.content);
        let Some(name) = identifier_at(&tokens, position).map(|token| token.lexeme.clone()) else {
            return Ok(None);
        };

        let declaration = declaration_of(&tokens, &name).map(|token| token.span);
        let mut locations: Vec<Location> = occurrences_of(&tokens, &name)
            .into_iter()
            .filter(|token| {
                // `include_declaration` is the editor's preference, and honouring
                // it is what stops "rename" from offering to rename the
                // declaration twice.
                include_declaration || Some(token.span) != declaration
            })
            .filter_map(|token| {
                to_lsp(token).map(|range| Location { uri: uri.clone(), range })
            })
            .collect();

        locations.sort_by_key(|location| (location.range.start.line, location.range.start.character));
        locations.dedup_by_key(|location| (location.range.start.line, location.range.start.character));

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&uri) else {
            return Ok(None);
        };

        // The item tree, not a line scan. It records where each top-level item is
        // and where its body is, without parsing inside it — so an outline costs
        // one pass over the tokens, and only an `impl` pays for its methods.
        let tree = ItemTree::parse(&document.content);
        let symbols: Vec<DocumentSymbol> = tree
            .items()
            .iter()
            .filter_map(|item| symbol_for(&tree, item))
            .collect();

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
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
                    // `uri` is NOT a field of FullDocumentDiagnosticReport. It belongs to
                    // the RelatedFullDocumentDiagnosticReport that wraps this one, which is
                    // what the code already constructs - so the field was both wrong and
                    // redundant. Keeping it made `cargo build` fail with E0560, and this
                    // workflow has failed 60 consecutive runs because of it.
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
