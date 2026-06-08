use std::collections::HashMap;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Diagnostic,
    DiagnosticRelatedInformation, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    InitializeParams, Location, MarkedString, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, SymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use musiclang_core::{Severity, BUILT_IN_STYLES};

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        completion_provider: Some(lsp_types::CompletionOptions::default()),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        ..ServerCapabilities::default()
    })?;
    let params = connection.initialize(capabilities)?;
    let _params: InitializeParams = serde_json::from_value(params)?;
    let mut documents = HashMap::<String, String>::new();

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
    documents: &HashMap<String, String>,
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
    documents: &mut HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;
            let uri = params.text_document.uri;
            documents.insert(uri.to_string(), params.text_document.text);
            publish_diagnostics(connection, uri, documents)?;
        }
        "textDocument/didChange" => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;
            if let Some(change) = params.content_changes.into_iter().last() {
                let uri = params.text_document.uri;
                documents.insert(uri.to_string(), change.text);
                publish_diagnostics(connection, uri, documents)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    documents: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let Some(source) = documents.get(&uri.to_string()) else {
        return Ok(());
    };
    let diagnostics = musiclang_compiler::diagnose_source(source)
        .into_iter()
        .map(|diagnostic| to_lsp_diagnostic(source, &uri, diagnostic))
        .collect();
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
    let data = diagnostic_data(&diagnostic);
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

fn diagnostic_data(diagnostic: &musiclang_core::Diagnostic) -> Option<serde_json::Value> {
    let mut data = serde_json::Map::new();
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

fn hover_at(documents: &HashMap<String, String>, params: &HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let source = documents.get(&uri.to_string())?;
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
        _ => return None,
    };
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(text.to_string())),
        range: None,
    })
}

fn definition_at(
    documents: &HashMap<String, String>,
    params: &GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let source = documents.get(&uri.to_string())?;
    let word = word_at(source, params.text_document_position_params.position)?;
    let position = find_definition(source, &word)?;
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: Range {
            start: position,
            end: Position::new(position.line, position.character + word.len() as u32),
        },
    }))
}

fn document_symbols(
    documents: &HashMap<String, String>,
    params: &DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = &params.text_document.uri;
    let source = documents.get(&uri.to_string())?;
    let symbols = source
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| source_symbol(line_index, line))
        .collect::<Vec<_>>();
    Some(DocumentSymbolResponse::Nested(symbols))
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

fn find_definition(source: &str, word: &str) -> Option<Position> {
    for (line_index, line) in source.lines().enumerate() {
        for keyword in ["fn", "let", "style"] {
            let pattern = format!("{keyword} {word}");
            if let Some(index) = line.find(&pattern) {
                return Some(Position::new(
                    line_index as u32,
                    (index + keyword.len() + 1) as u32,
                ));
            }
        }
    }
    None
}

