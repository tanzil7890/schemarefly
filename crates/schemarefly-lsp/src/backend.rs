//! LSP backend implementation for SchemaRefly
//!
//! This module implements the Language Server Protocol for SchemaRefly,
//! providing real-time diagnostics, hover information, and go-to-definition
//! for dbt SQL files.

use schemarefly_core::{Config, Diagnostic as SchemaDiagnostic, Severity};
use schemarefly_incremental::{queries, SchemaReflyDatabase};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, MarkedString, MessageType,
    NumberOrString, OneOf, Position, Range, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

/// LSP backend for SchemaRefly
///
/// Tracks all open documents in the workspace and provides LSP features.
/// Creates a fresh Salsa database for each request (Salsa handles caching internally).
pub struct Backend {
    /// LSP client for communicating with the editor
    client: Client,
    /// Currently open documents (URI -> text content)
    documents: Arc<RwLock<HashMap<Url, String>>>,
    /// SchemaRefly configuration
    config: Arc<RwLock<Config>>,
    /// dbt manifest JSON (loaded from workspace)
    manifest_json: Arc<RwLock<Option<String>>>,
    /// Project root directory
    root_uri: Arc<RwLock<Option<Url>>>,
}

impl Backend {
    /// Create a new LSP backend
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(Config::default())),
            manifest_json: Arc::new(RwLock::new(None)),
            root_uri: Arc::new(RwLock::new(None)),
        }
    }

    /// Load dbt manifest from workspace
    async fn load_manifest(&self) -> Option<String> {
        let root_uri = self.root_uri.read().await;
        let root_path = root_uri.as_ref()?.to_file_path().ok()?;

        // Try to find manifest.json in target/ directory
        let manifest_path = root_path.join("target").join("manifest.json");

        match tokio::fs::read_to_string(&manifest_path).await {
            Ok(content) => {
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Loaded manifest from {}", manifest_path.display()),
                    )
                    .await;
                Some(content)
            }
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("Failed to load manifest: {}", e),
                    )
                    .await;
                None
            }
        }
    }

    /// Load SchemaRefly configuration from workspace
    async fn load_config(&self) -> Config {
        let root_uri = self.root_uri.read().await;

        if let Some(root_path) = root_uri.as_ref().and_then(|u| u.to_file_path().ok()) {
            let config_path = root_path.join("schemarefly.toml");

            if let Ok(content) = tokio::fs::read_to_string(&config_path).await {
                if let Ok(config) = toml::from_str::<Config>(&content) {
                    self.client
                        .log_message(
                            MessageType::INFO,
                            format!("Loaded config from {}", config_path.display()),
                        )
                        .await;
                    return config;
                }
            }
        }

        // Return default config if not found
        Config::default()
    }

    /// Compute diagnostics for a document
    async fn compute_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        // Get document content
        let documents = self.documents.read().await;
        let content = match documents.get(uri) {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };
        drop(documents);

        // Get file path
        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        // Get manifest and config
        let manifest_json = self.manifest_json.read().await;
        let config = self.config.read().await;

        if manifest_json.is_none() {
            // No manifest loaded - can't run diagnostics
            return Vec::new();
        }

        // Create fresh Salsa database for this request
        // Salsa handles caching internally based on input values
        let db = SchemaReflyDatabase::default();

        // Create Salsa inputs
        let sql_file = queries::SqlFile::new(&db, file_path.clone(), content);
        let manifest_input =
            queries::ManifestInput::new(&db, manifest_json.as_ref().unwrap().clone());
        let config_input = queries::ConfigInput::new(&db, config.clone());

        // Run contract checking (returns SchemaRefly diagnostics)
        let schema_diagnostics = queries::check_contract(&db, sql_file, config_input, manifest_input);

        // Convert to LSP diagnostics
        schema_diagnostics
            .into_iter()
            .map(|d| self.convert_diagnostic(d))
            .collect()
    }

    /// Convert SchemaRefly diagnostic to LSP diagnostic
    fn convert_diagnostic(&self, diag: SchemaDiagnostic) -> Diagnostic {
        let severity = match diag.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warn => DiagnosticSeverity::WARNING,
            Severity::Info => DiagnosticSeverity::INFORMATION,
        };

        // For now, use the entire first line as the range
        // TODO: Use actual line/column from diagnostic if available
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 100,
            },
        };

        Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(diag.code.as_str().to_string())),
            source: Some("schemarefly".to_string()),
            message: diag.message,
            ..Default::default()
        }
    }

    /// Get hover information for a position in a document
    async fn get_hover(&self, uri: &Url, _position: Position) -> Option<Hover> {
        // Get document content
        let documents = self.documents.read().await;
        let content = documents.get(uri)?.clone();
        drop(documents);

        // Get file path
        let file_path = uri.to_file_path().ok()?;

        // Get manifest and config
        let manifest_json = self.manifest_json.read().await;
        let config = self.config.read().await;

        if manifest_json.is_none() {
            return None;
        }

        // Create fresh Salsa database
        let db = SchemaReflyDatabase::default();

        // Create Salsa inputs
        let sql_file = queries::SqlFile::new(&db, file_path, content);
        let manifest_input = queries::ManifestInput::new(&db, manifest_json.as_ref().unwrap().clone());
        let config_input = queries::ConfigInput::new(&db, config.clone());

        // Infer schema
        let schema = queries::infer_schema(&db, sql_file, config_input, manifest_input).ok()?;

        // Format schema as markdown
        let mut markdown = String::from("## Inferred Schema\n\n");
        markdown.push_str("| Column | Type |\n");
        markdown.push_str("|--------|------|\n");

        for col in &schema.columns {
            markdown.push_str(&format!("| `{}` | {} |\n", col.name, col.logical_type));
        }

        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(markdown)),
            range: None,
        })
    }

    /// Get go-to-definition location for a position in a document
    async fn get_definition(
        &self,
        uri: &Url,
        _position: Position,
    ) -> Option<GotoDefinitionResponse> {
        // Get document content
        let documents = self.documents.read().await;
        let _content = documents.get(uri)?.clone();
        drop(documents);

        // Get manifest
        let manifest_json = self.manifest_json.read().await;
        if manifest_json.is_none() {
            return None;
        }

        // Create fresh Salsa database
        let db = SchemaReflyDatabase::default();

        // Parse manifest
        let manifest_input = queries::ManifestInput::new(&db, manifest_json.as_ref().unwrap().clone());
        let _manifest = queries::manifest(&db, manifest_input)?;

        // TODO: Parse the SQL at the cursor position to identify:
        // 1. If it's a ref('model_name') -> find the model file
        // 2. If it's a contract column reference -> find the YAML definition

        // For now, return a placeholder that goes to the schema.yml
        let root_uri = self.root_uri.read().await;
        let root_path = root_uri.as_ref()?.to_file_path().ok()?;

        // Try to find schema.yml in models/ directory
        let schema_path = root_path.join("models").join("schema.yml");
        if schema_path.exists() {
            let schema_uri = Url::from_file_path(&schema_path).ok()?;

            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: schema_uri,
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
            }));
        }

        None
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Store root URI
        *self.root_uri.write().await = params.root_uri.clone();

        // Load manifest and config
        if let Some(manifest) = self.load_manifest().await {
            *self.manifest_json.write().await = Some(manifest);
        }

        *self.config.write().await = self.load_config().await;

        // Report initialization to client
        self.client
            .log_message(MessageType::INFO, "SchemaRefly LSP initialized")
            .await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Enable text document synchronization
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // Enable hover provider
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Enable go-to-definition provider
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "SchemaRefly LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();

        // Store document
        self.documents.write().await.insert(uri.clone(), text);

        // Compute and publish diagnostics
        let diagnostics = self.compute_diagnostics(&uri).await;
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        // Update document (full sync)
        if let Some(change) = params.content_changes.first() {
            self.documents
                .write()
                .await
                .insert(uri.clone(), change.text.clone());

            // Compute and publish diagnostics on change (if fast enough)
            let diagnostics = self.compute_diagnostics(&uri).await;
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        // Update document text if provided
        if let Some(text) = params.text {
            self.documents.write().await.insert(uri.clone(), text);
        }

        // Reload manifest and config on save
        if let Some(manifest) = self.load_manifest().await {
            *self.manifest_json.write().await = Some(manifest);
        }
        *self.config.write().await = self.load_config().await;

        // Compute and publish diagnostics
        let diagnostics = self.compute_diagnostics(&uri).await;
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Remove document from storage
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        Ok(self.get_hover(&uri, position).await)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        Ok(self.get_definition(&uri, position).await)
    }
}
