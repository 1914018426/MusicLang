use std::collections::HashMap;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Diagnostic,
    DiagnosticRelatedInformation, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, DocumentSymbol,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeKind,
    FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, InitializeParams, InlayHint, InlayHintLabel, InlayHintParams, Location,
    MarkedString, Position, PrepareRenameResponse, PublishDiagnosticsParams, Range,
    ReferenceParams, RenameOptions, RenameParams, SelectionRange, SelectionRangeParams,
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelp,
    SignatureHelpOptions, SignatureHelpParams, SignatureInformation, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri, WorkspaceEdit,
    WorkspaceSymbol, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use musiclang_core::{Severity, SourceId, SourceMap, BUILT_IN_STYLES};

#[derive(Debug, Default)]
struct DocumentStore {
    sources: SourceMap,
    documents: HashMap<String, OpenDocument>,
}

#[derive(Debug)]
struct OpenDocument {
    source_id: SourceId,
    text: String,
}

impl DocumentStore {
    fn open(&mut self, uri: &Uri, text: String) {
        let key = uri.to_string();
        let source_id = self.sources.add(key.clone(), text.clone());
        self.documents.insert(key, OpenDocument { source_id, text });
    }

    #[cfg(test)]
    fn insert(&mut self, uri: String, text: String) {
        let uri = uri.parse::<Uri>().expect("test URI should be valid");
        self.open(&uri, text);
    }

    fn change(&mut self, uri: &Uri, text: String) {
        self.open(uri, text);
    }

    fn close(&mut self, uri: &Uri) {
        self.documents.remove(&uri.to_string());
    }

    #[cfg(test)]
    fn contains_key(&self, uri: &str) -> bool {
        self.documents.contains_key(uri)
    }

    fn get_source(&self, uri: &Uri) -> Option<&str> {
        self.documents
            .get(&uri.to_string())
            .map(|document| document.text.as_str())
    }

    fn source_file(&self, uri: &Uri) -> Option<musiclang_core::SourceFile> {
        let document = self.documents.get(&uri.to_string())?;
        self.sources.get(document.source_id).cloned()
    }

    fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.documents
            .iter()
            .map(|(uri, document)| (uri.as_str(), document.text.as_str()))
    }
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        completion_provider: Some(lsp_types::CompletionOptions {
            trigger_characters: Some(vec![
                ":".to_string(),
                ".".to_string(),
                "(".to_string(),
                " ".to_string(),
            ]),
            ..lsp_types::CompletionOptions::default()
        }),
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
        inlay_hint_provider: Some(lsp_types::OneOf::Left(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string(), " ".to_string()]),
            retrigger_characters: None,
            work_done_progress_options: Default::default(),
        }),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        references_provider: Some(lsp_types::OneOf::Left(true)),
        document_highlight_provider: Some(lsp_types::OneOf::Left(true)),
        document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        rename_provider: Some(lsp_types::OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
        workspace_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                work_done_progress_options: Default::default(),
                legend: semantic_tokens_legend(),
                range: Some(false),
                full: Some(SemanticTokensFullOptions::Bool(true)),
            },
        )),
        ..ServerCapabilities::default()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(server_capabilities())?;
    let params = connection.initialize(capabilities)?;
    let _params: InitializeParams = serde_json::from_value(params)?;
    let mut documents = DocumentStore::default();

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                handle_request(&connection, request, &documents)?;
            }
            Message::Notification(notification) => {
                handle_notification(&connection, notification, &mut documents)?;
            }
            Message::Response(_) => {}
        }
    }

    io_threads.join()?;
    Ok(())
}

fn handle_request(
    connection: &Connection,
    request: Request,
    documents: &DocumentStore,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    match request.method.as_str() {
        "textDocument/hover" => {
            let params: HoverParams = serde_json::from_value(request.params)?;
            let hover = hover_at(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, hover)))?;
        }
        "textDocument/completion" => {
            let params: CompletionParams = serde_json::from_value(request.params)?;
            connection.sender.send(Message::Response(Response::new_ok(
                request.id,
                completion_items(documents, &params),
            )))?;
        }
        "textDocument/codeAction" => {
            let params: CodeActionParams = serde_json::from_value(request.params)?;
            let actions = code_actions(&params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, actions)))?;
        }
        "textDocument/definition" => {
            let params: GotoDefinitionParams = serde_json::from_value(request.params)?;
            let definition = definition_at(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, definition)))?;
        }
        "textDocument/documentSymbol" => {
            let params: DocumentSymbolParams = serde_json::from_value(request.params)?;
            let symbols = document_symbols(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, symbols)))?;
        }
        "textDocument/references" => {
            let params: ReferenceParams = serde_json::from_value(request.params)?;
            let references = references_at(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, references)))?;
        }
        "textDocument/documentHighlight" => {
            let params: DocumentHighlightParams = serde_json::from_value(request.params)?;
            let highlights = document_highlights(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, highlights)))?;
        }
        "textDocument/inlayHint" => {
            let params: InlayHintParams = serde_json::from_value(request.params)?;
            let hints = inlay_hints(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, hints)))?;
        }
        "textDocument/signatureHelp" => {
            let params: SignatureHelpParams = serde_json::from_value(request.params)?;
            let help = signature_help(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, help)))?;
        }
        "textDocument/formatting" => {
            let params: DocumentFormattingParams = serde_json::from_value(request.params)?;
            let edits = document_formatting(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, edits)))?;
        }
        "textDocument/foldingRange" => {
            let params: FoldingRangeParams = serde_json::from_value(request.params)?;
            let ranges = folding_ranges(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, ranges)))?;
        }
        "textDocument/rename" => {
            let params: RenameParams = serde_json::from_value(request.params)?;
            let edit = rename_symbol(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, edit)))?;
        }
        "textDocument/prepareRename" => {
            let params: lsp_types::TextDocumentPositionParams =
                serde_json::from_value(request.params)?;
            let response = prepare_rename(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, response)))?;
        }
        "textDocument/selectionRange" => {
            let params: SelectionRangeParams = serde_json::from_value(request.params)?;
            let ranges = selection_ranges(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, ranges)))?;
        }
        "textDocument/semanticTokens/full" => {
            let params: SemanticTokensParams = serde_json::from_value(request.params)?;
            let tokens = semantic_tokens(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, tokens)))?;
        }
        "workspace/symbol" => {
            let params: WorkspaceSymbolParams = serde_json::from_value(request.params)?;
            let symbols = workspace_symbols(documents, &params);
            connection
                .sender
                .send(Message::Response(Response::new_ok(request.id, symbols)))?;
        }
        _ => connection.sender.send(Message::Response(Response::new_ok(
            request.id,
            serde_json::Value::Null,
        )))?,
    }
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    documents: &mut DocumentStore,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;
            let uri = params.text_document.uri;
            documents.open(&uri, params.text_document.text);
            publish_diagnostics(connection, uri, documents)?;
        }
        "textDocument/didChange" => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;
            if let Some(change) = params.content_changes.into_iter().last() {
                let uri = params.text_document.uri;
                documents.change(&uri, change.text);
                publish_diagnostics(connection, uri, documents)?;
            }
        }
        "textDocument/didClose" => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params)?;
            close_document(documents, &params.text_document.uri);
            publish_empty_diagnostics(connection, params.text_document.uri)?;
        }
        _ => {}
    }
    Ok(())
}

fn close_document(documents: &mut DocumentStore, uri: &Uri) {
    documents.close(uri);
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    documents: &DocumentStore,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let Some(source) = documents.get_source(&uri) else {
        return Ok(());
    };
    let Some(source_file) = documents.source_file(&uri) else {
        return Ok(());
    };
    let diagnostics = musiclang_compiler::diagnose_source_file(&source_file)
        .into_iter()
        .map(|diagnostic| to_lsp_diagnostic(source, &uri, diagnostic))
        .collect();
    publish_diagnostics_payload(connection, uri, diagnostics)
}

fn publish_empty_diagnostics(
    connection: &Connection,
    uri: Uri,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    publish_diagnostics_payload(connection, uri, Vec::new())
}

fn publish_diagnostics_payload(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<Diagnostic>,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    connection
        .sender
        .send(Message::Notification(Notification::new(
            "textDocument/publishDiagnostics".to_string(),
            PublishDiagnosticsParams {
                uri,
                diagnostics,
                version: None,
            },
        )))?;
    Ok(())
}

fn to_lsp_diagnostic(
    source: &str,
    uri: &Uri,
    diagnostic: musiclang_core::Diagnostic,
) -> Diagnostic {
    let range = diagnostic_range(source, &diagnostic);
    let related_information = diagnostic_related_information(source, uri, &diagnostic);
    let data = diagnostic_data(&diagnostic, uri);
    Diagnostic {
        range,
        severity: Some(match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(lsp_types::NumberOrString::String(diagnostic.code)),
        source: Some("musiclang".to_string()),
        message: diagnostic.message,
        related_information,
        data,
        ..Diagnostic::default()
    }
}

fn diagnostic_related_information(
    source: &str,
    uri: &Uri,
    diagnostic: &musiclang_core::Diagnostic,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let mut information = diagnostic
        .labels
        .iter()
        .map(|label| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: span_range(source, label.span),
            },
            message: label.message.clone(),
        })
        .collect::<Vec<_>>();
    information.extend(
        diagnostic
            .related
            .iter()
            .map(|related| DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: span_range(source, related.span),
                },
                message: related.message.clone(),
            }),
    );
    (!information.is_empty()).then_some(information)
}

fn diagnostic_data(
    diagnostic: &musiclang_core::Diagnostic,
    uri: &Uri,
) -> Option<serde_json::Value> {
    let mut data = serde_json::Map::new();
    if let Some(span) = diagnostic.span {
        data.insert(
            "source_id".to_string(),
            serde_json::Value::from(span.source_id.0),
        );
        data.insert(
            "source_name".to_string(),
            serde_json::Value::String(uri.to_string()),
        );
    }
    if let Some(rule) = &diagnostic.rule {
        data.insert("rule".to_string(), serde_json::Value::String(rule.clone()));
    }
    if let Some(style) = &diagnostic.style {
        data.insert(
            "style".to_string(),
            serde_json::Value::String(style.clone()),
        );
    }
    if let Some(help) = &diagnostic.help {
        data.insert("help".to_string(), serde_json::Value::String(help.clone()));
    }
    (!data.is_empty()).then_some(serde_json::Value::Object(data))
}

fn diagnostic_range(source: &str, diagnostic: &musiclang_core::Diagnostic) -> Range {
    let Some(span) = diagnostic.span else {
        let line = diagnostic.line.saturating_sub(1) as u32;
        let column = diagnostic.column.saturating_sub(1) as u32;
        return Range {
            start: Position::new(line, column),
            end: Position::new(line, column + 1),
        };
    };

    span_range(source, span)
}

fn span_range(source: &str, span: musiclang_core::Span) -> Range {
    if span.start <= span.end && span.end <= source.len() {
        let start = byte_offset_to_position(source, span.start);
        let mut end = byte_offset_to_position(source, span.end);
        if start == end {
            end.character += 1;
        }
        return Range { start, end };
    }

    let line = span.line.saturating_sub(1) as u32;
    let column = span.column.saturating_sub(1) as u32;
    Range {
        start: Position::new(line, column),
        end: Position::new(line, column + 1),
    }
}

fn byte_offset_to_position(source: &str, byte_offset: usize) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;
    for (index, ch) in source.char_indices() {
        if index >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }
    Position::new(line, character)
}

fn code_actions(params: &CodeActionParams) -> Option<CodeActionResponse> {
    let actions = params
        .context
        .diagnostics
        .iter()
        .filter_map(help_code_action)
        .collect::<Vec<_>>();
    (!actions.is_empty()).then_some(actions)
}

fn help_code_action(diagnostic: &Diagnostic) -> Option<CodeActionOrCommand> {
    let help = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("help"))
        .and_then(|help| help.as_str())?;
    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("MusicLang: {help}"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        is_preferred: Some(false),
        ..CodeAction::default()
    }))
}

