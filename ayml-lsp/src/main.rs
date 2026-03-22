#![allow(clippy::mutable_key_type)] // Uri from lsp-types has interior mutability but is used as a map key by convention.

mod convert;
mod locate;
mod schema;

use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, MarkupContent, MarkupKind, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    notification::Notification as _, request::Request as _,
};
use std::collections::HashMap;

fn main() {
    if let Err(e) = run() {
        eprintln!("ayml-lsp error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (connection, io_threads) = Connection::stdio();

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..Default::default()
    };

    let init_params = connection.initialize(serde_json::to_value(&capabilities)?)?;
    let _init_params: InitializeParams = serde_json::from_value(init_params)?;

    main_loop(&connection)?;

    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut documents: HashMap<Uri, String> = HashMap::new();
    let mut schema_cache: HashMap<String, serde_json::Value> = HashMap::new();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                match req.method.as_str() {
                    lsp_types::request::HoverRequest::METHOD => {
                        let (id, params): (_, HoverParams) =
                            req.extract(lsp_types::request::HoverRequest::METHOD)?;
                        let hover = handle_hover(&params, &documents, &mut schema_cache);
                        let resp = Response::new_ok(id, hover);
                        connection.sender.send(Message::Response(resp))?;
                    }
                    _ => {
                        let resp = Response::new_err(
                            req.id,
                            lsp_server::ErrorCode::MethodNotFound as i32,
                            format!("unhandled method: {}", req.method),
                        );
                        connection.sender.send(Message::Response(resp))?;
                    }
                }
            }
            Message::Notification(not) => match not.method.as_str() {
                lsp_types::notification::DidOpenTextDocument::METHOD => {
                    let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
                    let uri = params.text_document.uri.clone();
                    let text = params.text_document.text.clone();
                    documents.insert(uri.clone(), text.clone());
                    publish_diagnostics(connection, uri, &text, &mut schema_cache)?;
                }
                lsp_types::notification::DidChangeTextDocument::METHOD => {
                    let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)?;
                    let uri = params.text_document.uri.clone();
                    if let Some(change) = params.content_changes.into_iter().next() {
                        documents.insert(uri.clone(), change.text.clone());
                        publish_diagnostics(connection, uri, &change.text, &mut schema_cache)?;
                    }
                }
                lsp_types::notification::DidCloseTextDocument::METHOD => {
                    let params: DidCloseTextDocumentParams = serde_json::from_value(not.params)?;
                    documents.remove(&params.text_document.uri);
                    let params = PublishDiagnosticsParams {
                        uri: params.text_document.uri,
                        diagnostics: vec![],
                        version: None,
                    };
                    send_notification::<lsp_types::notification::PublishDiagnostics>(
                        connection, params,
                    )?;
                }
                _ => {}
            },
            Message::Response(_) => {}
        }
    }

    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    text: &str,
    schema_cache: &mut HashMap<String, serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut diagnostics = Vec::new();

    match ayml_core::parse(text) {
        Err(e) => {
            let line = e.line.saturating_sub(1) as u32;
            let col = e.column.saturating_sub(1) as u32;
            diagnostics.push(Diagnostic {
                range: Range::new(Position::new(line, col), Position::new(line, col)),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("ayml".to_string()),
                message: format!("{e}"),
                ..Default::default()
            });
        }
        Ok(node) => {
            // Check for schema directive in the document comment.
            if let Some(comment) = &node.comment
                && let Some(schema_url) = ayml_core::schema_uri(comment)
            {
                let schema_diagnostics =
                    validate_with_schema(&node, text, schema_url, schema_cache);
                diagnostics.extend(schema_diagnostics);
            }
        }
    }

    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    send_notification::<lsp_types::notification::PublishDiagnostics>(connection, params)?;
    Ok(())
}

fn validate_with_schema(
    node: &ayml_core::Node,
    text: &str,
    schema_url: &str,
    cache: &mut HashMap<String, serde_json::Value>,
) -> Vec<Diagnostic> {
    let schema_value = match cache.get(schema_url) {
        Some(v) => v.clone(),
        None => match fetch_schema(schema_url) {
            Ok(v) => {
                cache.insert(schema_url.to_string(), v.clone());
                v
            }
            Err(e) => {
                return vec![Diagnostic {
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("ayml".to_string()),
                    message: format!("failed to fetch schema: {e}"),
                    ..Default::default()
                }];
            }
        },
    };

    let validator = match jsonschema::validator_for(&schema_value) {
        Ok(v) => v,
        Err(e) => {
            return vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("ayml".to_string()),
                message: format!("invalid schema: {e}"),
                ..Default::default()
            }];
        }
    };

    let json_value = convert::node_to_json(node);

    validator
        .iter_errors(&json_value)
        .map(|error| {
            let path = error.instance_path().to_string();
            let range = resolve_instance_path(node, &path)
                .map(|span| span_to_range(text, span))
                .unwrap_or(Range::new(Position::new(0, 0), Position::new(0, 0)));
            let message = if path.is_empty() {
                format!("{error}")
            } else {
                format!("{path}: {error}")
            };
            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("ayml-schema".to_string()),
                message,
                ..Default::default()
            }
        })
        .collect()
}

