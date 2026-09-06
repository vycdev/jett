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
        references_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions::default()),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            ..SignatureHelpOptions::default()
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        position_encoding: Some(PositionEncodingKind::UTF16),
        ..ServerCapabilities::default()
    }
}

/// Return a zero-based logical source line without its line ending.
///
/// Jett accepts LF, CRLF, and lone CR, so LSP conversions must recognize all
/// three forms consistently with the compiler and query layer.
fn source_line(source: &str, target_line: usize) -> Option<&str> {
    let bytes = source.as_bytes();
    let mut line = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                if line == target_line {
                    return Some(&source[start..index]);
                }
                index += 1;
                if bytes.get(index) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                start = index;
            }
            b'\n' => {
                if line == target_line {
                    return Some(&source[start..index]);
                }
                index += 1;
                line += 1;
                start = index;
            }
            _ => index += 1,
        }
    }

    (line == target_line).then_some(&source[start..])
}

/// Convert a zero-based LSP UTF-16 position into the driver's one-based
/// Unicode-scalar line and column representation.
fn driver_position(source: &str, position: Position) -> Option<(u32, u32)> {
    let line_index = usize::try_from(position.line).ok()?;
    let line_source = source_line(source, line_index)?;
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

    let bytes = source.as_bytes();
    let mut line = 0usize;
    let mut line_start = 0usize;
    let mut index = 0usize;
    while index < end {
        match bytes[index] {
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    if index + 1 >= end {
                        break;
                    }
                    index += 2;
                } else {
                    index += 1;
                }
                line += 1;
                line_start = index;
            }
            b'\n' => {
                index += 1;
                line += 1;
                line_start = index;
            }
            _ => index += 1,
        }
    }
    let line_prefix = &source[line_start..end];
    let line_prefix = line_prefix.strip_suffix('\r').unwrap_or(line_prefix);
    let character = line_prefix.encode_utf16().count();

    Position::new(line as u32, character as u32)
}

fn lsp_position_from_driver(source: &str, line: u32, column: u32) -> Option<Position> {
    let line_index = usize::try_from(line.checked_sub(1)?).ok()?;
    let scalar_index = usize::try_from(column.checked_sub(1)?).ok()?;
    let line_source = source_line(source, line_index)?;
    if scalar_index > line_source.chars().count() {
        return None;
    }
    let utf16_column = line_source
        .chars()
        .take(scalar_index)
        .map(char::len_utf16)
        .sum::<usize>();
    Some(Position::new(
        u32::try_from(line_index).ok()?,
        u32::try_from(utf16_column).ok()?,
    ))
}

fn document_symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "namespace" => SymbolKind::MODULE,
        "function" | "verify" | "property" => SymbolKind::FUNCTION,
        "interface" => SymbolKind::INTERFACE,
        "struct" | "bitfield" => SymbolKind::STRUCT,
        "enum" => SymbolKind::ENUM,
        "machine" | "actor" | "resource" => SymbolKind::CLASS,
        "variable" => SymbolKind::VARIABLE,
        "type" => SymbolKind::TYPE_PARAMETER,
        "implement" => SymbolKind::OBJECT,
        _ => SymbolKind::OBJECT,
    }
}

#[allow(deprecated)]
fn document_symbols_for_source(source: &str) -> Option<Vec<DocumentSymbol>> {
    let outline = jett_driver::query_source_file_symbols(source, "<lsp-document>").ok()?;
    let symbols = outline
        .symbols
        .into_iter()
        .filter_map(|symbol| {
            let start = lsp_position_from_driver(source, symbol.line, symbol.column)?;
            let end = lsp_position_from_driver(source, symbol.end_line, symbol.end_column)?;
            let range = Range::new(start, end);
            Some(DocumentSymbol {
                name: symbol.name,
                detail: symbol.signature,
                kind: document_symbol_kind(&symbol.kind),
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            })
        })
        .collect();
    Some(symbols)
}

