#![allow(clippy::mutable_key_type)] // Uri from lsp-types has interior mutability but is used as a map key by convention.

use ayml_core::{Node, Span, Value, directive::document_schema_uri};
use ayml_lsp::{convert, locate, schema};
use jsonschema::error::ValidationErrorKind;
use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, MarkupContent, MarkupKind, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    notification::Notification as _, request::Request as _,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

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
    let mut schemas = SchemaCache::new();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                if req.method == lsp_types::request::HoverRequest::METHOD {
                    let (id, params): (_, HoverParams) =
                        req.extract(lsp_types::request::HoverRequest::METHOD)?;
                    let hover = handle_hover(&params, &documents, &mut schemas);
                    let resp = Response::new_ok(id, hover);
                    connection.sender.send(Message::Response(resp))?;
                } else {
                    let resp = Response::new_err(
                        req.id,
                        lsp_server::ErrorCode::MethodNotFound as i32,
                        format!("unhandled method: {}", req.method),
                    );
                    connection.sender.send(Message::Response(resp))?;
                }
            }
            Message::Notification(not) => match not.method.as_str() {
                lsp_types::notification::DidOpenTextDocument::METHOD => {
                    let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
                    let doc = params.text_document;
                    publish_diagnostics(connection, doc.uri.clone(), &doc.text, &mut schemas)?;
                    documents.insert(doc.uri, doc.text);
                }
                lsp_types::notification::DidChangeTextDocument::METHOD => {
                    let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)?;
                    // With full sync, the last change holds the whole document.
                    if let Some(change) = params.content_changes.into_iter().last() {
                        let uri = params.text_document.uri;
                        publish_diagnostics(connection, uri.clone(), &change.text, &mut schemas)?;
                        documents.insert(uri, change.text);
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

/// A JSON Schema and its compiled validator.
struct Schema {
    json: serde_json::Value,
    validator: jsonschema::Validator,
}

/// Schemas by URL. Loading happens on the server's only thread, so fetches
/// time out, and a failure is remembered for a while rather than retried on
/// every keystroke.
struct SchemaCache {
    agent: ureq::Agent,
    entries: HashMap<String, (Instant, Result<Schema, String>)>,
}

impl SchemaCache {
    const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
    const RETRY_FAILURE_AFTER: Duration = Duration::from_secs(60);

    fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Self::FETCH_TIMEOUT))
            .build()
            .into();
        Self {
            agent,
            entries: HashMap::new(),
        }
    }

    fn get(&mut self, url: &str) -> Result<&Schema, &str> {
        let stale = self.entries.get(url).is_none_or(|(loaded, schema)| {
            schema.is_err() && loaded.elapsed() > Self::RETRY_FAILURE_AFTER
        });
        if stale {
            let schema = self.load(url);
            self.entries
                .insert(url.to_owned(), (Instant::now(), schema));
        }
        self.entries[url].1.as_ref().map_err(String::as_str)
    }

    fn load(&self, url: &str) -> Result<Schema, String> {
        let json = self
            .fetch(url)
            .map_err(|e| format!("failed to fetch schema: {e}"))?;
        let validator =
            jsonschema::validator_for(&json).map_err(|e| format!("invalid schema: {e}"))?;
        Ok(Schema { json, validator })
    }

    fn fetch(&self, url: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let text = if let Some(path) = url.strip_prefix("file://") {
            std::fs::read_to_string(path)?
        } else if url.starts_with("https://") || url.starts_with("http://") {
            self.agent.get(url).call()?.body_mut().read_to_string()?
        } else {
            return Err("only http, https, and file URLs are supported".into());
        };
        Ok(serde_json::from_str(&text)?)
    }
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    text: &str,
    schemas: &mut SchemaCache,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = match ayml_core::parse(text) {
        Err(e) => vec![Diagnostic {
            range: span_to_range(text, e.span),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("ayml".to_string()),
            message: format!("{e}"),
            ..Default::default()
        }],
        Ok(node) => match document_schema_uri(&node) {
            Some(url) => validate_with_schema(&node, text, url, schemas),
            None => Vec::new(),
        },
    };
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    send_notification::<lsp_types::notification::PublishDiagnostics>(connection, params)?;
    Ok(())
}

fn validate_with_schema(
    node: &Node,
    text: &str,
    schema_url: &str,
    schemas: &mut SchemaCache,
) -> Vec<Diagnostic> {
    let schema = match schemas.get(schema_url) {
        Ok(schema) => schema,
        Err(message) => {
            return vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("ayml".to_string()),
                message: message.to_owned(),
                ..Default::default()
            }];
        }
    };
    let json_value = convert::node_to_json(node);
    let mut diagnostics = Vec::new();
    for error in schema.validator.iter_errors(&json_value) {
        collect_leaf_errors(&error, node, text, &schema.json, &mut diagnostics);
    }
    diagnostics
}