fn hover_at(documents: &DocumentStore, params: &HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let source = documents.get_source(uri)?;
    let position = params.text_document_position_params.position;
    let word = word_at(source, position)?;
    if local_style_names(source).iter().any(|style| style == &word) {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "style `{word}`: local MusicLang style declaration"
            ))),
            range: None,
        });
    }
    if local_function_names(source)
        .iter()
        .any(|function| function == &word)
    {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "fn `{word}`: local MusicLang function"
            ))),
            range: None,
        });
    }
    if local_variable_names(source)
        .iter()
        .any(|variable| variable == &word)
    {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "let `{word}`: local MusicLang value"
            ))),
            range: None,
        });
    }

    let text = match word.as_str() {
        "score" => "score block: top-level musical work",
        "voice" => "voice block: one MIDI track / musical line",
        "note" => "note pitch_expr, duration_expr",
        "chord" => "chord [pitch_expr, ...], duration_expr",
        "style" => "style declaration activates music-theory checks",
        "override" => "override rule allow { ... } suppresses a local style rule",
        "scale" => "style rule: pitch classes must belong to active scale",
        "chord_vocab" => "style rule: chords must belong to configured vocabulary",
        "meter" => "score metadata and style rule for time signature",
        "tempo_range" => "style rule: tempo must be inside configured BPM range",
        "instrument_range" => "style rule: pitch must fit configured MIDI program range",
        "voice_spacing" => {
            "style rule: simultaneous pitched voices must stay within a maximum interval"
        }
        "phrase_concept" => {
            "style rule: phrase structure idioms such as periodic_phrase and motivic_development"
        }
        "at" => "at(collection, index): returns the list or tuple item at a zero-based index",
        "len" => "len(collection): returns the number of items in a list or tuple",
        "with" => "with(dict, patch): returns a dict with patch fields merged over the original",
        "merge" => "merge(dict, patch): returns a dict with patch fields merged over the original",
        "cat" => "cat(values...): concatenates values into a list, flattening lists and non-note tuples",
        "concat" => "concat(values...): concatenates values into a list, flattening lists and non-note tuples",
        "map" => "map(collection, fn) or collection.map(fn): maps a function over each element",
        "filter" => "filter(collection, fn) or collection.filter(fn): filters using a predicate function",
        "mapi" => "mapi(collection, fn) or collection.mapi(fn): maps a function with index over each element",
        "transpose" => {
            "transpose(collection, interval): transposes all pitches in a collection by an interval"
        }
        "repeat" => "repeat(value, count): repeats a value N times into a list",
        "stretch" => {
            "stretch(collection, factor): stretches all durations in a collection by a factor"
        }
        "duration" => "duration(string): parses a duration string into a duration value",
        "pitch" => "pitch(string): parses a pitch string into a pitch value",
        "first" => "first(collection): returns the first element of a non-empty list or tuple",
        _ => return None,
    };
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(text.to_string())),
        range: None,
    })
}

fn definition_at(
    documents: &DocumentStore,
    params: &GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let source = documents.get_source(uri)?;
    let word = word_at(source, params.text_document_position_params.position)?;
    let position = find_definition(source, &word)?;
    let word_width = word.encode_utf16().count() as u32;
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: Range {
            start: position,
            end: Position::new(position.line, position.character + word_width),
        },
    }))
}

fn references_at(documents: &DocumentStore, params: &ReferenceParams) -> Option<Vec<Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let source = documents.get_source(uri)?;
    let word = word_at(source, params.text_document_position.position)?;
    Some(
        word_occurrences(source, &word)
            .into_iter()
            .map(|highlight| Location {
                uri: uri.clone(),
                range: highlight.range,
            })
            .collect(),
    )
}

fn document_highlights(
    documents: &DocumentStore,
    params: &DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let source = documents.get_source(uri)?;
    let word = word_at(source, params.text_document_position_params.position)?;
    Some(word_occurrences(source, &word))
}

fn word_occurrences(source: &str, word: &str) -> Vec<DocumentHighlight> {
    source
        .lines()
        .enumerate()
        .flat_map(|(line_index, line)| line_word_occurrences(line_index as u32, line, word))
        .collect()
}

fn line_word_occurrences(line_index: u32, line: &str, word: &str) -> Vec<DocumentHighlight> {
    let mut highlights = Vec::new();
    let mut offset = 0usize;
    while let Some(relative_start) = line[offset..].find(word) {
        let start = offset + relative_start;
        let end = start + word.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if before.is_none_or(|ch| !is_word(ch)) && after.is_none_or(|ch| !is_word(ch)) {
            highlights.push(DocumentHighlight {
                range: Range {
                    start: Position::new(line_index, utf16_len(&line[..start])),
                    end: Position::new(line_index, utf16_len(&line[..end])),
                },
                kind: Some(DocumentHighlightKind::TEXT),
            });
        }
        offset = end;
    }
    highlights
}

const SEMANTIC_KEYWORD: u32 = 0;
const SEMANTIC_FUNCTION: u32 = 1;
const SEMANTIC_VARIABLE: u32 = 2;
const SEMANTIC_CLASS: u32 = 3;
const SEMANTIC_NUMBER: u32 = 4;
const SEMANTIC_STRING: u32 = 5;
const SEMANTIC_COMMENT: u32 = 6;
const SEMANTIC_OPERATOR: u32 = 7;
const SEMANTIC_DECLARATION: u32 = 1;

fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::CLASS,
            SemanticTokenType::NUMBER,
            SemanticTokenType::STRING,
            SemanticTokenType::COMMENT,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: vec![SemanticTokenModifier::DECLARATION],
    }
}

fn semantic_tokens(
    documents: &DocumentStore,
    params: &SemanticTokensParams,
) -> Option<SemanticTokensResult> {
    let uri = &params.text_document.uri;
    let source = documents.get_source(uri)?;
    let raw_tokens = source_semantic_tokens(source);
    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: encode_semantic_tokens(raw_tokens),
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

fn source_semantic_tokens(source: &str) -> Vec<RawSemanticToken> {
    let local_functions = local_function_names(source);
    let local_variables = local_variable_names(source);
    let local_styles = local_style_names(source);
    source
        .lines()
        .enumerate()
        .flat_map(|(line_index, line)| {
            line_semantic_tokens(
                line_index as u32,
                line,
                &local_functions,
                &local_variables,
                &local_styles,
            )
        })
        .collect()
}

fn line_semantic_tokens(
    line_index: u32,
    line: &str,
    local_functions: &[String],
    local_variables: &[String],
    local_styles: &[String],
) -> Vec<RawSemanticToken> {
    let mut tokens = Vec::new();
    let declaration = declaration_name(line);
    let mut index = 0usize;
    while index < line.len() {
        let Some(ch) = line[index..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }
        if line[index..].starts_with("//") || line[index..].starts_with('#') {
            push_semantic_token(
                &mut tokens,
                line_index,
                line,
                index,
                line.len(),
                SEMANTIC_COMMENT,
                0,
            );
            break;
        }
        if ch == '"' {
            let end = string_end(line, index);
            push_semantic_token(
                &mut tokens,
                line_index,
                line,
                index,
                end,
                SEMANTIC_STRING,
                0,
            );
            index = end;
            continue;
        }
        if is_operator_char(ch) {
            let end = index + ch.len_utf8();
            push_semantic_token(
                &mut tokens,
                line_index,
                line,
                index,
                end,
                SEMANTIC_OPERATOR,
                0,
            );
            index = end;
            continue;
        }
        if ch.is_ascii_digit() {
            let end = numeric_token_end(line, index);
            push_semantic_token(
                &mut tokens,
                line_index,
                line,
                index,
                end,
                SEMANTIC_NUMBER,
                0,
            );
            index = end;
            continue;
        }
        if is_identifier_start(ch) {
            let end = identifier_token_end(line, index);
            let text = &line[index..end];
            let (token_type, modifiers) = semantic_token_kind(
                text,
                declaration,
                local_functions,
                local_variables,
                local_styles,
            );
            push_semantic_token(
                &mut tokens,
                line_index,
                line,
                index,
                end,
                token_type,
                modifiers,
            );
            index = end;
            continue;
        }
        index += ch.len_utf8();
    }
    tokens
}

fn push_semantic_token(
    tokens: &mut Vec<RawSemanticToken>,
    line_index: u32,
    line: &str,
    start: usize,
    end: usize,
    token_type: u32,
    token_modifiers_bitset: u32,
) {
    if start >= end {
        return;
    }
    tokens.push(RawSemanticToken {
        line: line_index,
        start: utf16_len(&line[..start]),
        length: utf16_len(&line[start..end]),
        token_type,
        token_modifiers_bitset,
    });
}

fn semantic_token_kind(
    text: &str,
    declaration: Option<(&str, &str)>,
    local_functions: &[String],
    local_variables: &[String],
    local_styles: &[String],
) -> (u32, u32) {
    if is_keyword(text) {
        return (SEMANTIC_KEYWORD, 0);
    }
    if declaration == Some(("fn", text)) {
        return (SEMANTIC_FUNCTION, SEMANTIC_DECLARATION);
    }
    if declaration == Some(("let", text)) {
        return (SEMANTIC_VARIABLE, SEMANTIC_DECLARATION);
    }
    if declaration == Some(("style", text)) {
        return (SEMANTIC_CLASS, SEMANTIC_DECLARATION);
    }
    if local_functions.iter().any(|name| name == text) {
        return (SEMANTIC_FUNCTION, 0);
    }
    if local_variables.iter().any(|name| name == text) {
        return (SEMANTIC_VARIABLE, 0);
    }
    if local_styles.iter().any(|name| name == text)
        || BUILT_IN_STYLES.iter().any(|style| style.id == text)
    {
        return (SEMANTIC_CLASS, 0);
    }
    if is_pitch_like(text) || is_duration_like(text) {
        return (SEMANTIC_NUMBER, 0);
    }
    (SEMANTIC_VARIABLE, 0)
}

fn encode_semantic_tokens(raw_tokens: Vec<RawSemanticToken>) -> Vec<SemanticToken> {
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    raw_tokens
        .into_iter()
        .map(|token| {
            let delta_line = token.line - previous_line;
            let delta_start = if delta_line == 0 {
                token.start - previous_start
            } else {
                token.start
            };
            previous_line = token.line;
            previous_start = token.start;
            SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            }
        })
        .collect()
}

fn signature_help(
    documents: &DocumentStore,
    params: &SignatureHelpParams,
) -> Option<SignatureHelp> {
    let source = documents.get_source(&params.text_document_position_params.text_document.uri)?;
    source_signature_help(source, params.text_document_position_params.position)
}

fn source_signature_help(source: &str, position: Position) -> Option<SignatureHelp> {
    let line = source.lines().nth(position.line as usize)?;
    let cursor = utf16_character_to_byte_index(line, position.character);
    let prefix = &line[..cursor];
    let trimmed = prefix.trim_start();
    let keyword = trimmed.split_whitespace().next()?;
    if matches!(keyword, "note" | "chord") {
        return Some(SignatureHelp {
            signatures: vec![statement_signature(keyword)],
            active_signature: Some(0),
            active_parameter: Some(count_top_level_commas(prefix) as u32),
        });
    }
    let call = active_call(prefix)?;
    let signature = expression_signature(&call.name, call.method)
        .or_else(|| (!call.method).then(|| local_function_signature(source, &call.name))?)?;
    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter: Some(call.active_parameter as u32),
    })
}

struct ActiveCall {
    name: String,
    method: bool,
    active_parameter: usize,
}

