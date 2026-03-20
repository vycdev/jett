use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// The Jett LSP backend.
pub struct JettBackend {
    client: Client,
}

impl JettBackend {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Run the Jett compiler pipeline on the given source text and publish
    /// diagnostics back to the client.
    async fn validate(&self, uri: Url, text: &str) {
        let file_path = uri
            .to_file_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| uri.to_string());

        let result = jett_driver::build_source(text, &file_path);

        let diagnostics: Vec<Diagnostic> = result
            .diagnostics
            .iter()
            .map(|d| {
                let (start_line, start_col) =
                    jett_diagnostics::render::line_col(&result.source, d.span.start);
                let (end_line, end_col) =
                    jett_diagnostics::render::line_col(&result.source, d.span.end);

                let severity = match d.severity {
                    jett_diagnostics::Severity::Error => Some(DiagnosticSeverity::ERROR),
                    jett_diagnostics::Severity::Warning => Some(DiagnosticSeverity::WARNING),
                    jett_diagnostics::Severity::Info => Some(DiagnosticSeverity::INFORMATION),
                };

                // LSP positions are 0-based; line_col returns 1-based.
                let range = Range::new(
                    Position::new(start_line as u32 - 1, start_col as u32 - 1),
                    Position::new(end_line as u32 - 1, end_col as u32 - 1),
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
            .collect();

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for JettBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
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
        self.validate(
            params.text_document.uri,
            &params.text_document.text,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // We requested FULL sync, so the last content change is the full text.
        if let Some(change) = params.content_changes.into_iter().last() {
            self.validate(params.text_document.uri, &change.text).await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let _position = params.text_document_position_params.position;

        // Basic MVP: show which file the cursor is in.
        // A richer implementation would resolve the token at the cursor position
        // and display its type information.
        let contents = HoverContents::Markup(MarkupContent {
            kind: MarkupKind::PlainText,
            value: format!("Jett source: {}", uri),
        });

        Ok(Some(Hover {
            contents,
            range: None,
        }))
    }
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
}
