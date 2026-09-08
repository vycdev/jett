use std::collections::HashMap;
use std::path::Path;

use jett_lexer::{Lexer, TokenKind};
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
        document_highlight_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions::default()),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            ..SignatureHelpOptions::default()
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(
            SemanticTokensOptions {
                work_done_progress_options: WorkDoneProgressOptions::default(),
                legend: SemanticTokensLegend {
                    token_types: vec![
                        SemanticTokenType::KEYWORD,
                        SemanticTokenType::TYPE,
                        SemanticTokenType::NUMBER,
                        SemanticTokenType::STRING,
                        SemanticTokenType::OPERATOR,
                        SemanticTokenType::COMMENT,
                    ],
                    token_modifiers: Vec::new(),
                },
                range: Some(false),
                full: Some(SemanticTokensFullOptions::Bool(true)),
            }
            .into(),
        ),
        position_encoding: Some(PositionEncodingKind::UTF16),
        ..ServerCapabilities::default()
    }
}

/// Return a zero-based logical source line without its line ending.
///
/// Jett accepts LF, CRLF, and lone CR, so LSP conversions must recognize all
/// three forms consistently with the compiler and query layer.
fn source_line(source: &str, target_line: usize) -> Option<&str> {
    let (start, end) = source_line_bounds(source, target_line)?;
    Some(&source[start..end])
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

fn source_line_bounds(source: &str, target_line: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut line = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' | b'\n' => {
                if line == target_line {
                    return Some((start, index));
                }
                if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                index += 1;
                line += 1;
                start = index;
            }
            _ => index += 1,
        }
    }

    (line == target_line).then_some((start, source.len()))
}

fn byte_offset_for_position(source: &str, position: Position) -> Option<u32> {
    let line = usize::try_from(position.line).ok()?;
    let (line_start, line_end) = source_line_bounds(source, line)?;
    let mut utf16_column = 0u32;

    for (relative_offset, ch) in source[line_start..line_end].char_indices() {
        if utf16_column == position.character {
            return u32::try_from(line_start + relative_offset).ok();
        }
        let next_column = utf16_column.checked_add(ch.len_utf16() as u32)?;
        if position.character < next_column {
            return None;
        }
        utf16_column = next_column;
    }

    (utf16_column == position.character)
        .then(|| u32::try_from(line_end).ok())
        .flatten()
}

fn selection_range_for_position(source: &str, position: Position) -> Option<SelectionRange> {
    let byte_offset = byte_offset_for_position(source, position)?;
    let line = usize::try_from(position.line).ok()?;
    let (line_start, line_end) = source_line_bounds(source, line)?;
    let line_source = &source[line_start..line_end];
    let (trimmed_start, trimmed_end) = if line_source.trim().is_empty() {
        (line_start, line_end)
    } else {
        (
            line_start + (line_source.len() - line_source.trim_start().len()),
            line_start + line_source.trim_end().len(),
        )
    };
    // A selection must contain the requested cursor even in indentation or
    // trailing whitespace on a nonblank line.
    let (selection_start, selection_end) =
        if (trimmed_start..=trimmed_end).contains(&(byte_offset as usize)) {
            (trimmed_start, trimmed_end)
        } else {
            (line_start, line_end)
        };
    let line_range = Range::new(
        lsp_position(source, u32::try_from(selection_start).ok()?),
        lsp_position(source, u32::try_from(selection_end).ok()?),
    );
    let document_range = Range::new(
        Position::new(0, 0),
        lsp_position(source, u32::try_from(source.len()).unwrap_or(u32::MAX)),
    );

    let mut selection = SelectionRange {
        range: document_range,
        parent: None,
    };
    if line_range != selection.range {
        selection = SelectionRange {
            range: line_range,
            parent: Some(Box::new(selection)),
        };
    }

    let token_range = jett_lexer::tokenize(source, jett_common::FileId::new(0))
        .tokens
        .into_iter()
        .find(|token| {
            token.span.start <= byte_offset
                && byte_offset < token.span.end
                && !matches!(
                    token.kind,
                    jett_lexer::TokenKind::Newline
                        | jett_lexer::TokenKind::Indent
                        | jett_lexer::TokenKind::Dedent
                        | jett_lexer::TokenKind::Eof
                        | jett_lexer::TokenKind::InvalidToken
                )
        })
        .map(|token| {
            Range::new(
                lsp_position(source, token.span.start),
                lsp_position(source, token.span.end),
            )
        });
    if let Some(token_range) = token_range
        && token_range != selection.range
        && selection.range.start <= token_range.start
        && token_range.end <= selection.range.end
    {
        selection = SelectionRange {
            range: token_range,
            parent: Some(Box::new(selection)),
        };
    }

    Some(selection)
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

fn completion_item_kind(kind_name: &str) -> CompletionItemKind {
    match kind_name {
        "function" => CompletionItemKind::FUNCTION,
        "namespace" => CompletionItemKind::MODULE,
        "struct" | "bitfield" => CompletionItemKind::STRUCT,
        "enum" => CompletionItemKind::ENUM,
        "interface" => CompletionItemKind::INTERFACE,
        "machine" | "actor" | "resource" => CompletionItemKind::CLASS,
        "type" => CompletionItemKind::TYPE_PARAMETER,
        "constant" => CompletionItemKind::CONSTANT,
        _ => CompletionItemKind::VARIABLE,
    }
}

fn completion_item(
    name: String,
    kind_name: &str,
    rank: u32,
    detail: Option<String>,
    prefix: &str,
) -> CompletionItem {
    let filter_text = if prefix.contains('.') {
        name.clone()
    } else {
        name.rsplit_once('.')
            .map_or_else(|| name.clone(), |(_, leaf)| leaf.to_string())
    };

    CompletionItem {
        label: name.clone(),
        kind: Some(completion_item_kind(kind_name)),
        detail,
        filter_text: Some(filter_text),
        sort_text: Some(format!("{rank:03}:{name}:{kind_name}")),
        ..CompletionItem::default()
    }
}

fn completion_prefix_for_source(source: &str, position: Position) -> Option<String> {
    let (_, column) = driver_position(source, position)?;
    let line = source_line(source, usize::try_from(position.line).ok()?)?;
    let before_cursor: String = line
        .chars()
        .take(usize::try_from(column.checked_sub(1)?).ok()?)
        .collect();
    let start = before_cursor
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii_alphanumeric() && *ch != '_' && *ch != '.')
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    Some(before_cursor[start..].to_string())
}