fn active_call(prefix: &str) -> Option<ActiveCall> {
    let open = prefix.rfind('(')?;
    let before = prefix[..open].trim_end();
    let end = before
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_identifier_char(*ch))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let name = before[end..].trim();
    if name.is_empty() {
        return None;
    }
    Some(ActiveCall {
        name: name.to_string(),
        method: before[..end].trim_end().ends_with('.'),
        active_parameter: count_top_level_commas(&prefix[open + 1..]),
    })
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn count_top_level_commas(value: &str) -> usize {
    let mut depth = 0usize;
    let mut count = 0usize;
    for ch in value.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

fn expression_signature(name: &str, method: bool) -> Option<SignatureInformation> {
    let params = match (name, method) {
        ("at", false) => vec!["collection", "index"],
        ("len", false) => vec!["collection"],
        ("with", false) => vec!["dict", "patch"],
        ("with", true) => vec!["patch"],
        ("merge", false) => vec!["dict", "patch"],
        ("merge", true) => vec!["patch"],
        ("cat" | "concat", false) => vec!["values..."],
        ("map", false) => vec!["collection", "function_name"],
        ("map", true) => vec!["function_name"],
        ("filter", false) => vec!["collection", "function_name"],
        ("filter", true) => vec!["function_name"],
        ("mapi", false) => vec!["collection", "function_name"],
        ("mapi", true) => vec!["function_name"],
        ("transpose", false) => vec!["collection", "interval"],
        ("transpose", true) => vec!["interval"],
        ("repeat", false) => vec!["value", "count"],
        ("stretch", false) => vec!["collection", "factor"],
        ("stretch", true) => vec!["factor"],
        ("duration", false) => vec!["string"],
        ("pitch", false) => vec!["string"],
        ("first", false) => vec!["collection"],
        _ => return None,
    };
    let label = format!("{name}({})", params.join(", "));
    Some(SignatureInformation {
        label,
        documentation: None,
        parameters: Some(
            params
                .into_iter()
                .map(|param| lsp_types::ParameterInformation {
                    label: lsp_types::ParameterLabel::Simple(param.to_string()),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter: None,
    })
}

fn local_function_signature(source: &str, name: &str) -> Option<SignatureInformation> {
    let signature = local_function_signatures(source)
        .into_iter()
        .find(|signature| signature.name == name)?;
    let label = format!("{}({})", signature.name, signature.params.join(", "));
    Some(SignatureInformation {
        label,
        documentation: None,
        parameters: Some(
            signature
                .params
                .into_iter()
                .map(|param| lsp_types::ParameterInformation {
                    label: lsp_types::ParameterLabel::Simple(param),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter: None,
    })
}

fn statement_signature(keyword: &str) -> SignatureInformation {
    let label = match keyword {
        "chord" => "chord pitch_list, duration",
        _ => "note pitch, duration",
    };
    SignatureInformation {
        label: label.to_string(),
        documentation: None,
        parameters: Some(vec![
            lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::Simple("pitch".to_string()),
                documentation: None,
            },
            lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::Simple("duration".to_string()),
                documentation: None,
            },
        ]),
        active_parameter: None,
    }
}

fn inlay_hints(documents: &DocumentStore, params: &InlayHintParams) -> Option<Vec<InlayHint>> {
    let source = documents.get_source(&params.text_document.uri)?;
    Some(source_inlay_hints(source, params.range))
}

fn source_inlay_hints(source: &str, range: Range) -> Vec<InlayHint> {
    source
        .lines()
        .enumerate()
        .filter(|(line_index, _)| {
            let line = *line_index as u32;
            range.start.line <= line && line <= range.end.line
        })
        .flat_map(|(line_index, line)| line_inlay_hints(line_index as u32, line))
        .collect()
}

fn line_inlay_hints(line_index: u32, line: &str) -> Vec<InlayHint> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let Some(keyword) = trimmed.split_whitespace().next() else {
        return Vec::new();
    };
    if !matches!(keyword, "note" | "chord") {
        return Vec::new();
    }
    let Some(comma) = line.rfind(',') else {
        return Vec::new();
    };
    let first_arg_start = leading
        + keyword.len()
        + line[leading + keyword.len()..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
    let second_arg_start = comma
        + 1
        + line[comma + 1..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
    vec![
        parameter_inlay_hint(line_index, line, first_arg_start, "pitch:"),
        parameter_inlay_hint(line_index, line, second_arg_start, "duration:"),
    ]
}

fn parameter_inlay_hint(line_index: u32, line: &str, byte_index: usize, label: &str) -> InlayHint {
    InlayHint {
        position: Position::new(line_index, utf16_len(&line[..byte_index])),
        label: InlayHintLabel::String(label.to_string()),
        kind: Some(lsp_types::InlayHintKind::PARAMETER),
        text_edits: None,
        tooltip: None,
        padding_left: Some(false),
        padding_right: Some(true),
        data: None,
    }
}

fn document_symbols(
    documents: &DocumentStore,
    params: &DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = &params.text_document.uri;
    let source = documents.get_source(uri)?;
    let symbols = source
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| source_symbol(line_index, line))
        .collect::<Vec<_>>();
    Some(DocumentSymbolResponse::Nested(symbols))
}

fn selection_ranges(
    documents: &DocumentStore,
    params: &SelectionRangeParams,
) -> Option<Vec<SelectionRange>> {
    let source = documents.get_source(&params.text_document.uri)?;
    Some(
        params
            .positions
            .iter()
            .filter_map(|position| source_selection_range(source, *position))
            .collect(),
    )
}

fn source_selection_range(source: &str, position: Position) -> Option<SelectionRange> {
    let line = source.lines().nth(position.line as usize)?;
    let word_range = word_range_at(position.line, line, position.character)?;
    let line_range = Range {
        start: Position::new(position.line, 0),
        end: Position::new(position.line, utf16_len(line)),
    };
    let block_range = containing_block_range(source, position.line).unwrap_or(line_range);
    Some(SelectionRange {
        range: word_range,
        parent: Some(Box::new(SelectionRange {
            range: line_range,
            parent: (block_range != line_range).then(|| {
                Box::new(SelectionRange {
                    range: block_range,
                    parent: None,
                })
            }),
        })),
    })
}

fn word_range_at(line_index: u32, line: &str, character: u32) -> Option<Range> {
    let cursor = utf16_character_to_byte_index(line, character);
    let mut start = cursor;
    while start > 0 {
        let Some((index, ch)) = line[..start].char_indices().next_back() else {
            break;
        };
        if !is_word(ch) {
            break;
        }
        start = index;
    }
    let mut end = cursor;
    while end < line.len() {
        let Some(ch) = line[end..].chars().next() else {
            break;
        };
        if !is_word(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    (start < end).then(|| Range {
        start: Position::new(line_index, utf16_len(&line[..start])),
        end: Position::new(line_index, utf16_len(&line[..end])),
    })
}

fn containing_block_range(source: &str, line: u32) -> Option<Range> {
    source_folding_ranges(source)
        .into_iter()
        .filter(|range| range.start_line < line && line < range.end_line)
        .min_by_key(|range| range.end_line - range.start_line)
        .map(|range| Range {
            start: Position::new(range.start_line, 0),
            end: Position::new(
                range.end_line,
                source
                    .lines()
                    .nth(range.end_line as usize)
                    .map(utf16_len)
                    .unwrap_or(0),
            ),
        })
}

fn folding_ranges(
    documents: &DocumentStore,
    params: &FoldingRangeParams,
) -> Option<Vec<FoldingRange>> {
    let source = documents.get_source(&params.text_document.uri)?;
    Some(source_folding_ranges(source))
}

fn source_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut stack = Vec::new();
    let mut ranges = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => stack.push(line_index as u32),
                '}' => {
                    if let Some(start_line) = stack.pop() {
                        let end_line = line_index as u32;
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
                }
                _ => {}
            }
        }
    }
    ranges
}

fn prepare_rename(
    documents: &DocumentStore,
    params: &lsp_types::TextDocumentPositionParams,
) -> Option<PrepareRenameResponse> {
    let source = documents.get_source(&params.text_document.uri)?;
    let line = source.lines().nth(params.position.line as usize)?;
    let cursor = utf16_character_to_byte_index(line, params.position.character);
    let ch = line[cursor..].chars().next()?;
    if !is_word(ch) {
        return None;
    }
    let range = word_range_at(params.position.line, line, params.position.character)?;
    Some(PrepareRenameResponse::Range(range))
}

fn rename_symbol(documents: &DocumentStore, params: &RenameParams) -> Option<WorkspaceEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let source = documents.get_source(uri)?;
    let word = word_at(source, params.text_document_position.position)?;
    let edits = word_occurrences(source, &word)
        .into_iter()
        .map(|highlight| TextEdit {
            range: highlight.range,
            new_text: params.new_name.clone(),
        })
        .collect::<Vec<_>>();
    (!edits.is_empty()).then(|| WorkspaceEdit {
        changes: Some(HashMap::from([(uri.clone(), edits)])),
        document_changes: None,
        change_annotations: None,
    })
}

fn document_formatting(
    documents: &DocumentStore,
    params: &DocumentFormattingParams,
) -> Option<Vec<TextEdit>> {
    let source = documents.get_source(&params.text_document.uri)?;
    let formatted = format_musiclang_source(source, params.options.tab_size as usize);
    (formatted != source).then(|| {
        vec![TextEdit {
            range: full_document_range(source),
            new_text: formatted,
        }]
    })
}

fn format_musiclang_source(source: &str, tab_size: usize) -> String {
    let indent_width = tab_size.max(1);
    let mut indent = 0usize;
    let mut lines = source
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('}') {
                indent = indent.saturating_sub(1);
            }
            let formatted = if trimmed.is_empty() {
                String::new()
            } else {
                format!("{}{}", " ".repeat(indent * indent_width), trimmed)
            };
            let opens = trimmed.matches('{').count();
            let closes = trimmed.matches('}').count();
            indent = indent.saturating_add(opens);
            indent = indent.saturating_sub(if trimmed.starts_with('}') {
                closes.saturating_sub(1)
            } else {
                closes
            });
            formatted
        })
        .collect::<Vec<_>>()
        .join("\n");
    if source.ends_with('\n') {
        lines.push('\n');
    }
    lines
}

fn full_document_range(source: &str) -> Range {
    let line_count = source.lines().count() as u32;
    let last_line = source.lines().next_back().unwrap_or("");
    Range {
        start: Position::new(0, 0),
        end: Position::new(line_count, utf16_len(last_line)),
    }
}

fn workspace_symbols(
    documents: &DocumentStore,
    params: &WorkspaceSymbolParams,
) -> Option<WorkspaceSymbolResponse> {
    let query = params.query.to_ascii_lowercase();
    let symbols = documents
        .iter()
        .flat_map(|(uri, source)| source_workspace_symbols(uri, source, &query))
        .collect::<Vec<_>>();
    Some(WorkspaceSymbolResponse::Nested(symbols))
}

fn source_workspace_symbols(uri: &str, source: &str, query: &str) -> Vec<WorkspaceSymbol> {
    let Ok(uri) = uri.parse::<Uri>() else {
        return Vec::new();
    };
    source
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let symbol = source_symbol(line_index, line)?;
            (query.is_empty() || symbol.name.to_ascii_lowercase().contains(query)).then(|| {
                WorkspaceSymbol {
                    name: symbol.name,
                    kind: symbol.kind,
                    tags: symbol.tags,
                    container_name: symbol.detail,
                    location: lsp_types::OneOf::Left(Location {
                        uri: uri.clone(),
                        range: symbol.selection_range,
                    }),
                    data: None,
                }
            })
        })
        .collect()
}

#[allow(deprecated)]
fn source_symbol(line_index: usize, line: &str) -> Option<DocumentSymbol> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let words = trimmed.split_whitespace().collect::<Vec<_>>();
    let (kind, name_index, detail) = match words.as_slice() {
        ["style", name, ..] => (SymbolKind::CLASS, Some(*name), Some("style".to_string())),
        ["fn", name, ..] => (SymbolKind::FUNCTION, Some(*name), None),
        ["score", name, ..] => (
            SymbolKind::NAMESPACE,
            Some(*name),
            Some("score".to_string()),
        ),
        ["voice", name, ..] => (SymbolKind::OBJECT, Some(*name), Some("voice".to_string())),
        ["section", name, ..] => (SymbolKind::MODULE, Some(*name), Some("section".to_string())),
        _ => return None,
    };
    let raw_name = name_index?;
    let name = raw_name.trim_end_matches('{').to_string();
    let name_start = line.find(raw_name).unwrap_or(indent);
    let line_len = line.chars().map(|ch| ch.len_utf16() as u32).sum::<u32>();
    let selection_start = line[..name_start]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum::<u32>();
    let selection_width = raw_name
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum::<u32>();
    Some(DocumentSymbol {
        name,
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: Range {
            start: Position::new(line_index as u32, 0),
            end: Position::new(line_index as u32, line_len.max(1)),
        },
        selection_range: Range {
            start: Position::new(line_index as u32, selection_start),
            end: Position::new(line_index as u32, selection_start + selection_width),
        },
        children: None,
    })
}

fn declaration_name(line: &str) -> Option<(&str, &str)> {
    let mut words = line.split_whitespace();
    let kind = words.next()?;
    match kind {
        "fn" | "let" | "style" => words.next().map(|name| (kind, name.trim_end_matches('{'))),
        _ => None,
    }
}

fn string_end(line: &str, start: usize) -> usize {
    let mut escaped = false;
    for (offset, ch) in line[start + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return start + 1 + offset + ch.len_utf8();
        }
    }
    line.len()
}