fn reference_locations(
    source: &str,
    uri: &Url,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let (line, column) = driver_position(source, position)?;
    let mut spans = jett_driver::references_at(source, line, column);
    if include_declaration
        && let Some(definition) = jett_driver::goto_definition(source, line, column)
        && !spans.contains(&definition)
    {
        spans.push(definition);
        spans.sort_unstable();
    }

    (!spans.is_empty()).then(|| {
        spans
            .into_iter()
            .map(|(start, end)| Location {
                uri: uri.clone(),
                range: Range::new(lsp_position(source, start), lsp_position(source, end)),
            })
            .collect()
    })
}

fn formatting_edits(source: &str) -> Option<Vec<TextEdit>> {
    let result = jett_fmt::format_source(source, jett_common::FileId::new(0));
    if !result.errors.is_empty() || result.output == source {
        return None;
    }

    let end_offset = u32::try_from(source.len()).unwrap_or(u32::MAX);
    Some(vec![TextEdit {
        range: Range::new(Position::new(0, 0), lsp_position(source, end_offset)),
        new_text: result.output,
    }])
}

fn signature_help_source_offset(source: &str, position: Position) -> Option<usize> {
    let line_source = source_line(source, usize::try_from(position.line).ok()?)?;
    let line_start = line_source.as_ptr() as usize - source.as_ptr() as usize;
    let mut utf16_column = 0u32;
    for (offset, ch) in line_source.char_indices() {
        if utf16_column == position.character {
            return Some(line_start + offset);
        }
        utf16_column = utf16_column.checked_add(ch.len_utf16() as u32)?;
        if position.character < utf16_column {
            return None;
        }
    }
    (utf16_column == position.character).then_some(line_start + line_source.len())
}

fn call_context_at(source: &str, position: Position) -> Option<(String, u32)> {
    use jett_lexer::TokenKind;

    struct Delimiter {
        kind: TokenKind,
        callee: Option<String>,
        commas: u32,
    }

    let cursor = signature_help_source_offset(source, position)?;
    let lexed = jett_lexer::tokenize(source, jett_common::FileId::new(0));
    let mut delimiters = Vec::new();
    for (index, token) in lexed.tokens.iter().enumerate() {
        if token.span.start as usize >= cursor {
            break;
        }
        match token.kind {
            TokenKind::LParen | TokenKind::LBracket => delimiters.push(Delimiter {
                kind: token.kind,
                callee: (token.kind == TokenKind::LParen)
                    .then(|| signature_callee_before(source, &lexed.tokens[..index]))
                    .flatten(),
                commas: 0,
            }),
            TokenKind::RParen | TokenKind::RBracket => {
                let opening = if token.kind == TokenKind::RParen {
                    TokenKind::LParen
                } else {
                    TokenKind::LBracket
                };
                if let Some(position) = delimiters.iter().rposition(|entry| entry.kind == opening) {
                    delimiters.truncate(position);
                }
            }
            TokenKind::Comma => {
                if let Some(delimiter) = delimiters.last_mut() {
                    delimiter.commas = delimiter.commas.saturating_add(1);
                }
            }
            _ => {}
        }
    }
    delimiters
        .into_iter()
        .rev()
        .find_map(|call| call.callee.map(|name| (name, call.commas)))
}