fn completion_items_for_source(
    source: &str,
    path: &Path,
    position: Position,
) -> Option<Vec<CompletionItem>> {
    let (line, column) = driver_position(source, position)?;
    let prefix = completion_prefix_for_source(source, position)?;
    let signatures: HashMap<String, String> =
        jett_driver::query_source_file_symbols(source, "<lsp-document>")
            .ok()?
            .symbols
            .into_iter()
            .filter_map(|symbol| symbol.signature.map(|signature| (symbol.name, signature)))
            .collect();
    let mut items = Vec::new();

    // The open buffer is authoritative even when the saved file is invalid or
    // does not exist yet. Discover sibling/stdlib metadata around its path.
    if let Ok(result) = jett_driver::query_source_completions_at(source, path, 1, 1) {
        let current_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        for candidate in result.candidates {
            let candidate_path = Path::new(&candidate.file_path)
                .canonicalize()
                .unwrap_or_else(|_| candidate.file_path.clone().into());
            if candidate_path == current_path {
                continue;
            }
            let Some(rank) = jett_driver::completion_match_rank(&candidate.name, &prefix) else {
                continue;
            };
            let kind_name = jett_driver::query_kind_name(candidate.kind);
            items.push(completion_item(
                candidate.name,
                kind_name,
                rank,
                candidate.signature,
                &prefix,
            ));
        }
    }

    // Parse the open buffer separately so unsaved and private declarations take
    // precedence over stale on-disk declarations while retaining their details.
    for (name, kind) in jett_driver::completions_at(source, line, column) {
        let kind_name = jett_driver::query_kind_name(kind);
        let Some(rank) = jett_driver::completion_match_rank(&name, &prefix) else {
            continue;
        };
        let item_kind = completion_item_kind(kind_name);
        if items
            .iter()
            .any(|item| item.label == name && item.kind == Some(item_kind))
        {
            continue;
        }
        let detail = signatures.get(&name).cloned();
        items.push(completion_item(name, kind_name, rank, detail, &prefix));
    }

    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    let source_line = source_line(source, usize::try_from(position.line).ok()?)?;
    let suffix_length = source_line
        .chars()
        .skip(column.saturating_sub(1) as usize)
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .count() as u32;
    let replacement_range = Range::new(
        Position::new(
            position.line,
            position
                .character
                .checked_sub(prefix.encode_utf16().count() as u32)?,
        ),
        Position::new(
            position.line,
            position.character.checked_add(suffix_length)?,
        ),
    );
    for item in &mut items {
        item.text_edit = Some(CompletionTextEdit::Edit(TextEdit {
            range: replacement_range,
            new_text: item.label.clone(),
        }));
    }
    Some(items)
}

#[allow(deprecated)]
fn workspace_symbols_for_documents(
    documents: &HashMap<Url, DocumentState>,
    query: &str,
) -> Vec<SymbolInformation> {
    let query = query.to_lowercase();
    let mut symbols = Vec::new();

    for (uri, document) in documents {
        let Ok(outline) = jett_driver::query_source_file_symbols(&document.text, "<lsp-document>")
        else {
            continue;
        };

        symbols.extend(outline.symbols.into_iter().filter_map(|symbol| {
            if !symbol.name.to_lowercase().contains(&query) {
                return None;
            }
            let start = lsp_position_from_driver(&document.text, symbol.line, symbol.column)?;
            let end = lsp_position_from_driver(&document.text, symbol.end_line, symbol.end_column)?;
            Some(SymbolInformation {
                name: symbol.name,
                kind: document_symbol_kind(&symbol.kind),
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: Range::new(start, end),
                },
                container_name: symbol.namespace,
            })
        }));
    }

    symbols.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.location.uri.as_str().cmp(right.location.uri.as_str()))
            .then_with(|| left.location.range.start.cmp(&right.location.range.start))
    });
    symbols
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

fn document_highlights(source: &str, position: Position) -> Option<Vec<DocumentHighlight>> {
    let (line, column) = driver_position(source, position)?;
    let definition = jett_driver::goto_definition(source, line, column);
    let mut spans = jett_driver::references_at(source, line, column);

    if let Some(definition) = definition
        && !spans.contains(&definition)
    {
        spans.push(definition);
    }
    spans.sort_unstable();
    spans.dedup();

    (!spans.is_empty()).then(|| {
        spans
            .into_iter()
            .map(|span| DocumentHighlight {
                range: Range::new(lsp_position(source, span.0), lsp_position(source, span.1)),
                kind: Some(
                    if Some(span) == definition || is_assignment_target(source, span.1) {
                        DocumentHighlightKind::WRITE
                    } else {
                        DocumentHighlightKind::READ
                    },
                ),
            })
            .collect()
    })
}