fn numeric_token_end(line: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in line[start..].char_indices() {
        if ch.is_ascii_digit() || ch == '/' {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn identifier_token_end(line: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in line[start..].char_indices() {
        if is_identifier_continue(ch) {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn utf16_len(text: &str) -> u32 {
    text.chars().map(|ch| ch.len_utf16() as u32).sum()
}

fn is_keyword(text: &str) -> bool {
    matches!(
        text,
        "style"
            | "extends"
            | "score"
            | "voice"
            | "section"
            | "tempo"
            | "meter"
            | "key"
            | "program"
            | "instrument"
            | "dynamic"
            | "velocity"
            | "articulation"
            | "ornament"
            | "non_chord_tone"
            | "tuning_system"
            | "world_tradition"
            | "historical_era"
            | "harmonic_function"
            | "note"
            | "chord"
            | "let"
            | "duration"
            | "fn"
            | "call"
            | "for"
            | "in"
            | "if"
            | "not"
            | "true"
            | "false"
            | "override"
            | "allow"
            | "reason"
    )
}

fn is_pitch_like(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('A'..='G'))
        && chars.any(|ch| ch.is_ascii_digit())
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '#')
}

fn is_duration_like(text: &str) -> bool {
    text.contains('/') && text.chars().all(|ch| ch.is_ascii_digit() || ch == '/')
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '#')
}

fn is_operator_char(ch: char) -> bool {
    matches!(
        ch,
        '{' | '}' | '[' | ']' | '(' | ')' | ',' | ':' | '=' | '+' | '-' | '*' | '/' | '.'
    )
}

fn find_definition(source: &str, word: &str) -> Option<Position> {
    for (line_index, line) in source.lines().enumerate() {
        for keyword in ["fn", "let", "style"] {
            let pattern = format!("{keyword} {word}");
            if let Some(index) = line.find(&pattern) {
                let name_start = index + keyword.len() + 1;
                return Some(Position::new(
                    line_index as u32,
                    utf16_len(&line[..name_start]),
                ));
            }
        }
    }
    None
}

fn completion_items(documents: &DocumentStore, params: &CompletionParams) -> CompletionResponse {
    let uri = &params.text_document_position.text_document.uri;
    let Some(source) = documents.get_source(uri) else {
        let mut items = general_completion_items();
        items.extend(expression_builtin_completion_items());
        items.extend(style_rule_completion_items());
        items.extend(builtin_style_completion_items());
        return CompletionResponse::Array(items);
    };

    let position = params.text_document_position.position;
    let line_prefix = line_prefix(source, position);
    if is_method_context(&line_prefix) {
        return CompletionResponse::Array(method_completion_items());
    }
    if is_call_context(&line_prefix) {
        return CompletionResponse::Array(local_function_completion_items(source));
    }
    if is_score_style_context(&line_prefix) {
        return CompletionResponse::Array(style_completion_items(source));
    }
    if let Some(items) = style_value_completion_items(&line_prefix) {
        return CompletionResponse::Array(items);
    }
    if is_style_key_context(source, position, &line_prefix) {
        return CompletionResponse::Array(style_rule_completion_items());
    }

    let mut items = general_completion_items();
    items.extend(expression_builtin_completion_items());
    items.extend(style_completion_items(source));
    items.extend(local_function_completion_items(source));
    items.extend(local_variable_completion_items(source));
    CompletionResponse::Array(items)
}

fn general_completion_items() -> Vec<CompletionItem> {
    [
        "style",
        "score",
        "voice",
        "tempo",
        "meter",
        "program",
        "instrument",
        "note",
        "chord",
        "let",
        "for",
        "if",
        "fn",
        "call",
        "override",
        "allow",
        "reason",
    ]
    .into_iter()
    .map(keyword_item)
    .collect()
}

fn expression_builtin_completion_items() -> Vec<CompletionItem> {
    [
        (
            "at",
            CompletionItemKind::FUNCTION,
            "at(collection, index)",
            "Returns the list or tuple item at a zero-based index.",
        ),
        (
            "len",
            CompletionItemKind::FUNCTION,
            "len(collection)",
            "Returns the number of items in a list or tuple.",
        ),
        (
            "with",
            CompletionItemKind::FUNCTION,
            "with(dict, patch)",
            "Returns a dict with patch fields merged over the original.",
        ),
        (
            "merge",
            CompletionItemKind::FUNCTION,
            "merge(dict, patch)",
            "Returns a dict with patch fields merged over the original.",
        ),
        (
            "not",
            CompletionItemKind::KEYWORD,
            "not bool_expr",
            "Negates a boolean expression.",
        ),
        (
            "cat",
            CompletionItemKind::FUNCTION,
            "cat(values...)",
            "Concatenates values into a list, flattening lists and non-note tuples.",
        ),
        (
            "concat",
            CompletionItemKind::FUNCTION,
            "concat(values...)",
            "Concatenates values into a list, flattening lists and non-note tuples.",
        ),
        (
            "map",
            CompletionItemKind::FUNCTION,
            "map(collection, function_name) / collection.map(function_name)",
            "Maps a function over each element; the function name may be a bare identifier or string.",
        ),
        (
            "filter",
            CompletionItemKind::FUNCTION,
            "filter(collection, function_name) / collection.filter(function_name)",
            "Filters a collection using a predicate function; the function name may be a bare identifier or string.",
        ),
        (
            "mapi",
            CompletionItemKind::FUNCTION,
            "mapi(collection, function_name) / collection.mapi(function_name)",
            "Maps a function with index over each element; the function name may be a bare identifier or string.",
        ),
        (
            "transpose",
            CompletionItemKind::FUNCTION,
            "transpose(collection, interval)",
            "Transposes all pitches in a collection by an interval.",
        ),
        (
            "repeat",
            CompletionItemKind::FUNCTION,
            "repeat(value, count)",
            "Repeats a value N times into a list.",
        ),
        (
            "stretch",
            CompletionItemKind::FUNCTION,
            "stretch(collection, factor)",
            "Stretches all durations in a collection by a factor.",
        ),
        (
            "duration",
            CompletionItemKind::FUNCTION,
            "duration(string)",
            "Parses a duration string into a duration value.",
        ),
        (
            "pitch",
            CompletionItemKind::FUNCTION,
            "pitch(string)",
            "Parses a pitch string into a pitch value.",
        ),
        (
            "first",
            CompletionItemKind::FUNCTION,
            "first(collection)",
            "Returns the first element of a non-empty list or tuple.",
        ),
    ]
    .into_iter()
    .map(|(label, kind, detail, documentation)| CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(detail.to_string()),
        documentation: Some(lsp_types::Documentation::String(documentation.to_string())),
        insert_text: Some(label.to_string()),
        ..CompletionItem::default()
    })
    .collect()
}

fn method_completion_items() -> Vec<CompletionItem> {
    [
        (
            "map",
            "map(function_name)",
            "Maps a function over each element; the function name may be a bare identifier or string.",
        ),
        (
            "filter",
            "filter(function_name)",
            "Filters a collection using a predicate function; the function name may be a bare identifier or string.",
        ),
        (
            "mapi",
            "mapi(function_name)",
            "Maps a function with index over each element; the function name may be a bare identifier or string.",
        ),
        (
            "with",
            "with(patch)",
            "Returns a dict with patch fields merged over the original.",
        ),
        (
            "merge",
            "merge(patch)",
            "Returns a dict with patch fields merged over the original.",
        ),
        (
            "transpose",
            "transpose(interval)",
            "Transposes all pitches in a collection by an interval.",
        ),
        (
            "stretch",
            "stretch(factor)",
            "Stretches all durations in a collection by a factor.",
        ),
    ]
    .into_iter()
    .map(|(label, detail, documentation)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::METHOD),
        detail: Some(detail.to_string()),
        documentation: Some(lsp_types::Documentation::String(documentation.to_string())),
        insert_text: Some(label.to_string()),
        ..CompletionItem::default()
    })
    .collect()
}

fn style_rule_completion_items() -> Vec<CompletionItem> {
    [
        "scale",
        "scale_pattern",
        "mode_pattern",
        "chord_vocab",
        "chord_quality_vocab",
        "meter",
        "meter_catalog",
        "tempo_range",
        "instrument_range",
        "dynamic_vocab",
        "articulation_vocab",
        "ornament",
        "non_chord_tone",
        "harmonic_function",
        "set_class_vocab",
        "tuning_system",
        "world_tradition",
        "historical_era",
        "rhythm_vocab",
        "rhythm_concept",
        "melodic_concept",
        "phrase_concept",
        "ensemble_concept",
        "bass_concept",
        "form",
        "texture",
        "cadence",
        "max_melodic_leap",
        "contrapuntal_motion",
        "voice_spacing",
        "harmonic_progression",
    ]
    .into_iter()
    .map(keyword_item)
    .collect()
}

fn style_completion_items(source: &str) -> Vec<CompletionItem> {
    let mut items = builtin_style_completion_items();
    items.extend(
        local_style_names(source)
            .into_iter()
            .map(|label| CompletionItem {
                label,
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("MusicLang style".to_string()),
                ..CompletionItem::default()
            }),
    );
    items
}

fn builtin_style_completion_items() -> Vec<CompletionItem> {
    BUILT_IN_STYLES
        .iter()
        .map(|style| CompletionItem {
            label: style.id.to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(style.name.to_string()),
            documentation: Some(lsp_types::Documentation::String(
                style.description.to_string(),
            )),
            ..CompletionItem::default()
        })
        .collect()
}

fn local_function_completion_items(source: &str) -> Vec<CompletionItem> {
    local_function_names(source)
        .into_iter()
        .map(|label| CompletionItem {
            label,
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("MusicLang function".to_string()),
            ..CompletionItem::default()
        })
        .collect()
}

fn local_variable_completion_items(source: &str) -> Vec<CompletionItem> {
    local_variable_names(source)
        .into_iter()
        .map(|label| CompletionItem {
            label,
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("MusicLang value".to_string()),
            ..CompletionItem::default()
        })
        .collect()
}

fn idiom_completion_items(labels: &[&str]) -> Vec<CompletionItem> {
    labels
        .iter()
        .map(|label| CompletionItem {
            label: (*label).to_string(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some("MusicLang idiom concept".to_string()),
            insert_text: Some((*label).to_string()),
            ..CompletionItem::default()
        })
        .collect()
}

fn theory_entry_completion_items(domain: musiclang_core::TheoryDomain) -> Vec<CompletionItem> {
    let catalog = musiclang_core::theory_catalog();
    catalog
        .entries(domain)
        .iter()
        .map(|entry| CompletionItem {
            label: entry.id.to_string(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(entry.name.to_string()),
            documentation: Some(lsp_types::Documentation::String(
                entry.description.to_string(),
            )),
            ..CompletionItem::default()
        })
        .collect()
}

fn keyword_item(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        insert_text: Some(label.to_string()),
        ..CompletionItem::default()
    }
}

fn line_prefix(source: &str, position: Position) -> String {
    let Some(line) = source.lines().nth(position.line as usize) else {
        return String::new();
    };
    let byte_index = utf16_character_to_byte_index(line, position.character);
    line[..byte_index].to_string()
}

fn utf16_character_to_byte_index(line: &str, character: u32) -> usize {
    let mut units = 0u32;
    for (index, ch) in line.char_indices() {
        if units >= character {
            return index;
        }
        units += ch.len_utf16() as u32;
    }
    line.len()
}

fn is_method_context(line_prefix: &str) -> bool {
    let trimmed = line_prefix.trim_end();
    trimmed.ends_with('.') && !trimmed.ends_with("..")
}

fn is_call_context(line_prefix: &str) -> bool {
    let words = line_prefix.split_whitespace().collect::<Vec<_>>();
    words.last() == Some(&"call") || words.iter().rev().nth(1) == Some(&"call")
}

fn is_score_style_context(line_prefix: &str) -> bool {
    let words = line_prefix.split_whitespace().collect::<Vec<_>>();
    words.first() == Some(&"score") && words.contains(&"style") && !line_prefix.contains('{')
}

fn is_style_key_context(source: &str, position: Position, line_prefix: &str) -> bool {
    let trimmed = line_prefix.trim_start();
    !trimmed.contains(':')
        && !trimmed.contains('{')
        && current_block_kind(source, position) == Some("style")
}

fn current_block_kind(source: &str, position: Position) -> Option<&'static str> {
    let mut stack = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        if line_index > position.line as usize {
            break;
        }
        let prefix = if line_index == position.line as usize {
            let byte_index = utf16_character_to_byte_index(line, position.character);
            &line[..byte_index]
        } else {
            line
        };
        if prefix.contains('}') {
            stack.pop();
        }
        if prefix.contains('{') {
            let first = prefix.split_whitespace().next().unwrap_or_default();
            stack.push(match first {
                "style" => "style",
                "score" => "score",
                "voice" => "voice",
                _ => "block",
            });
        }
    }
    stack.last().copied()
}

