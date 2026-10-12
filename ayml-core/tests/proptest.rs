//! Property tests for the AYML parser and emitter.
//!
//! The central properties:
//!
//! * **Parser oracle** — documents rendered in randomized style from a tree
//!   (see `ayml_testgen::render`) parse back to exactly that tree, comments
//!   included.
//! * **Emitter round trip** — `parse(emit(tree))` equals the documented
//!   canonical form of the tree, comments included, and emitting is
//!   idempotent.
//! * **Scalars** — every string, integer, and float survives a round trip in
//!   every position, and number syntax is resolved exactly as specified.
//! * **Robustness** — arbitrary and mutated input never panics, errors point
//!   inside the input, and anything accepted round-trips.
//! * **JSON** — JSON written by `serde_json` parses to the same data.

use std::fmt::Write as _;

use ayml_core::scalar::{self, Context};
use ayml_core::{ErrorKind, Key, Location, Mapping, Node, Span, Value, emit, parse};
use ayml_testgen::config;
use ayml_testgen::mutate::arb_mutated_doc;
use ayml_testgen::render::{arb_doc, render};
use ayml_testgen::strings::{arb_char, arb_f64, arb_i64, arb_key, arb_string, is_comment_char};
use ayml_testgen::tree::{arb_node, arb_plain_node, canonicalize, diff, diff_values_only};
use proptest::prelude::*;

fn parse_ok(text: &str) -> Result<Node, TestCaseError> {
    parse(text)
        .map_err(|e| TestCaseError::fail(format!("parse failed: {e}\n--- input ---\n{text}\n---")))
}

fn check_diff(expected: &Node, actual: &Node, text: &str) -> Result<(), TestCaseError> {
    match diff(expected, actual) {
        None => Ok(()),
        Some(d) => Err(TestCaseError::fail(format!(
            "{d}\n--- text ---\n{text}\n---"
        ))),
    }
}

/// Check span invariants: in bounds, nested, keys before values, and scalar
/// span text that re-parses to the same scalar.
fn check_spans(node: &Node, text: &str, parent: Option<Span>) -> Result<(), TestCaseError> {
    let span = node.span;
    prop_assert!(
        span.start <= span.end && span.end <= text.len(),
        "bad span {span:?}"
    );
    prop_assert!(text.is_char_boundary(span.start) && text.is_char_boundary(span.end));
    if let Some(p) = parent {
        prop_assert!(
            p.start <= span.start && span.end <= p.end,
            "span {span:?} not inside parent {p:?}"
        );
    }
    match &node.value {
        Value::Seq(items) => {
            for item in items {
                check_spans(item, text, Some(span))?;
            }
        }
        Value::Map(map) => {
            for (key, value) in map {
                let ks = key.span;
                prop_assert!(span.start <= ks.start && ks.end <= value.span.start);
                let key_text = &text[ks.start..ks.end];
                if key_text.starts_with('"') {
                    let reparsed = parse_ok(key_text)?;
                    prop_assert_eq!(reparsed.value.as_str(), Some(key.as_str()));
                } else {
                    prop_assert_eq!(key_text, key.as_str());
                }
                check_spans(value, text, Some(span))?;
            }
        }
        scalar_value => {
            // Re-parse inside a sequence entry: a scalar in the middle of a
            // document may start with U+FEFF, which cannot start a document.
            let scalar_text = &text[span.start..span.end];
            let entry = parse_ok(&format!("- {scalar_text}"))?;
            let reparsed = entry.value.as_sequence().unwrap()[0].clone();
            let expected = Node::new(scalar_value.clone());
            if let Some(d) = diff_values_only(&expected, &reparsed) {
                return Err(TestCaseError::fail(format!(
                    "scalar span {scalar_text:?}: {d}"
                )));
            }
        }
    }
    Ok(())
}

/// A string in every position: root, mapping value, sequence entry, key.
fn string_positions(s: &str) -> Vec<Node> {
    let leaf = || Node::new(Value::Str(s.to_owned()));
    let mut map = Mapping::new();
    map.insert(Key::new("k"), leaf());
    let mut keyed = Mapping::new();
    keyed.insert(Key::new(s), Node::new(Value::Int(1)));
    let mut nested = Mapping::new();
    nested.insert(Key::new("outer"), Node::new(Value::Map(map.clone())));
    vec![
        leaf(),
        Node::new(Value::Map(map)),
        Node::new(Value::Seq(vec![leaf(), leaf()])),
        Node::new(Value::Map(keyed)),
        Node::new(Value::Map(nested)),
        Node::new(Value::Seq(vec![Node::new(Value::Seq(vec![leaf()]))])),
    ]
}

