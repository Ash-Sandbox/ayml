use ayml_core::{
    error::Span,
    value::{MapKey, Node, Value},
};
use indexmap::IndexMap;
use proptest::prelude::*;

/// Generate a random MapKey.
fn arb_map_key() -> impl Strategy<Value = MapKey> {
    prop_oneof![
        any::<bool>().prop_map(MapKey::Bool),
        prop_oneof![any::<i64>(), Just(i64::MIN), Just(i64::MAX), Just(0_i64),]
            .prop_map(MapKey::Int),
        prop_oneof![
            "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
            // Keys that need quoting
            Just(String::new()),
            Just("null".into()),
            Just("true".into()),
            Just("false".into()),
            Just("42".into()),
            Just("with: colon".into()),
            Just("has #hash".into()),
            Just("back\\slash".into()),
        ]
        .prop_map(MapKey::String),
    ]
}

/// Generate a random string that is safe for AYML bare or quoted scalars.
fn arb_scalar_string() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple alphanumeric
        "[a-zA-Z][a-zA-Z0-9 _-]{0,30}",
        // Strings containing special chars that force quoting
        "[a-zA-Z0-9 :{}\\[\\]#,]{1,20}",
        // Empty string (needs quoting)
        Just(String::new()),
        // Reserved words (must round-trip as strings, not parsed values)
        Just("null".into()),
        Just("true".into()),
        Just("false".into()),
        Just("inf".into()),
        Just("+inf".into()),
        Just("-inf".into()),
        Just("nan".into()),
        // Strings that look like numbers
        Just("42".into()),
        Just("3.14".into()),
        Just("-7".into()),
        // Strings with control characters
        Just("tab\there".into()),
        Just("cr\rhere".into()),
        // Multiline strings (exercises triple-quoting)
        Just("line one\nline two".into()),
        Just("trailing newline\n".into()),
        Just("three\nlines\nhere".into()),
        // Trailing newline edge cases
        Just("end\n\n".into()),
        Just("\n".into()),
        Just("a\nb\n".into()),
        // Leading/trailing whitespace
        Just(" leading".into()),
        Just("trailing ".into()),
        Just(" both ".into()),
        // Dash-space prefix (looks like sequence indicator)
        Just("- item".into()),
        Just("- ".into()),
    ]
}

/// Generate a random Value tree with bounded depth.
fn arb_value(depth: u32) -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        prop_oneof![
            any::<i64>(),
            Just(i64::MIN),
            Just(i64::MAX),
            Just(i64::MIN + 1),
            Just(i64::MAX - 1),
            Just(0_i64),
        ]
        .prop_map(Value::Int),
        // Use finite, non-subnormal floats that round-trip cleanly
        (-1e15_f64..1e15_f64)
            .prop_filter("finite and normal", |f| f.is_finite())
            .prop_map(|f| Value::Float((f * 1e6).round() / 1e6)),
        prop_oneof![
            Just(Value::Float(f64::INFINITY)),
            Just(Value::Float(f64::NEG_INFINITY)),
            Just(Value::Float(f64::NAN)),
            Just(Value::Float(0.0)),
            Just(Value::Float(-0.0)),
            Just(Value::Float(1.0)),
            Just(Value::Float(-1.0)),
            Just(Value::Float(1e20)),
            Just(Value::Float(-1e20)),
            Just(Value::Float(1.5e-10)),
            Just(Value::Float(-1.5e-10)),
            Just(Value::Float(f64::MAX)),
            Just(Value::Float(f64::MIN)),
            Just(Value::Float(f64::MIN_POSITIVE)),
        ],
        arb_scalar_string().prop_map(Value::Str),
    ];

    if depth == 0 {
        leaf.boxed()
    } else {
        prop_oneof![
            4 => leaf,
            // Sequences of 0..5 elements
            1 => prop::collection::vec(arb_node(depth - 1), 0..5)
                .prop_map(Value::Seq),
            // Mappings of 0..5 entries
            1 => prop::collection::vec(
                    (arb_map_key(), arb_node(depth - 1)),
                    0..5
                )
                .prop_map(|entries| {
                    let map: IndexMap<MapKey, Node> = entries.into_iter().collect();
                    Value::Map(map)
                }),
        ]
        .boxed()
    }
}

/// Generate a random Node (no comments — comments don't affect value round-tripping).
fn arb_node(depth: u32) -> BoxedStrategy<Node> {
    arb_value(depth).prop_map(Node::new).boxed()
}

