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

    let mut diagnostics = Vec::new();
    for error in validator.iter_errors(&json_value) {
        collect_leaf_errors(&error, node, text, &schema_value, &mut diagnostics);
    }
    diagnostics
}

/// Recursively collect the most specific (leaf) validation errors.
/// Composition errors (anyOf, oneOf) are expanded into their sub-errors
/// so diagnostics point to the actual invalid values rather than the
/// outer composition keyword.
fn collect_leaf_errors(
    error: &jsonschema::ValidationError<'_>,
    node: &ayml_core::Node,
    text: &str,
    schema_root: &serde_json::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use jsonschema::error::ValidationErrorKind;

    match error.kind() {
        ValidationErrorKind::AnyOf { context } | ValidationErrorKind::OneOfNotValid { context } => {
            // Check if all variants failed at the same instance path (i.e.
            // the error is about the value itself, not a nested property).
            // In that case, produce a combined message describing valid options.
            let error_path = error.instance_path().to_string();
            let all_same_path = context.iter().all(|variant_errors| {
                variant_errors
                    .iter()
                    .all(|e| e.instance_path().to_string() == error_path)
            });

            if all_same_path {
                // All variants failed at this same path — describe what's expected.
                let path = &error_path;
                let range = resolve_instance_path(node, path)
                    .map(|span| span_to_range(text, span))
                    .unwrap_or(Range::new(Position::new(0, 0), Position::new(0, 0)));

                // Try to build a helpful message from the schema.
                let path_segments: Vec<&str> = if path.is_empty() {
                    vec![]
                } else {
                    path.strip_prefix('/').unwrap_or(path).split('/').collect()
                };
                let message =
                    if let Some(sub) = schema::resolve_sub_schema(schema_root, &path_segments) {
                        let ty = schema::hover_content(schema_root, sub)
                            .and_then(|content| {
                                // Extract just the type line.
                                content
                                    .lines()
                                    .find(|l| l.starts_with("**Type:**"))
                                    .map(|l| {
                                        l.trim_start_matches("**Type:** `")
                                            .trim_end_matches('`')
                                            .to_string()
                                    })
                            })
                            .unwrap_or_else(|| "a valid value".to_string());
                        if path.is_empty() {
                            format!("expected {ty}")
                        } else {
                            format!("{path}: expected {ty}")
                        }
                    } else if path.is_empty() {
                        format!("{error}")
                    } else {
                        format!("{path}: {error}")
                    };

                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("ayml-schema".to_string()),
                    message,
                    ..Default::default()
                });
                return;
            }

            // Variants failed at different paths — recurse into the deepest.
            let deepest = context
                .iter()
                .map(|variant_errors| {
                    variant_errors
                        .iter()
                        .map(|e: &jsonschema::ValidationError<'_>| e.instance_path().iter().count())
                        .max()
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            for variant_errors in context {
                let variant_depth = variant_errors
                    .iter()
                    .map(|e: &jsonschema::ValidationError<'_>| e.instance_path().iter().count())
                    .max()
                    .unwrap_or(0);
                if variant_depth >= deepest {
                    for sub_error in variant_errors {
                        collect_leaf_errors(sub_error, node, text, schema_root, diagnostics);
                    }
                    return;
                }
            }
        }
        _ => {}
    }

    // Leaf error — emit a diagnostic.
    let path = error.instance_path().to_string();
    let range = resolve_instance_path(node, &path)
        .map(|span| span_to_range(text, span))
        .unwrap_or(Range::new(Position::new(0, 0), Position::new(0, 0)));
    let message = if path.is_empty() {
        format!("{error}")
    } else {
        format!("{path}: {error}")
    };
    diagnostics.push(Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("ayml-schema".to_string()),
        message,
        ..Default::default()
    });
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
    let content = schema::hover_content(&schema_value, sub_schema)?;

    let hover_range = compute_hover_range(&node, &path_segments, text);

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: hover_range,
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

/// Compute the hover highlight range for a mapping entry.
///
/// - Scalar values: highlight the whole `key: value` pair.
/// - Block values (maps, non-empty seqs): highlight just `key:`.
/// - Non-mapping paths (e.g. sequence indices): highlight the value span.
fn compute_hover_range(root: &ayml_core::Node, path: &[String], text: &str) -> Option<Range> {
    if path.is_empty() {
        return Some(span_to_range(text, root.span));
    }

    // Walk to the parent node and get the last segment (the key).
    let mut current = root;
    for segment in &path[..path.len() - 1] {
        match &current.value {
            ayml_core::Value::Map(map) => {
                let key = ayml_core::MapKey::String(segment.clone());
                current = map.get(&key)?;
            }
            ayml_core::Value::Seq(items) => {
                let idx: usize = segment.parse().ok()?;
                current = items.get(idx)?;
            }
            _ => return None,
        }
    }

    let last_segment = path.last()?;

    // If parent is a sequence, highlight just the value span.
    let ayml_core::Value::Map(map) = &current.value else {
        if let ayml_core::Value::Seq(items) = &current.value {
            let idx: usize = last_segment.parse().ok()?;
            let item = items.get(idx)?;
            return Some(span_to_range(text, item.span));
        }
        return None;
    };

    let key = ayml_core::MapKey::String(last_segment.clone());
    let value_node = map.get(&key)?;

    let is_block = matches!(
        &value_node.value,
        ayml_core::Value::Map(m) if !m.is_empty()
    ) || matches!(
        &value_node.value,
        ayml_core::Value::Seq(s) if !s.is_empty()
    );

    // Find where `key:` appears before the value span.
    // Search backwards from the value for the colon, then backwards from
    // the colon for the key text start.
    let before_value = &text[..value_node.span.start];
    let colon = before_value.rfind(':')?;
    // The key text is just before the colon (possibly with spaces between
    // key and colon, though AYML doesn't allow that in block context).
    // Search backwards from colon past any whitespace to find the key end,
    // then find the key start.
    let key_end = text[..colon]
        .bytes()
        .rposition(|b| b != b' ' && b != b'\t')
        .map(|i| i + 1)
        .unwrap_or(colon);
    // Key start: scan backwards from key_end past non-delimiter chars.
    // Stop at whitespace, `{`, `,`, `-`, or start of string.
    let key_start = text[..key_end]
        .bytes()
        .rposition(|b| {
            b == b' '
                || b == b'\t'
                || b == b'\n'
                || b == b'\r'
                || b == b'{'
                || b == b','
                || b == b'-'
        })
        .map(|i| i + 1)
        .unwrap_or(0);

    if is_block {
        // Highlight `key:` (including the colon).
        let start = offset_to_position(text, key_start);
        let end = offset_to_position(text, colon + 1);
        Some(Range::new(start, end))
    } else {
        // Scalar or empty collection: highlight the whole `key: value` pair.
        let start = offset_to_position(text, key_start);
        let end = offset_to_position(text, value_node.span.end);
        Some(Range::new(start, end))
    }
}

/// Check if the byte offset falls within a comment — either a full comment
/// line (first non-space is `#`) or an inline comment (after ` #` on a line).
fn is_in_comment(text: &str, offset: usize) -> bool {
    let line_start = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse input and compute hover range for the given path.
    fn hover_range_for(input: &str, path: &[&str]) -> Option<Range> {
        let node = ayml_core::parse(input).unwrap();
        let path: Vec<String> = path.iter().map(|s| s.to_string()).collect();
        compute_hover_range(&node, &path, input)
    }

    /// Extract the highlighted text from a Range.
    fn range_text(input: &str, range: Range) -> String {
        let start = position_to_offset(input, range.start);
        let end = position_to_offset(input, range.end);
        input[start..end].to_string()
    }

    // ── compute_hover_range: scalar values ──────────────────────

    #[test]
    fn scalar_value_highlights_whole_pair() {
        let input = "name: Alice";
        let range = hover_range_for(input, &["name"]).unwrap();
        assert_eq!(range_text(input, range), "name: Alice");
    }

    #[test]
    fn scalar_value_skips_indentation() {
        let input = "outer:\n  name: Alice";
        let range = hover_range_for(input, &["outer", "name"]).unwrap();
        assert_eq!(range_text(input, range), "name: Alice");
    }

    #[test]
    fn scalar_value_in_compact_mapping_skips_dash() {
        let input = "- host: *.github.com";
        let range = hover_range_for(input, &["0", "host"]).unwrap();
        assert_eq!(range_text(input, range), "host: *.github.com");
    }

    #[test]
    fn scalar_value_with_inline_comment_excludes_comment() {
        let input = "host: example.com # a comment";
        let range = hover_range_for(input, &["host"]).unwrap();
        assert_eq!(range_text(input, range), "host: example.com");
    }

    #[test]
    fn scalar_value_deeply_nested() {
        let input = "a:\n  b:\n    c: 42";
        let range = hover_range_for(input, &["a", "b", "c"]).unwrap();
        assert_eq!(range_text(input, range), "c: 42");
    }

    // ── compute_hover_range: block values ───────────────────────

    #[test]
    fn block_mapping_highlights_key_colon() {
        let input = "network:\n  rules: ok";
        let range = hover_range_for(input, &["network"]).unwrap();
        assert_eq!(range_text(input, range), "network:");
    }

    #[test]
    fn block_sequence_highlights_key_colon() {
        let input = "items:\n  - one\n  - two";
        let range = hover_range_for(input, &["items"]).unwrap();
        assert_eq!(range_text(input, range), "items:");
    }

    #[test]
    fn nested_block_highlights_key_colon() {
        let input = "a:\n  b:\n    c: 42";
        let range = hover_range_for(input, &["a", "b"]).unwrap();
        assert_eq!(range_text(input, range), "b:");
    }

    #[test]
    fn block_value_skips_indentation() {
        let input = "outer:\n  inner:\n    x: 1";
        let range = hover_range_for(input, &["outer", "inner"]).unwrap();
        assert_eq!(range_text(input, range), "inner:");
    }

    // ── compute_hover_range: sequence elements ────────────────────

    #[test]
    fn scalar_seq_element_value_only() {
        let input = "items:\n  - foo\n  - bar";
        let range = hover_range_for(input, &["items", "0"]).unwrap();
        assert_eq!(range_text(input, range), "foo");
    }

    #[test]
    fn scalar_seq_element_indented() {
        let input = "ports:\n    - 443\n    - 22";
        let range = hover_range_for(input, &["ports", "0"]).unwrap();
        assert_eq!(range_text(input, range), "443");
        let range = hover_range_for(input, &["ports", "1"]).unwrap();
        assert_eq!(range_text(input, range), "22");
    }

    #[test]
    fn scalar_seq_element_with_inline_comment() {
        let input = "- 22 # SSH\n- 443 # HTTPS";
        let range = hover_range_for(input, &["0"]).unwrap();
        assert_eq!(range_text(input, range), "22");
        let range = hover_range_for(input, &["1"]).unwrap();
        assert_eq!(range_text(input, range), "443");
    }

    // ── compute_hover_range: flow mapping elements ──────────────

    #[test]
    fn flow_mapping_scalar_value() {
        let input = "- {host: example.com, action: allow}";
        let range = hover_range_for(input, &["0", "action"]).unwrap();
        assert_eq!(range_text(input, range), "action: allow");
    }

    #[test]
    fn flow_mapping_first_entry() {
        let input = "- {host: example.com, action: allow}";
        let range = hover_range_for(input, &["0", "host"]).unwrap();
        assert_eq!(range_text(input, range), "host: example.com");
    }

    // ── is_in_comment ───────────────────────────────────────────

    #[test]
    fn full_comment_line() {
        let input = "  # this is a comment\nkey: val";
        // Every position on the comment line should be detected.
        for col in 0..21 {
            let offset = col;
            assert!(
                is_in_comment(input, offset),
                "offset {offset} should be in comment"
            );
        }
    }

    #[test]
    fn not_in_comment_on_key_value() {
        let input = "key: value";
        for offset in 0..input.len() {
            assert!(
                !is_in_comment(input, offset),
                "offset {offset} should not be in comment"
            );
        }
    }

    #[test]
    fn inline_comment_detected() {
        let input = "key: value # comment";
        // Offsets on "key: value" (0..10) are not in comment.
        for offset in 0..10 {
            assert!(
                !is_in_comment(input, offset),
                "offset {offset} should not be in comment"
            );
        }
        // The space before `#` (offset 10) is not in comment.
        assert!(!is_in_comment(input, 10));
        // The `#` itself (offset 11) and after are in comment.
        for offset in 11..input.len() {
            assert!(
                is_in_comment(input, offset),
                "offset {offset} should be in comment"
            );
        }
    }

    #[test]
    fn hash_in_quoted_string_not_comment() {
        let input = "key: \"foo#bar\"";
        // No position should be detected as comment.
        for offset in 0..input.len() {
            assert!(
                !is_in_comment(input, offset),
                "offset {offset} should not be in comment (hash is inside quotes)"
            );
        }
    }

    #[test]
    fn schema_directive_line_is_comment() {
        let input = "# language-server: $schema=https://example.com\nkey: val";
        assert!(is_in_comment(input, 0));
        assert!(is_in_comment(input, 10));
        // After the newline, on "key: val" line.
        let key_offset = input.find("key").unwrap();
        assert!(!is_in_comment(input, key_offset));
    }

    // ── position_to_offset / offset_to_position roundtrip ───────

    #[test]
    fn position_offset_roundtrip() {
        let input = "line0\nline1\nline2";
        for offset in 0..input.len() {
            let pos = offset_to_position(input, offset);
            let back = position_to_offset(input, pos);
            assert_eq!(
                back, offset,
                "roundtrip failed for offset {offset} -> pos ({}, {}) -> {back}",
                pos.line, pos.character
            );
        }
    }
}