fn is_assignment_target(source: &str, end: u32) -> bool {
    source
        .get(end as usize..)
        .map(|tail| tail.trim_start_matches([' ', '\t']))
        .and_then(|tail| tail.strip_prefix('='))
        .is_some_and(|tail| !tail.starts_with('='))
}

fn rename_edit(
    source: &str,
    uri: &Url,
    position: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    // Contextual keywords (for example `value`) can be valid binding names;
    // validate their actual declaration context with the compiler below.
    if !new_name
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        || !new_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    let (line, column) = driver_position(source, position)?;
    // External declarations cannot be renamed by editing only their local uses.
    let definition = jett_driver::goto_definition(source, line, column)?;
    let old_name = source.get(definition.0 as usize..definition.1 as usize)?;
    let mut spans = jett_driver::references_at(source, line, column);
    spans.push(definition);
    for span in &mut spans {
        let text = source.get(span.0 as usize..span.1 as usize)?;
        if text != old_name {
            // Resolved qualified calls cover their entire namespace path.
            if !text.strip_suffix(old_name)?.ends_with('.') {
                return None;
            }
            span.0 = span.1.checked_sub(u32::try_from(old_name.len()).ok()?)?;
        }
    }
    spans.sort_unstable();
    spans.dedup();
    let mut renamed = source.to_string();
    for &(start, end) in spans.iter().rev() {
        renamed.replace_range(start as usize..end as usize, new_name);
    }
    // Enforce declaration naming rules, duplicate bindings and no-shadowing
    // through the compiler rather than emitting a refactor that breaks them.
    if jett_driver::build_source(&renamed, "<lsp-rename>")
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
    {
        return None;
    }
    let edits = spans
        .into_iter()
        .map(|(start, end)| TextEdit {
            range: Range::new(lsp_position(source, start), lsp_position(source, end)),
            new_text: new_name.to_string(),
        })
        .collect();

    Some(WorkspaceEdit {
        changes: Some(HashMap::from([(uri.clone(), edits)])),
        document_changes: None,
        change_annotations: None,
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

fn folding_ranges_for_source(source: &str) -> Vec<FoldingRange> {
    let lexed = Lexer::new(source, jett_common::FileId::new(0)).tokenize();
    let mut starts = Vec::new();
    let mut ranges = Vec::new();
    let mut previous_code_offset = 0;

    for token in lexed.tokens {
        match token.kind {
            TokenKind::Indent => {
                // Blank and comment-only lines do not produce indentation
                // tokens. Keep the actual header visible when folding.
                starts.push(lsp_position(source, previous_code_offset).line);
            }
            TokenKind::Dedent => {
                let Some(start_line) = starts.pop() else {
                    continue;
                };
                let dedent_line = lsp_position(source, token.span.start).line;
                let at_unterminated_eof =
                    token.span.start as usize == source.len() && !source.ends_with(['\n', '\r']);
                let end_line = if at_unterminated_eof {
                    dedent_line
                } else {
                    dedent_line.saturating_sub(1)
                };
                if end_line > start_line {
                    ranges.push(FoldingRange {
                        start_line,
                        start_character: None,
                        end_line,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }
            TokenKind::Newline | TokenKind::Eof => {}
            _ => previous_code_offset = token.span.start,
        }
    }

    ranges.sort_by_key(|range| (range.start_line, std::cmp::Reverse(range.end_line)));
    ranges
}

fn tab_indentation_code_actions(
    source: &str,
    uri: &Url,
    diagnostics: &[Diagnostic],
) -> CodeActionResponse {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            if diagnostic.source.as_deref() != Some("jett")
                || !diagnostic.message.starts_with("tabs are not allowed")
            {
                return None;
            }

            let line_index = usize::try_from(diagnostic.range.start.line).ok()?;
            let indentation: String = source_line(source, line_index)?
                .chars()
                .take_while(|ch| matches!(ch, ' ' | '\t'))
                .collect();
            if !indentation.contains('\t') {
                return None;
            }

            let end_character = u32::try_from(indentation.encode_utf16().count()).ok()?;
            let edit = TextEdit {
                range: Range::new(
                    Position::new(diagnostic.range.start.line, 0),
                    Position::new(diagnostic.range.start.line, end_character),
                ),
                new_text: indentation.replace('\t', "    "),
            };
            let changes = HashMap::from([(uri.clone(), vec![edit])]);

            Some(CodeActionOrCommand::CodeAction(CodeAction {
                title: "Replace tab indentation with spaces".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..WorkspaceEdit::default()
                }),
                is_preferred: Some(true),
                ..CodeAction::default()
            }))
        })
        .collect()
}

const SEMANTIC_KEYWORD: u32 = 0;
const SEMANTIC_TYPE: u32 = 1;
const SEMANTIC_NUMBER: u32 = 2;
const SEMANTIC_STRING: u32 = 3;
const SEMANTIC_OPERATOR: u32 = 4;
const SEMANTIC_COMMENT: u32 = 5;

fn semantic_token_type(kind: jett_lexer::TokenKind) -> Option<u32> {
    use jett_lexer::TokenKind;

    match kind {
        TokenKind::Int8
        | TokenKind::Int16
        | TokenKind::Int32
        | TokenKind::Int64
        | TokenKind::Uint8
        | TokenKind::Uint16
        | TokenKind::Uint32
        | TokenKind::Uint64
        | TokenKind::Float32
        | TokenKind::Float64
        | TokenKind::String_
        | TokenKind::Bool_
        | TokenKind::Bytes_
        | TokenKind::List_
        | TokenKind::Map_
        | TokenKind::Set_ => Some(SEMANTIC_TYPE),
        TokenKind::IntLiteral | TokenKind::FloatLiteral => Some(SEMANTIC_NUMBER),
        TokenKind::StringStart
        | TokenKind::StringMid
        | TokenKind::StringEnd
        | TokenKind::StringLiteral => Some(SEMANTIC_STRING),
        TokenKind::Eq
        | TokenKind::EqEq
        | TokenKind::NotEq
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::LtEq
        | TokenKind::GtEq
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::AmpAmp
        | TokenKind::PipePipe
        | TokenKind::Bang
        | TokenKind::Modulo
        | TokenKind::And
        | TokenKind::Or
        | TokenKind::Not
        | TokenKind::Is
        | TokenKind::Within => Some(SEMANTIC_OPERATOR),
        TokenKind::Ident
        | TokenKind::Value
        | TokenKind::Dot
        | TokenKind::Comma
        | TokenKind::Colon
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::LBracket
        | TokenKind::RBracket
        | TokenKind::Hash
        | TokenKind::Newline
        | TokenKind::Indent
        | TokenKind::Dedent
        | TokenKind::Eof
        | TokenKind::InvalidToken => None,
        _ => Some(SEMANTIC_KEYWORD),
    }
}

fn semantic_tokens_for_source(source: &str) -> Vec<SemanticToken> {
    let lexed = jett_lexer::tokenize(source, jett_common::FileId::new(0));
    let mut spans = lexed
        .tokens
        .iter()
        .filter_map(|token| {
            semantic_token_type(token.kind)
                .map(|token_type| (token.span.start, token.span.end, token_type))
        })
        .collect::<Vec<_>>();
    spans.extend(
        lexed
            .comments
            .iter()
            .map(|comment| (comment.span.start, comment.span.end, SEMANTIC_COMMENT)),
    );
    spans.sort_unstable_by_key(|(start, end, token_type)| (*start, *end, *token_type));

    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    let mut tokens = Vec::with_capacity(spans.len());
    // Lexical spans are ordered and non-overlapping. Walk source characters
    // once instead of rescanning every prefix twice per token.
    let mut characters = source.char_indices().peekable();
    let mut position = Position::new(0, 0);
    let mut previous_was_cr = false;
    let mut position_at = |offset: u32| {
        while characters
            .peek()
            .is_some_and(|(index, _)| *index < offset as usize)
        {
            let (_, character) = characters.next().expect("peeked character");
            match character {
                '\r' => {
                    position.line += 1;
                    position.character = 0;
                }
                '\n' => {
                    if !previous_was_cr {
                        position.line += 1;
                    }
                    position.character = 0;
                }
                _ => position.character += character.len_utf16() as u32,
            }
            previous_was_cr = character == '\r';
        }
        position
    };
    for (start, end, token_type) in spans {
        let start = position_at(start);
        let end = position_at(end);
        if start.line != end.line || start.character >= end.character {
            continue;
        }
        let delta_line = start.line - previous_line;
        let delta_start = if delta_line == 0 {
            start.character - previous_start
        } else {
            start.character
        };
        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length: end.character - start.character,
            token_type,
            token_modifiers_bitset: 0,
        });
        previous_line = start.line;
        previous_start = start.character;
    }
    tokens
}