/// Generate a random comment string. May contain blank lines between
/// comment lines to exercise blank-line preservation.
fn arb_comment() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        3 => Just(None),
        2 => "[a-zA-Z0-9 ]{1,30}".prop_map(|s| Some(s)),
        // Two lines, no blank line between
        1 => ("[a-zA-Z0-9 ]{1,20}", "[a-zA-Z0-9 ]{1,20}")
            .prop_map(|(a, b)| Some(format!("{a}\n{b}"))),
        // Two lines with a blank line between
        1 => ("[a-zA-Z0-9 ]{1,20}", "[a-zA-Z0-9 ]{1,20}")
            .prop_map(|(a, b)| Some(format!("{a}\n\n{b}"))),
        // Three lines with blank lines
        1 => ("[a-zA-Z0-9 ]{1,15}", "[a-zA-Z0-9 ]{1,15}", "[a-zA-Z0-9 ]{1,15}")
            .prop_map(|(a, b, c)| Some(format!("{a}\n\n{b}\n\n{c}"))),
    ]
}

/// Generate a random Node with comments attached.
fn arb_node_with_comments(depth: u32) -> BoxedStrategy<Node> {
    (arb_value_with_comments(depth), arb_comment())
        .prop_map(|(value, comment)| {
            let mut node = Node::new(value);
            node.comment = comment;
            node
        })
        .boxed()
}

/// Generate a random Value tree with comments on children.
fn arb_value_with_comments(depth: u32) -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::Int),
        (-1e15_f64..1e15_f64)
            .prop_filter("finite and normal", |f| f.is_finite())
            .prop_map(|f| Value::Float((f * 1e6).round() / 1e6)),
        arb_scalar_string().prop_map(Value::Str),
    ];

    if depth == 0 {
        leaf.boxed()
    } else {
        prop_oneof![
            4 => leaf,
            1 => prop::collection::vec(arb_node_with_comments(depth - 1), 0..4)
                .prop_map(Value::Seq),
            1 => prop::collection::vec(
                    (arb_map_key(), arb_node_with_comments(depth - 1)),
                    0..4
                )
                .prop_map(|entries| {
                    let map: IndexMap<MapKey, Node> = entries.into_iter().collect();
                    Value::Map(map)
                }),
        ]
        .boxed()
    }
}

/// Compare two Values, treating NaN == NaN.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => (a.is_nan() && b.is_nan()) || a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Seq(a), Value::Seq(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| values_equal(&a.value, &b.value))
        }
        (Value::Map(a), Value::Map(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|bv| values_equal(&v.value, &bv.value)))
        }
        _ => false,
    }
}

