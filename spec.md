# AYML version 1.0

**2026-10-13**

**Status of this Document**

This is the AYML specification v1.0.
It defines the AYML 1.0 data language.

## Abstract

AYML is a software configuration language that looks like YAML and acts like JSON.
It is meant to be a human-friendly, cross language, Unicode based software
configuration language. Unlike YAML, it is not designed as a fully featured
data serialization framework. There are no references, tags, types beyond the
JSON-like core, unordered sets, duplicate keys, document prefixes, or other
complex features supported by YAML.

## Goals

AYML's design goals are, in decreasing priority:

1. it is easy to read and understand
2. it is easy to write correctly (and hard to write incorrectly)
3. it deserializes naturally into strongly typed data structures
4. it deserializes naturally into the core types of dynamically typed languages
5. it incorporates comments as a formal part of the document structure

## Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD",
"SHOULD NOT", "RECOMMENDED",  "MAY", and "OPTIONAL" in this document are to be
interpreted as described in RFC 2119.

A _processor_ is any program that reads (a _parser_ or _deserializer_) or
writes (an _emitter_ or _serializer_) AYML text.

## Language Overview

This section provides a non-normative overview of AYML.
The full specification follows in later sections.

### Collections

A block collection uses indentation for scope and begins each entry on its own line.
A block sequence indicates each entry with a dash and space `- `.
A mapping uses a colon and space `: ` to mark each key/value pair. A mapping may not have duplicate keys.
A comment begins with a `#`.

**Example: Sequence of Scalars (ball players)**

```
- Mark McGwire
- Sammy Sosa
- Ken Griffey
```

**Example: Mapping Scalars to Scalars (player statistics)**

```
hr:  65    # Home runs
avg: 0.278 # Batting average
rbi: 147   # Runs Batted In
```

**Example: Mapping Scalars to Sequences (ball clubs in each league)**

```
american:
  - Boston Red Sox
  - Detroit Tigers
  - New York Yankees
national:
  - New York Mets
  - Chicago Cubs
  - Atlanta Braves
```

