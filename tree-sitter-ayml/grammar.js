/// <reference types="tree-sitter-cli/dsl" />

// A simplified grammar for AYML focused on syntax highlighting.
// Identifies tokens for coloring without modeling indentation nesting.
//
// Key design: mapping_key includes the trailing `:` as a single token
// (e.g. `host:`) so bare identifiers like `outbound` and `deny` are
// never mistaken for mapping keys.

module.exports = grammar({
  name: "ayml",

  extras: ($) => [/[ \t]/, $._newline],

  rules: {
    document: ($) => repeat($._statement),

    _statement: ($) =>
      choice(
        $.comment,
        $.mapping_pair,
        $.sequence_entry,
        $.flow_sequence,
        $.flow_mapping,
        $._scalar,
      ),

    _newline: (_) => /\r?\n/,

    // ── Comments ──────────────────────────────────────────────

    comment: (_) => token(seq("#", /.*/)),

    // ── Mapping pair ──────────────────────────────────────────

    mapping_pair: ($) =>
      prec.right(1,
        seq(
          $.mapping_key,
          optional(field("value", choice($._scalar, $.flow_sequence, $.flow_mapping))),
        ),
      ),

    // Mapping key including the colon: `key:`.
    // The colon is immediately after the identifier with no space.
    mapping_key: (_) => token(prec(2, /[a-zA-Z_][a-zA-Z0-9_]*:/)),

    // ── Sequence entry ────────────────────────────────────────

    sequence_entry: ($) =>
      seq(
        $.sequence_indicator,
        choice(
          $.mapping_pair,
          $.flow_sequence,
          $.flow_mapping,
          $._scalar,
        ),
      ),

    sequence_indicator: (_) => "-",

    // ── Scalars ───────────────────────────────────────────────

    _scalar: ($) =>
      choice(
        $.null_literal,
        $.boolean_literal,
        $.float_literal,
        $.integer_literal,
        $.triple_quoted_string,
        $.double_quoted_string,
        $.bare_string,
      ),

    null_literal: (_) => prec(1, "null"),

    boolean_literal: (_) => prec(1, choice("true", "false")),

    float_literal: (_) =>
      token(
        prec(1, choice(
          /[+-]?[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?/,
          /[+-]?[0-9]+[eE][+-]?[0-9]+/,
          "inf",
          "+inf",
          "-inf",
          "nan",
        )),
      ),

    integer_literal: (_) =>
      token(
        prec(1, choice(
          /[+-]?0x[0-9a-fA-F]+/,
          /[+-]?0o[0-7]+/,
          /[+-]?0b[01]+/,
          /[+-]?[0-9]+/,
        )),
      ),

    double_quoted_string: (_) =>
      token(seq('"', repeat(choice(/[^"\\\n\r]/, /\\./)), '"')),

    // Triple-quoted string: `"""` newline content `"""`.
    // The content regex matches anything that isn't `"""`.
    // We use a negative pattern: any char that isn't `"`, or `"` not
    // followed by `""`.
    triple_quoted_string: (_) =>
      token(prec(2, seq(
        '"""',
        repeat(choice(
          /[^"]/,       // any non-quote char (including newlines)
          /\"[^"]/,     // single quote not followed by quote
          /\"\"[^"]/,   // two quotes not followed by quote
        )),
        '"""',
      ))),

    // Bare string: anything that doesn't match a more specific token.
    bare_string: (_) => token(prec(-1, /[^\s\[\]{},#"\\][^\n\r#]*/)),

    // ── Flow Collections ──────────────────────────────────────

    flow_sequence: ($) =>
      seq(
        "[",
        optional(seq($._flow_value, repeat(seq(",", $._flow_value)), optional(","))),
        "]",
      ),

    flow_mapping: ($) =>
      seq(
        "{",
        optional(seq($.flow_pair, repeat(seq(",", $.flow_pair)), optional(","))),
        "}",
      ),

    flow_pair: ($) =>
      prec.right(1,
        seq(
          $.mapping_key,
          optional(field("value", $._flow_value)),
        ),
      ),

    _flow_value: ($) =>
      choice(
        $.null_literal,
        $.boolean_literal,
        $.float_literal,
        $.integer_literal,
        $.triple_quoted_string,
        $.double_quoted_string,
        $.flow_bare_string,
        $.flow_sequence,
        $.flow_mapping,
      ),

    // Bare string inside flow collections: stops at `,`, `}`, `]`.
    flow_bare_string: (_) => token(prec(-1, /[^\s\[\]{},#"\\][^\n\r#,}\]]*/)),
  },
});