proptest! {
    #![proptest_config(config(2000))]

    // ── Parser oracle ───────────────────────────────────────────────

    /// Documents in randomized style parse to exactly the tree they were
    /// rendered from, including every comment.
    #[test]
    fn rendered_documents_parse_to_their_tree(doc in arb_doc()) {
        let parsed = parse_ok(&doc.text)?;
        check_diff(&doc.expected, &parsed, &doc.text)?;
    }

    /// Spans of parsed documents are well-formed.
    #[test]
    fn rendered_document_spans_are_valid(doc in arb_doc()) {
        let parsed = parse_ok(&doc.text)?;
        check_spans(&parsed, &doc.text, None)?;
    }

    /// Re-emitting a parsed document preserves the tree exactly.
    #[test]
    fn rendered_documents_survive_reformatting(doc in arb_doc()) {
        let parsed = parse_ok(&doc.text)?;
        let emitted = emit(&parsed);
        let reparsed = parse_ok(&emitted)?;
        check_diff(&parsed, &reparsed, &emitted)?;
    }

    // ── Emitter round trip ──────────────────────────────────────────

    /// Emitting any tree and parsing it back yields the tree's canonical
    /// form: same values, same comments, moved only as documented.
    #[test]
    fn emit_parse_round_trip(node in arb_node()) {
        let emitted = emit(&node);
        let parsed = parse_ok(&emitted)?;
        check_diff(&canonicalize(&node), &parsed, &emitted)?;
    }

    /// Emitting is idempotent: the text of a re-parsed document is the text
    /// of the canonical tree.
    #[test]
    fn emit_is_idempotent(node in arb_node()) {
        let once = emit(&canonicalize(&node));
        prop_assert_eq!(&emit(&parse_ok(&emit(&node))?), &once);
        prop_assert_eq!(&emit(&parse_ok(&once)?), &once);
    }

    /// Canonicalization is idempotent and is a fixed point of the emitter.
    #[test]
    fn canonical_trees_round_trip_exactly(node in arb_node()) {
        let canonical = canonicalize(&node);
        prop_assert!(diff(&canonical, &canonicalize(&canonical)).is_none());
        let emitted = emit(&canonical);
        check_diff(&canonical, &parse_ok(&emitted)?, &emitted)?;
    }

    /// The same canonical tree rendered with two different styles parses to
    /// the same tree.
    #[test]
    fn rendering_style_does_not_matter(node in arb_node(), a in any::<u64>(), b in any::<u64>()) {
        let canonical = canonicalize(&node);
        let first = parse_ok(&render(&canonical, a))?;
        let second = parse_ok(&render(&canonical, b))?;
        prop_assert!(diff(&first, &second).is_none());
    }

    // ── Scalars ─────────────────────────────────────────────────────

    /// Any string round-trips in every position.
    #[test]
    fn strings_round_trip_everywhere(s in arb_string()) {
        for node in string_positions(&s) {
            let emitted = emit(&node);
            let parsed = parse_ok(&emitted)?;
            check_diff(&node, &parsed, &emitted)?;
        }
    }

    /// Strings made of arbitrary characters round-trip.
    #[test]
    fn arbitrary_char_strings_round_trip(chars in prop::collection::vec(any::<char>(), 0..12)) {
        let s: String = chars.into_iter().collect();
        for node in string_positions(&s) {
            let emitted = emit(&node);
            check_diff(&node, &parse_ok(&emitted)?, &emitted)?;
        }
    }

    /// The emitter writes a string bare only when the parser reads that
    /// exact text back as the same string, and a bare string never has
    /// invisible edges.
    #[test]
    fn bare_strings_are_unambiguous(s in arb_string()) {
        if scalar::is_bare_value(&s, Context::Block) {
            let parsed = parse_ok(&s)?;
            prop_assert_eq!(parsed.value, Value::Str(s.clone()));
            prop_assert!(!s.starts_with(char::is_whitespace) && !s.ends_with(char::is_whitespace));
        }
        if scalar::is_bare_value(&s, Context::Flow) {
            let parsed = parse_ok(&format!("[{s}]"))?;
            prop_assert_eq!(parsed.value, Value::Seq(vec![Node::new(Value::Str(s.clone()))]));
        }
        if scalar::is_bare_key(&s, Context::Block) {
            let parsed = parse_ok(&format!("{s}: 1"))?;
            prop_assert!(parsed.get(&s).is_some());
        }
    }

    /// Any key round-trips; quoted and bare spellings of a key collide.
    #[test]
    fn keys_round_trip_and_duplicates_are_rejected(k in arb_key()) {
        let mut map = Mapping::new();
        map.insert(Key::new(k.clone()), Node::new(Value::Null));
        let node = Node::new(Value::Map(map));
        let emitted = emit(&node);
        check_diff(&node, &parse_ok(&emitted)?, &emitted)?;

        let line = emitted.trim_end();
        let quoted_line = {
            let mut q = String::new();
            q.push('"');
            for c in k.chars() {
                match c {
                    '"' => q.push_str("\\\""),
                    '\\' => q.push_str("\\\\"),
                    c => {
                        let _ = write!(q, "\\U{:08X}", c as u32);
                    }
                }
            }
            q.push_str("\": null");
            q
        };
        let duplicate = format!("{line}\n{quoted_line}\n");
        let err = parse(&duplicate).expect_err(&duplicate);
        prop_assert_eq!(err.kind, ErrorKind::DuplicateKey(k.clone()));
        // The error points at the second spelling of the key.
        prop_assert_eq!(err.span.start, line.len() + 1);
        prop_assert_eq!(err.location.line, line.matches(['\n', '\r']).count() + 2);
    }

    /// Every `i64` parses back from every notation the spec allows.
    #[test]
    fn integers_in_any_notation(i in arb_i64(), plus in any::<bool>()) {
        let m = i.unsigned_abs();
        let sign = if i < 0 { "-" } else if plus { "+" } else { "" };
        for digits in [m.to_string(), format!("0x{m:x}"), format!("0x{m:X}"), format!("0o{m:o}"), format!("0b{m:b}")] {
            let text = format!("{sign}{digits}");
            prop_assert_eq!(parse_ok(&text)?.value, Value::Int(i), "{}", text);
        }
    }

    /// Integers outside the 64-bit signed range are errors in every notation.
    #[test]
    fn integer_overflow_is_an_error(extra in 1u128..=u128::from(u64::MAX), negative in any::<bool>()) {
        let magnitude: u128 = if negative { (1u128 << 63) + extra } else { (1u128 << 63) - 1 + extra };
        let sign = if negative { "-" } else { "" };
        for digits in [magnitude.to_string(), format!("0x{magnitude:x}"), format!("0o{magnitude:o}"), format!("0b{magnitude:b}")] {
            let text = format!("{sign}{digits}");
            let err = parse(&text).expect_err(&text);
            prop_assert_eq!(err.kind, ErrorKind::IntegerOverflow);
        }
    }

    /// Decimal numbers with leading zeros are strings, not numbers.
    #[test]
    fn leading_zeros_make_strings(digits in "0[0-9]{1,8}", frac in proptest::option::of("[0-9]{1,3}")) {
        let text = match frac { Some(f) => format!("{digits}.{f}"), None => digits };
        prop_assert_eq!(parse_ok(&text)?.value, Value::Str(text.clone()));
    }

    /// Every `f64` survives a round trip bit for bit (any NaN reads as NaN).
    #[test]
    fn floats_round_trip(f in arb_f64()) {
        let node = Node::new(Value::Float(f));
        let emitted = emit(&node);
        check_diff(&node, &parse_ok(&emitted)?, &emitted)?;
        let mut text = String::new();
        scalar::write_float(&mut text, f);
        prop_assert!(scalar::parse_float(&text).is_some(), "{} is not AYML float syntax", text);
    }

    /// Float literals are correctly rounded, and only overflow is an error.
    #[test]
    fn float_literals_resolve_like_ieee(
        sign in "[-+]?",
        int in "0|[1-9][0-9]{0,20}",
        frac in proptest::option::of("[0-9]{1,20}"),
        exp in proptest::option::of(("[eE]", "[-+]?", 0u32..400)),
    ) {
        if frac.is_none() && exp.is_none() {
            return Ok(());
        }
        let mut text = format!("{sign}{int}");
        if let Some(f) = &frac { text.push('.'); text.push_str(f); }
        if let Some((e, s, x)) = &exp {
            let _ = write!(text, "{e}{s}{x}");
        }
        let expected: f64 = text.parse().unwrap();
        match parse(&text) {
            Ok(node) => {
                prop_assert!(expected.is_finite());
                prop_assert_eq!(node.value.as_f64().map(f64::to_bits), Some(expected.to_bits()));
            }
            Err(e) => {
                prop_assert!(expected.is_infinite(), "{} failed: {}", text, e);
                prop_assert_eq!(e.kind, ErrorKind::FloatOverflow);
            }
        }
    }

    /// Unicode escapes decode to their character; surrogates and values
    /// beyond U+10FFFF are errors.
    #[test]
    fn unicode_escapes(cp in 0u32..0x11_0000) {
        let text = format!("\"\\U{cp:08X}\"");
        match char::from_u32(cp) {
            Some(c) => prop_assert_eq!(parse_ok(&text)?.value, Value::Str(c.to_string())),
            None => prop_assert!(matches!(parse(&text).map_err(|e| e.kind), Err(ErrorKind::InvalidEscape(_)))),
        }
        if cp <= 0xFFFF {
            let short = format!("\"\\u{cp:04x}\"");
            prop_assert_eq!(parse(&short).ok().map(|n| n.value), char::from_u32(cp).map(|c| Value::Str(c.to_string())));
        }
    }

    // ── Comments ────────────────────────────────────────────────────

    /// Arbitrary comment text is preserved up to the documented
    /// normalization: line breaks split lines, trailing spaces and tabs are
    /// trimmed, and characters that cannot appear in comments become U+FFFD.
    #[test]
    fn comment_text_is_normalized(text in prop::collection::vec(arb_char(), 0..30).prop_map(|v| v.into_iter().collect::<String>())) {
        let normalize_line = |line: &str| -> String {
            line.chars()
                .map(|c| if is_comment_char(c) { c } else { '\u{FFFD}' })
                .collect::<String>()
                .trim_end_matches([' ', '\t'])
                .to_owned()
        };
        let lines: Vec<String> = text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(normalize_line).collect();
        let mut map = Mapping::new();
        map.insert(Key::new("a"), Node::new(Value::Int(1)).with_inline_comment(text.clone()));
        map.insert(Key::new("b"), Node::new(Value::Int(2)).with_comment(text.clone()));
        let node = Node::new(Value::Map(map));
        let parsed = parse_ok(&emit(&node))?;
        prop_assert_eq!(parsed.get("b").unwrap().comment.clone(), Some(lines.join("\n")));
        let inline = normalize_line(&text.replace("\r\n", " ").replace(['\n', '\r'], " "));
        prop_assert_eq!(parsed.get("a").unwrap().inline_comment.clone(), Some(inline));
    }

    // ── Robustness ──────────────────────────────────────────────────

    /// Mutated documents never panic. Errors point inside the input, and
    /// whatever is accepted survives re-emitting.
    #[test]
    fn mutated_documents(text in arb_mutated_doc()) {
        match parse(&text) {
            Ok(node) => {
                check_spans(&node, &text, None)?;
                let emitted = emit(&node);
                check_diff(&canonicalize(&node), &parse_ok(&emitted)?, &emitted)?;
            }
            Err(e) => {
                prop_assert!(e.span.start <= e.span.end && e.span.end <= text.len());
                prop_assert_eq!(e.location, Location::of(&text, e.span.start));
            }
        }
    }

    /// Arbitrary short inputs never panic.
    #[test]
    fn arbitrary_input(text in prop::collection::vec(arb_char(), 0..40).prop_map(|v| v.into_iter().collect::<String>())) {
        if let Ok(node) = parse(&text) {
            let emitted = emit(&node);
            check_diff(&canonicalize(&node), &parse_ok(&emitted)?, &emitted)?;
        }
    }

    /// Every character of a document is validated: the first character
    /// outside `c-printable` is reported, wherever it falls relative to the
    /// reader's eight-byte fast path.
    #[test]
    fn validation_matches_a_naive_check(
        chars in prop::collection::vec(
            prop_oneof![
                8 => prop::char::range(' ', '~'),
                2 => prop::sample::select(vec!['\t', '\u{0}', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{1b}', '\u{1f}', '\u{7f}', '\u{80}', '\u{85}', '\u{9f}', '\u{a0}', '\u{fffe}', '\u{ffff}', 'é', '😀']),
            ],
            0..64,
        ),
        newlines in prop::collection::vec(any::<prop::sample::Index>(), 0..4),
    ) {
        let mut text: String = chars.into_iter().collect();
        for at in newlines {
            let boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
            let at = boundaries[at.index(boundaries.len())];
            text.insert_str(at, "\n# ");
        }
        let doc = format!("# {text}\nx: 1\n");
        let first_bad = doc.char_indices().find(|&(_, c)| !scalar::is_printable(c));
        match (ayml_core::parse(&doc), first_bad) {
            (Ok(_), None) => {}
            (Err(e), Some((at, c))) => {
                prop_assert_eq!(e.kind, ErrorKind::NonPrintable(c));
                prop_assert_eq!(e.span.start, at);
            }
            (result, expected) => {
                return Err(TestCaseError::fail(format!("{result:?} but first non-printable is {expected:?} in {doc:?}")));
            }
        }
    }

    /// Locations count lines (CR, LF, CRLF) and characters.
    #[test]
    fn locations_count_lines_and_chars(text in "[a\\r\\né\n]{0,30}", at in any::<prop::sample::Index>()) {
        let boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
        let offset = boundaries[at.index(boundaries.len())];
        let before = &text[..offset];
        let normalized = before.replace("\r\n", "\n").replace('\r', "\n");
        let line = normalized.matches('\n').count() + 1;
        let column = normalized.rsplit('\n').next().unwrap().chars().count() + 1;
        prop_assert_eq!(Location::of(&text, offset), Location { line, column });
    }

    // ── Depth limit ─────────────────────────────────────────────────

    /// Nesting deeper than the limit is an error, never a stack overflow.
    #[test]
    fn depth_limit(depth in 1usize..300, style in 0u8..4) {
        let text = match style {
            0 => format!("{}1{}", "[".repeat(depth), "]".repeat(depth)),
            1 => format!("{}1{}", "{a: ".repeat(depth), "}".repeat(depth)),
            2 => {
                let mut text = String::new();
                for i in 0..depth {
                    let _ = writeln!(text, "{}k:", " ".repeat(i));
                }
                text + &" ".repeat(depth) + "k: 1"
            }
            _ => format!("{}x", "- ".repeat(depth)),
        };
        let levels = if style == 2 { depth + 1 } else { depth };
        match ayml_core::parse_with_max_depth(&text, 128) {
            Ok(_) => prop_assert!(levels <= 128),
            Err(e) => {
                prop_assert!(levels > 128, "{}", e);
                prop_assert_eq!(e.kind, ErrorKind::RecursionLimit);
            }
        }
    }

    // ── JSON ────────────────────────────────────────────────────────

    /// JSON written by `serde_json` (with integers in range and without the
    /// literal control characters it does not escape) is an AYML document
    /// with the same data.
    #[test]
    fn json_is_ayml(json in arb_json(), pretty in any::<bool>()) {
        let text = if pretty { serde_json::to_string_pretty(&json) } else { serde_json::to_string(&json) }.unwrap();
        let parsed = parse_ok(&text)?;
        if let Some(d) = diff_values_only(&json_to_node(&json), &parsed) {
            return Err(TestCaseError::fail(format!("{d}\n--- json ---\n{text}")));
        }
    }

    /// Value-only trees round-trip exactly through the emitter.
    #[test]
    fn plain_trees_round_trip(node in arb_plain_node()) {
        let emitted = emit(&node);
        check_diff(&node, &parse_ok(&emitted)?, &emitted)?;
    }
}

fn arb_json() -> impl Strategy<Value = serde_json::Value> {
    use serde_json::Value as J;
    // JSON writes DEL, C1 controls, and noncharacters literally, and AYML
    // does not allow them literally; everything else JSON escapes.
    let json_string = arb_string().prop_map(|s| {
        s.chars()
            .filter(|&c| c < ' ' || scalar::is_printable(c))
            .collect::<String>()
    });
    let leaf = prop_oneof![
        Just(J::Null),
        any::<bool>().prop_map(J::Bool),
        arb_i64().prop_map(J::from),
        arb_f64()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(J::from),
        json_string.clone().prop_map(J::String),
    ];
    leaf.prop_recursive(4, 32, 5, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(J::Array),
            prop::collection::vec((json_string.clone(), inner), 0..5)
                .prop_map(|entries| J::Object(entries.into_iter().collect())),
        ]
    })
}

fn json_to_node(json: &serde_json::Value) -> Node {
    use serde_json::Value as J;
    Node::new(match json {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => n
            .as_i64()
            .map_or_else(|| Value::Float(n.as_f64().unwrap()), Value::Int),
        J::String(s) => Value::Str(s.clone()),
        J::Array(items) => Value::Seq(items.iter().map(json_to_node).collect()),
        J::Object(map) => Value::Map(
            map.iter()
                .map(|(k, v)| (Key::new(k.clone()), json_to_node(v)))
                .collect(),
        ),
    })
}