fn completion_items(
    documents: &HashMap<String, String>,
    params: &CompletionParams,
) -> CompletionResponse {
    let uri = &params.text_document_position.text_document.uri;
    let Some(source) = documents.get(&uri.to_string()) else {
        let mut items = general_completion_items();
        items.extend(style_rule_completion_items());
        items.extend(builtin_style_completion_items());
        return CompletionResponse::Array(items);
    };

    let position = params.text_document_position.position;
    let line_prefix = line_prefix(source, position);
    if is_call_context(&line_prefix) {
        return CompletionResponse::Array(local_function_completion_items(source));
    }
    if is_score_style_context(&line_prefix) {
        return CompletionResponse::Array(style_completion_items(source));
    }
    if let Some(domain) = style_value_context(&line_prefix) {
        return CompletionResponse::Array(theory_entry_completion_items(domain));
    }
    if is_style_key_context(source, position, &line_prefix) {
        return CompletionResponse::Array(style_rule_completion_items());
    }

    let mut items = general_completion_items();
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

fn style_rule_completion_items() -> Vec<CompletionItem> {
    [
        "scale",
        "mode",
        "chord_vocab",
        "chord_quality_vocab",
        "meter",
        "meter_catalog",
        "tempo_range",
        "instrument_range",
        "dynamic_vocab",
        "articulation_vocab",
        "ornament_vocab",
        "non_chord_tone_vocab",
        "harmonic_function_vocab",
        "set_class_vocab",
        "tuning_system_vocab",
        "world_tradition_vocab",
        "historical_era_vocab",
        "rhythm_vocab",
        "rhythm_concept",
        "form",
        "texture",
        "cadence",
        "parallel_fifths",
        "voice_crossing",
        "max_melodic_leap",
        "contrapuntal_motion",
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

fn style_value_context(line_prefix: &str) -> Option<musiclang_core::TheoryDomain> {
    let key = line_prefix.split_once(':')?.0.trim();
    style_key_domain(key)
}

fn style_key_domain(key: &str) -> Option<musiclang_core::TheoryDomain> {
    match key {
        "scale" => Some(musiclang_core::TheoryDomain::Scales),
        "mode" => Some(musiclang_core::TheoryDomain::Modes),
        "chord_vocab" | "chord_quality_vocab" => Some(musiclang_core::TheoryDomain::ChordQualities),
        "meter" | "meter_catalog" => Some(musiclang_core::TheoryDomain::Meters),
        "dynamic_vocab" => Some(musiclang_core::TheoryDomain::Dynamics),
        "articulation_vocab" | "ornament_vocab" => Some(musiclang_core::TheoryDomain::Ornaments),
        "non_chord_tone_vocab" => Some(musiclang_core::TheoryDomain::NonChordTones),
        "harmonic_function_vocab" => Some(musiclang_core::TheoryDomain::HarmonicFunctions),
        "set_class_vocab" => Some(musiclang_core::TheoryDomain::SetClasses),
        "tuning_system_vocab" => Some(musiclang_core::TheoryDomain::TuningSystems),
        "world_tradition_vocab" => Some(musiclang_core::TheoryDomain::WorldTraditions),
        "historical_era_vocab" => Some(musiclang_core::TheoryDomain::StyleEras),
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
    local_declaration_names(source, "fn")
}

fn local_variable_names(source: &str) -> Vec<String> {
    local_declaration_names(source, "let")
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
    let chars = line.chars().collect::<Vec<_>>();
    let mut start = position.character.min(chars.len() as u32) as usize;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = position.character.min(chars.len() as u32) as usize;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    (start < end).then(|| chars[start..end].iter().collect())
}

fn is_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#')
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

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
        let mut documents = HashMap::new();
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
        let documents = HashMap::new();

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
    fn completion_includes_style_rules() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let documents = HashMap::new();
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
    fn completion_includes_builtin_style_names() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let documents = HashMap::new();
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
        let mut documents = HashMap::new();
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
        let mut documents = HashMap::new();
        documents.insert(
            uri.to_string(),
            "fn motif {\n  note C4, 1/4\n}\nscore demo {\n  voice lead {\n    let d = duration 1/4\n    note C4, \n  }\n}".to_string(),
        );
        let CompletionResponse::Array(items) = completion_items(
            &documents,
            &CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    position: Position::new(6, 13),
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
            .any(|item| item.label == "d" && item.kind == Some(CompletionItemKind::VARIABLE)));
    }

    #[test]
    fn completion_suggests_functions_after_call() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = HashMap::new();
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
        let mut documents = HashMap::new();
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
        let mut documents = HashMap::new();
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
        assert!(items.iter().any(|item| item.label == "cadence"));
        assert!(!items.iter().any(|item| item.label == "score"));
    }

    #[test]
    fn completion_suggests_theory_entries_for_style_values() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = HashMap::new();
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
        let mut documents = HashMap::new();
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
    fn hover_describes_local_style() {
        let uri = Uri::from_str("file:///demo.music").unwrap();
        let mut documents = HashMap::new();
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
        let mut documents = HashMap::new();
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
}
