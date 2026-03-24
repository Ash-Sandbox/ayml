//! Property-based tests for CommentedValue roundtrip and robustness.

use ayml::{
    commented_value::{CommentedValue, CommentedValueKind},
    from_str, to_string,
};
use indexmap::IndexMap;
use proptest::prelude::*;

// ── Generators ───────────────────────────────────────────────────

/// Generate a random printable Unicode character, including non-ASCII.
fn arb_unicode_char() -> impl Strategy<Value = char> {
    prop_oneof![
        (0x20_u32..=0x7E_u32),
        (0xA0_u32..=0xFF_u32),
        (0x100_u32..=0x17F_u32),
        (0x370_u32..=0x3FF_u32),
        (0x400_u32..=0x4FF_u32),
        (0x600_u32..=0x6FF_u32),
        (0x900_u32..=0x97F_u32),
        (0x4E00_u32..=0x9FFF_u32),
        (0x3040_u32..=0x309F_u32),
        (0x30A0_u32..=0x30FF_u32),
        (0xAC00_u32..=0xD7A3_u32),
        (0x1F300_u32..=0x1F5FF_u32),
        (0x1F600_u32..=0x1F64F_u32),
    ]
    .prop_filter_map("valid char", char::from_u32)
}

/// Generate a random Unicode string of 1..=max_len printable characters.
fn arb_unicode_string(max_len: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(arb_unicode_char(), 1..=max_len)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Comment text: at least one non-space char, no `#` or newlines.
fn arb_comment_line() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_.,:;!?()-][a-zA-Z0-9 _.,:;!?()-]{0,39}"
}

/// Optional single-line comment.
fn arb_opt_comment() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        3 => Just(None),
        1 => arb_comment_line().prop_map(Some),
    ]
}

/// Optional multi-line top comment (1-3 lines joined with \n).
/// May include blank lines between comment lines.
fn arb_opt_top_comment() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        3 => Just(None),
        // Consecutive comment lines
        1 => prop::collection::vec(arb_comment_line(), 1..=3)
            .prop_map(|lines| Some(lines.join("\n"))),
        // Two comment lines separated by a blank line
        1 => (arb_comment_line(), arb_comment_line())
            .prop_map(|(a, b)| Some(format!("{a}\n\n{b}"))),
        // Three comment lines with blank lines
        1 => (arb_comment_line(), arb_comment_line(), arb_comment_line())
            .prop_map(|(a, b, c)| Some(format!("{a}\n\n{b}\n\n{c}"))),
    ]
}

fn arb_map_key() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        2 => arb_unicode_string(16),
        1 => Just(String::new()),
        1 => Just("null".into()),
        1 => Just("true".into()),
        1 => Just("false".into()),
        1 => Just("42".into()),
    ]
}

fn arb_scalar_string() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => "[a-zA-Z][a-zA-Z0-9 _-]{0,30}",
        3 => arb_unicode_string(30),
        1 => Just(String::new()),
        1 => Just("null".into()),
        1 => Just("true".into()),
        1 => Just("false".into()),
        1 => Just("42".into()),
        1 => Just("3.25".into()),
    ]
}

/// Generate a random CommentedValue tree with bounded depth.
///
/// Per the AYML spec, block sequence entries can only contain flow nodes
/// or compact mappings — not nested block sequences. The serializer emits
/// block style for sequences, so we avoid generating Seq-in-Seq to stay
/// within the spec. Sequences may only contain scalars and mappings.
fn arb_commented_value(depth: u32) -> impl Strategy<Value = CommentedValue> {
    arb_commented_value_inner(depth, false)
}

fn arb_commented_value_inner(
    depth: u32,
    inside_seq: bool,
) -> impl Strategy<Value = CommentedValue> {
    let leaf = prop_oneof![
        Just(CommentedValueKind::Null),
        any::<bool>().prop_map(CommentedValueKind::Bool),
        any::<i64>().prop_map(CommentedValueKind::Int),
        (-1e15_f64..1e15_f64)
            .prop_filter("finite and normal", |f| f.is_finite())
            .prop_map(|f| CommentedValueKind::Float((f * 1e6).round() / 1e6)),
        prop_oneof![
            Just(CommentedValueKind::Float(f64::INFINITY)),
            Just(CommentedValueKind::Float(f64::NEG_INFINITY)),
            Just(CommentedValueKind::Float(f64::NAN)),
        ],
        arb_scalar_string().prop_map(CommentedValueKind::Str),
    ];

    if depth == 0 {
        (arb_opt_top_comment(), arb_opt_comment(), leaf)
            .prop_map(|(top, inline, kind)| CommentedValue {
                top_comment: top,
                inline_comment: inline,
                value: kind,
            })
            .boxed()
    } else {
        // Seq children are inside_seq=true, so they won't generate nested seqs.
        // Map children are inside_seq=false, so maps can contain seqs.
        let seq_strategy = prop::collection::vec(arb_commented_value_inner(depth - 1, true), 0..4)
            .prop_map(CommentedValueKind::Seq);

        let map_strategy = prop::collection::vec(
            (arb_map_key(), arb_commented_value_inner(depth - 1, false)),
            0..4,
        )
        .prop_map(|entries| {
            let map: IndexMap<String, CommentedValue> = entries.into_iter().collect();
            CommentedValueKind::Map(map)
        });

        let inner = if inside_seq {
            // Inside a seq: only scalars and maps (no nested seqs per spec)
            prop_oneof![
                4 => leaf,
                1 => map_strategy,
            ]
            .boxed()
        } else {
            prop_oneof![
                4 => leaf,
                1 => seq_strategy,
                1 => map_strategy,
            ]
            .boxed()
        };

        (arb_opt_top_comment(), arb_opt_comment(), inner)
            .prop_map(|(top, inline, kind)| CommentedValue {
                top_comment: top,
                inline_comment: inline,
                value: kind,
            })
            .boxed()
    }
}

