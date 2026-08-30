use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// The Jett LSP backend.
pub struct JettBackend {
    client: Client,
    /// In-memory document store: URI → latest source text and LSP version.
    documents: tokio::sync::RwLock<HashMap<Url, DocumentState>>,
}

#[derive(Debug, Clone)]
struct DocumentState {
    text: String,
    version: i32,
}

fn should_publish_diagnostics(
    documents: &HashMap<Url, DocumentState>,
    uri: &Url,
    version: i32,
) -> bool {
    documents
        .get(uri)
        .is_some_and(|document| document.version == version)
}

impl JettBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Run the Jett compiler pipeline on the given source text and publish
    /// diagnostics back to the client.
    async fn validate(&self, uri: Url, version: i32, text: &str) {
        let file_path = uri
            .to_file_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| uri.to_string());

        let diagnostics = diagnostics_for_source(text, &file_path);

        let documents = self.documents.read().await;
        if should_publish_diagnostics(&documents, &uri, version) {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }
}

fn document_for_save<'a>(
    documents: &'a HashMap<Url, DocumentState>,
    uri: &Url,
) -> Option<&'a DocumentState> {
    documents.get(uri)
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                ..TextDocumentSyncOptions::default()
            },
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions::default()),
        position_encoding: Some(PositionEncodingKind::UTF16),
        ..ServerCapabilities::default()
    }
}

/// Convert a zero-based LSP UTF-16 position into the driver's one-based
/// Unicode-scalar line and column representation.
fn driver_position(source: &str, position: Position) -> Option<(u32, u32)> {
    let line_index = usize::try_from(position.line).ok()?;
    let line_source = source.split('\n').nth(line_index)?;
    let line_source = line_source.strip_suffix('\r').unwrap_or(line_source);
    let line = position.line.checked_add(1)?;

    let mut utf16_column = 0u32;
    let mut scalar_column = 1u32;
    for ch in line_source.chars() {
        if utf16_column == position.character {
            return Some((line, scalar_column));
        }

        let next_utf16_column = utf16_column.checked_add(ch.len_utf16() as u32)?;
        if position.character < next_utf16_column {
            // The position points into the middle of a UTF-16 surrogate pair.
            return None;
        }

        utf16_column = next_utf16_column;
        scalar_column = scalar_column.checked_add(1)?;
    }

    (utf16_column == position.character).then_some((line, scalar_column))
}

/// Convert a byte offset into a zero-based LSP UTF-16 position.
fn lsp_position(source: &str, byte_offset: u32) -> Position {
    let mut end = (byte_offset as usize).min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }

    let prefix = &source[..end];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let line_prefix = &prefix[line_start..];
    let line_prefix = line_prefix.strip_suffix('\r').unwrap_or(line_prefix);
    let character = line_prefix.encode_utf16().count();

    Position::new(line as u32, character as u32)
}