fn style_value_completion_items(line_prefix: &str) -> Option<Vec<CompletionItem>> {
    let key = line_prefix.split_once(':')?.0.trim();
    if let Some(domain) = style_key_domain(key) {
        return Some(theory_entry_completion_items(domain));
    }
    match key {
        "melodic_concept" => Some(idiom_completion_items(&["blues_inflection"])),
        "phrase_concept" => Some(idiom_completion_items(&[
            "periodic_phrase",
            "motivic_development",
        ])),
        "ensemble_concept" => Some(idiom_completion_items(&["call_response"])),
        "bass_concept" => Some(idiom_completion_items(&["walking_or_riff_bass"])),
        _ => None,
    }
}

fn style_key_domain(key: &str) -> Option<musiclang_core::TheoryDomain> {
    match key {
        "scale" | "scale_pattern" => Some(musiclang_core::TheoryDomain::Scales),
        "mode_pattern" => Some(musiclang_core::TheoryDomain::Modes),
        "chord_vocab" | "chord_quality_vocab" => Some(musiclang_core::TheoryDomain::ChordQualities),
        "meter" | "meter_catalog" => Some(musiclang_core::TheoryDomain::Meters),
        "dynamic_vocab" => Some(musiclang_core::TheoryDomain::Dynamics),
        "articulation_vocab" | "ornament" => Some(musiclang_core::TheoryDomain::Ornaments),
        "non_chord_tone" => Some(musiclang_core::TheoryDomain::NonChordTones),
        "harmonic_function" => Some(musiclang_core::TheoryDomain::HarmonicFunctions),
        "set_class_vocab" => Some(musiclang_core::TheoryDomain::SetClasses),
        "tuning_system" => Some(musiclang_core::TheoryDomain::TuningSystems),
        "world_tradition" => Some(musiclang_core::TheoryDomain::WorldTraditions),
        "historical_era" => Some(musiclang_core::TheoryDomain::StyleEras),
        "rhythm_vocab" | "rhythm_concept" => Some(musiclang_core::TheoryDomain::Rhythms),
        "form" => Some(musiclang_core::TheoryDomain::Forms),
        "texture" => Some(musiclang_core::TheoryDomain::Textures),
        "cadence" => Some(musiclang_core::TheoryDomain::Cadences),
        "contrapuntal_motion" => Some(musiclang_core::TheoryDomain::ContrapuntalMotions),
        _ => None,
    }
}

fn local_style_names(source: &str) -> Vec<String> {
    local_declaration_names(source, "style")
}

fn local_function_names(source: &str) -> Vec<String> {
    local_function_signatures(source)
        .into_iter()
        .map(|signature| signature.name)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSignature {
    name: String,
    params: Vec<String>,
}

fn local_function_signatures(source: &str) -> Vec<FunctionSignature> {
    source
        .lines()
        .filter_map(parse_function_signature)
        .collect()
}

fn parse_function_signature(line: &str) -> Option<FunctionSignature> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("fn ")?;
    let name_end = rest
        .char_indices()
        .find(|(_, ch)| !is_identifier_char(*ch))
        .map_or(rest.len(), |(index, _)| index);
    let name = rest[..name_end].to_string();
    if name.is_empty() {
        return None;
    }
    let rest = rest[name_end..].trim_start();
    let params = if let Some(after_open) = rest.strip_prefix('(') {
        let close = after_open.find(')')?;
        after_open[..close]
            .split(',')
            .map(str::trim)
            .filter(|param| !param.is_empty())
            .map(ToString::to_string)
            .collect()
    } else {
        Vec::new()
    };
    Some(FunctionSignature { name, params })
}

fn local_variable_names(source: &str) -> Vec<String> {
    let mut names = local_declaration_names(source, "let");
    names.extend(comprehension_binding_names(source));
    names
}