/// Recursively collect the most specific (leaf) validation errors.
/// Composition errors (anyOf, oneOf) are expanded into their sub-errors
/// so diagnostics point to the actual invalid values rather than the
/// outer composition keyword.
fn collect_leaf_errors(
    error: &jsonschema::ValidationError<'_>,
    node: &Node,
    text: &str,
    schema_root: &serde_json::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
                let range = resolve_instance_path(node, path).map_or(
                    Range::new(Position::new(0, 0), Position::new(0, 0)),
                    |span| span_to_range(text, span),
                );

                // Try to build a helpful message from the schema.
                let path_segments: Vec<&str> = if path.is_empty() {
                    vec![]
                } else {
                    path.strip_prefix('/').unwrap_or(path).split('/').collect()
                };
                let message =
                    if let Some(sub) = schema::resolve_sub_schema(schema_root, &path_segments) {
                        let ty = schema::type_string(schema_root, sub)
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
    let range = resolve_instance_path(node, &path).map_or(
        Range::new(Position::new(0, 0), Position::new(0, 0)),
        |span| span_to_range(text, span),
    );
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
fn resolve_instance_path(node: &Node, path: &str) -> Option<Span> {
    if path.is_empty() {
        return Some(node.span);
    }

    let mut current = node;
    for segment in path.strip_prefix('/')?.split('/') {
        current = child(current, &unescape_pointer_segment(segment))?;
    }
    Some(current.span)
}

/// Decode a JSON pointer segment (RFC 6901): `~1` is `/` and `~0` is `~`.
fn unescape_pointer_segment(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

/// The child of a mapping (by key) or sequence (by index).
fn child<'a>(node: &'a Node, segment: &str) -> Option<&'a Node> {
    match &node.value {
        Value::Map(map) => map.get(segment),
        Value::Seq(items) => items.get(segment.parse::<usize>().ok()?),
        _ => None,
    }
}

/// Convert a byte-offset Span to an LSP Range using the source text.
fn span_to_range(text: &str, span: Span) -> Range {
    let start = offset_to_position(text, span.start);
    let end = offset_to_position(text, span.end);
    Range::new(start, end)
}

/// Convert a byte offset to a 0-based LSP Position. LSP columns count
/// UTF-16 code units; CR, LF, and CRLF each end a line.
fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let mut line = 0u32;
    let mut col = 0u32;
    let mut chars = text[..offset].chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                line += 1;
                col = 0;
            }
            '\n' => {
                line += 1;
                col = 0;
            }
            c => col += c.len_utf16() as u32,
        }
    }
    Position::new(line, col)
}

fn handle_hover(
    params: &HoverParams,
    documents: &HashMap<Uri, String>,
    schemas: &mut SchemaCache,
) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let text = documents.get(uri)?;
    let node = ayml_core::parse(text).ok()?;
    let schema = &schemas.get(document_schema_uri(&node)?).ok()?.json;

    // Map cursor position to byte offset, then to a path in the node tree.
    let pos = params.text_document_position_params.position;
    let offset = position_to_offset(text, pos);

    // Don't show hover on comment lines.
    let mut tokens = Vec::new();
    locate::token_spans(&node, &mut tokens);
    if is_in_comment(text, offset, &tokens) {
        return None;
    }

    let path_segments = locate::path_at_offset(&node, offset);
    let path_refs: Vec<&str> = path_segments.iter().map(String::as_str).collect();

    // Walk the schema to the sub-schema at that path.
    let sub_schema = schema::resolve_sub_schema(schema, &path_refs)?;
    let content = schema::hover_content(schema, sub_schema)?;

    let hover_range = compute_hover_range(&node, &path_segments, text);

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: hover_range,
    })
}

/// Convert a 0-based LSP Position (UTF-16 columns) to a byte offset in the
/// source text. Positions past the end of a line clamp to the line end; a
/// position inside a surrogate pair resolves to the start of its character.
fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if line == pos.line && (col >= pos.character || c == '\n' || c == '\r') {
            return i;
        }
        match c {
            '\r' => {
                if matches!(chars.peek(), Some((_, '\n'))) {
                    chars.next();
                }
                line += 1;
                col = 0;
            }
            '\n' => {
                line += 1;
                col = 0;
            }
            c => {
                let width = c.len_utf16() as u32;
                if line == pos.line && col + width > pos.character {
                    return i;
                }
                col += width;
            }
        }
    }
    text.len()
}