fn semantic_tokens_response(source: &str) -> SemanticTokensResult {
    SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: semantic_tokens_for_source(source),
    })
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

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        let Some(document) = documents.get(uri) else {
            return Ok(None);
        };
        Ok(document_highlights(&document.text, position))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let documents = self.documents.read().await;
        let Some(document) = documents.get(uri) else {
            return Ok(None);
        };

        Ok(rename_edit(&document.text, uri, position, &params.new_name))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };

        Ok(formatting_edits(&document.text))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(folding_ranges_for_source(&document.text)))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        if params.context.only.as_ref().is_some_and(|kinds| {
            !kinds
                .iter()
                .any(|kind| kind.as_str().is_empty() || *kind == CodeActionKind::QUICKFIX)
        }) {
            return Ok(None);
        }
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let actions = tab_indentation_code_actions(
            &document.text,
            &params.text_document.uri,
            &params.context.diagnostics,
        );
        Ok((!actions.is_empty()).then_some(actions))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(semantic_tokens_response(&document.text)))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;

        let docs = self.documents.read().await;
        let Some(document) = docs.get(uri) else {
            return Ok(None);
        };
        let position = params.text_document_position.position;
        let path = uri
            .to_file_path()
            .unwrap_or_else(|_| std::path::PathBuf::from("<lsp-document>"));
        let Some(items) = completion_items_for_source(&document.text, &path, position) else {
            return Ok(None);
        };

        Ok((!items.is_empty()).then_some(CompletionResponse::Array(items)))
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

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ranges) = params
            .positions
            .into_iter()
            .map(|position| selection_range_for_position(&document.text, position))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };

        Ok(Some(ranges))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let documents = self.documents.read().await;
        Ok(Some(workspace_symbols_for_documents(
            &documents,
            &params.query,
        )))
    }
}