#[tower_lsp::async_trait]
impl LanguageServer for JettBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: server_capabilities(),
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Jett language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        self.documents.write().await.insert(
            uri.clone(),
            DocumentState {
                text: text.clone(),
                version: params.text_document.version,
            },
        );
        self.validate(uri, params.text_document.version, &text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // We requested FULL sync, so the last content change is the full text.
        if let Some(change) = params.content_changes.into_iter().last() {
            let uri = params.text_document.uri.clone();
            let text = change.text.clone();
            self.documents.write().await.insert(
                uri.clone(),
                DocumentState {
                    text: text.clone(),
                    version: params.text_document.version,
                },
            );
            self.validate(uri, params.text_document.version, &text)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let document = {
            let documents = self.documents.read().await;
            document_for_save(&documents, &uri).cloned()
        };
        if let Some(document) = document {
            self.validate(uri, document.version, &document.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);

        // A client keeps the last published diagnostics after a document is
        // closed unless the server explicitly clears them.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(document) = docs.get(uri) else {
            return Ok(None);
        };
        let source = &document.text;

        let Some((line, col)) = driver_position(source, position) else {
            return Ok(None);
        };

        let type_info = jett_driver::hover_type(source, line, col);

        let Some(type_str) = type_info else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::PlainText,
                value: type_str,
            }),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(document) = docs.get(uri) else {
            return Ok(None);
        };
        let source = &document.text;

        let Some((line, col)) = driver_position(source, position) else {
            return Ok(None);
        };

        let Some((start, end)) = jett_driver::goto_definition(source, line, col) else {
            return Ok(None);
        };

        let range = Range::new(lsp_position(source, start), lsp_position(source, end));

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range,
        })))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;

        let docs = self.documents.read().await;
        let Some(document) = docs.get(uri) else {
            return Ok(None);
        };
        let source = &document.text;

        let position = params.text_document_position.position;
        let Some((line, col)) = driver_position(source, position) else {
            return Ok(None);
        };
        let candidates = jett_driver::completions_at(source, line, col);
        if candidates.is_empty() {
            return Ok(None);
        }

        use jett_resolve::scope::DefKind;
        let items: Vec<CompletionItem> = candidates
            .into_iter()
            .map(|(name, kind)| {
                let kind = match kind {
                    DefKind::Function => CompletionItemKind::FUNCTION,
                    DefKind::Struct => CompletionItemKind::STRUCT,
                    DefKind::Enum => CompletionItemKind::ENUM,
                    DefKind::Interface => CompletionItemKind::INTERFACE,
                    DefKind::Machine => CompletionItemKind::CLASS,
                    DefKind::Actor => CompletionItemKind::CLASS,
                    DefKind::Variable | DefKind::Param => CompletionItemKind::VARIABLE,
                    DefKind::Type => CompletionItemKind::TYPE_PARAMETER,
                    DefKind::Constant => CompletionItemKind::CONSTANT,
                    DefKind::Namespace => CompletionItemKind::MODULE,
                    DefKind::Bitfield => CompletionItemKind::STRUCT,
                };
                CompletionItem {
                    label: name,
                    kind: Some(kind),
                    ..CompletionItem::default()
                }
            })
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }
}

fn diagnostics_for_source(source: &str, file_path: &str) -> Vec<Diagnostic> {
    let result = jett_driver::build_source(source, file_path);

    result
        .diagnostics
        .iter()
        .map(|d| {
            let severity = match d.severity {
                jett_diagnostics::Severity::Error => Some(DiagnosticSeverity::ERROR),
                jett_diagnostics::Severity::Warning => Some(DiagnosticSeverity::WARNING),
                jett_diagnostics::Severity::Info => Some(DiagnosticSeverity::INFORMATION),
            };

            let range = Range::new(
                lsp_position(&result.source, d.span.start),
                lsp_position(&result.source, d.span.end),
            );

            Diagnostic {
                range,
                severity,
                code: Some(NumberOrString::String(d.code.to_string())),
                source: Some("jett".to_string()),
                message: d.message.clone(),
                ..Diagnostic::default()
            }
        })
        .collect()
}