/// Compute the hover highlight range for the node at `path`.
///
/// - Scalar values: highlight the whole `key: value` pair.
/// - Block values (maps, non-empty seqs): highlight just `key:`.
/// - Non-mapping paths (e.g. sequence indices): highlight the value span.
fn compute_hover_range(root: &Node, path: &[String], text: &str) -> Option<Range> {
    let Some((last, parents)) = path.split_last() else {
        return Some(span_to_range(text, root.span));
    };
    let mut parent = root;
    for segment in parents {
        parent = child(parent, segment)?;
    }

    let map = match &parent.value {
        Value::Map(map) => map,
        Value::Seq(_) => return Some(span_to_range(text, child(parent, last)?.span)),
        _ => return None,
    };
    let (key, value) = map.get_key_value(last.as_str())?;

    let is_block = match &value.value {
        Value::Map(m) => !m.is_empty(),
        Value::Seq(s) => !s.is_empty(),
        _ => false,
    };
    let end = if is_block {
        // Only white space separates a block key from its `:`.
        key.span.end + text[key.span.end..].find(':')? + 1
    } else {
        value.span.end
    };
    Some(span_to_range(text, Span::new(key.span.start, end)))
}

/// True if the byte offset falls within a comment. `tokens` are the spans of
/// every key and scalar (see [`locate::token_spans`]); a `#` inside a token,
/// such as in a quoted or triple-quoted string, does not start a comment.
fn is_in_comment(text: &str, offset: usize, tokens: &[Span]) -> bool {
    // Tokens are in document order and do not overlap.
    let in_token = |i: usize| {
        let after = tokens.partition_point(|s| s.start <= i);
        after > 0 && i < tokens[after - 1].end
    };
    if in_token(offset) {
        return false;
    }
    let bytes = text.as_bytes();
    let line_start = text[..offset].rfind(['\n', '\r']).map_or(0, |i| i + 1);
    let line_end = text[line_start..]
        .find(['\n', '\r'])
        .map_or(text.len(), |i| line_start + i);
    let starts_comment = |i: usize| {
        bytes[i] == b'#'
            && !in_token(i)
            && (i == line_start || matches!(bytes[i - 1], b' ' | b'\t'))
    };
    // A full comment line: everything on it counts, including indentation.
    if let Some(first) = (line_start..line_end).find(|&i| !matches!(bytes[i], b' ' | b'\t'))
        && starts_comment(first)
    {
        return true;
    }
    (line_start..(offset + 1).min(line_end)).any(starts_comment)
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
        let path: Vec<String> = path.iter().map(ToString::to_string).collect();
        compute_hover_range(&node, &path, input)
    }

    /// Whether `offset` is in a comment of the (valid) document `input`.
    fn comment_at(input: &str, offset: usize) -> bool {
        let node = ayml_core::parse(input).unwrap();
        let mut tokens = Vec::new();
        locate::token_spans(&node, &mut tokens);
        is_in_comment(input, offset, &tokens)
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
                comment_at(input, offset),
                "offset {offset} should be in comment"
            );
        }
    }

    #[test]
    fn not_in_comment_on_key_value() {
        let input = "key: value";
        for offset in 0..input.len() {
            assert!(
                !comment_at(input, offset),
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
                !comment_at(input, offset),
                "offset {offset} should not be in comment"
            );
        }
        // The space before `#` (offset 10) is not in comment.
        assert!(!comment_at(input, 10));
        // The `#` itself (offset 11) and after are in comment.
        for offset in 11..input.len() {
            assert!(
                comment_at(input, offset),
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
                !comment_at(input, offset),
                "offset {offset} should not be in comment (hash is inside quotes)"
            );
        }
    }

    #[test]
    fn schema_directive_line_is_comment() {
        let input = "# language-server: $schema=https://example.com\nkey: val";
        assert!(comment_at(input, 0));
        assert!(comment_at(input, 10));
        // After the newline, on "key: val" line.
        let key_offset = input.find("key").unwrap();
        assert!(!comment_at(input, key_offset));
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

    #[test]
    fn escaped_quote_with_hash_not_comment() {
        // The # is inside a string with escaped quotes — not a comment.
        let input = "key: \"hello\\\"#world\"";
        let hash_offset = input.find('#').unwrap();
        assert!(
            !comment_at(input, hash_offset),
            "hash inside escaped-quote string should not be a comment"
        );
    }

    #[test]
    fn position_offset_roundtrip_crlf() {
        let input = "line0\r\nline1\r\nline2";
        let bytes = input.as_bytes();
        for offset in 0..input.len() {
            // Skip the \n byte inside \r\n pairs — it has no distinct
            // LSP position (the \r already advanced to the next line).
            if offset > 0 && bytes[offset] == b'\n' && bytes[offset - 1] == b'\r' {
                continue;
            }
            let pos = offset_to_position(input, offset);
            let back = position_to_offset(input, pos);
            assert_eq!(
                back, offset,
                "CRLF roundtrip failed for offset {offset} -> pos ({}, {}) -> {back}",
                pos.line, pos.character
            );
        }
    }

    #[test]
    fn crlf_offset_to_position_absolute() {
        // "AB\r\nCD"
        //  0  1  2 3  4 5
        // Line 0: A(0,0) B(0,1) \r(0,2) \n(still line 0)
        // Line 1: C(1,0) D(1,1)
        let input = "AB\r\nCD";
        assert_eq!(offset_to_position(input, 0), Position::new(0, 0)); // A
        assert_eq!(offset_to_position(input, 1), Position::new(0, 1)); // B
        assert_eq!(offset_to_position(input, 4), Position::new(1, 0)); // C
        assert_eq!(offset_to_position(input, 5), Position::new(1, 1)); // D
    }

    #[test]
    fn crlf_position_to_offset_absolute() {
        let input = "AB\r\nCD";
        assert_eq!(position_to_offset(input, Position::new(0, 0)), 0); // A
        assert_eq!(position_to_offset(input, Position::new(0, 1)), 1); // B
        assert_eq!(position_to_offset(input, Position::new(1, 0)), 4); // C
        assert_eq!(position_to_offset(input, Position::new(1, 1)), 5); // D
    }

    // ── key spans ───────────────────────────────────────────────

    #[test]
    fn quoted_key_highlights_quotes() {
        let input = "\"my key\": 1";
        let range = hover_range_for(input, &["my key"]).unwrap();
        assert_eq!(range_text(input, range), "\"my key\": 1");
    }

    #[test]
    fn spaces_before_colon_block_key() {
        let input = "city    :\n  name: x";
        let range = hover_range_for(input, &["city"]).unwrap();
        assert_eq!(range_text(input, range), "city    :");
    }

    #[test]
    fn key_containing_colon_and_dash() {
        let input = "outer:\n  a:b-c: 1";
        let range = hover_range_for(input, &["outer", "a:b-c"]).unwrap();
        assert_eq!(range_text(input, range), "a:b-c: 1");
    }

    #[test]
    fn pointer_segments_are_unescaped() {
        let input = "a/b:\n  c~d: 1";
        let node = ayml_core::parse(input).unwrap();
        let span = resolve_instance_path(&node, "/a~1b/c~0d").unwrap();
        assert_eq!(&input[span.start..span.end], "1");
    }

    // ── comments ────────────────────────────────────────────────

    #[test]
    fn hash_in_triple_quoted_string_not_comment() {
        let input = "code: \"\"\"\n  # not a comment\n  \"\"\"\n";
        let hash = input.find('#').unwrap();
        assert!(!comment_at(input, hash));
        assert!(!comment_at(input, hash + 3));
    }

    #[test]
    fn hash_inside_bare_string_impossible_but_flow_comment_detected() {
        let input = "a: [1, # one\n  2]";
        let hash = input.find('#').unwrap();
        assert!(comment_at(input, hash + 2));
        assert!(!comment_at(input, input.find('2').unwrap()));
    }

    // ── UTF-16 positions ────────────────────────────────────────

    #[test]
    fn positions_count_utf16_code_units() {
        let input = "é😀x: 1";
        let x = input.find('x').unwrap();
        // é is 1 UTF-16 unit, 😀 is 2.
        assert_eq!(offset_to_position(input, x), Position::new(0, 3));
        assert_eq!(position_to_offset(input, Position::new(0, 3)), x);
        // A position inside the surrogate pair resolves to the emoji's start.
        assert_eq!(position_to_offset(input, Position::new(0, 2)), "é".len());
    }

    #[test]
    fn position_past_line_end_clamps() {
        let input = "ab\ncd";
        assert_eq!(position_to_offset(input, Position::new(0, 10)), 2);
        assert_eq!(position_to_offset(input, Position::new(5, 0)), input.len());
    }

    #[test]
    fn position_offset_roundtrip_unicode() {
        let input = "ключ: значение\n- 😀\r\n";
        for (offset, _) in input.char_indices() {
            if offset > 0
                && input.as_bytes()[offset] == b'\n'
                && input.as_bytes()[offset - 1] == b'\r'
            {
                continue;
            }
            let pos = offset_to_position(input, offset);
            assert_eq!(position_to_offset(input, pos), offset, "offset {offset}");
        }
    }

    #[test]
    fn hover_range_with_non_ascii_prefix() {
        let input = "ключ:\n  имя: Алиса";
        let range = hover_range_for(input, &["ключ", "имя"]).unwrap();
        assert_eq!(range.start, Position::new(1, 2));
        assert_eq!(range_text(input, range), "имя: Алиса");
    }
}