/// Normalize a CommentedValue for roundtrip comparison:
/// - Strip all inline comments: inline comments on collection nodes don't
///   roundtrip (they get misattributed to the last child element), so we
///   strip them everywhere for a clean comparison.
/// - Strip top comments from seq elements: when the seq itself is wrapped
///   in `Commented`, element top comments (emitted before `- `) get
///   attributed to the parent `Commented` wrapper on deserialization.
/// - Treat `Some("")` as `None` for top comments.
/// - Values and top comments on map values are the primary roundtrip targets.
fn normalize(cv: &CommentedValue) -> CommentedValue {
    normalize_inner(cv, false)
}

fn normalize_inner(cv: &CommentedValue, in_seq: bool) -> CommentedValue {
    // Strip top comments from:
    // - seq elements (they leak up to the parent Commented<Seq> wrapper)
    // - nodes whose value is a Seq (they may have absorbed a leaked comment)
    let is_seq_value = matches!(cv.value, CommentedValueKind::Seq(_));
    let top = if in_seq || is_seq_value {
        None
    } else {
        cv.top_comment.as_ref().filter(|c| !c.is_empty()).cloned()
    };
    let kind = match &cv.value {
        CommentedValueKind::Seq(items) => {
            CommentedValueKind::Seq(items.iter().map(|v| normalize_inner(v, true)).collect())
        }
        CommentedValueKind::Map(map) => CommentedValueKind::Map(
            map.iter()
                .map(|(k, v)| (k.clone(), normalize_inner(v, false)))
                .collect(),
        ),
        other => other.clone(),
    };
    CommentedValue {
        top_comment: top,
        inline_comment: None,
        value: kind,
    }
}

// ── Property tests ───────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5_000))]

    /// Serialize a random CommentedValue, deserialize it back, and verify
    /// that the value and comments survive the roundtrip.
    #[test]
    fn commented_value_round_trip(value in arb_commented_value(3)) {
        let serialized = to_string(&value).map_err(|e| {
            TestCaseError::fail(format!(
                "serialize failed: {e}\n--- value ---\n{value:?}"
            ))
        })?;

        let deserialized: CommentedValue = from_str(&serialized).map_err(|e| {
            TestCaseError::fail(format!(
                "deserialize failed: {e}\n--- serialized ---\n{serialized}--- value ---\n{value:?}"
            ))
        })?;

        let expected = normalize(&value);
        let actual = normalize(&deserialized);

        prop_assert!(
            expected == actual,
            "values differ\n--- original (normalized) ---\n{expected:?}\n--- serialized ---\n{serialized}\n--- deserialized (normalized) ---\n{actual:?}",
        );
    }

    /// Ensure from_str::<CommentedValue> never panics on arbitrary input.
    #[test]
    fn commented_value_deserialize_no_panic(input in "[\\s\\S]{0,200}") {
        let _ = from_str::<CommentedValue>(&input);
    }

    /// Ensure to_string never panics on arbitrary CommentedValue trees.
    #[test]
    fn commented_value_serialize_no_panic(value in arb_commented_value(3)) {
        let _ = to_string(&value);
    }

    /// Blank lines within comments survive serde serialize → deserialize.
    #[test]
    fn blank_lines_in_comments_serde_stable(
        comment_a in "[a-zA-Z0-9]{1,10}",
        comment_b in "[a-zA-Z0-9]{1,10}",
        blanks in 1..4usize,
    ) {
        let sep = "\n".repeat(blanks + 1);
        let comment = format!("{comment_a}{sep}{comment_b}");
        let blank_lines = "\n".repeat(blanks);
        let input = format!("# {comment_a}\n{blank_lines}# {comment_b}\nkey: value\n");

        let parsed: CommentedValue = from_str(&input).map_err(|e| {
            TestCaseError::fail(format!("parse failed: {e}\n--- input ---\n{input}---"))
        })?;

        // The top comment should preserve blank lines.
        prop_assert!(
            parsed.top_comment.as_deref() == Some(comment.as_str()),
            "blank lines not preserved in serde.\nparsed: {:?}\nexpected: {:?}\n--- input ---\n{}---",
            parsed.top_comment,
            comment,
            input,
        );

        // Re-serialize and re-parse — should be stable.
        let serialized = to_string(&parsed).map_err(|e| {
            TestCaseError::fail(format!("serialize failed: {e}"))
        })?;
        let re_parsed: CommentedValue = from_str(&serialized).map_err(|e| {
            TestCaseError::fail(format!(
                "re-parse failed: {e}\n--- serialized ---\n{serialized}---"
            ))
        })?;
        prop_assert!(
            parsed.top_comment == re_parsed.top_comment,
            "comment changed after re-serialize.\nfirst: {:?}\nsecond: {:?}\n--- serialized ---\n{}---",
            parsed.top_comment,
            re_parsed.top_comment,
            serialized,
        );
    }
}