fn comprehension_binding_names(source: &str) -> Vec<String> {
    source
        .lines()
        .flat_map(|line| {
            let words = line
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>();
            words
                .windows(3)
                .filter(|window| window[0] == "for" && window[2] == "in")
                .map(|window| window[1].to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn local_declaration_names(source: &str, keyword: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if words.next()? == keyword {
                return Some(words.next()?.trim_end_matches('{').to_string());
            }
            None
        })
        .collect()
}

fn word_at(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let cursor = utf16_character_to_byte_index(line, position.character);
    let mut start = cursor;
    while start > 0 {
        let Some((index, ch)) = line[..start].char_indices().next_back() else {
            break;
        };
        if !is_word(ch) {
            break;
        }
        start = index;
    }
    let mut end = cursor;
    while end < line.len() {
        let Some(ch) = line[end..].chars().next() else {
            break;
        };
        if !is_word(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    (start < end).then(|| line[start..end].to_string())
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '#')
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn inlay_hint_label_text(hint: &InlayHint) -> &str {
        match &hint.label {
            InlayHintLabel::String(label) => label,
            InlayHintLabel::LabelParts(_) => "",
        }
    }

    #[test]
    fn server_capabilities_advertise_completion_triggers() {
        let capabilities = server_capabilities();
        let completion = capabilities.completion_provider.unwrap();
        let triggers = completion.trigger_characters.unwrap();

        assert!(triggers.iter().any(|trigger| trigger == ":"));
        assert!(triggers.iter().any(|trigger| trigger == "."));
        assert!(triggers.iter().any(|trigger| trigger == "("));
        assert!(triggers.iter().any(|trigger| trigger == " "));
        assert_eq!(
            capabilities.code_action_provider,
            Some(lsp_types::CodeActionProviderCapability::Simple(true))
        );
        assert_eq!(
            capabilities.inlay_hint_provider,
            Some(lsp_types::OneOf::Left(true))
        );
        let signature_help = capabilities.signature_help_provider.unwrap();
        let triggers = signature_help.trigger_characters.unwrap();
        assert!(triggers.iter().any(|trigger| trigger == "("));
        assert!(triggers.iter().any(|trigger| trigger == ","));
        assert!(triggers.iter().any(|trigger| trigger == " "));
        assert_eq!(
            capabilities.document_highlight_provider,
            Some(lsp_types::OneOf::Left(true))
        );
        assert_eq!(
            capabilities.references_provider,
            Some(lsp_types::OneOf::Left(true))
        );
        assert_eq!(
            capabilities.workspace_symbol_provider,
            Some(lsp_types::OneOf::Left(true))
        );
        assert_eq!(
            capabilities.document_formatting_provider,
            Some(lsp_types::OneOf::Left(true))
        );
        let Some(lsp_types::OneOf::Right(rename_options)) = capabilities.rename_provider else {
            panic!("expected rename options");
        };
        assert_eq!(rename_options.prepare_provider, Some(true));
        assert_eq!(
            capabilities.folding_range_provider,
            Some(lsp_types::FoldingRangeProviderCapability::Simple(true))
        );
        assert_eq!(
            capabilities.selection_range_provider,
            Some(lsp_types::SelectionRangeProviderCapability::Simple(true))
        );
    }

    #[test]
    fn signature_help_reports_active_note_and_chord_parameters() {
        let source =
            "score demo {\n  voice lead {\n    note C4, 1/4\n    chord [C4, E4, G4], 1/2\n  }\n}";
        let note_help = source_signature_help(source, Position::new(2, 11)).unwrap();
        let chord_help = source_signature_help(source, Position::new(3, 24)).unwrap();

        assert_eq!(note_help.signatures[0].label, "note pitch, duration");
        assert_eq!(note_help.active_parameter, Some(0));
        assert_eq!(chord_help.signatures[0].label, "chord pitch_list, duration");
        assert_eq!(chord_help.active_parameter, Some(1));
        assert_eq!(
            chord_help.signatures[0].parameters.as_ref().unwrap().len(),
            2
        );
    }

    #[test]
    fn signature_help_reports_expression_builtin_parameters() {
        let source = "fn demo(xs) = map(xs, lift)";
        let help = source_signature_help(source, Position::new(0, 21)).unwrap();

        assert_eq!(help.signatures[0].label, "map(collection, function_name)");
        assert_eq!(help.active_parameter, Some(1));
        assert_eq!(help.signatures[0].parameters.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn signature_help_reports_method_builtin_parameters() {
        let source = "fn demo(xs) = xs.map(lift).filter(keep).stretch(2)";
        let map_help = source_signature_help(source, Position::new(0, 22)).unwrap();
        let stretch_help = source_signature_help(source, Position::new(0, 50)).unwrap();

        assert_eq!(map_help.signatures[0].label, "map(function_name)");
        assert_eq!(map_help.active_parameter, Some(0));
        assert_eq!(stretch_help.signatures[0].label, "stretch(factor)");
        assert_eq!(stretch_help.active_parameter, Some(0));
    }

    #[test]
    fn signature_help_ignores_nested_collection_commas() {
        let source = "fn demo(xs) = with({p:C4, d:1/8}, {d:1/4})";
        let help = source_signature_help(source, Position::new(0, 25)).unwrap();

        assert_eq!(help.signatures[0].label, "with(dict, patch)");
        assert_eq!(help.active_parameter, Some(0));
    }

    #[test]
    fn signature_help_reports_local_function_parameters() {
        let source = "fn motif(root, dur) = [{p:root, d:dur}]\nfn demo() = motif(C4, 1/4)";
        let help = source_signature_help(source, Position::new(1, 22)).unwrap();

        assert_eq!(help.signatures[0].label, "motif(root, dur)");
        assert_eq!(help.active_parameter, Some(1));
        assert_eq!(help.signatures[0].parameters.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn signature_help_reports_zero_arg_local_function() {
        let source = "fn intro() = [{p:C4, d:1/4}]\nfn demo() = intro()";
        let help = source_signature_help(source, Position::new(1, 18)).unwrap();

        assert_eq!(help.signatures[0].label, "intro()");
        assert_eq!(help.active_parameter, Some(0));
        assert_eq!(help.signatures[0].parameters.as_ref().unwrap().len(), 0);
    }

    #[test]
    fn signature_help_does_not_treat_local_functions_as_methods() {
        let source = "fn motif(root) = [{p:root, d:1/4}]\nfn demo(xs) = xs.motif(C4)";

        assert!(source_signature_help(source, Position::new(1, 24)).is_none());
    }

    #[test]
    fn signature_help_returns_none_outside_supported_statements() {
        assert!(source_signature_help("score demo {}", Position::new(0, 7)).is_none());
    }

    #[test]
    fn inlay_hints_label_note_and_chord_arguments() {
        let source =
            "score demo {\n  voice lead {\n    note C4, 1/4\n    chord [C4, E4, G4], 1/2\n  }\n}";
        let hints = source_inlay_hints(
            source,
            Range {
                start: Position::new(0, 0),
                end: Position::new(5, 1),
            },
        );

        assert_eq!(hints.len(), 4);
        assert_eq!(hints[0].position, Position::new(2, 9));
        assert_eq!(inlay_hint_label_text(&hints[0]), "pitch:");
        assert_eq!(hints[1].position, Position::new(2, 13));
        assert_eq!(inlay_hint_label_text(&hints[1]), "duration:");
        assert_eq!(hints[2].position, Position::new(3, 10));
        assert_eq!(inlay_hint_label_text(&hints[2]), "pitch:");
        assert_eq!(hints[3].position, Position::new(3, 24));
        assert_eq!(inlay_hint_label_text(&hints[3]), "duration:");
        assert!(hints
            .iter()
            .all(|hint| hint.kind == Some(lsp_types::InlayHintKind::PARAMETER)));
    }

    #[test]
    fn selection_ranges_expand_from_word_to_line_to_block() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(
            &uri,
            "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}\n".to_string(),
        );
        let ranges = selection_ranges(
            &documents,
            &SelectionRangeParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                positions: vec![Position::new(2, 10)],
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].range.start, Position::new(2, 9));
        assert_eq!(ranges[0].range.end, Position::new(2, 11));
        let line_range = ranges[0].parent.as_ref().unwrap();
        assert_eq!(line_range.range.start, Position::new(2, 0));
        assert_eq!(line_range.range.end, Position::new(2, 16));
        let block_range = line_range.parent.as_ref().unwrap();
        assert_eq!(block_range.range.start, Position::new(1, 0));
        assert_eq!(block_range.range.end, Position::new(3, 3));
    }

    #[test]
    fn folding_ranges_cover_nested_brace_blocks() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(
            &uri,
            "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}\n".to_string(),
        );
        let ranges = folding_ranges(
            &documents,
            &FoldingRangeParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].end_line, 3);
        assert_eq!(ranges[0].kind, Some(FoldingRangeKind::Region));
        assert_eq!(ranges[1].start_line, 0);
        assert_eq!(ranges[1].end_line, 4);
    }

    #[test]
    fn prepare_rename_returns_symbol_range() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(
            &uri,
            "score demo {\n  voice lead {\n    let motif = C4\n  }\n}".to_string(),
        );
        let response = prepare_rename(
            &documents,
            &lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: Position::new(2, 10),
            },
        )
        .unwrap();

        assert_eq!(
            response,
            PrepareRenameResponse::Range(Range {
                start: Position::new(2, 8),
                end: Position::new(2, 13),
            })
        );
    }

    #[test]
    fn prepare_rename_returns_none_outside_symbol() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(&uri, "score demo {}".to_string());
        let response = prepare_rename(
            &documents,
            &lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: Position::new(0, 5),
            },
        );

        assert!(response.is_none());
    }

    #[test]
    fn prepare_rename_returns_none_at_end_of_line() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(&uri, "score demo {}".to_string());
        let response = prepare_rename(
            &documents,
            &lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: Position::new(0, 13),
            },
        );

        assert!(response.is_none());
    }

    #[test]
    fn rename_symbol_returns_workspace_edit_for_word_occurrences() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let source =
            "score demo {\n  voice lead {\n    let motif = C4\n    note motif, 1/4\n  }\n}";
        let mut documents = DocumentStore::default();
        documents.open(&uri, source.to_string());
        let edit = rename_symbol(
            &documents,
            &RenameParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(2, 8),
                },
                new_name: "theme".to_string(),
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();
        let value = serde_json::to_value(edit).unwrap();
        let edits = value["changes"][uri.to_string()].as_array().unwrap();

        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0]["range"]["start"]["line"], 2);
        assert_eq!(edits[0]["range"]["start"]["character"], 8);
        assert_eq!(edits[0]["newText"], "theme");
        assert_eq!(edits[1]["range"]["start"]["line"], 3);
        assert_eq!(edits[1]["range"]["start"]["character"], 9);
        assert_eq!(edits[1]["newText"], "theme");
    }

    #[test]
    fn code_actions_surface_diagnostic_help_as_quick_fix() {
        let diagnostic = Diagnostic {
            range: Range {
                start: Position::new(1, 2),
                end: Position::new(1, 7),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(lsp_types::NumberOrString::String(
                "ML_STYLE_SCALE".to_string(),
            )),
            source: Some("musiclang".to_string()),
            message: "Pitch is outside the configured scale".to_string(),
            data: Some(serde_json::json!({
                "help": "choose a pitch in the active scale"
            })),
            ..Diagnostic::default()
        };
        let response = code_actions(&CodeActionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: Uri::from_str("file:///demo.music").unwrap(),
            },
            range: diagnostic.range,
            context: lsp_types::CodeActionContext {
                diagnostics: vec![diagnostic],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .unwrap();

        assert_eq!(response.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &response[0] else {
            panic!("expected code action");
        };
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(
            action.title,
            "MusicLang: choose a pitch in the active scale"
        );
        assert_eq!(action.diagnostics.as_ref().unwrap().len(), 1);
        assert!(action.edit.is_none());
    }

    #[test]
    fn code_actions_return_none_without_helpful_diagnostics() {
        let response = code_actions(&CodeActionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: Uri::from_str("file:///demo.music").unwrap(),
            },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 1),
            },
            context: lsp_types::CodeActionContext {
                diagnostics: vec![Diagnostic::default()],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        });

        assert!(response.is_none());
    }

    #[test]
    fn document_formatting_returns_full_document_edit() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(
            &uri,
            "score demo {\nvoice lead {\nnote C4, 1/4\n}\n}\n".to_string(),
        );
        let edits = document_formatting(
            &documents,
            &DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                options: lsp_types::FormattingOptions {
                    tab_size: 2,
                    insert_spaces: true,
                    properties: Default::default(),
                    trim_trailing_whitespace: None,
                    insert_final_newline: None,
                    trim_final_newlines: None,
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range.start, Position::new(0, 0));
        assert_eq!(
            edits[0].new_text,
            "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}\n"
        );
    }

    #[test]
    fn document_formatting_returns_none_when_unchanged() {
        assert_eq!(
            format_musiclang_source("score demo {\n  voice lead {}\n}\n", 2),
            "score demo {\n  voice lead {}\n}\n"
        );
    }

    #[test]
    fn workspace_symbols_search_open_documents() {
        let first_uri = Uri::from_str("file:///first.music").unwrap();
        let second_uri = Uri::from_str("file:///second.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(
            &first_uri,
            "style Classical {}\nscore prelude {\n  voice lead {}\n}".to_string(),
        );
        documents.open(&second_uri, "fn motif() = [C4]\nscore fugue {}".to_string());
        let response = workspace_symbols(
            &documents,
            &WorkspaceSymbolParams {
                query: "fu".to_string(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .unwrap();
        let WorkspaceSymbolResponse::Nested(symbols) = response else {
            panic!("expected nested workspace symbols");
        };

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "fugue");
        assert_eq!(symbols[0].kind, SymbolKind::NAMESPACE);
        assert_eq!(
            symbols[0].location,
            lsp_types::OneOf::Left(Location {
                uri: second_uri,
                range: Range {
                    start: Position::new(1, 6),
                    end: Position::new(1, 11),
                },
            })
        );
    }

    #[test]
    fn references_match_word_occurrences() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let source =
            "score demo {\n  voice lead {\n    let motif = C4\n    note motif, 1/4\n  }\n}";
        let mut documents = DocumentStore::default();
        documents.open(&uri, source.to_string());
        let references = references_at(
            &documents,
            &ReferenceParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(3, 10),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: lsp_types::ReferenceContext {
                    include_declaration: true,
                },
            },
        )
        .unwrap();

        assert_eq!(references.len(), 2);
        assert!(references.iter().all(|reference| reference.uri == uri));
        assert_eq!(references[0].range.start, Position::new(2, 8));
        assert_eq!(references[1].range.start, Position::new(3, 9));
    }

    #[test]
    fn document_highlights_match_word_occurrences() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let source =
            "score demo {\n  voice lead {\n    let motif = C4\n    note motif, 1/4\n  }\n}";
        let mut documents = DocumentStore::default();
        documents.open(&uri, source.to_string());
        let highlights = document_highlights(
            &documents,
            &DocumentHighlightParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(2, 8),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(highlights.len(), 2);
        assert_eq!(highlights[0].range.start, Position::new(2, 8));
        assert_eq!(highlights[1].range.start, Position::new(3, 9));
        assert!(highlights
            .iter()
            .all(|highlight| highlight.kind == Some(DocumentHighlightKind::TEXT)));
    }

    #[test]
    fn finds_word_at_position() {
        let source = "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}";

        assert_eq!(
            word_at(source, Position::new(2, 5)).as_deref(),
            Some("note")
        );
    }

    #[test]
    fn document_symbols_include_major_declarations() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Strict {\n  scale: C D E\n}\nfn motif {\n  note C4, 1/4\n}\nscore demo {\n  voice lead {\n    section A\n  }\n}".to_string(),
        );
        let Some(DocumentSymbolResponse::Nested(symbols)) = document_symbols(
            &documents,
            &DocumentSymbolParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        ) else {
            panic!("expected nested document symbols");
        };

        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "Strict" && symbol.kind == SymbolKind::CLASS));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "motif" && symbol.kind == SymbolKind::FUNCTION));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "demo" && symbol.kind == SymbolKind::NAMESPACE));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "lead" && symbol.kind == SymbolKind::OBJECT));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "A" && symbol.kind == SymbolKind::MODULE));
    }

    #[test]
    fn document_symbols_use_utf16_selection_ranges() {
        let symbol = source_symbol(0, "style 室内乐 {").unwrap();

        assert_eq!(symbol.name, "室内乐");
        assert_eq!(symbol.selection_range.start, Position::new(0, 6));
        assert_eq!(symbol.selection_range.end, Position::new(0, 9));
    }

    #[test]
    fn document_symbols_return_none_for_unknown_document() {
        let uri = Uri::from_str("file:///missing.music").unwrap();
        let documents = DocumentStore::default();

        assert!(document_symbols(
            &documents,
            &DocumentSymbolParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .is_none());
    }

    #[test]
    fn close_document_removes_cached_source() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(uri.to_string(), "score demo {}".to_string());

        close_document(&mut documents, &uri);

        assert!(!documents.contains_key(&uri.to_string()));
    }

    #[test]
    fn semantic_tokens_classify_musiclang_source() {
        let source = "style Chamber {\n  scale: C D E\n}\nfn motif {\n  note C4, 1/4 // theme\n}\nscore demo style Chamber {\n  voice lead {\n    let d = duration 1/8\n    call motif\n  }\n}";
        let tokens = source_semantic_tokens(source);

        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 0
            && token.length == 5
            && token.token_type == SEMANTIC_KEYWORD));
        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 6
            && token.length == 7
            && token.token_type == SEMANTIC_CLASS
            && token.token_modifiers_bitset == SEMANTIC_DECLARATION));
        assert!(tokens.iter().any(|token| token.line == 3
            && token.start == 3
            && token.length == 5
            && token.token_type == SEMANTIC_FUNCTION
            && token.token_modifiers_bitset == SEMANTIC_DECLARATION));
        assert!(tokens.iter().any(|token| token.line == 4
            && token.start == 7
            && token.length == 2
            && token.token_type == SEMANTIC_NUMBER));
        assert!(tokens
            .iter()
            .any(|token| token.line == 4 && token.token_type == SEMANTIC_COMMENT));
        assert!(tokens.iter().any(|token| token.line == 8
            && token.start == 8
            && token.length == 1
            && token.token_type == SEMANTIC_VARIABLE
            && token.token_modifiers_bitset == SEMANTIC_DECLARATION));
        assert!(tokens.iter().any(|token| token.line == 9
            && token.start == 9
            && token.length == 5
            && token.token_type == SEMANTIC_FUNCTION));
    }

    #[test]
    fn semantic_tokens_classify_algorithmic_expression_features() {
        let source =
            "fn shape(events) = [event.with({d:1/4}) for event in events if not event.skip]";
        let tokens = source_semantic_tokens(source);

        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 40
            && token.length == 3
            && token.token_type == SEMANTIC_KEYWORD));
        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 44
            && token.length == 5
            && token.token_type == SEMANTIC_VARIABLE));
        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 50
            && token.length == 2
            && token.token_type == SEMANTIC_KEYWORD));
        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 63
            && token.length == 3
            && token.token_type == SEMANTIC_KEYWORD));
    }

    #[test]
    fn semantic_tokens_use_utf16_lengths() {
        let tokens = source_semantic_tokens("style 室内乐 {\n}");

        assert!(tokens.iter().any(|token| token.line == 0
            && token.start == 6
            && token.length == 3
            && token.token_type == SEMANTIC_CLASS));
    }

    #[test]
    fn semantic_tokens_encode_lsp_deltas() {
        let encoded = encode_semantic_tokens(vec![
            RawSemanticToken {
                line: 0,
                start: 0,
                length: 5,
                token_type: SEMANTIC_KEYWORD,
                token_modifiers_bitset: 0,
            },
            RawSemanticToken {
                line: 0,
                start: 6,
                length: 4,
                token_type: SEMANTIC_CLASS,
                token_modifiers_bitset: SEMANTIC_DECLARATION,
            },
            RawSemanticToken {
                line: 2,
                start: 2,
                length: 4,
                token_type: SEMANTIC_KEYWORD,
                token_modifiers_bitset: 0,
            },
        ]);

        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[0].delta_start, 0);
        assert_eq!(encoded[1].delta_line, 0);
        assert_eq!(encoded[1].delta_start, 6);
        assert_eq!(encoded[2].delta_line, 2);
        assert_eq!(encoded[2].delta_start, 2);
    }

    #[test]
    fn semantic_tokens_return_none_for_unknown_document() {
        let uri = Uri::from_str("file:///missing.music").unwrap();
        let documents = DocumentStore::default();

        assert!(semantic_tokens(
            &documents,
            &SemanticTokensParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .is_none());
    }

    #[test]
    fn completion_includes_style_rules() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let documents = DocumentStore::default();
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items.iter().any(|item| item.label == "chord_vocab"));
        assert!(items.iter().any(|item| item.label == "instrument_range"));
    }

    #[test]
    fn completion_includes_expression_builtins() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "fn shape(events) = [event.with({d:1/4}) for event in events if ".to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(0, 64),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        for (label, detail, documentation) in [
            ("at", "at(collection, index)", "zero-based index"),
            ("len", "len(collection)", "number of items"),
            ("with", "with(dict, patch)", "patch fields merged"),
            ("merge", "merge(dict, patch)", "patch fields merged"),
            ("not", "not bool_expr", "Negates"),
            ("cat", "cat(values...)", "Concatenates"),
            ("concat", "concat(values...)", "Concatenates"),
            (
                "map",
                "map(collection, function_name) / collection.map(function_name)",
                "bare identifier or string",
            ),
            (
                "filter",
                "filter(collection, function_name) / collection.filter(function_name)",
                "bare identifier or string",
            ),
            (
                "mapi",
                "mapi(collection, function_name) / collection.mapi(function_name)",
                "bare identifier or string",
            ),
            ("transpose", "transpose(collection, interval)", "Transposes"),
            ("repeat", "repeat(value, count)", "Repeats"),
            ("stretch", "stretch(collection, factor)", "Stretches"),
            ("duration", "duration(string)", "Parses"),
            ("pitch", "pitch(string)", "Parses"),
            ("first", "first(collection)", "first element"),
        ] {
            assert!(items.iter().any(|item| item.label == label
                && item.detail.as_deref() == Some(detail)
                && matches!(
                    &item.documentation,
                    Some(lsp_types::Documentation::String(value)) if value.contains(documentation)
                )));
        }
    }

    #[test]
    fn completion_after_dot_includes_method_builtins() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(uri.to_string(), "fn demo(xs) = xs.".to_string());
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(0, 17),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        for (label, detail) in [
            ("map", "map(function_name)"),
            ("filter", "filter(function_name)"),
            ("mapi", "mapi(function_name)"),
            ("with", "with(patch)"),
            ("merge", "merge(patch)"),
            ("transpose", "transpose(interval)"),
            ("stretch", "stretch(factor)"),
        ] {
            assert!(items.iter().any(|item| item.label == label
                && item.kind == Some(CompletionItemKind::METHOD)
                && item.detail.as_deref() == Some(detail)));
        }
    }

    #[test]
    fn expression_builtin_completions_are_accepted_by_compiler() {
        let source = r#"
fn up(event) = event |> transpose(M2)
fn keep(event) = true
fn choose(i, event) = if i == 1 then event |> transpose(M2) else event
score demo {
  voice lead {
    let pitches = [C4, E4]
    let phrases = [{p:C4,d:1/8}, {p:E4,d:1/8}]
    let a = at(pitches, 1)
    let b = first(pitches)
    let c = transpose(C4, M3)
    let d = pitch("G4")
    let e = first(cat([C4], [E4]))
    let f = first(concat([C4], [E4]))
    let g = len(pitches)
    let h = repeat(C4, 2)
    let i = duration("1/4")
    let j = map(phrases, "up")
    let k = filter(phrases, "keep")
    let l = mapi(phrases, "choose")
    let method_chain = phrases.mapi(choose).filter(keep).map(up)
    let m = stretch(method_chain, 2)
    let n = with({p:C4,d:1/8}, {d:1/4})
    let o = merge({p:C4}, {d:1/4})
    note a, 1/4
  }
}
"#;
        let diagnostics = musiclang_compiler::diagnose_source(source);
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "ML_EVAL_UNSUPPORTED_OP"),
            "unexpected unsupported builtin diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn completion_includes_builtin_style_names() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let documents = DocumentStore::default();
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items.iter().any(|item| item.label == "Classical"
            && item.detail.as_deref() == Some("Classical common-practice")));
        assert!(items
            .iter()
            .any(|item| item.label == "Jazz" && item.kind == Some(CompletionItemKind::CLASS)));
    }

    #[test]
    fn completion_includes_local_style_names() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Chamber {\n  scale: C D E\n}\nscore demo style Chamber {\n}".to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(3, 19),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items
            .iter()
            .any(|item| item.label == "Chamber" && item.kind == Some(CompletionItemKind::CLASS)));
    }

    #[test]
    fn completion_includes_local_functions_and_variables() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "fn motif {\n  note C4, 1/4\n}\nfn shape(root, dur) = [{p:root, d:dur}]\nscore demo {\n  voice lead {\n    let d = duration 1/4\n    note C4, \n  }\n}"
                .to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(7, 13),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items
            .iter()
            .any(|item| item.label == "motif" && item.kind == Some(CompletionItemKind::FUNCTION)));
        assert!(items
            .iter()
            .any(|item| item.label == "shape" && item.kind == Some(CompletionItemKind::FUNCTION)));
        assert!(!items.iter().any(|item| item.label == "shape(root,"));
        assert!(items
            .iter()
            .any(|item| item.label == "d" && item.kind == Some(CompletionItemKind::VARIABLE)));
    }

    #[test]
    fn completion_suggests_functions_after_call() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "fn motif {\n  note C4, 1/4\n}\nscore demo {\n  voice lead {\n    call \n  }\n}"
                .to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(5, 9),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items
            .iter()
            .any(|item| item.label == "motif" && item.kind == Some(CompletionItemKind::FUNCTION)));
        assert!(!items.iter().any(|item| item.label == "score"));
    }

    #[test]
    fn completion_suggests_styles_after_score_style() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Chamber {\n  scale: C D E\n}\nscore demo style ".to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(3, 17),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items.iter().any(|item| item.label == "Chamber"));
        assert!(items.iter().any(|item| item.label == "Classical"));
        assert!(!items.iter().any(|item| item.label == "note"));
    }

    #[test]
    fn completion_suggests_style_keys_in_style_block() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(uri.to_string(), "style Strict {\n  ".to_string());
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(1, 2),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items.iter().any(|item| item.label == "scale"));
        assert!(items.iter().any(|item| item.label == "scale_pattern"));
        assert!(items.iter().any(|item| item.label == "mode_pattern"));
        assert!(items.iter().any(|item| item.label == "ornament"));
        assert!(items.iter().any(|item| item.label == "non_chord_tone"));
        assert!(items.iter().any(|item| item.label == "harmonic_function"));
        assert!(items.iter().any(|item| item.label == "tuning_system"));
        assert!(items.iter().any(|item| item.label == "world_tradition"));
        assert!(items.iter().any(|item| item.label == "historical_era"));
        assert!(items.iter().any(|item| item.label == "cadence"));
        assert!(items.iter().any(|item| item.label == "melodic_concept"));
        assert!(items.iter().any(|item| item.label == "phrase_concept"));
        assert!(items.iter().any(|item| item.label == "ensemble_concept"));
        assert!(items.iter().any(|item| item.label == "bass_concept"));
        assert!(!items.iter().any(|item| item.label == "mode"));
        assert!(!items.iter().any(|item| item.label == "ornament_vocab"));
        assert!(!items
            .iter()
            .any(|item| item.label == "non_chord_tone_vocab"));
        assert!(!items.iter().any(|item| item.label == "score"));
    }

    #[test]
    fn style_key_completions_are_accepted_by_compiler() {
        let unsupported = style_rule_completion_items()
            .into_iter()
            .filter_map(|item| {
                let source = format!(
                    "style Probe {{\n  {}: {}\n}}\nscore demo style Probe {{\n}}",
                    item.label,
                    sample_style_value(&item.label)
                );
                musiclang_compiler::diagnose_source(&source)
                    .into_iter()
                    .any(|diagnostic| diagnostic.code == "ML_STYLE_UNKNOWN_KEY")
                    .then_some(item.label)
            })
            .collect::<Vec<_>>();

        assert!(
            unsupported.is_empty(),
            "unsupported style keys: {unsupported:?}"
        );
    }

    fn sample_style_value(key: &str) -> &'static str {
        match key {
            "scale" => "C D E F G A B",
            "scale_pattern" => "C major",
            "mode_pattern" => "D dorian",
            "chord_vocab" => "C E G",
            "chord_quality_vocab" => "major",
            "meter" => "4/4",
            "meter_catalog" => "simple_quadruple",
            "tempo_range" => "60..140",
            "instrument_range" => "40 C4 C5",
            "dynamic_vocab" => "mf",
            "articulation_vocab" => "staccato",
            "ornament" => "trill",
            "non_chord_tone" => "passing",
            "harmonic_function" => "tonic",
            "set_class_vocab" => "3-11",
            "tuning_system" => "equal_temperament",
            "world_tradition" => "western_classical",
            "historical_era" => "common_practice",
            "rhythm_vocab" => "1/4",
            "rhythm_concept" => "swing",
            "melodic_concept" => "blues_inflection",
            "phrase_concept" => "periodic_phrase",
            "ensemble_concept" => "call_response",
            "bass_concept" => "walking_or_riff_bass",
            "form" => "aaba",
            "texture" => "homophony",
            "cadence" => "authentic",
            "max_melodic_leap" => "P8",
            "contrapuntal_motion" => "contrary",
            "voice_spacing" => "P8",
            "harmonic_progression" => "tonic dominant",
            _ => "tonic",
        }
    }

    #[test]
    fn completion_suggests_idiom_entries_for_style_values() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Strict {\n  phrase_concept: ".to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(1, 18),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items.iter().any(|item| item.label == "periodic_phrase"));
        assert!(items.iter().any(|item| item.label == "motivic_development"));
        assert!(!items.iter().any(|item| item.label == "authentic"));
    }

    #[test]
    fn completion_suggests_theory_entries_for_style_values() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(uri.to_string(), "style Strict {\n  cadence: ".to_string());
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(1, 11),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        ) else {
            panic!("expected completion array");
        };

        assert!(items.iter().any(|item| item.label == "authentic"));
        assert!(items
            .iter()
            .any(|item| item.detail.as_deref() == Some("authentic cadence")));
        assert!(!items.iter().any(|item| item.label == "score"));
    }

    #[test]
    fn hover_describes_style_rule() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Test {\n  scale: C D E\n}".to_string(),
        );
        let hover = hover_at(
            &documents,
            &HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(1, 3),
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();

        assert!(matches!(
            hover.contents,
            HoverContents::Scalar(MarkedString::String(value)) if value.contains("pitch classes")
        ));
    }

    #[test]
    fn hover_describes_voice_spacing_style_rule() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Test {\n  voice_spacing: P8\n}".to_string(),
        );
        let hover = hover_at(
            &documents,
            &HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(1, 3),
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();

        assert!(matches!(
            hover.contents,
            HoverContents::Scalar(MarkedString::String(value))
                if value.contains("simultaneous pitched voices") && value.contains("maximum interval")
        ));
    }

    #[test]
    fn hover_describes_phrase_concept_style_rule() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Test {\n  phrase_concept: periodic_phrase\n}".to_string(),
        );
        let hover = hover_at(
            &documents,
            &HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(1, 3),
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();

        assert!(matches!(
            hover.contents,
            HoverContents::Scalar(MarkedString::String(value))
                if value.contains("periodic_phrase") && value.contains("motivic_development")
        ));
    }

    #[test]
    fn hover_describes_local_style() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "style Chamber {\n  scale: C D E\n}\nscore demo style Chamber {\n}".to_string(),
        );
        let hover = hover_at(
            &documents,
            &HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(3, 18),
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();

        assert!(matches!(
            hover.contents,
            HoverContents::Scalar(MarkedString::String(value)) if value.contains("local MusicLang style")
        ));
    }

    #[test]
    fn hover_describes_local_function_and_variable() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.insert(
            uri.to_string(),
            "fn motif {\n  note C4, 1/4\n}\nscore demo {\n  voice lead {\n    let d = duration 1/4\n    call motif\n    note C4, d\n  }\n}".to_string(),
        );
        let function_hover = hover_at(
            &documents,
            &HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(6, 10),
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();
        assert!(matches!(
            function_hover.contents,
            HoverContents::Scalar(MarkedString::String(value)) if value.contains("local MusicLang function")
        ));

        let variable_hover = hover_at(
            &documents,
            &HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(7, 14),
                },
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();
        assert!(matches!(
            variable_hover.contents,
            HoverContents::Scalar(MarkedString::String(value)) if value.contains("local MusicLang value")
        ));
    }

    #[test]
    fn hover_describes_expression_builtins() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let source = "fn line() = [with({p:at([C4], i), d:1/8}, {d:1/4}) for i in 0..1]\nfn size(xs) = len(xs)\nfn patch(x) = merge(x, {d:1/2})\nfn transforms(xs) = concat(xs.map(up), first(xs), xs.filter(keep), xs.mapi(mark))";
        let mut documents = DocumentStore::default();
        documents.insert(uri.to_string(), source.to_string());

        for (word, expected) in [
            ("at", "zero-based index"),
            ("with", "patch fields merged"),
            ("len", "number of items"),
            ("merge", "patch fields merged"),
            ("concat", "flattening lists and non-note tuples"),
            ("map", "collection.map"),
            ("first", "non-empty list or tuple"),
            ("filter", "collection.filter"),
            ("mapi", "collection.mapi"),
        ] {
            let offset = source.find(word).unwrap();
            let hover = hover_at(
                &documents,
                &HoverParams {
                    text_document_position_params: lsp_types::TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                        position: byte_offset_to_position(source, offset),
                    },
                    work_done_progress_params: Default::default(),
                },
            )
            .unwrap();

            assert!(matches!(
                hover.contents,
                HoverContents::Scalar(MarkedString::String(value)) if value.contains(expected)
            ));
        }
    }

    #[test]
    fn converts_compiler_diagnostic_to_lsp_range() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let source = "score demo {\n  note C4, 1/4\n}";
        let start = source.find("note").unwrap();
        let span = musiclang_core::Span {
            source_id: musiclang_core::SourceId(0),
            start,
            end: start + "note".len(),
            line: 2,
            column: 3,
        };
        let diagnostic = to_lsp_diagnostic(
            source,
            &uri,
            musiclang_core::Diagnostic::error("ML_TEST", "example diagnostic", 1, 1)
                .with_span(span),
        );

        assert_eq!(diagnostic.range.start, Position::new(1, 2));
        assert_eq!(diagnostic.range.end, Position::new(1, 6));
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("musiclang"));
    }

    #[test]
    fn converts_span_width_to_lsp_range() {
        let source = "score demo {\n  note C4, 1/4\n}";
        let start = source.find("note").unwrap();
        let span = musiclang_core::Span {
            source_id: musiclang_core::SourceId(0),
            start,
            end: start + "note C4".len(),
            line: 2,
            column: 3,
        };
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let diagnostic = to_lsp_diagnostic(
            source,
            &uri,
            musiclang_core::Diagnostic::error("ML_TEST", "example diagnostic", 1, 1)
                .with_span(span),
        );

        assert_eq!(diagnostic.range.start, Position::new(1, 2));
        assert_eq!(diagnostic.range.end, Position::new(1, 9));
    }

    #[test]
    fn converts_multiline_span_to_lsp_range() {
        let source = "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}";
        let start = source.find("voice").unwrap();
        let end = source.find("note C4").unwrap() + "note".len();
        let span = musiclang_core::Span {
            source_id: musiclang_core::SourceId(0),
            start,
            end,
            line: 2,
            column: 3,
        };
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let diagnostic = to_lsp_diagnostic(
            source,
            &uri,
            musiclang_core::Diagnostic::error("ML_TEST", "example diagnostic", 1, 1)
                .with_span(span),
        );

        assert_eq!(diagnostic.range.start, Position::new(1, 2));
        assert_eq!(diagnostic.range.end, Position::new(2, 8));
    }

    #[test]
    fn converts_utf8_byte_span_to_utf16_lsp_range() {
        let source = "// π\nnote C4, 1/4";
        let start = source.find("note").unwrap();
        let span = musiclang_core::Span {
            source_id: musiclang_core::SourceId(0),
            start,
            end: source.len(),
            line: 2,
            column: 1,
        };
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let diagnostic = to_lsp_diagnostic(
            source,
            &uri,
            musiclang_core::Diagnostic::error("ML_TEST", "example diagnostic", 1, 1)
                .with_span(span),
        );

        assert_eq!(diagnostic.range.start, Position::new(1, 0));
        assert_eq!(diagnostic.range.end, Position::new(1, 12));
    }

    #[test]
    fn converts_missing_span_with_line_column_fallback() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut diagnostic =
            musiclang_core::Diagnostic::error("ML_TEST", "example diagnostic", 4, 2);
        diagnostic.span = None;
        let diagnostic = to_lsp_diagnostic("", &uri, diagnostic);

        assert_eq!(diagnostic.range.start, Position::new(3, 1));
        assert_eq!(diagnostic.range.end, Position::new(3, 2));
    }

    #[test]
    fn maps_diagnostic_metadata_to_lsp() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let source = "style Strict {\n  scale: C D E\n}\nscore demo {\n  note F4, 1/4\n}";
        let label_start = source.find("scale").unwrap();
        let related_start = source.find("note").unwrap();
        let label_span = musiclang_core::Span {
            source_id: musiclang_core::SourceId(0),
            start: label_start,
            end: label_start + "scale".len(),
            line: 2,
            column: 3,
        };
        let related_span = musiclang_core::Span {
            source_id: musiclang_core::SourceId(0),
            start: related_start,
            end: related_start + "note".len(),
            line: 5,
            column: 3,
        };
        let diagnostic = to_lsp_diagnostic(
            source,
            &uri,
            musiclang_core::Diagnostic::error("ML_STYLE_SCALE", "outside scale", 5, 3)
                .with_label(label_span, "configured scale")
                .with_related(related_span, "offending note")
                .with_rule("scale")
                .with_style("Strict")
                .with_help("Use a pitch from the configured scale."),
        );

        let related = diagnostic.related_information.unwrap();
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].location.uri, uri);
        assert_eq!(related[0].location.range.start, Position::new(1, 2));
        assert_eq!(related[0].message, "configured scale");
        assert_eq!(related[1].location.range.start, Position::new(4, 2));
        assert_eq!(related[1].message, "offending note");
        let data = diagnostic.data.unwrap();
        assert_eq!(data["rule"], "scale");
        assert_eq!(data["style"], "Strict");
        assert_eq!(data["help"], "Use a pitch from the configured scale.");
    }

    #[test]
    fn maps_duplicate_function_related_information_to_lsp() {
        let uri = Uri::from_str("file:///duplicate.music").unwrap();
        let source = r#"
fn motif {
  note C4, 1/4
}
fn motif {
  note D4, 1/4
}
score demo {
  voice lead {
    call motif
  }
}
"#;
        let compiler_diagnostic = musiclang_compiler::diagnose_source(source)
            .into_iter()
            .find(|diagnostic| diagnostic.code == "ML_RESOLVE_DUPLICATE_NAME")
            .unwrap();
        let diagnostic = to_lsp_diagnostic(source, &uri, compiler_diagnostic);

        let related = diagnostic.related_information.unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].location.uri, uri);
        assert_eq!(related[0].location.range.start, Position::new(1, 0));
        assert_eq!(related[0].message, "first function definition");
        let data = diagnostic.data.unwrap();
        assert_eq!(
            data["help"],
            "rename one function or remove the duplicate definition"
        );
    }

    #[test]
    fn source_map_diagnostics_include_document_source_id_in_lsp_data() {
        let uri = Uri::from_str("file:///workspace/demo.music").unwrap();
        let source = r#"
style Classical
score demo {
  voice lead {
    note F#4, 1/4
  }
}
"#;
        let other_uri = Uri::from_str("file:///workspace/other.music").unwrap();
        let mut documents = DocumentStore::default();
        documents.open(
            &other_uri,
            "score other { voice lead { note C4, 1/4 } }".to_string(),
        );
        documents.open(&uri, source.to_string());
        let source_file = documents.source_file(&uri).unwrap();
        let source_id = source_file.id;
        let compiler_diagnostic = musiclang_compiler::diagnose_source_file(&source_file)
            .into_iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_SCALE")
            .unwrap();
        let diagnostic = to_lsp_diagnostic(source, &uri, compiler_diagnostic);
        let data = diagnostic.data.unwrap();

        assert_eq!(source_file.name, uri.to_string());
        assert_eq!(source_id, SourceId(1));
        assert_eq!(data["source_id"], source_id.0);
        assert_eq!(data["source_name"], uri.to_string());
        assert_eq!(diagnostic.range.start, Position::new(4, 4));
    }

    #[test]
    fn maps_compiler_style_diagnostic_data_to_lsp() {
        let uri = Uri::from_str("file:///weak-jazz.music").unwrap();
        let source = r#"
score weak_jazz style Jazz {
  tempo 112
  meter 4/4
  key C major
  voice lead {
    note C4, 1/4
    note E4, 1/4
    note G4, 1/2
  }
  voice bass {
    instrument bass
    note C2, 1/4
    note E2, 1/4
    note G2, 1/4
    note B2, 1/4
  }
}
"#;
        let compiler_diagnostic = musiclang_compiler::diagnose_source(source)
            .into_iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_MELODIC_CONCEPT")
            .unwrap();
        let diagnostic = to_lsp_diagnostic(source, &uri, compiler_diagnostic);

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            diagnostic.code,
            Some(lsp_types::NumberOrString::String(
                "ML_STYLE_MELODIC_CONCEPT".to_string()
            ))
        );
        let data = diagnostic.data.unwrap();
        assert_eq!(data["rule"], "melodic_concept");
        assert_eq!(data["style"], "Jazz");
        assert_eq!(
            data["help"],
            "adjust the active style rule `melodic_concept` or use an explicit audited override for intentional local exceptions"
        );
    }

    #[test]
    fn maps_phrase_concept_diagnostic_data_to_lsp() {
        let uri = Uri::from_str("file:///fragment.music").unwrap();
        let source = r#"
style Periodic {
  phrase_concept: periodic_phrase
}
score fragment style Periodic {
  voice lead {
    section A {
      note C4, 1/4
    }
  }
}
"#;
        let compiler_diagnostic = musiclang_compiler::diagnose_source(source)
            .into_iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_PHRASE_CONCEPT")
            .unwrap();
        let diagnostic = to_lsp_diagnostic(source, &uri, compiler_diagnostic);

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            diagnostic.code,
            Some(lsp_types::NumberOrString::String(
                "ML_STYLE_PHRASE_CONCEPT".to_string()
            ))
        );
        let data = diagnostic.data.unwrap();
        assert_eq!(data["rule"], "phrase_concept");
        assert_eq!(data["style"], "Periodic");
        assert_eq!(
            data["help"],
            "adjust the active style rule `phrase_concept` or use an explicit audited override for intentional local exceptions"
        );
    }

    #[test]
    fn finds_function_definition() {
        let source = "fn motif {\n  note C4, 1/4\n}\nscore demo {\n  call motif\n}";

        assert_eq!(find_definition(source, "motif"), Some(Position::new(0, 3)));
    }

    #[test]
    fn finds_variable_definition() {
        let source =
            "score demo {\n  voice lead {\n    let d = duration 1/4\n    note C4, d\n  }\n}";

        assert_eq!(find_definition(source, "d"), Some(Position::new(2, 8)));
    }

    #[test]
    fn finds_style_definition() {
        let source = "style Chamber {\n  scale: C D E\n}\nscore demo style Chamber {\n}";

        assert_eq!(
            find_definition(source, "Chamber"),
            Some(Position::new(0, 6))
        );
    }

    #[test]
    fn definition_response_uses_utf16_range() {
        let uri: Uri = "file:///score.music".parse().unwrap();
        let source = "fn 旋律 {\n  note C4, 1/4\n}\nscore demo {\n  call 旋律\n}";
        let mut documents = DocumentStore::default();
        documents.insert(uri.to_string(), source.to_string());

        let response = definition_at(
            &documents,
            &GotoDefinitionParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                    position: Position::new(4, 7),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )
        .unwrap();

        let GotoDefinitionResponse::Scalar(location) = response else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.uri, uri);
        assert_eq!(location.range.start, Position::new(0, 3));
        assert_eq!(location.range.end, Position::new(0, 5));
    }
}