fn diagnostics_for_source(source: &str, file_path: &str) -> Vec<Diagnostic> {
    let result = jett_driver::build_source(source, file_path);
    // Windows drive paths also parse as URLs with a one-letter scheme.
    // Prefer a filesystem URL for native absolute paths from validate().
    let uri = Url::from_file_path(file_path)
        .ok()
        .or_else(|| Url::parse(file_path).ok());

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
            let related_information = uri.as_ref().and_then(|uri| {
                let labels = d
                    .labels
                    .iter()
                    .filter(|label| label.span.file == d.span.file)
                    .map(|label| DiagnosticRelatedInformation {
                        location: Location {
                            uri: uri.clone(),
                            range: Range::new(
                                lsp_position(&result.source, label.span.start),
                                lsp_position(&result.source, label.span.end),
                            ),
                        },
                        message: label.message.clone(),
                    })
                    .collect::<Vec<_>>();
                (!labels.is_empty()).then_some(labels)
            });

            Diagnostic {
                range,
                severity,
                code: Some(NumberOrString::String(d.code.to_string())),
                source: Some("jett".to_string()),
                message: d.message.clone(),
                related_information,
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
    use std::path::Path;
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

    #[test]
    fn diagnostics_for_source_preserves_compiler_labels_as_related_information() {
        let source = "namespace app\n\nfunction main() returns nothing:\n    int64 value = 1\n    int64 value = 2\n    return nothing\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        let diagnostics = diagnostics_for_source(source, uri.as_str());
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                matches!(
                    diagnostic.code,
                    Some(NumberOrString::String(ref code)) if code == "E0204"
                )
            })
            .expect("duplicate binding should diagnose");
        let related = diagnostic
            .related_information
            .as_ref()
            .expect("the original binding label should be preserved");

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].location.uri, uri);
        assert_eq!(related[0].message, "previously defined here");
        assert_eq!(
            related[0].location.range,
            Range::new(Position::new(3, 10), Position::new(3, 15))
        );
    }

    #[cfg(windows)]
    #[test]
    fn diagnostic_labels_use_file_urls_for_windows_paths() {
        let source = "function main() returns nothing:\n    int64 value = 1\n    int64 value = 2\n    return nothing\n";
        let diagnostics = diagnostics_for_source(source, r"C:\workspace\main.jett");
        let related = diagnostics
            .iter()
            .find_map(|diagnostic| diagnostic.related_information.as_ref())
            .unwrap();
        assert_eq!(
            related[0].location.uri.as_str(),
            "file:///C:/workspace/main.jett"
        );
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
    fn completion_items_preserve_driver_ranking_and_signature_metadata() {
        let source = "namespace app\n\nexport function beta(value: int64) returns int64:\n    return value\n\nfunction better() returns int64:\n    return 1\n\nfunction main() returns int64:\n    return be\n";

        let items = completion_items_for_source(
            source,
            Path::new("/workspace/main.jett"),
            Position::new(9, 13),
        )
        .expect("valid completion query");

        let beta = items
            .iter()
            .find(|item| item.label == "app.beta")
            .expect("unsaved function completion");
        assert_eq!(beta.kind, Some(CompletionItemKind::FUNCTION));
        assert_eq!(
            beta.detail.as_deref(),
            Some("app.beta(value: int64) returns int64")
        );
        assert_eq!(beta.filter_text.as_deref(), Some("beta"));
        assert_eq!(beta.sort_text.as_deref(), Some("020:app.beta:function"));
        assert!(
            items.iter().any(|item| item.label == "app.better"),
            "private declarations in the current namespace must remain visible"
        );
        assert!(items.windows(2).all(|items| {
            items[0].sort_text.as_deref().unwrap_or_default()
                <= items[1].sort_text.as_deref().unwrap_or_default()
        }));
    }

    #[test]
    fn completion_items_merge_project_symbols_without_stale_document_declarations() {
        let root = std::env::temp_dir().join(format!(
            "jett-lsp-completions-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp project directory");
        std::fs::write(root.join("jett.proj"), "name: lsp_completion_fixture\n")
            .expect("project marker");
        std::fs::write(
            root.join("util.jett"),
            "namespace util\n\nexport function helper(value: int64) returns int64:\n    return value\n",
        )
        .expect("project helper");
        let path = root.join("main.jett");
        std::fs::write(
            &path,
            "namespace app\n\nexport function helper_old() returns int64:\n    return 1\n",
        )
        .expect("stale on-disk document");
        let source = "namespace app\n\nfunction helper_local() returns int64:\n    return 1\n\nfunction main() returns int64:\n    return hel\n";

        let items = completion_items_for_source(source, &path, Position::new(6, 14))
            .expect("valid completion query");

        let project_helper = items
            .iter()
            .find(|item| item.label == "util.helper")
            .expect("sibling project completion");
        assert_eq!(
            project_helper.detail.as_deref(),
            Some("util.helper(value: int64) returns int64")
        );
        assert!(items.iter().any(|item| item.label == "app.helper_local"));
        assert!(!items.iter().any(|item| item.label == "app.helper_old"));

        for saved_text in [Some("function broken("), None] {
            if let Some(saved_text) = saved_text {
                std::fs::write(&path, saved_text).unwrap();
            } else {
                std::fs::remove_file(&path).unwrap();
            }
            let items = completion_items_for_source(source, &path, Position::new(6, 14)).unwrap();
            assert!(
                items.iter().any(|item| item.label == "util.helper"),
                "invalid or missing saved file must not suppress sibling completions"
            );
            assert!(items.iter().any(|item| item.label == "app.helper_local"));
        }
        std::fs::remove_dir_all(&root).expect("remove temp project");
    }

    #[test]
    fn qualified_completion_replaces_namespace_prefix_and_identifier_suffix() {
        let source = "namespace app\nfunction beta() returns int64:\n    return 1\nfunction main() returns int64:\n    return app.beta\n";
        let offset = source.find("app.beta").unwrap() + "app.be".len();
        let items = completion_items_for_source(
            source,
            Path::new("/workspace/main.jett"),
            lsp_position(source, offset as u32),
        )
        .unwrap();
        let item = items.iter().find(|item| item.label == "app.beta").unwrap();
        assert_eq!(
            item.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(4, 11), Position::new(4, 19)),
                new_text: "app.beta".to_string(),
            }))
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
    fn server_capabilities_advertise_selection_ranges() {
        let capabilities = server_capabilities();

        assert_eq!(
            capabilities.selection_range_provider,
            Some(SelectionRangeProviderCapability::Simple(true))
        );
    }

    #[test]
    fn server_capabilities_advertise_document_highlights() {
        let capabilities = server_capabilities();

        assert_eq!(
            capabilities.document_highlight_provider,
            Some(OneOf::Left(true))
        );
    }

    #[test]
    fn selection_ranges_expand_from_token_to_line_and_document() {
        let source = "function greet(name: string) returns string:\n    return \"🙂 \" + name\n";

        let selection = selection_range_for_position(source, Position::new(1, 19))
            .expect("name token should be selectable");
        assert_eq!(
            selection.range,
            Range::new(Position::new(1, 19), Position::new(1, 23))
        );

        let line = selection.parent.expect("trimmed line parent");
        assert_eq!(
            line.range,
            Range::new(Position::new(1, 4), Position::new(1, 23))
        );

        let document = line.parent.expect("document parent");
        assert_eq!(
            document.range,
            Range::new(Position::new(0, 0), Position::new(2, 0))
        );
        assert!(document.parent.is_none());
    }

    #[test]
    fn selection_ranges_keep_whitespace_positions_contained() {
        let source = "function main() returns nothing:\n    \n";

        let selection = selection_range_for_position(source, Position::new(1, 2))
            .expect("whitespace position should be selectable");

        assert_eq!(
            selection.range,
            Range::new(Position::new(1, 0), Position::new(1, 4))
        );
    }

    #[test]
    fn server_capabilities_advertise_full_semantic_tokens() {
        let capabilities = server_capabilities();
        let Some(SemanticTokensServerCapabilities::SemanticTokensOptions(options)) =
            capabilities.semantic_tokens_provider
        else {
            panic!("expected semantic token options");
        };

        assert_eq!(options.range, Some(false));
        assert_eq!(options.full, Some(SemanticTokensFullOptions::Bool(true)));
        assert_eq!(
            options.legend.token_types,
            vec![
                SemanticTokenType::KEYWORD,
                SemanticTokenType::TYPE,
                SemanticTokenType::NUMBER,
                SemanticTokenType::STRING,
                SemanticTokenType::OPERATOR,
                SemanticTokenType::COMMENT,
            ]
        );
    }

    #[test]
    fn selection_ranges_contain_nonblank_indentation_and_trailing_whitespace() {
        for newline in ["\n", "\r", "\r\n"] {
            let source =
                format!("function main() returns nothing:{newline}    return nothing   {newline}");
            for column in [0, 2, 20, 21] {
                let position = Position::new(1, column);
                let mut selection = selection_range_for_position(&source, position).unwrap();
                assert!(selection.range.start <= position && position <= selection.range.end);
                while let Some(parent) = selection.parent {
                    assert!(parent.range.start <= selection.range.start);
                    assert!(selection.range.end <= parent.range.end);
                    selection = *parent;
                }
            }
        }
    }

    #[test]
    fn semantic_tokens_cover_jett_syntax_with_utf16_lengths() {
        let source = concat!(
            "namespace demo\n",
            "# note\n",
            "function f(value: int64) returns bool:\n",
            "    return value >= 42 and \"🙂\" != \"\"\n",
        );

        let tokens = semantic_tokens_for_source(source);
        let mut line = 0u32;
        let mut start = 0u32;
        let absolute = tokens
            .into_iter()
            .map(|token| {
                line += token.delta_line;
                start = if token.delta_line == 0 {
                    start + token.delta_start
                } else {
                    token.delta_start
                };
                (line, start, token.length, token.token_type)
            })
            .collect::<Vec<_>>();

        assert_eq!(
            absolute,
            vec![
                (0, 0, 9, 0),
                (1, 0, 6, 5),
                (2, 0, 8, 0),
                (2, 18, 5, 1),
                (2, 25, 7, 0),
                (2, 33, 4, 1),
                (3, 4, 6, 0),
                (3, 17, 2, 4),
                (3, 20, 2, 2),
                (3, 23, 3, 4),
                (3, 27, 4, 3),
                (3, 32, 2, 4),
                (3, 35, 2, 3),
            ]
        );
    }

    #[test]
    fn server_capabilities_advertise_workspace_symbol_search() {
        let capabilities = server_capabilities();

        assert_eq!(
            capabilities.workspace_symbol_provider,
            Some(OneOf::Left(true))
        );
    }

    #[test]
    fn semantic_tokens_response_wraps_full_document_tokens() {
        let response = semantic_tokens_response("return 1\n");
        let SemanticTokensResult::Tokens(tokens) = response else {
            panic!("expected a full semantic token response");
        };

        assert_eq!(tokens.result_id, None);
        assert_eq!(tokens.data.len(), 2);
        assert_eq!(tokens.data[0].token_type, SEMANTIC_KEYWORD);
        assert_eq!(tokens.data[1].token_type, SEMANTIC_NUMBER);
    }

    #[test]
    fn semantic_token_positions_match_utf16_across_mixed_line_endings() {
        let source = "return \"😀\"\r\n# explanation\rreturn 2\nreturn \"{1 + 2}\"\n";
        let lexed = jett_lexer::tokenize(source, jett_common::FileId::new(0));
        let mut expected: Vec<_> = lexed
            .tokens
            .iter()
            .filter_map(|token| {
                let kind = semantic_token_type(token.kind)?;
                let start = lsp_position(source, token.span.start);
                let end = lsp_position(source, token.span.end);
                (start.line == end.line && start.character < end.character).then_some((
                    start,
                    end.character - start.character,
                    kind,
                ))
            })
            .collect();
        expected.extend(lexed.comments.iter().map(|comment| {
            let start = lsp_position(source, comment.span.start);
            let end = lsp_position(source, comment.span.end);
            (start, end.character - start.character, SEMANTIC_COMMENT)
        }));
        expected.sort_by_key(|entry| entry.0);
        let mut line = 0;
        let mut column = 0;
        let actual: Vec<_> = semantic_tokens_for_source(source)
            .into_iter()
            .map(|token| {
                line += token.delta_line;
                column = if token.delta_line == 0 {
                    column + token.delta_start
                } else {
                    token.delta_start
                };
                (Position::new(line, column), token.length, token.token_type)
            })
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn workspace_symbols_search_all_open_documents_case_insensitively() {
        let first_uri = Url::parse("file:///workspace/auth.jett").unwrap();
        let second_uri = Url::parse("file:///workspace/admin.jett").unwrap();
        let mut documents = HashMap::new();
        documents.insert(
            first_uri.clone(),
            DocumentState {
                text:
                    "namespace auth\n\nexport function LoginUser() returns int64:\n    return 1\n"
                        .to_string(),
                version: 1,
            },
        );
        documents.insert(
            second_uri,
            DocumentState {
                text: "namespace admin\n\nexport function logout() returns int64:\n    return 2\n"
                    .to_string(),
                version: 1,
            },
        );

        let symbols = workspace_symbols_for_documents(&documents, "login");

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "auth.LoginUser");
        assert_eq!(symbols[0].kind, SymbolKind::FUNCTION);
        assert_eq!(symbols[0].location.uri, first_uri);
        assert_eq!(
            symbols[0].location.range,
            Range::new(Position::new(2, 16), Position::new(2, 25))
        );
        assert_eq!(symbols[0].container_name.as_deref(), Some("auth"));
    }

    #[test]
    fn server_capabilities_advertise_symbol_rename() {
        let capabilities = server_capabilities();

        assert_eq!(capabilities.rename_provider, Some(OneOf::Left(true)));
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
    fn server_capabilities_advertise_code_actions() {
        let capabilities = server_capabilities();

        assert_eq!(
            capabilities.code_action_provider,
            Some(CodeActionProviderCapability::Simple(true))
        );
    }

    #[test]
    fn tab_indentation_diagnostic_offers_a_four_space_quick_fix() {
        let source = "namespace app\nfunction main() returns int64:\n\treturn 1\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        let diagnostics = diagnostics_for_source(source, "main.jett");

        let actions = tab_indentation_code_actions(source, &uri, &diagnostics);

        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected a code action");
        };
        assert_eq!(action.title, "Replace tab indentation with spaces");
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(action.diagnostics.as_deref(), Some(&diagnostics[..1]));
        let changes = action
            .edit
            .as_ref()
            .and_then(|edit| edit.changes.as_ref())
            .expect("workspace edit changes");
        assert_eq!(
            changes.get(&uri),
            Some(&vec![TextEdit {
                range: Range::new(Position::new(2, 0), Position::new(2, 1)),
                new_text: "    ".to_string(),
            }])
        );
    }

    #[tokio::test]
    async fn tab_quick_fixes_respect_requested_code_action_kinds() {
        let source = "function main() returns int64:\n\treturn 1\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        let (service, _socket) = tower_lsp::LspService::new(JettBackend::new);
        let backend = service.inner();
        backend.documents.write().await.insert(
            uri.clone(),
            DocumentState {
                text: source.to_string(),
                version: 1,
            },
        );
        for (kind, expected) in [
            (CodeActionKind::SOURCE_ORGANIZE_IMPORTS, false),
            (CodeActionKind::QUICKFIX, true),
        ] {
            let result = backend
                .code_action(CodeActionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    range: Range::new(Position::new(1, 0), Position::new(1, 1)),
                    context: CodeActionContext {
                        diagnostics: diagnostics_for_source(source, "main.jett"),
                        only: Some(vec![kind]),
                        trigger_kind: None,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                })
                .await
                .unwrap();
            assert_eq!(result.is_some(), expected);
        }
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
    fn document_highlights_mark_the_declaration_as_write_and_uses_as_read() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n\nfunction main() returns int64:\n    return double(21)\n";

        let highlights = document_highlights(source, Position::new(6, 11))
            .expect("call should resolve to document highlights");

        assert_eq!(
            highlights,
            vec![
                DocumentHighlight {
                    range: Range::new(Position::new(2, 9), Position::new(2, 15)),
                    kind: Some(DocumentHighlightKind::WRITE),
                },
                DocumentHighlight {
                    range: Range::new(Position::new(6, 11), Position::new(6, 17)),
                    kind: Some(DocumentHighlightKind::READ),
                },
            ]
        );
    }

    #[test]
    fn document_highlights_distinguish_assignments_from_reads_and_equality() {
        let source = "function main() returns int64:\n    mutable int64 total = 1\n    total = total + 1\n    if total == 2:\n        return total\n    return 0\n";
        let highlights = document_highlights(source, Position::new(2, 5)).unwrap();
        let kinds: Vec<_> = highlights
            .iter()
            .map(|highlight| highlight.kind.unwrap())
            .collect();
        assert_eq!(
            kinds,
            vec![
                DocumentHighlightKind::WRITE,
                DocumentHighlightKind::WRITE,
                DocumentHighlightKind::READ,
                DocumentHighlightKind::READ,
                DocumentHighlightKind::READ
            ]
        );
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
    fn rename_edits_replace_a_declaration_and_all_its_references() {
        let source = "namespace app\n\nfunction double(value: int64) returns int64:\n    return value + value\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();

        let edit = rename_edit(source, &uri, Position::new(3, 11), "number")
            .expect("parameter reference should resolve");
        let changes = edit.changes.expect("document changes");
        let edits = changes.get(&uri).expect("current document edits");

        assert_eq!(edits.len(), 3);
        assert!(edits.iter().all(|edit| edit.new_text == "number"));
        assert_eq!(edits[0].range.start, Position::new(2, 16));
        assert_eq!(edits[1].range.start, Position::new(3, 11));
        assert_eq!(edits[2].range.start, Position::new(3, 19));
    }

    #[test]
    fn rename_rejects_invalid_names_collisions_and_external_declarations() {
        let source = "namespace app\nfunction f(value: int64, other: int64) returns int64:\n    return value + other\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        for name in [
            "",
            "return",
            "two words",
            "foo.bar",
            "1value",
            " other",
            "other",
        ] {
            assert!(
                rename_edit(source, &uri, Position::new(2, 11), name).is_none(),
                "{name:?}"
            );
        }
        let external =
            "namespace app\nfunction main() returns int64:\n    return int64.max(1, 2)\n";
        assert!(rename_edit(external, &uri, Position::new(2, 18), "bigger").is_none());
    }

    #[test]
    fn rename_keeps_namespace_qualifiers() {
        let source = "namespace app\nfunction double(value: int64) returns int64:\n    return value + value\nfunction main() returns int64:\n    return app.double(21)\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        let edit = rename_edit(source, &uri, Position::new(4, 18), "twice").unwrap();
        let changes = edit.changes.unwrap();
        let edits = &changes[&uri];
        assert_eq!(edits.len(), 2);
        assert_eq!(
            edits[1].range,
            Range::new(Position::new(4, 15), Position::new(4, 21))
        );
    }

    #[test]
    fn rename_accepts_contextual_binding_names() {
        let source = "function double(number: int64) returns int64:\n    return number + number\n";
        let uri = Url::parse("file:///workspace/main.jett").unwrap();
        assert!(rename_edit(source, &uri, Position::new(1, 12), "value").is_some());
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

    #[test]
    fn folding_ranges_follow_nested_jett_blocks() {
        let source = "namespace app\nfunction main() returns int64:\n    mutable int64 total = 2\n    if total > 0:\n        while total > 1:\n            total = total - 1\n    return total\n";

        let ranges = folding_ranges_for_source(source);
        let lines = ranges
            .iter()
            .map(|range| (range.start_line, range.end_line))
            .collect::<Vec<_>>();

        assert_eq!(lines, vec![(1, 6), (3, 5), (4, 5)]);
        assert!(
            ranges
                .iter()
                .all(|range| range.kind == Some(FoldingRangeKind::Region))
        );
    }

    #[test]
    fn folding_ranges_keep_headers_before_comments_and_blank_lines() {
        for newline in ["\n", "\r", "\r\n"] {
            let source = [
                "function main() returns int64:",
                "",
                "    # explanation",
                "    if true:",
                "        # nested explanation",
                "",
                "        return 1",
                "    return 0",
            ]
            .join(newline);
            let ranges = folding_ranges_for_source(&source);
            let lines: Vec<_> = ranges
                .iter()
                .map(|range| (range.start_line, range.end_line))
                .collect();
            assert_eq!(lines, vec![(0, 7), (3, 6)], "{newline:?}");
        }
    }

    #[test]
    fn server_capabilities_advertise_folding_ranges() {
        let capabilities = server_capabilities();

        assert!(matches!(
            capabilities.folding_range_provider,
            Some(FoldingRangeProviderCapability::Simple(true))
        ));
    }
}