/// Walk a JSON pointer path (e.g. "/servers/0/port") through the Node tree
/// and return the span of the target node.
fn resolve_instance_path(node: &ayml_core::Node, path: &str) -> Option<ayml_core::Span> {
    if path.is_empty() {
        return Some(node.span);
    }

    let segments: Vec<&str> = path.strip_prefix('/')?.split('/').collect();
    let mut current = node;

    for segment in &segments {
        match &current.value {
            ayml_core::Value::Map(map) => {
                let key = ayml_core::MapKey::String(segment.to_string());
                current = map.get(&key)?;
            }
            ayml_core::Value::Seq(items) => {
                let index: usize = segment.parse().ok()?;
                current = items.get(index)?;
            }
            _ => return None,
        }
    }

    Some(current.span)
}

/// Convert a byte-offset Span to an LSP Range using the source text.
fn span_to_range(text: &str, span: ayml_core::Span) -> Range {
    let start = offset_to_position(text, span.start);
    let end = offset_to_position(text, span.end);
    Range::new(start, end)
}

/// Convert a byte offset to a 0-based LSP Position.
fn offset_to_position(text: &str, offset: usize) -> Position {
    let offset = offset.min(text.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text[..offset].char_indices() {
        if ch == '\n' {
            line += 1;
            col = 0;
        } else if ch == '\r' {
            line += 1;
            col = 0;
            // Skip the \n in \r\n
            if text.as_bytes().get(i + 1) == Some(&b'\n') {
                continue;
            }
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

fn handle_hover(
    params: &HoverParams,
    documents: &HashMap<Uri, String>,
    schema_cache: &mut HashMap<String, serde_json::Value>,
) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let text = documents.get(uri)?;
    let node = ayml_core::parse(text).ok()?;

    // Find the schema URL from the document comment.
    let schema_url = node.comment.as_deref().and_then(ayml_core::schema_uri)?;

    let schema_value = match schema_cache.get(schema_url) {
        Some(v) => v.clone(),
        None => {
            let v = fetch_schema(schema_url).ok()?;
            schema_cache.insert(schema_url.to_string(), v.clone());
            v
        }
    };

    // Map cursor position to byte offset, then to a path in the node tree.
    let pos = params.text_document_position_params.position;
    let offset = position_to_offset(text, pos);

    // Don't show hover on comment lines.
    if is_in_comment(text, offset) {
        return None;
    }

    let path_segments = locate::path_at_offset(&node, offset);
    let path_refs: Vec<&str> = path_segments.iter().map(|s| s.as_str()).collect();

    // Walk the schema to the sub-schema at that path.
    let sub_schema = schema::resolve_sub_schema(&schema_value, &path_refs)?;
    let content = schema::hover_content(sub_schema)?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: None,
    })
}

/// Convert a 0-based LSP Position to a byte offset in the source text.
fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if line == pos.line && col == pos.character {
            return i;
        }
        if ch == '\n' || ch == '\r' {
            if line == pos.line {
                return i; // cursor is past end of this line
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    text.len()
}

/// Check if the byte offset falls within a comment — either a full comment
/// line (first non-space is `#`) or an inline comment (after ` #` on a line).
fn is_in_comment(text: &str, offset: usize) -> bool {
    let line_start = text[..offset]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line = &text[line_start..];

    // Full comment line: first non-whitespace is `#`.
    if line.bytes().find(|&b| b != b' ' && b != b'\t') == Some(b'#') {
        return true;
    }

    // Inline comment: check if offset is at or past a ` #` on this line.
    // We need to skip `#` inside quoted strings.
    let offset_in_line = offset - line_start;
    let mut in_quote = false;
    let mut i = 0;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_quote = !in_quote;
        } else if !in_quote && bytes[i] == b'#' {
            // Found a comment marker — cursor is in a comment if at or past it.
            if offset_in_line >= i {
                return true;
            }
            break;
        } else if bytes[i] == b'\n' || bytes[i] == b'\r' {
            break;
        }
        i += 1;
    }

    false
}

fn fetch_schema(url: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if url.starts_with("file://") {
        let path = url.strip_prefix("file://").unwrap();
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        let body = ureq::get(url).call()?.body_mut().read_to_string()?;
        Ok(serde_json::from_str(&body)?)
    }
}

fn send_notification<N: lsp_types::notification::Notification>(
    connection: &Connection,
    params: N::Params,
) -> Result<(), Box<dyn std::error::Error>> {
    let not = Notification::new(N::METHOD.to_string(), params);
    connection.sender.send(Message::Notification(not))?;
    Ok(())
}