**Example: Sequence of Mappings (players' statistics)**

```
- name: Mark McGwire
  hr:   65
  avg:  0.278
- name: Sammy Sosa
  hr:   63
  avg:  0.288
```

AYML also allows flow styles that use explicit indicators to denote scope.
A flow sequence is written as a comma-separated list within square brackets `[]`.
A flow mapping uses curly braces.

**Example: Sequence of Sequences**

```
- [name, hr, avg]
- [Mark McGwire, 65, 0.278]
- [Sammy Sosa, 63, 0.288]
```

**Example: Mapping of Mappings**

```
Mark McGwire: {hr: 65, avg: 0.278}
Sammy Sosa: {
  hr: 63,
  avg: 0.288,
}
```

**Example: Document with Two Comments**

```
hr: # 1998 hr ranking
  - Mark McGwire
  - Sammy Sosa
# 1998 rbi ranking
rbi:
  - Sammy Sosa
  - Ken Griffey
```

**Example: Compact Nested Mapping**

```
# Products purchased
- item: Super Hoop
  quantity: 1
- item: Basketball
  quantity: 4
- item: Big Shoes
  quantity: 1
```

### Primitives

Supported primitives are `bool`, `int`, `float`, `str`, and `null`.

**Boolean**

A boolean is either `true` or `false`, with no quotes.

```
is_good: true
is_bad: false
```

**Integer**

Integers are 64-bit signed values. Decimal, binary, octal, and hexadecimal
notations are supported.

```
decimal: 12345
binary: 0b10101010
octal: 0o14
hexadecimal: 0xC
```

**Float**

Floats are 64-bit (double precision) values, including `inf` and `nan`:

```
canonical: 1.23015e+3
exponential: 12.3015e+02
fixed: 1230.15
negative infinity: -inf
not a number: nan
```

**Strings**

There are two kinds of single-line strings: bare and double-quoted.

```
unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
hex esc: "\x0d\x0a is \r\n"
howdy: "\"Howdy!\" he cried."
```

Triple-quoted strings (`"""`) are the only multi-line string format. The
indentation of the closing `"""` determines how much leading white space is
stripped. Escape sequences are supported, and a `\` at the end of a line
suppresses the line break.

```
plain: This unquoted scalar is single-line only.

multi-line: """
  This string
  spans many lines.
  """

code: """
  You can add "quotes" and 'quotes' here.
  Anything goes inside triple-quoted strings.
  """

folded: """
  This is where \
  you might think there is a line break \
  but there is no line break here.
  """
```

**Null**

The null value is `null` with no quotes.

```
optional field: null
```

#### Comments

A comment is a form of documentation that is associated with a particular
AYML node. Deserializers into application data types typically ignore
comments; processors that edit documents (formatters, linters, language
servers) preserve them. Each comment belongs to exactly one node:

* comment lines above a node belong to that node
* a comment at the end of a line belongs to the node that ends on that line
* comment lines after the last entry of a collection belong to that collection

**Example: Top and Side Comments**

```
# Network connection rules
# Use these rules to allow socket connections
network:
  rules:
    - host: github.com
      ports:
        - 22 # Git (SSH)
        - 443 # Site
        # - 8080 (disabled)
```

A comment must not appear inside a scalar. Inside triple-quoted strings (`"""`),
all content — including `#` characters — is literal string content, not comments.

**Example: Triple-Quoted String With Literal `#`**

```
code: """
  # thread-safe work
  mutex.lock()
  do_work()
  mutex.unlock() # done
  """
```

Comments are allowed as an extension to the object model because they are often critical to
understanding the purpose of a configuration block, and operations like formatting and linting
should be supported without destroying that understanding.

### Full Length Example

Below are two full-length examples of AYML.
The first is a sample invoice; the second is a sample log file.

**Example: Invoice**

```
invoice: 34843
date: 2001-01-23
bill-to:
  given: Chris
  family: Dumars
  address:
    lines: """
      458 Walkman Dr.
      Suite #292
      """
    city    : Royal Oak
    state   : MI
    postal  : 48046
ship-to: null
product:
  - sku         : BL394D
    quantity    : 4
    description : Basketball
    price       : 450.00
  - sku         : BL4438H
    quantity    : 1
    description : Super Hoop
    price       : 2392.00
tax  : 251.42
total: 4443.42
comments: """
  Late afternoon is best.
  Backup contact is Nancy
  Billsmer @ 338-4338.
  """
```

**Example: Log File**

```
Date: 2001-11-23T15:03:17-5:00
User: ed
Fatal: Unknown variable "bar"
Stack:
  - file: TopClass.py
    line: 23
    code: """
      x = MoreObject("345\n")
      """
  - file: MoreClass.py
    line: 58
    code: """
      foo = bar
      """
```

# Chapter: Data Model

An AYML document represents exactly one _node_. A node is either a scalar or a
collection.

* **null** — the single value `null`.
* **bool** — `true` or `false`.
* **int** — an integer in the range −2<sup>63</sup> to 2<sup>63</sup>−1.
* **float** — an IEEE 754 binary64 value, including positive and negative
  infinity and NaN.
* **str** — a sequence of Unicode scalar values. Through escape sequences, a
  string may contain any Unicode scalar value, including characters that may
  not appear literally in an AYML document.
* **sequence** — an ordered list of nodes.
* **mapping** — a collection of key/value pairs. Keys are strings and MUST be
  unique within a mapping. Values are nodes.

Two keys are equal if they are the same sequence of Unicode scalar values after
quoted keys are unescaped. For example, `name` and `"name"` are the same key.

AYML does not define an ordering for mapping keys. Implementations MAY preserve
insertion order; the reference implementation does.

Comments are not part of the data model seen by deserializers, but they are
part of the document structure. See [Comment Association](#chapter-comment-association).

# Chapter: Conventions

## Production Syntax

Productions are defined using the syntax `production-name ::= term`, where a
term is either:

An atomic term:

* A quoted string (`"abc"`), which matches that concatenation of characters. A
  single character is usually written with single quotes (`'a'`).
* A hexadecimal number (`x0A`), which matches the character at that Unicode
  code point.
* A range of hexadecimal numbers (`[x20-x7E]`), which matches any character
  whose Unicode code point is within that range.
* The name of a production (`c-printable`), which matches that production.

A lookaround:

* `[ lookahead = term ]`, which matches the empty string if `term` would match.
* `[ lookahead ≠ term ]`, which matches the empty string if `term` would not
  match.

A special production:

* `<start-of-line>`, which matches the empty string at the beginning of a line.
* `<end-of-input>`, which matches the empty string at the end of the input.

A parenthesized term matches its contents.

A concatenation is `term-one term-two`, which matches `term-one` followed by `term-two`.

An alternation is `term-one | term-two`, which matches the `term-one` if possible, or
`term-two` otherwise.

A difference is `term-one - term-two`, which matches `term-one` if `term-two`
does not match the same text.

A quantified term:

* `term?`, which matches `(term | <empty>)`.
* `term*`, which matches `(term term* | <empty>)`.
* `term+`, which matches `(term term*)`.
* `term{n}`, which matches exactly `n` consecutive occurrences of `term`.

> Note: Quantified terms are always greedy.

The order of precedence is parenthesization, then quantification, then
difference, then concatenation, then alternation.

Comments in productions (`# like this`) are informative only.

## Production Parameters

Some productions have parameters in parentheses after the name. A
parameterized production is shorthand for an (infinite) series of productions,
each with a fixed value for each parameter.

* `n`, `m` : An indentation level, measured in spaces. May be any non-negative
  integer.
* `c` : Context to distinguish between block and flow parsing. One of:
  * `BLOCK` — outside any flow collection
  * `FLOW` — inside a flow collection

## Production Naming Conventions

* `c-` : A production starting and ending with a special character.
* `b-` : A production matching a single line break.
* `nb-` : A production starting and ending with a non-break character.
* `s-` : A production starting and ending with a white space character.
* `ns-` : A production starting and ending with a non-space character.
* `l-` : A production matching complete line(s).

## Errors

A processor MUST reject any input that does not match the grammar or that
violates a constraint stated in this specification. A processor MUST NOT
recover from an error in a way that changes the meaning of the document (for
example, by discarding the rest of a line).

# Chapter: Character Productions

## Character Set

To ensure readability, AYML uses only the printable subset of the Unicode
character set.

```
c-printable ::=
    (
        x09                  # Tab (\t)
      | x0A                  # Line feed (LF \n)
      | x0D                  # Carriage return (CR \r)
      | [x20-x7E]            # Printable ASCII
      | x85                  # Next Line (NEL)
      | [xA0-xD7FF]          # Basic Multilingual Plane (BMP)
      | [xE000-xFFFD]        # Additional Unicode Areas
      | [x010000-x10FFFF]    # Supplementary planes
    )
  - c-bidi-control

c-bidi-control ::=
    [x202A-x202E]        # Bidirectional embeddings and overrides
  | [x2066-x2069]        # Bidirectional isolates
```

Every character of an AYML document MUST be in `c-printable`. Characters
outside this set (such as NUL, DEL, and the C1 controls other than NEL) can
only be represented through escape sequences in quoted strings. Bidirectional
control characters are excluded because they can make text display in a
different order than it is parsed, hiding what a document says.

## Character Encoding

AYML files MUST be encoded in UTF-8. No other encodings are supported.

A byte order mark (BOM, `xFEFF`) MUST NOT appear at the start of an AYML file.

## Line Break Characters

```
b-line-feed ::= x0A
b-carriage-return ::= x0D
b-char ::= b-line-feed | b-carriage-return

b-break ::=
    ( b-carriage-return b-line-feed )    # CR LF
  | b-carriage-return                    # CR
  | b-line-feed                          # LF
```

All other characters, including form feed and NEL, are non-break characters.

```
nb-char ::= c-printable - b-char
```

Line breaks inside triple-quoted string content MUST be normalized to a single
line feed (`x0A`). Outside scalar content, any line break may be used.

A _line_ is a maximal sequence of `nb-char` terminated by a `b-break` or by
the end of the input.

```
b-line-end ::= b-break | <end-of-input>
```

## White Space Characters

```
s-space ::= x20
s-tab ::= x09
s-white ::= s-space | s-tab

ns-char ::= nb-char - s-white
```

## Miscellaneous Characters

```
ns-dec-digit ::= [x30-x39]                                # 0-9
ns-hex-digit ::= ns-dec-digit | [x41-x46] | [x61-x66]     # 0-9 A-F a-f
ns-oct-digit ::= [x30-x37]                                # 0-7
ns-bin-digit ::= '0' | '1'
```

## Indicator Characters

_Indicators_ are characters that have special semantics.

```
c-sequence-entry ::= '-'     # block sequence entry
c-mapping-value  ::= ':'     # mapping value
c-collect-entry  ::= ','     # flow collection entry separator
c-sequence-start ::= '['
c-sequence-end   ::= ']'
c-mapping-start  ::= '{'
c-mapping-end    ::= '}'
c-comment        ::= '#'
c-double-quote   ::= '"'
c-escape         ::= '\'

c-indicator ::=
    c-sequence-entry | c-mapping-value | c-collect-entry
  | c-sequence-start | c-sequence-end
  | c-mapping-start | c-mapping-end
  | c-comment | c-double-quote | c-escape

c-flow-indicator ::=
    c-collect-entry
  | c-sequence-start | c-sequence-end
  | c-mapping-start | c-mapping-end
```

The single quote is _reserved_: it may not begin a bare string, so that
YAML-style single-quoted strings are rejected rather than silently read with
their quotes.

```
c-reserved ::= '''
```

> **Note:** AYML does not use the following YAML indicators: `?` (explicit
> mapping key), `&` (anchor), `*` (alias), `!` (tag), `|` and `>` (block
> scalars), `%` (directive), `@`, or `` ` ``. These characters have no special
> meaning in AYML.

## Escape Sequences

Escape sequences are interpreted in double-quoted and triple-quoted strings.
In bare strings, the "`\`" character has no special meaning.

```
c-ns-esc-char ::=
    c-escape
    (
        '0'                        # Null (x00)
      | 'a'                        # Bell (x07)
      | 'b'                        # Backspace (x08)
      | 't'                        # Horizontal tab (x09)
      | 'n'                        # Line feed (x0A)
      | 'v'                        # Vertical tab (x0B)
      | 'f'                        # Form feed (x0C)
      | 'r'                        # Carriage return (x0D)
      | 'e'                        # Escape (x1B)
      | x20                        # Space (x20)
      | '"'                        # Double quote (x22)
      | '/'                        # Slash (x2F)
      | '\'                        # Backslash (x5C)
      | ( 'x' ns-hex-digit{2} )    # Code point x00 – xFF
      | ( 'u' ns-hex-digit{4} )    # Code point x0000 – xFFFF
      | ( 'U' ns-hex-digit{8} )    # Code point x00000000 – x10FFFF
    )
```

The code point denoted by a `\x`, `\u`, or `\U` escape MUST be a Unicode scalar
value: surrogates (`xD800`–`xDFFF`) and values above `x10FFFF` are errors.
There are no surrogate pairs; use `\U` for supplementary characters.

Inside triple-quoted strings, a `\` immediately followed by a line break
suppresses that line break (line continuation).

```
c-escape-break ::= c-escape b-break
```

# Chapter: Indentation, Separation, and Comments

## Indentation

AYML uses spaces for indentation. The _indentation_ of a line is the number of
leading space characters.

```
s-indent(n) ::= s-space{n}
```

Tabs MUST NOT be used for indentation: the leading white space of every line
that is not blank MUST consist of spaces only, as must the white space between
a block sequence's `-` and its value (which sets the indentation of a compact
collection). Elsewhere, tabs may be used for separation, blank lines may
contain tabs, and triple-quoted string content may begin with tabs after its
indentation.

The entries of a block collection all begin at the same indentation. When a
nested block collection begins on a new line, its indentation is detected from
its first entry and MUST satisfy the constraints stated for that collection.

## Separation

```
s-separate-in-line ::= s-white+
```

## Comments

A comment begins with `#` and extends to the end of the line. A `#` starts a
comment only when it is the first non-white-space character of a line or when
it is preceded by white space. Anywhere else outside a quoted string, a `#` is
an error. For example, `password: abc#123` is rejected; write
`password: "abc#123"` instead.

```
c-nb-comment-text ::= c-comment nb-char*
```

The _text_ of a comment is the content after the `#`, with one leading space
removed if present and with trailing white space removed.

Every content line ends with optional white space and an optional comment:

```
s-l-end ::= ( s-separate-in-line c-nb-comment-text | s-white* ) b-line-end
```

A line that contains only a comment is a _comment line_. It may have any
indentation (made of spaces).

```
l-comment ::= s-space* c-nb-comment-text b-line-end
```

A blank line contains only white space.

```
l-blank ::= s-white* b-break
```

Between the lines of block collections, blank lines and comment lines may
appear. These are collectively called _gaps_.

```
l-gap ::= l-blank | l-comment
```

# Chapter: Scalar Productions

## Null

```
ns-null ::= "null"
```

## Boolean

```
ns-bool ::= "true" | "false"
```

## Integer

```
ns-sign ::= '-' | '+'

ns-dec-int ::= '0' | ( [x31-x39] ns-dec-digit* )     # no leading zeros

ns-integer ::=
    ns-sign? ns-dec-int                             # Decimal
  | ns-sign? "0b" ns-bin-digit+                     # Binary
  | ns-sign? "0o" ns-oct-digit+                     # Octal
  | ns-sign? "0x" ns-hex-digit+                     # Hexadecimal
```

An integer whose value lies outside the 64-bit signed range is an error; it is
not converted to a float or a string. Note that `-0x8000000000000000` is the
minimum integer.

Decimal integers MUST NOT have leading zeros. A bare word like `0755` or
`02134` does not match `ns-integer` and is therefore a string; write `0o755`
for an octal integer.

**Example:**

```
decimal: 12345
negative: -9876
binary: 0b10101010
octal: 0o14
hexadecimal: 0xC
```

## Float

```
ns-exponent ::= ( 'e' | 'E' ) ns-sign? ns-dec-digit+

ns-float ::=
    ns-sign? ns-dec-int '.' ns-dec-digit+ ns-exponent?    # Fixed/exponential
  | ns-sign? ns-dec-int ns-exponent                       # Pure exponential
  | ns-sign? "inf"                                        # Infinity
  | "nan"                                                 # Not a number
```

A float is the binary64 value nearest to its decimal value (round to nearest,
ties to even). A finite literal whose magnitude is too large to represent
(for example `1e400`) is an error; it does not become infinity.

**Example:**

```
canonical: 1.23015e+3
exponential: 12.3015e+02
fixed: 1230.15
negative infinity: -inf
not a number: nan
```

## Double-Quoted String

A double-quoted string is delimited by `"` characters and supports escape
sequences. Double-quoted strings are single-line only.

```
nb-double-char ::=
    c-ns-esc-char
  | ( nb-char - c-double-quote - c-escape )

c-double-quoted ::= c-double-quote nb-double-char* c-double-quote
```

**Example:**

```
unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
```

## Bare String

A bare (unquoted) string has no delimiters. Bare strings are single-line only.
In value position they undergo [scalar resolution](#scalar-resolution).

A bare string cannot contain `#` or `: ` (a colon followed by white space or the
end of the line), cannot begin with an indicator other than `-` or `:`, and
cannot begin with `'`. A bare string may begin with `-` or `:` only if the next
character could continue the string. Leading and trailing white space is not
part of a bare string.

In a **flow context**, bare strings additionally cannot contain flow indicators
(`,`, `[`, `]`, `{`, `}`).

```
ns-plain-safe(BLOCK) ::= ns-char - c-comment
ns-plain-safe(FLOW)  ::= ns-char - c-comment - c-flow-indicator

ns-plain-first-char(c) ::=
    ( ns-plain-safe(c) - c-indicator - c-reserved )
  | ( ( c-sequence-entry | c-mapping-value ) [ lookahead = ns-plain-safe(c) ] )

ns-plain-char(c) ::=
    ( ns-plain-safe(c) - c-mapping-value )
  | ( c-mapping-value [ lookahead = ns-plain-safe(c) ] )

ns-bare-string(c) ::= ns-plain-first-char(c) ( s-white* ns-plain-char(c) )*
```

**Example:**

```
plain: This unquoted scalar is single-line.
url: https://example.com/path
glob: **/*.rs
```

## Triple-Quoted String

A triple-quoted string begins with `"""` at the end of a line and ends with
`"""` at the start of a later line (after indentation). The indentation `m` of
the closing `"""` determines the whitespace stripping level: that many leading
spaces are removed from each content line, like Swift's multi-line string
literals.

```
c-triple-quote ::= '"""'

nb-triple-char ::=
    c-ns-esc-char
  | ( nb-char - c-escape )

l-triple-content(m) ::=
    s-indent(m) nb-triple-char* ( c-escape-break s-indent(m) nb-triple-char* )*
  | s-space{0,m}                                   # blank line, see below

c-triple-quoted(n) ::=
    c-triple-quote s-white* b-break
    ( l-triple-content(m) b-break )*
    s-indent(m) c-triple-quote                     # where m >= n
```

The rules for a triple-quoted string are:

* The opening `"""` MUST be followed by optional white space and a line break.
* The closing delimiter is the first subsequent line whose first non-white-space
  characters are `"""`. Its indentation MUST consist of spaces. A content line
  that must begin with `"""` escapes a quote (`\"""`).
* The closing indentation `m` MUST be at least the indentation `n` of the block
  entry that contains the string (zero at the top level).
* Every content line MUST begin with `m` spaces, which are removed. A line that
  consists of fewer than `m` spaces and nothing else is an empty line.
* The content is the stripped lines joined by line feeds. The line break
  before the closing delimiter is _not_ part of the content; to end the
  string with a line feed, leave an empty line before the closing `"""`.
* Escape sequences are interpreted. A `\` at the end of a content line
  removes the line break, joining the next line (after removing its `m`
  spaces) to the current one.
* All other characters, including `#`, `"` and indicators, are literal content.
* Triple-quoted strings MUST NOT be used as mapping keys.

**Example:**

```
multi-line: """
  This string
  spans many lines.
  """

with-escapes: """
  Line one\nhas an escape.
  Line two is normal.
  """

folded: """
  This is all \
  on one line.
  """

code: """
  You can add "quotes" and 'quotes' here.
  x = foo(bar) # this is not a comment
  """
```

## Scalar Resolution

A quoted string (`c-double-quoted`) or triple-quoted string (`c-triple-quoted`)
is always a string regardless of its content.

A bare scalar in value position is resolved by trying the following in order;
the first match determines the type:

1. `ns-null`
2. `ns-bool`
3. `ns-integer`
4. `ns-float`
5. otherwise, the bare scalar is a string

Each pattern must match the entire bare scalar.

Mapping keys are never resolved: a bare key is always a string.

```
ns-scalar(n,c) ::=
    c-triple-quoted(n)
  | c-double-quoted
  | ns-null
  | ns-bool
  | ns-integer
  | ns-float
  | ns-bare-string(c)
```

### Reserved Words

The following bare words are interpreted as non-string types and MUST be
quoted to be used as string values:

| Word    | Type  |
| ------- | ----- |
| `null`  | null  |
| `true`  | bool  |
| `false` | bool  |
| `inf`   | float |
| `+inf`  | float |
| `-inf`  | float |
| `nan`   | float |

Matching is case-sensitive. There are no other implicit type conversions.
Words like `yes`, `no`, `on`, `off`, `NULL`, `True`, `NaN`, numbers with
leading zeros, and date-like strings such as `2001-01-23` are strings.

# Chapter: Collection Productions

## Block Sequence

A block sequence is a series of entries, each indicated by a `-` followed by
one or more spaces. Each entry begins at the same indentation `n`.

The entry value MUST begin on the same line as the `-`. A `-` followed by only
white space or a comment is an error (there are no empty nodes in AYML).

The entry value is one of:

* a scalar or flow collection,
* a _compact mapping_: a block mapping whose first entry begins on the same
  line as the `-`, or
* a _compact sequence_: a block sequence whose first entry begins on the same
  line as the `-`.

The indentation of a compact collection is the column at which its first entry
begins. Its remaining entries begin at that same indentation on subsequent
lines.

The entries of a block sequence that is a mapping value may be at the same
indentation as the mapping's keys ("implicit indentation").

```
l-block-sequence(n) ::=
    s-indent(n) c-l-block-seq-entry(n)
    ( l-gap* s-indent(n) c-l-block-seq-entry(n) )*

c-l-block-seq-entry(n) ::=
    c-sequence-entry s-space{k}          # k >= 1; let m = n + 1 + k
    ( ns-l-compact-sequence(m)
    | ns-l-compact-mapping(m)
    | ns-flow-node(n,BLOCK) s-l-end )

ns-l-compact-sequence(m) ::=
    c-l-block-seq-entry(m)
    ( l-gap* s-indent(m) c-l-block-seq-entry(m) )*

ns-l-compact-mapping(m) ::=
    ns-l-block-map-entry(m)
    ( l-gap* s-indent(m) ns-l-block-map-entry(m) )*
```

**Example:**

```
- Mark McGwire
- Sammy Sosa
- - nested
  - sequence
-   name: aligned
    value: compact mapping
```

## Block Mapping

A block mapping is a series of key/value pairs, each beginning at the same
indentation `n`. A mapping MUST NOT have duplicate keys.

A mapping key is a double-quoted string or a bare string, on a single line.
White space may appear between the key and the `:`. The `:` MUST be followed by
white space or the end of the line.

```
ns-mapping-key(c) ::= c-double-quoted | ns-bare-string(c)
```

A mapping value either begins on the same line as its key, or is a block
collection beginning on a later line:

* A same-line value is a scalar or a flow collection.
* A value on later lines MUST be a block collection. Its indentation `m` MUST
  satisfy `m > n` for a mapping, or `m >= n` for a sequence. Scalars and flow
  collections cannot begin on the line after their key.

```
ns-l-block-map-entry(n) ::=
    ns-mapping-key(BLOCK) s-white* c-mapping-value
    ( s-separate-in-line ns-flow-node(n,BLOCK) s-l-end
    | s-l-end l-gap*
      ( l-block-sequence(m)                          # m >= n
      | l-block-mapping(m) ) )                       # m > n

l-block-mapping(n) ::=
    s-indent(n) ns-l-block-map-entry(n)
    ( l-gap* s-indent(n) ns-l-block-map-entry(n) )*
```

**Example:**

```
hr:  65
avg: 0.278
rbi: 147
players:
  - Mark McGwire
  - Sammy Sosa
```

## Flow Collections

A flow sequence is a comma-separated list within square brackets `[]`. A flow
mapping is a comma-separated list of key/value pairs within curly braces `{}`.
Trailing commas are allowed. Flow collections may span multiple lines and may
contain comments.

Indentation is not significant inside flow collections, but every non-blank
line after the first MUST be indented with at least `n` spaces, where `n` is
the indentation of the block entry containing the flow collection (zero at the
top level).

```
s-flow-sep(n) ::=
    s-white
  | b-break l-blank* s-indent(m) [ lookahead ≠ s-white ]    # m >= n
  | s-separate-in-line c-nb-comment-text
  | <start-of-line> c-nb-comment-text    # after the indentation of a line

ns-flow-seq-entry(n) ::= ns-flow-node(n,FLOW)

c-flow-sequence(n) ::=
    c-sequence-start s-flow-sep(n)*
    (
        ns-flow-seq-entry(n) s-flow-sep(n)*
        ( c-collect-entry s-flow-sep(n)* ns-flow-seq-entry(n) s-flow-sep(n)* )*
        ( c-collect-entry s-flow-sep(n)* )?
    )?
    c-sequence-end

ns-flow-map-entry(n) ::=
    ns-mapping-key(FLOW) s-flow-sep(n)*
    c-mapping-value s-flow-sep(n)*
    ns-flow-node(n,FLOW)

c-flow-mapping(n) ::=
    c-mapping-start s-flow-sep(n)*
    (
        ns-flow-map-entry(n) s-flow-sep(n)*
        ( c-collect-entry s-flow-sep(n)* ns-flow-map-entry(n) s-flow-sep(n)* )*
        ( c-collect-entry s-flow-sep(n)* )?
    )?
    c-mapping-end
```

Inside a flow mapping, a bare key followed directly by `:` and a non-space
character continues the key (`{a:1}` has the single key `a:1` and no value,
and is therefore an error). Quoted keys may be followed directly by `:`
(`{"a":1}`).

**Example:**

```
- [name, hr, avg]
- [Mark McGwire, 65, 0.278]
```

```
Sammy Sosa: {
  hr: 63,   # home runs
  avg: 0.288,
}
```

# Chapter: Node and Document Productions

## Nodes

A flow node is a scalar or flow collection:

```
ns-flow-node(n,c) ::=
    c-flow-sequence(n)
  | c-flow-mapping(n)
  | ns-scalar(n,c)
```

A block node is a block collection or a flow node:

```
ns-l-block-node(n) ::=
    l-block-sequence(n)
  | l-block-mapping(n)
  | s-indent(n) ns-flow-node(n,BLOCK) s-l-end
```

## Document

An AYML file contains exactly one document. A document is a single root node
starting at column zero, surrounded by optional blank lines and comment lines.
A document with no root node is an error. There are no document markers: a
first line of `---` is a bare string.

```
l-ayml-document ::=
    l-gap*
    ns-l-block-node(0)
    l-gap*
    <end-of-input>
```

**Example:**

```
# Server configuration
host: localhost
port: 8080
```

# Chapter: Comment Association

This chapter is normative for processors that preserve comments. Every comment
in a valid document belongs to exactly one node, as one of three kinds:

* the node's **leading comment** — comment lines before the node,
* the node's **inline comment** — a comment at the end of the node's line, or
* the node's **trailing comment** — comment lines after the last entry of a
  collection, inside that collection.

Consecutive comment lines that form one leading or trailing comment are joined
with line feeds. A blank line between two comment lines of the same group is
represented as an empty line in the joined text; blank lines before the first
or after the last comment line of a group are not represented. An empty comment
(`#` alone) is also an empty line. Serializers emit empty lines as blank lines.

## Leading Comments

The comment lines between the previous entry (or the start of the document or
collection) and an entry belong to that entry's node. For a mapping entry, the
leading comment is attached to the entry's value node.

A root block collection, or a compact collection in a sequence entry, begins
on the line of its first entry and takes the comment lines before that line:
comment lines before a root block collection belong to the root node, not its
first entry, and comment lines before a sequence entry holding a compact
collection belong to the sequence entry. (A deserializer into typed data may
hand such a comment to the first entry when the outer node has no place for
it.) A block collection that is a mapping value begins on the line after its
key, so comment lines between the key and the first entry belong to that
entry.

## Inline Comments

A comment at the end of a line belongs to the node that ends on that line:

* `key: value # c` and `- value # c` — the scalar or flow collection `value`.
* `key: # c` followed by a nested block collection — that block collection.
* `"""  # c` closing a triple-quoted string — the string.
* In a flow collection, a comment after an entry (and its comma, if any) on
  the same line — that entry.

## Trailing Comments

A block collection with indentation `n` ends at the next content line with
indentation `k < n`, or at the end of the input (let `k = −1`). The gap between
the collection's last entry and that line is divided: the collection takes the
longest leading part of the gap in which every comment line has indentation at
least `n` and greater than `k`. The rest of the gap is processed by the
enclosing collection.

A block sequence at the same indentation as its parent mapping's keys
(implicit indentation) ends at the next mapping entry or at any line with less
indentation; it takes only comment lines indented more than its `-`
indicators (as if `k = n`).

Comment lines immediately before the `]` or `}` of a flow collection are that
collection's trailing comment. Inside a flow collection, every other comment
before an entry (including a comment after the opening bracket and comments
between a key and its value) is part of the entry's leading comment, attached
to the entry's value node. Comment lines after the root node that are not
claimed by a collection are the root node's trailing comment.

**Example:**

```
# leading comment of the root mapping
server:
  # leading comment of `host`
  host: localhost # inline comment of `localhost`
  # trailing comment of the `server` mapping
# leading comment of `ports`
ports: # inline comment of the `ports` sequence
  - 80
  - 443 # inline comment of `443`
  # trailing comment of the `ports` sequence
# trailing comment of the root mapping
```

# Chapter: Serialization

A serializer MUST produce a document that a conforming parser reads back as
the same data. In particular, a serializer:

* MUST quote a string that would otherwise be resolved as null, bool, int, or
  float, or that is not a valid bare string;
* MUST escape characters outside `c-printable`;
* MUST use triple-quoted or double-quoted strings for strings containing line
  breaks;
* MUST emit exactly one space after each block sequence `-`, and MUST nest a
  block sequence that is a mapping value two spaces deeper than its key;
* SHOULD escape characters that are invisible or that render as line breaks
  (such as `xFEFF`, NEL, `x2028`, and `x2029`).

# Chapter: Limits

A processor MAY limit the nesting depth of collections, the size of a document,
or the length of a scalar. A processor that imposes such a limit MUST report an
error when the limit is exceeded. The reference implementation limits nesting
depth to 128 by default.