/// Start the LSP server on stdin/stdout. This is the main entry point called
/// by `jett lsp`.
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = tower_lsp::LspService::new(|client| JettBackend::new(client));
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Url;

    #[test]
    fn stale_document_versions_are_not_publishable() {
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            uri.clone(),
            DocumentState {
                text: "new source".to_string(),
                version: 2,
            },
        );

        assert!(should_publish_diagnostics(&documents, &uri, 2));
        assert!(!should_publish_diagnostics(&documents, &uri, 1));

        documents.remove(&uri);
        assert!(!should_publish_diagnostics(&documents, &uri, 2));
    }

    #[test]
    fn driver_position_converts_utf16_columns_to_scalar_columns() {
        let source = "🙂x\né界\r\n";

        assert_eq!(driver_position(source, Position::new(0, 0)), Some((1, 1)));
        assert_eq!(driver_position(source, Position::new(0, 1)), None);
        assert_eq!(driver_position(source, Position::new(0, 2)), Some((1, 2)));
        assert_eq!(driver_position(source, Position::new(0, 3)), Some((1, 3)));
        assert_eq!(driver_position(source, Position::new(0, 4)), None);
        assert_eq!(driver_position(source, Position::new(1, 2)), Some((2, 3)));
        assert_eq!(driver_position(source, Position::new(1, 3)), None);
        assert_eq!(driver_position(source, Position::new(2, 0)), Some((3, 1)));
        assert_eq!(driver_position(source, Position::new(3, 0)), None);
    }

    #[test]
    fn lsp_position_converts_byte_offsets_to_utf16_columns() {
        let source = "🙂x\r\né";

        assert_eq!(lsp_position(source, 0), Position::new(0, 0));
        assert_eq!(lsp_position(source, 2), Position::new(0, 0));
        assert_eq!(lsp_position(source, 4), Position::new(0, 2));
        assert_eq!(lsp_position(source, 5), Position::new(0, 3));
        assert_eq!(lsp_position(source, 6), Position::new(0, 3));
        assert_eq!(lsp_position(source, 7), Position::new(1, 0));
        assert_eq!(lsp_position(source, 9), Position::new(1, 1));
        assert_eq!(lsp_position(source, u32::MAX), Position::new(1, 1));
    }

    /// Verify that `build_source` produces diagnostics for invalid Jett code.
    /// This exercises the same path the LSP uses to validate documents.
    #[test]
    fn build_source_returns_diagnostics_for_bad_code() {
        let source = "this is not valid jett code !!!";
        let result = jett_driver::build_source(source, "test.jett");
        assert!(
            result.has_errors,
            "expected errors for invalid source, got none"
        );
        assert!(
            !result.diagnostics.is_empty(),
            "expected at least one diagnostic"
        );
    }

    /// Verify that valid (empty) source produces no errors.
    #[test]
    fn build_source_empty_is_ok() {
        let result = jett_driver::build_source("", "empty.jett");
        assert!(
            !result.has_errors,
            "expected no errors for empty source, got: {:?}",
            result
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn diagnostics_for_source_maps_compiler_diagnostic_to_lsp_fields() {
        let diagnostics = diagnostics_for_source("this is not valid jett code !!!", "test.jett");
        let diagnostic = diagnostics.first().expect("invalid source should diagnose");

        assert_eq!(diagnostic.source.as_deref(), Some("jett"));
        assert!(matches!(
            diagnostic.code,
            Some(NumberOrString::String(ref code)) if code.starts_with('E')
        ));
        assert_eq!(diagnostic.range.start.line, 0);
        assert_eq!(diagnostic.range.start.character, 0);
    }

    #[test]
    fn diagnostics_for_source_uses_utf16_columns_after_supplementary_characters() {
        let source = "namespace test\nfunction f() returns string:\n    return \"🙂\" !!!\n";
        let compiler_result = jett_driver::build_source(source, "test.jett");
        let diagnostics = diagnostics_for_source(source, "test.jett");

        assert_eq!(diagnostics.len(), compiler_result.diagnostics.len());
        for (diagnostic, compiler_diagnostic) in
            diagnostics.iter().zip(&compiler_result.diagnostics)
        {
            assert_eq!(
                diagnostic.range,
                Range::new(
                    lsp_position(source, compiler_diagnostic.span.start),
                    lsp_position(source, compiler_diagnostic.span.end),
                )
            );
        }
    }

    /// Verify that hover_type returns a type for a known expression.
    #[test]
    fn hover_type_returns_type_for_identifier() {
        let source = "namespace test\n\nfunction main() returns nothing:\n    int64 x = 42\n    return nothing\n";
        // Line 4, col 5 = start of "int64 x" — the literal 42 is on the same line
        // col 15 = the '4' in '42'
        let ty = jett_driver::hover_type(source, 4, 15);
        assert_eq!(ty, Some("int64".to_string()), "expected int64 hover type");
    }

    #[test]
    fn server_capabilities_advertise_save_notifications() {
        let capabilities = server_capabilities();
        let Some(TextDocumentSyncCapability::Options(options)) = capabilities.text_document_sync
        else {
            panic!("expected explicit text document synchronization options");
        };

        assert_eq!(options.change, Some(TextDocumentSyncKind::FULL));
        assert!(matches!(
            options.save,
            Some(TextDocumentSyncSaveOptions::Supported(true))
        ));
    }

    #[test]
    fn save_validation_reads_the_latest_open_document() {
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            uri.clone(),
            DocumentState {
                text: "latest source".to_string(),
                version: 7,
            },
        );

        let document = document_for_save(&documents, &uri).expect("open document");
        assert_eq!(document.text, "latest source");
        assert_eq!(document.version, 7);
    }
}
