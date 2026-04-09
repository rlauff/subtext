// src/main.rs

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// Import scanner module
mod scanner;
use scanner::scan;

/// Defines the supported token types for the editor.
/// The indices of this array MUST EXACTLY match what `TokenType::as_lsp_index()` returns
/// in your scanner module.
fn get_supported_token_types() -> Vec<SemanticTokenType> {
    vec![
        SemanticTokenType::KEYWORD,   // Index 0: Def
        SemanticTokenType::FUNCTION,  // Index 1: NameAfterDef
        SemanticTokenType::OPERATOR,  // Index 2: Brace
        SemanticTokenType::OPERATOR,  // Index 3: InputPatternSeparator
        SemanticTokenType::OPERATOR,  // Index 4: PatternOutputSeparator
        SemanticTokenType::OPERATOR,  // Index 5: NewArmSeparator
        SemanticTokenType::FUNCTION,  // Index 6: FunctionCall
        SemanticTokenType::STRING,    // Index 7: RegisterCall
        SemanticTokenType::COMMENT,   // Index 8: Comment
        SemanticTokenType::KEYWORD,   // Index 9: GhostChar
        SemanticTokenType::PARAMETER, // Index 10: Input
        SemanticTokenType::REGEXP,    // Index 11: Pattern
        SemanticTokenType::STRING,    // Index 12: Output
    ]
}

/// The main state of the Language Server.
#[derive(Debug)]
struct Backend {
    /// The client handle used to send messages/logs back to the editor.
    client: Client,
    /// A thread-safe map storing the current text of all opened documents.
    /// Key: Document URI (as String), Value: Document Source Code.
    document_map: DashMap<String, String>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// Called when the editor first connects to the server.
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Tell the editor we want the FULL text every time the user types
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // Tell the editor we provide Semantic Tokens (Syntax Highlighting)
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                            legend: SemanticTokensLegend {
                                token_types: get_supported_token_types(),
                                token_modifiers: vec![], // No modifiers needed for now
                            },
                            range: Some(false), // We scan the whole document, not just ranges
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// Confirms initialization to the editor.
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Subtext Language Server initialized!")
            .await;
    }

    /// Handles clean shutdown of the server.
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    // --- DOCUMENT SYNCHRONIZATION ---

    /// Triggered when a file is opened in the editor.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text;
        self.document_map.insert(uri, text);
    }

    /// Triggered when the user types or modifies a file.
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        if let Some(change) = params.content_changes.into_iter().next() {
            self.document_map.insert(uri, change.text);
        }
    }

    /// Triggered when a file is closed.
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.document_map
            .remove(&params.text_document.uri.to_string());
    }

    // --- SEMANTIC TOKENS (SYNTAX HIGHLIGHTING) ---

    /// Triggered when the editor requests syntax highlighting for a document.
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.to_string();

        // 1. Retrieve the current text of the document.
        let text = match self.document_map.get(&uri) {
            Some(doc) => doc.value().clone(),
            None => return Ok(None),
        };

        // 2. Call scanner! It returns a flat Vec<u32>.
        let flat_tokens = scan(&text);

        // 3. Convert the flat array into the official LSP `SemanticToken` structs.
        // The LSP specification defines that every 5 integers represent one token.
        let mut lsp_tokens = Vec::with_capacity(flat_tokens.len() / 5);
        for chunk in flat_tokens.chunks_exact(5) {
            lsp_tokens.push(SemanticToken {
                delta_line: chunk[0],
                delta_start: chunk[1],
                length: chunk[2],
                token_type: chunk[3],
                token_modifiers_bitset: chunk[4],
            });
        }

        // 4. Return the colored tokens to the editor.
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: lsp_tokens,
        })))
    }
}

// --- ENTRY POINT ---

#[tokio::main]
async fn main() {
    // The language server communicates with the editor via standard input/output.
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: DashMap::new(),
    });

    // Start listening for editor requests
    Server::new(stdin, stdout, socket).serve(service).await;
}
