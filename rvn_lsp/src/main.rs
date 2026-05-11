#![allow(deprecated)]

mod analysis;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use analysis::{Analysis, ProjectIndex, RvnSymbolKind};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use url::Url;

#[derive(Default)]
struct State {
    documents: HashMap<Url, String>,
    workspace_roots: Vec<PathBuf>,
    last_index: ProjectIndex,
}

struct Backend {
    client: Client,
    state: Arc<RwLock<State>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let mut state = self.state.write().await;
        state.workspace_roots = workspace_roots(&params);
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "rvn-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), " ".to_string()]),
                    ..CompletionOptions::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                        resolve_provider: Some(false),
                        ..CodeActionOptions::default()
                    },
                )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "rvn-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.state.write().await.documents.insert(uri.clone(), text);
        self.refresh_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.state
                .write()
                .await
                .documents
                .insert(uri.clone(), change.text);
            self.refresh_diagnostics(uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.refresh_diagnostics(params.text_document.uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.state.write().await.documents.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let text_params = params.text_document_position_params;
        let uri = text_params.text_document.uri;
        let position = text_params.position;
        let state = self.state.read().await;
        let Some(text) = state.document_text(&uri) else {
            return Ok(None);
        };
        let Some(target) = analysis::label_reference_at(text, position) else {
            return Ok(None);
        };
        drop(state);

        let analysis = self.analyze_for(&uri).await;
        if let Some(label) = analysis.index.labels.get(&target) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: label.uri.clone(),
                range: label.range,
            })));
        }
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let analysis = self.analyze_for(&params.text_document.uri).await;
        let symbols = analysis
            .index
            .symbols_for(&params.text_document.uri)
            .into_iter()
            .map(|symbol| DocumentSymbol {
                name: symbol.name,
                detail: symbol.detail,
                kind: symbol.kind,
                tags: None,
                deprecated: None,
                range: symbol.range,
                selection_range: symbol.selection_range,
                children: None,
            })
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let text_params = params.text_document_position;
        let uri = text_params.text_document.uri;
        let position = text_params.position;
        let state = self.state.read().await;
        let Some(text) = state.document_text(&uri).map(str::to_string) else {
            return Ok(None);
        };
        drop(state);

        let analysis = self.analyze_for(&uri).await;
        let items = analysis.index.completion_items(&text, position);
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let text_params = params.text_document_position_params;
        let uri = text_params.text_document.uri;
        let position = text_params.position;
        let state = self.state.read().await;
        let Some(text) = state.document_text(&uri).map(str::to_string) else {
            return Ok(None);
        };
        drop(state);

        let analysis = self.analyze_for(&uri).await;
        Ok(analysis.index.hover(&text, &uri, position))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        let state = self.state.read().await;
        let roots = state.workspace_roots.clone();
        let documents = state.documents.clone();
        drop(state);
        let index = analysis::analyze_workspace(&roots, &documents).index;
        let query = params.query.to_lowercase();
        let mut symbols = Vec::new();
        for label in index.labels.values() {
            if query.is_empty() || label.name.to_lowercase().contains(&query) {
                symbols.push(SymbolInformation {
                    name: label.name.clone(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: label.uri.clone(),
                        range: label.range,
                    },
                    container_name: None,
                });
            }
        }
        Ok(Some(symbols))
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let text_params = params.text_document_position;
        let uri = text_params.text_document.uri;
        let position = text_params.position;
        let analysis = self.analyze_for(&uri).await;
        let Some(symbol) = analysis.index.symbol_at(&uri, position) else {
            return Ok(None);
        };
        let mut references = analysis.index.references_for(&symbol);
        if !params.context.include_declaration {
            references.retain(|location| match symbol.kind {
                RvnSymbolKind::Label => {
                    analysis.index.labels.get(&symbol.name).is_none_or(|label| {
                        label.uri != location.uri || label.range != location.range
                    })
                }
                RvnSymbolKind::Character => {
                    analysis
                        .index
                        .characters
                        .get(&symbol.name)
                        .is_none_or(|character| {
                            character.uri != location.uri || character.range != location.range
                        })
                }
            });
        }
        Ok(Some(references))
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let analysis = self.analyze_for(&uri).await;
        let Some(symbol) = analysis.index.symbol_at(&uri, position) else {
            return Ok(None);
        };
        Ok(analysis.index.rename_edit(&symbol, &params.new_name))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let state = self.state.read().await;
        let Some(text) = state.document_text(&uri).map(str::to_string) else {
            return Ok(None);
        };
        drop(state);

        let analysis = self.analyze_for(&uri).await;
        let mut actions = Vec::new();
        for diagnostic in params.context.diagnostics {
            match diagnostic_code(&diagnostic).as_deref() {
                Some("unknown-label-target") => {
                    if let Some(typo) = text_in_range(&text, diagnostic.range) {
                        if let Some(suggestion) = analysis.index.suggested_label(&typo) {
                            actions.push(quickfix_replace(
                                &uri,
                                &diagnostic,
                                format!("Replace with `{suggestion}`"),
                                diagnostic.range,
                                suggestion,
                            ));
                        }
                    }
                }
                Some("undefined-character") => {
                    if let Some(typo) = text_in_range(&text, diagnostic.range) {
                        if let Some(suggestion) = analysis.index.suggested_character(&typo) {
                            actions.push(quickfix_replace(
                                &uri,
                                &diagnostic,
                                format!("Replace with `{suggestion}`"),
                                diagnostic.range,
                                suggestion,
                            ));
                        }
                    }
                }
                Some("invalid-assignment") => {
                    if let Some((range, replacement)) =
                        assignment_quickfix_replacement(&text, diagnostic.range.start.line)
                    {
                        actions.push(quickfix_replace(
                            &uri,
                            &diagnostic,
                            "Add `set` to assignment".to_string(),
                            range,
                            replacement,
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok((!actions.is_empty()).then_some(actions))
    }
}

impl Backend {
    async fn refresh_diagnostics(&self, uri: Url) {
        let analysis = self.analyze_for(&uri).await;
        let diagnostics = analysis.index.diagnostics_for(&uri);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn analyze_for(&self, uri: &Url) -> Analysis {
        let state = self.state.read().await;
        let roots = if state.workspace_roots.is_empty() {
            uri.to_file_path()
                .ok()
                .and_then(|path| path.parent().map(PathBuf::from))
                .into_iter()
                .collect()
        } else {
            state.workspace_roots.clone()
        };
        let documents = state.documents.clone();
        drop(state);

        let analysis = analysis::analyze_for_document(uri, &roots, &documents);
        self.state.write().await.last_index = analysis.index.clone();
        analysis
    }
}

impl State {
    fn document_text(&self, uri: &Url) -> Option<&str> {
        self.documents.get(uri).map(String::as_str)
    }
}

fn workspace_roots(params: &InitializeParams) -> Vec<PathBuf> {
    if let Some(folders) = &params.workspace_folders {
        return folders
            .iter()
            .filter_map(|folder| folder.uri.to_file_path().ok())
            .collect();
    }
    params
        .root_uri
        .as_ref()
        .and_then(|uri| uri.to_file_path().ok())
        .into_iter()
        .collect()
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<String> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(code) => Some(code.clone()),
        NumberOrString::Number(code) => Some(code.to_string()),
    }
}

fn quickfix_replace(
    uri: &Url,
    diagnostic: &Diagnostic,
    title: String,
    range: Range,
    new_text: String,
) -> CodeActionOrCommand {
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), vec![TextEdit::new(range, new_text)]);
    CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        is_preferred: Some(true),
        ..CodeAction::default()
    })
}

fn text_in_range(text: &str, range: Range) -> Option<String> {
    if range.start.line != range.end.line {
        return None;
    }
    let line = text.lines().nth(range.start.line as usize)?;
    let start = range.start.character as usize;
    let end = range.end.character as usize;
    (start <= end && end <= line.len()).then(|| line[start..end].to_string())
}

fn assignment_quickfix_replacement(text: &str, line_idx: u32) -> Option<(Range, String)> {
    let line = text.lines().nth(line_idx as usize)?;
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with("set ") {
        return None;
    }
    Some((
        Range {
            start: Position {
                line: line_idx,
                character: indent as u32,
            },
            end: Position {
                line: line_idx,
                character: line.len() as u32,
            },
        },
        format!("set {trimmed}"),
    ))
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: Arc::new(RwLock::new(State::default())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