fn signature_callee_before(source: &str, tokens: &[jett_lexer::Token]) -> Option<String> {
    use jett_lexer::TokenKind;
    let mut end = tokens.len();
    if tokens.last()?.kind == TokenKind::RBracket {
        let mut depth = 1usize;
        end -= 1;
        while end > 0 && depth > 0 {
            end -= 1;
            match tokens[end].kind {
                TokenKind::RBracket => depth += 1,
                TokenKind::LBracket => depth -= 1,
                _ => {}
            }
        }
        if depth != 0 {
            return None;
        }
    }
    let last = tokens.get(end.checked_sub(1)?)?;
    // Match parser contextual identifiers too: public calls such as list.map
    // and json.serialize end in tokens that also have keyword/type meanings.
    if !matches!(
        last.kind,
        TokenKind::Ident
            | TokenKind::Self_
            | TokenKind::Other
            | TokenKind::Error
            | TokenKind::Value
            | TokenKind::Serialize
            | TokenKind::Network
            | TokenKind::Default
            | TokenKind::Ok
            | TokenKind::Fail
            | TokenKind::Clone
            | TokenKind::Send
            | TokenKind::Run
            | TokenKind::Join
            | TokenKind::Cancel
            | TokenKind::Trace
            | TokenKind::Transition
            | TokenKind::Type
            | TokenKind::Bit
            | TokenKind::Bits
            | TokenKind::States
            | TokenKind::Map_
            | TokenKind::List_
            | TokenKind::Set_
    ) {
        return None;
    }
    let mut start = end - 1;
    while start >= 2 && tokens[start - 1].kind == TokenKind::Dot {
        let component =
            &source[tokens[start - 2].span.start as usize..tokens[start - 2].span.end as usize];
        if !component
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            break;
        }
        start -= 2;
    }
    Some(
        tokens[start..end]
            .iter()
            .map(|token| &source[token.span.start as usize..token.span.end as usize])
            .collect(),
    )
}

fn signature_parameter_label(parameter: &jett_driver::SignatureParam) -> String {
    let mut label = String::new();
    if parameter.view {
        label.push_str("view ");
    }
    if parameter.mutable {
        label.push_str("mutable ");
    }
    label.push_str(&parameter.name);
    label.push_str(": ");
    label.push_str(&parameter.type_name);
    label
}