/// Recursively verify span invariants on a parsed node tree.
fn check_span_invariants(
    node: &Node,
    input: &str,
    parent_span: Option<Span>,
) -> Result<(), TestCaseError> {
    let span = node.span;

    // start <= end
    prop_assert!(
        span.start <= span.end,
        "span start ({}) > end ({})",
        span.start,
        span.end
    );

    // within input bounds
    prop_assert!(
        span.end <= input.len(),
        "span end ({}) > input len ({})",
        span.end,
        input.len()
    );

    // child within parent (if provided)
    if let Some(parent) = parent_span {
        prop_assert!(
            span.start >= parent.start && span.end <= parent.end,
            "child span {}..{} not within parent span {}..{}",
            span.start,
            span.end,
            parent.start,
            parent.end,
        );
    }

    // For scalars, verify the span text re-parses to the same value.
    if node.value.is_scalar() {
        let span_text = &input[span.start..span.end];
        if let Ok(reparsed) = ayml_core::parse(span_text) {
            prop_assert!(
                values_equal(&node.value, &reparsed.value),
                "scalar span text {:?} re-parsed to {:?}, expected {:?}",
                span_text,
                reparsed.value,
                node.value,
            );
        }
    }

    // Recurse into children
    match &node.value {
        Value::Seq(items) => {
            for item in items {
                check_span_invariants(item, input, Some(span))?;
            }
        }
        Value::Map(map) => {
            for (_, value_node) in map {
                check_span_invariants(value_node, input, Some(span))?;
            }
        }
        _ => {}
    }

    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// Emit a random Value tree, parse it back, and verify semantic equality.
    #[test]
    fn round_trip_random_values(node in arb_node(4)) {
        let emitted = ayml_core::emit(&node);

        let parsed = ayml_core::parse(&emitted).map_err(|e| {
            TestCaseError::fail(format!(
                "parse failed: {e}\n--- emitted ---\n{emitted}---"
            ))
        })?;

        prop_assert!(
            values_equal(&node.value, &parsed.value),
            "values differ\n--- original ---\n{:?}\n--- emitted ---\n{}\n--- parsed ---\n{:?}",
            node.value,
            emitted,
            parsed.value,
        );
    }

    /// Ensure the parser never panics on arbitrary input.
    #[test]
    fn parser_no_panic(input in "[\\s\\S]{0,200}") {
        // We don't care about the result — just that it doesn't panic.
        let _ = ayml_core::parse(&input);
    }

    /// All parsed spans must be valid: start <= end, within input bounds,
    /// and child spans nested within parent spans.
    #[test]
    fn spans_valid_after_round_trip(node in arb_node(4)) {
        let emitted = ayml_core::emit(&node);
        let Ok(parsed) = ayml_core::parse(&emitted) else {
            return Ok(());
        };

        check_span_invariants(&parsed, &emitted, None)?;
    }

    /// Spans are valid on arbitrary (possibly invalid) input.
    #[test]
    fn spans_valid_on_arbitrary_input(input in "[\\s\\S]{0,200}") {
        if let Ok(parsed) = ayml_core::parse(&input) {
            check_span_invariants(&parsed, &input, None)?;
        }
    }

    /// Emitting a tree with comments and parsing it back must not panic
    /// and must produce a valid tree.
    #[test]
    fn comments_emit_parse_no_panic(node in arb_node_with_comments(3)) {
        let emitted = ayml_core::emit(&node);
        let parsed = ayml_core::parse(&emitted);
        prop_assert!(
            parsed.is_ok(),
            "parse failed: {}\n--- emitted ---\n{}---",
            parsed.unwrap_err(),
            emitted,
        );
    }

    /// Blank lines within comments survive a parse → emit → re-parse cycle.
    #[test]
    fn blank_lines_in_comments_stable(
        comment_a in "[a-zA-Z0-9]{1,10}",
        comment_b in "[a-zA-Z0-9]{1,10}",
        blanks in 1..4usize,
    ) {
        // In the comment text, blank lines are represented as empty lines:
        // "a\n\nb" means line "a", blank line, line "b".
        let sep = "\n".repeat(blanks + 1); // +1: the \n after comment_a + blank lines
        let comment = format!("{comment_a}{sep}{comment_b}");
        // In source: "# a\n" + blank_lines + "# b\n..."
        let blank_lines = "\n".repeat(blanks);
        let input = format!("# {comment_a}\n{blank_lines}# {comment_b}\nkey: value\n");

        let parsed = ayml_core::parse(&input).map_err(|e| {
            TestCaseError::fail(format!("parse failed: {e}\n--- input ---\n{input}---"))
        })?;

        prop_assert!(
            parsed.comment.as_deref() == Some(comment.as_str()),
            "blank lines in comment not preserved.\n--- input ---\n{}---",
            input,
        );

        // Emit and re-parse — should be stable.
        let emitted = ayml_core::emit(&parsed);
        let re_parsed = ayml_core::parse(&emitted).map_err(|e| {
            TestCaseError::fail(format!("re-parse failed: {e}\n--- emitted ---\n{emitted}---"))
        })?;
        prop_assert!(
            parsed.comment == re_parsed.comment,
            "comment changed after re-emit.\n--- emitted ---\n{}---",
            emitted,
        );
    }

    /// Triple-quoted strings inside flow collections parse correctly.
    #[test]
    fn triple_quoted_in_flow_collections(
        text in "[a-zA-Z0-9 ]{1,20}",
        kind in 0..3u8,
    ) {
        let input = match kind {
            0 => format!("[1, \"\"\"\n  {text}\n  \"\"\"]\n"),
            1 => format!("{{a: \"\"\"\n  {text}\n  \"\"\"}}\n"),
            _ => format!("{{a: \"\"\"\n  {text}\n  \"\"\", b: 1}}\n"),
        };
        let parsed = ayml_core::parse(&input);
        prop_assert!(
            parsed.is_ok(),
            "parse failed: {}\n--- input ---\n{}---",
            parsed.unwrap_err(),
            input,
        );
    }
}
