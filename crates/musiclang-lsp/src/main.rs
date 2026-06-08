use std::collections::HashMap;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Diagnostic,
    DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    InitializeParams, Location, MarkedString, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use musiclang_core::{Severity, BUILT_IN_STYLES};

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        completion_provider: Some(lsp_types::CompletionOptions::default()),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
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
        .map(to_lsp_diagnostic)
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

fn to_lsp_diagnostic(diagnostic: musiclang_core::Diagnostic) -> Diagnostic {
    let line = diagnostic.line.saturating_sub(1) as u32;
    let column = diagnostic.column.saturating_sub(1) as u32;
    Diagnostic {
        range: Range {
            start: Position::new(line, column),
            end: Position::new(line, column + 1),
        },
        severity: Some(match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(lsp_types::NumberOrString::String(diagnostic.code)),
        source: Some("musiclang".to_string()),
        message: diagnostic.message,
        ..Diagnostic::default()
    }
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
    let keywords = [
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
        "scale",
        "chord_vocab",
        "tempo_range",
        "instrument_range",
        "parallel_fifths",
        "voice_crossing",
    ];
    let mut items = keywords
        .into_iter()
        .map(|label| CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            insert_text: Some(label.to_string()),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    items.extend(BUILT_IN_STYLES.iter().map(|style| CompletionItem {
        label: style.id.to_string(),
        kind: Some(CompletionItemKind::CLASS),
        detail: Some(style.name.to_string()),
        documentation: Some(lsp_types::Documentation::String(
            style.description.to_string(),
        )),
        ..CompletionItem::default()
    }));

    let uri = &params.text_document_position.text_document.uri;
    if let Some(source) = documents.get(&uri.to_string()) {
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
    }

    CompletionResponse::Array(items)
}

fn local_style_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if words.next()? == "style" {
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
    fn converts_compiler_diagnostic_to_lsp_range() {
        let diagnostic = to_lsp_diagnostic(musiclang_core::Diagnostic::error(
            "ML_TEST",
            "example diagnostic",
            2,
            4,
        ));

        assert_eq!(diagnostic.range.start, Position::new(1, 3));
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("musiclang"));
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