fn signature_help_for_source(source: &str, position: Position) -> Option<SignatureHelp> {
    let (callee, active_parameter) = call_context_at(source, position)?;
    let offset = u32::try_from(signature_help_source_offset(source, position)?).ok()?;
    let signature = jett_driver::query_source_signature_at(source, &callee, offset)
        .ok()
        .flatten()?;
    let type_params = if signature.type_params.is_empty() {
        String::new()
    } else {
        format!("[{}]", signature.type_params.join(", "))
    };
    let parameter_labels: Vec<String> = signature
        .params
        .iter()
        .map(signature_parameter_label)
        .collect();
    let label = format!(
        "{}{}({}) returns {}",
        signature.name,
        type_params,
        parameter_labels.join(", "),
        signature.return_type
    );
    let parameters = parameter_labels
        .into_iter()
        .map(|label| ParameterInformation {
            label: ParameterLabel::Simple(label),
            documentation: None,
        })
        .collect();
    let active_parameter = signature
        .params
        .len()
        .checked_sub(1)
        .and_then(|last| u32::try_from(last).ok())
        .map(|last| active_parameter.min(last));

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter: None,
        }],
        active_signature: Some(0),
        active_parameter,
    })
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

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let documents = self.documents.read().await;
        let Some(document) = documents.get(uri) else {
            return Ok(None);
        };

        Ok(reference_locations(
            &document.text,
            uri,
            position,
            params.context.include_declaration,
        ))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };

        Ok(formatting_edits(&document.text))
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
                    DefKind::Resource => CompletionItemKind::CLASS,
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

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let documents = self.documents.read().await;
        let Some(document) = documents.get(uri) else {
            return Ok(None);
        };

        Ok(signature_help_for_source(
            &document.text,
            params.text_document_position_params.position,
        ))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let documents = self.documents.read().await;
        let Some(document) = documents.get(uri) else {
            return Ok(None);
        };
        let Some(symbols) = document_symbols_for_source(&document.text) else {
            return Ok(None);
        };

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
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

    #[test]
    fn position_conversions_support_lone_cr_lines() {
        let source = "a\r🙂b";

        assert_eq!(driver_position(source, Position::new(1, 0)), Some((2, 1)));
        assert_eq!(driver_position(source, Position::new(1, 2)), Some((2, 2)));
        assert_eq!(lsp_position(source, 2), Position::new(1, 0));
        assert_eq!(lsp_position(source, 6), Position::new(1, 2));
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
    fn server_capabilities_advertise_sync_document_symbols_and_signature_help() {
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
        assert_eq!(
            capabilities.document_symbol_provider,
            Some(OneOf::Left(true))
        );
        assert_eq!(
            capabilities.signature_help_provider,
            Some(SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                retrigger_characters: None,
                work_done_progress_options: WorkDoneProgressOptions::default(),
            })
        );
    }

    #[test]
    fn document_symbols_map_unsaved_source_outline() {
        let source = "namespace api\n\nexport function login() returns int64:\n    return 1\n";

        let symbols = document_symbols_for_source(source).expect("valid source outline");

        let namespace = symbols
            .iter()
            .find(|symbol| symbol.name == "api")
            .expect("namespace symbol");
        assert_eq!(namespace.kind, SymbolKind::MODULE);
        assert_eq!(namespace.selection_range.start, Position::new(0, 10));
        assert_eq!(namespace.selection_range.end, Position::new(0, 13));

        let login = symbols
            .iter()
            .find(|symbol| symbol.name == "api.login")
            .expect("function symbol");
        assert_eq!(login.kind, SymbolKind::FUNCTION);
        assert_eq!(login.detail.as_deref(), Some("api.login() returns int64"));
        assert_eq!(login.selection_range.start, Position::new(2, 16));
        assert_eq!(login.selection_range.end, Position::new(2, 21));
    }

    #[test]
    fn document_symbols_map_lone_cr_source_lines() {
        let source = "namespace api\r\rexport function login() returns int64:\r    return 1\r";

        let symbols = document_symbols_for_source(source).expect("valid source outline");
        let login = symbols
            .iter()
            .find(|symbol| symbol.name == "api.login")
            .expect("function symbol");

        assert_eq!(login.selection_range.start, Position::new(2, 16));
        assert_eq!(login.selection_range.end, Position::new(2, 21));
    }

    #[test]
    fn signature_help_tracks_the_active_parameter_of_nested_calls() {
        let source = "namespace app\n\nexport function add(left: int64, right: int64) returns int64:\n    return left + right\n\nfunction main() returns int64:\n    return app.add(1, app.add(2, 3))\n";
        let cursor_offset = source.find("2, 3").expect("inner arguments") + "2, ".len();

        let help = signature_help_for_source(
            source,
            lsp_position(source, u32::try_from(cursor_offset).unwrap()),
        )
        .expect("nested call should provide signature help");

        assert_eq!(help.active_signature, Some(0));
        assert_eq!(help.active_parameter, Some(1));
        assert_eq!(help.signatures.len(), 1);
        assert_eq!(
            help.signatures[0].label,
            "app.add(left: int64, right: int64) returns int64"
        );
        assert_eq!(
            help.signatures[0].parameters.as_ref().unwrap()[1].label,
            ParameterLabel::Simple("right: int64".to_string())
        );
    }

    #[test]
    fn signature_help_resolves_private_unqualified_document_calls() {
        let source = "namespace app\n\nfunction helper(value: string) returns string:\n    return value\n\nfunction main() returns string:\n    return helper(\"value\")\n";
        let cursor_offset = source.find("\"value\"").expect("call argument");

        let help = signature_help_for_source(
            source,
            lsp_position(source, u32::try_from(cursor_offset).unwrap()),
        )
        .expect("private call should provide signature help");

        assert_eq!(
            help.signatures[0].label,
            "app.helper(value: string) returns string"
        );
        assert_eq!(help.active_parameter, Some(0));
    }

    #[test]
    fn signature_help_survives_an_incomplete_call() {
        let source = "namespace app\n\nexport function add(left: int64, right: int64) returns int64:\n    return left + right\n\nfunction main() returns int64:\n    return app.add(";
        let cursor = lsp_position(source, u32::try_from(source.len()).unwrap());

        let help = signature_help_for_source(source, cursor)
            .expect("an unfinished call should still provide signature help");

        assert_eq!(
            help.signatures[0].label,
            "app.add(left: int64, right: int64) returns int64"
        );
        assert_eq!(help.active_parameter, Some(0));
    }

    #[test]
    fn signature_help_supports_zero_parameters() {
        let source = "namespace app\nfunction f() returns int64:\n    return 1\nfunction main() returns int64:\n    return f(";
        let help =
            signature_help_for_source(source, lsp_position(source, source.len() as u32)).unwrap();
        assert_eq!(help.active_parameter, None);
        assert_eq!(help.signatures[0].parameters.as_ref().unwrap().len(), 0);
    }

    #[test]
    fn signature_context_handles_grouping_and_interpolation() {
        for (source, expected) in [
            ("f(1, (2 + ", ("f", 1)),
            ("f(\"{g(1, 2)}\")", ("g", 1)),
            ("f(1, list.of[int64](2, ", ("list.of", 1)),
            ("list.map[int64, string](values, ", ("list.map", 1)),
            ("json.serialize(", ("json.serialize", 0)),
        ] {
            let offset = if source.contains("{g") {
                source.find("2)").unwrap()
            } else {
                source.len()
            };
            assert_eq!(
                call_context_at(source, lsp_position(source, offset as u32)),
                Some((expected.0.to_string(), expected.1))
            );
        }
    }

    #[test]
    fn signature_help_uses_the_calling_namespace() {
        let source = "namespace first\nfunction helper(value: int64) returns int64:\n    return value\nnamespace second\nfunction helper(value: string) returns string:\n    return value\nfunction main() returns string:\n    return helper(";
        let help =
            signature_help_for_source(source, lsp_position(source, source.len() as u32)).unwrap();
        assert_eq!(
            help.signatures[0].label,
            "second.helper(value: string) returns string"
        );
    }

    #[test]
    fn document_symbol_kind_maps_resources() {
        assert_eq!(document_symbol_kind("resource"), SymbolKind::CLASS);
    }

    #[test]
    fn server_capabilities_advertise_find_references() {
        let capabilities = server_capabilities();

        assert_eq!(capabilities.references_provider, Some(OneOf::Left(true)));
    }

    #[test]
    fn reference_locations_map_driver_spans_to_lsp_ranges() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n\nfunction main() returns int64:\n    return double(21)\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();

        let locations = reference_locations(source, &uri, Position::new(6, 11), false)
            .expect("call should resolve");

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, uri);
        assert_eq!(
            locations[0].range,
            Range::new(Position::new(6, 11), Position::new(6, 17))
        );
    }

    #[test]
    fn server_capabilities_advertise_document_formatting() {
        let capabilities = server_capabilities();

        assert_eq!(
            capabilities.document_formatting_provider,
            Some(OneOf::Left(true))
        );
    }

    #[test]
    fn reference_locations_include_the_declaration_when_requested() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n\nfunction main() returns int64:\n    return double(21)\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();

        let locations = reference_locations(source, &uri, Position::new(6, 11), true)
            .expect("call should resolve");

        assert_eq!(locations.len(), 2);
    }

    #[test]
    fn reference_locations_support_requests_on_the_declaration() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n\nfunction main() returns int64:\n    return double(21)\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();

        let locations = reference_locations(source, &uri, Position::new(2, 9), true)
            .expect("declaration should resolve");

        assert_eq!(locations.len(), 2);
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

    #[test]
    fn document_formatting_replaces_the_open_buffer_with_canonical_source() {
        let source = "namespace app\n\nfunction f() returns int64:\n    return  1\n";

        let edits = formatting_edits(source).expect("valid source should format");

        assert_eq!(edits.len(), 1);
        assert_eq!(
            edits[0].range,
            Range::new(Position::new(0, 0), Position::new(4, 0))
        );
        assert_eq!(
            edits[0].new_text,
            "namespace app\nfunction f() returns int64:\n    return 1\n"
        );
    }
}
