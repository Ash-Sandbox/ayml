//! Generators for characters, strings, numbers, and comments.

use ayml_core::scalar;
use proptest::prelude::*;
use proptest::sample::select;

/// Characters with special meaning in AYML or in YAML.
const STRUCTURAL: &[char] = &[
    '-', ':', '#', ',', '[', ']', '{', '}', '"', '\\', '\'', ' ', ' ', ' ', '\t', '/', '*', '&',
    '!', '|', '>', '?', '%', '@', '`', '.', '+', 'e', 'E', '0', '1', '7', '9', 'x', 'o', 'b', 'n',
    'a', 'u', 'U', 'l', 'i', 'f',
];

/// Characters that are not printable, invisible, or render as line breaks.
const SPECIAL: &[char] = &[
    '\0',
    '\x01',
    '\x07',
    '\x08',
    '\x0B',
    '\x0C',
    '\x1B',
    '\x1F',
    '\x7F',
    '\u{80}',
    '\u{85}',
    '\u{9F}',
    '\u{A0}',
    '\u{FEFF}',
    '\u{FFFE}',
    '\u{FFFF}',
    '\u{2028}',
    '\u{2029}',
    '\u{3000}',
    '\u{200B}',
    '\u{1680}',
    '\u{D7FF}',
    '\u{E000}',
    '\u{FFFD}',
    '\u{10FFFF}',
];

/// Any character, weighted toward ones that stress the grammar.
pub fn arb_char() -> BoxedStrategy<char> {
    prop_oneof![
        8 => prop::char::range(' ', '~'),
        5 => select(STRUCTURAL),
        2 => select(&['\n', '\r'][..]),
        2 => select(SPECIAL),
        2 => prop::char::range('\u{A0}', '\u{2FFF}'),
        1 => prop::char::range('\u{1F300}', '\u{1F64F}'),
        1 => any::<char>(),
    ]
    .boxed()
}

/// Strings that look like non-string scalars, or like syntax.
pub fn arb_lookalike() -> BoxedStrategy<String> {
    let fixed: Vec<String> = [
        "null",
        "true",
        "false",
        "inf",
        "+inf",
        "-inf",
        "nan",
        "-nan",
        "+nan",
        "NaN",
        "Inf",
        "NULL",
        "True",
        "yes",
        "no",
        "~",
        "-",
        "--",
        "---",
        ":",
        "::",
        "- ",
        "-a",
        "- a",
        "a:",
        "a: b",
        "a:b",
        "a :b",
        "#",
        "a #b",
        "a#b",
        "0",
        "-0",
        "+0",
        "00",
        "007",
        "-007",
        "0x",
        "0x1G",
        "0o8",
        "0b2",
        "1.",
        "1.e5",
        ".5",
        "-.5",
        "1e",
        "1e+",
        "1_000",
        "0X10",
        "1E5",
        "01.5",
        "0e0",
        "\"\"\"",
        "\"",
        "'a'",
        "a'b",
        "\\",
        "\\n",
        "[a]",
        "{a}",
        "a,b",
        "a]",
        "- - a",
        "a: - b",
        " ",
        "  a",
        "a  ",
        "\t",
        "\u{FEFF}a",
        "\u{A0}",
        "a\u{A0}",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    prop_oneof![
        3 => select(fixed),
        1 => any::<i64>().prop_map(|i| i.to_string()),
        1 => any::<i64>().prop_map(|i| format!("{i:#x}")),
        1 => any::<i64>().prop_map(|i| format!("-0o{:o}", i.unsigned_abs())),
        1 => any::<u64>().prop_map(|i| format!("+0b{i:b}")),
        1 => any::<f64>().prop_map(|f| format!("{f:?}")),
        1 => any::<f64>().prop_map(|f| format!("{f:e}")),
        1 => "[0-9]{19,24}",
        1 => "-?[1-9][0-9]{0,2}\\.[0-9]{1,3}e[0-9]{3,4}",
    ]
    .boxed()
}

/// A multi-line string.
pub fn arb_multiline() -> BoxedStrategy<String> {
    let line = prop_oneof![
        3 => "[ a-z\"#\\\\\t]{0,10}",
        1 => Just(String::new()),
        1 => Just("\"\"\"".to_owned()),
        1 => Just("  ".to_owned()),
        1 => Just("\\".to_owned()),
        1 => prop::collection::vec(arb_char(), 0..6).prop_map(|v| v.into_iter().collect()),
    ];
    prop::collection::vec(line, 1..5)
        .prop_map(|lines| lines.join("\n"))
        .boxed()
}

/// Any string: random characters, identifiers, look-alikes, multi-line.
pub fn arb_string() -> BoxedStrategy<String> {
    prop_oneof![
        4 => prop::collection::vec(arb_char(), 0..16).prop_map(|v| v.into_iter().collect()),
        3 => "[a-zA-Z][a-zA-Z0-9 _./-]{0,16}",
        2 => arb_lookalike(),
        1 => arb_multiline(),
    ]
    .boxed()
}

/// A mapping key: any string, biased toward identifiers.
pub fn arb_key() -> BoxedStrategy<String> {
    prop_oneof![
        3 => "[a-z_][a-z0-9_-]{0,10}",
        2 => arb_string(),
    ]
    .boxed()
}

/// Any `i64`, biased toward edge values.
pub fn arb_i64() -> BoxedStrategy<i64> {
    prop_oneof![
        4 => any::<i64>(),
        2 => -1000i64..1000,
        1 => select(&[i64::MIN, i64::MIN + 1, i64::MAX, i64::MAX - 1, 0, -1, 1][..]),
    ]
    .boxed()
}

/// Any `f64` bit pattern, biased toward special values.
pub fn arb_f64() -> BoxedStrategy<f64> {
    prop_oneof![
        4 => any::<f64>(),
        2 => (-1e6f64..1e6).prop_map(|f| (f * 100.0).round() / 100.0),
        1 => select(
            &[
                0.0,
                -0.0,
                1.0,
                -1.0,
                0.1,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NAN,
                f64::MIN_POSITIVE,
                5e-324,
                f64::MAX,
                f64::MIN,
                1e16,
                1e-7,
            ][..]
        ),
    ]
    .boxed()
}

/// True if `c` may appear literally in a comment.
pub fn is_comment_char(c: char) -> bool {
    scalar::is_printable(c) && c != '\n' && c != '\r'
}

/// A normalized comment line.
pub fn arb_comment_line() -> BoxedStrategy<String> {
    let comment_char = arb_char().prop_map(|c| if is_comment_char(c) { c } else { '\u{FFFD}' });
    prop::collection::vec(comment_char, 0..16)
        .prop_map(|chars| {
            chars
                .into_iter()
                .collect::<String>()
                .trim_end_matches([' ', '\t'])
                .to_owned()
        })
        .boxed()
}

/// An optional, normalized multi-line comment.
pub fn arb_comment() -> BoxedStrategy<Option<String>> {
    prop_oneof![
        4 => Just(None),
        1 => prop::collection::vec(arb_comment_line(), 1..4).prop_map(|l| Some(l.join("\n"))),
    ]
    .boxed()
}

/// An optional, normalized single-line comment.
pub fn arb_inline_comment() -> BoxedStrategy<Option<String>> {
    prop_oneof![
        4 => Just(None),
        1 => arb_comment_line().prop_map(Some),
    ]
    .boxed()
}
