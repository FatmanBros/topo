use dashmap::DashMap;
use ropey::Rope;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::completion::CompletionProvider;
use crate::diagnostics::DiagnosticsProvider;
use crate::formatting::FormattingProvider;
use crate::workspace::WorkspaceManager;

pub struct TopoBackend {
    client: Client,
    documents: DashMap<Url, Rope>,
    workspace: WorkspaceManager,
    completion: CompletionProvider,
    diagnostics: DiagnosticsProvider,
    formatting: FormattingProvider,
}

impl TopoBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            workspace: WorkspaceManager::new(),
            completion: CompletionProvider::new(),
            diagnostics: DiagnosticsProvider::new(),
            formatting: FormattingProvider::new(),
        }
    }

    async fn on_change(&self, uri: Url, text: &str) {
        // Update document
        self.documents.insert(uri.clone(), Rope::from_str(text));

        // Update workspace components
        if let Ok(path) = uri.to_file_path() {
            self.workspace.update_file(&path, text);
        }

        // Run diagnostics
        let diagnostics = self.diagnostics.diagnose(text, &self.workspace);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for TopoBackend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Scan workspace for .tp files
        if let Some(folders) = params.workspace_folders {
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    self.workspace.scan_directory(&path);
                }
            }
        } else if let Some(root) = params.root_uri {
            if let Ok(path) = root.to_file_path() {
                self.workspace.scan_directory(&path);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "(".to_string(),
                        "{".to_string(),
                        "$".to_string(),
                        "@".to_string(),
                        "\"".to_string(),
                        "'".to_string(),
                        " ".to_string(),
                        "/".to_string(),
                    ]),
                    resolve_provider: Some(true),
                    ..Default::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "topo-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Topo LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(params.text_document.uri, &params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().next() {
            self.on_change(params.text_document.uri, &change.text)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            self.on_change(params.text_document.uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.documents.get(&uri) {
            let text = doc.to_string();
            let items = self.completion.provide(&text, position, &self.workspace);
            return Ok(Some(CompletionResponse::Array(items)));
        }

        Ok(None)
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        // Add additional edit for auto-import if needed
        if let Some(data) = &item.data {
            if data.get("import").is_some() {
                // The import edit will be handled in completion provider
                return Ok(item);
            }
        }
        Ok(item)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.documents.get(&uri) {
            let text = doc.to_string();
            if let Some(hover) = self.completion.hover(&text, position, &self.workspace) {
                return Ok(Some(hover));
            }
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.documents.get(&uri) {
            let text = doc.to_string();
            if let Some(location) = self.workspace.find_definition(&text, position) {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }

        Ok(None)
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        if let Some(doc) = self.documents.get(&uri) {
            let text = doc.to_string();
            let formatted = self.formatting.format(&text, &params.options);

            let edit = TextEdit {
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(doc.len_lines() as u32, 0),
                },
                new_text: formatted,
            };

            return Ok(Some(vec![edit]));
        }

        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri.clone();
        let mut actions = Vec::new();

        // Check for missing imports in diagnostics
        for diag in &params.context.diagnostics {
            if diag.message.starts_with("Unknown component:") {
                let component_name = diag
                    .message
                    .strip_prefix("Unknown component: ")
                    .unwrap_or("");

                // Find component in workspace
                if let Some(import_path) = self.workspace.find_import_path(component_name, &uri) {
                    let import_statement =
                        format!("import {{ {} }} from \"{}\"\n", component_name, import_path);

                    let action = CodeAction {
                        title: format!("Import {} from {}", component_name, import_path),
                        kind: Some(CodeActionKind::QUICKFIX),
                        edit: Some(WorkspaceEdit {
                            changes: Some(
                                [(
                                    uri.clone(),
                                    vec![TextEdit {
                                        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                                        new_text: import_statement,
                                    }],
                                )]
                                .into(),
                            ),
                            ..Default::default()
                        }),
                        is_preferred: Some(true),
                        ..Default::default()
                    };
                    actions.push(CodeActionOrCommand::CodeAction(action));
                }
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}
