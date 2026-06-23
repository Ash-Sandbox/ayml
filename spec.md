# AYML version 1.0

**2026-03-07**

**Status of this Document**

This is the AYML specification v1.0.
It defines the AYML 1.0 data language.

## Abstract

AYML is a software configuration language that looks like YAML and acts like JSON.
It is meant to be a human-friendly, cross language, Unicode based software
configuration language. Unlike YAML, it is not designed as a fully featured
data serialization framework. There are no references, types, unordered sets,
duplicate nodes, document prefixes, or other complex features supported by YAML.

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

## Language Overview

This section provides an overview of AYML. 
The full specification follows in a later section.

### Collections

A block collection uses indentation for scope and begins each entry on its own line.
A block sequence indicates each entry with a dash and space `- `.
A mapping uses a colon and space `: ` to mark each key/value pair. A mapping may not have duplicate keys.
A comment begins with a `#`.

#### Mapping Key Ordering

AYML does not define an ordering for mapping keys. Implementations MAY preserve insertion order.

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

An AYML decoder MUST support 64-bit signed integers.

```
decimal: 12345
```

**Float**

An AYML decoder MUST support 64-bit (double precision) floating point numbers, including `inf` and `nan`:

```
canonical: 1.23015e+3
exponential: 12.3015e+02
fixed: 1230.15
negative infinity: -inf
not a number: nan
```

**Strings**

There are two kinds of single-line strings: bare and double-quoted.
For multi-line strings, use triple-quoted strings (`"""`).

```
unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
hex esc: "\x0d\x0a is \r\n"
howdy: "\"Howdy!\" he cried."
```

Triple-quoted strings (`"""`) are the only multi-line string format. The closing
`"""` indentation determines whitespace stripping. Escape sequences
are supported. A `\` at the end of a line suppresses the line break.

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

A comment is a form of documentation that may be associated with a particular AYML element.
By default, all comments are ignored during deserialization.
A deserializer may decide to allow comments; if so, a comment may be associated to:

* the node below it, if the comment is on its own line
* the node it follows, if the comment follows a node

Multi-line comments are allowed.

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

## BNF Grammar

This section defines the BNF grammar for AYML.
Whenever possible, basic structures are specified before the more complex
structures using them in a "bottom up" fashion.

Each rule is accompanied by one or more examples.

### Production Syntax

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
* `[ lookbehind = term ]`, which matches the empty string if `term` would match
  beginning at any prior point on the line and ending at the current position.

A special production:

* `<start-of-line>`, which matches the empty string at the beginning of a line.
* `<end-of-input>`, matches the empty string at the end of the input.
* `<empty>`, which (always) matches the empty string.

A parenthesized term matches its contents.

A concatenation is `term-one term-two`, which matches `term-one` followed by `term-two`.

An alternation is `term-one | term-two`, which matches the `term-one` if possible, or
`term-two` otherwise.

A quantified term:

* `term?`, which matches `(term | <empty>)`.
* `term*`, which matches `(term term* | <empty>)`.
* `term+`, which matches `(term term*)`.
* `term{n}`, which matches exactly `n` consecutive occurrences of `term`.

> Note: Quantified terms are always greedy.

The order of precedence is parenthesization, then quantification, then concatenation, then alternation.

Some lines in a production definition may have a comment like:

```
production-a ::=
  production-b      # clarifying comment
```

These comments are meant to be informative only.
For instance a comment that says `# not followed by non-ws char` just means
that you should be aware that actual production rules will behave as described
even though it might not be obvious from the content of that particular
production alone.


### Production Parameters

Some productions have parameters in parentheses after the name, such as `c-triple-quoted(n)`.
A parameterized production is shorthand for an (infinite) series of productions,
each with a fixed value for each parameter.

The parameters are as follows:

* `n` : The current indentation level. May be any non-negative integer.
* `m` : An auto-detected indentation level. See
  [Indentation Auto-Detection](#indentation-auto-detection).
* `c`: Context to distinguish between block and flow parsing. May be one of:
  * `BLOCK` -- inside a block collection
  * `FLOW` -- inside a flow collection


### Production Naming Conventions

To make it easier to follow production combinations, production names use a
prefix-style naming convention.

* `c-` : A production starting and ending with a special character.
* `b-` : A production matching a single line break.
* `nb-` : A production starting and ending with a non-break character.
* `s-` : A production starting and ending with a white space character.
* `ns-` : A production starting and ending with a non-space character.
* `l-` : A production matching complete line(s).


# Chapter: Character Productions

## Character Set

To ensure readability, AYML uses only the printable subset of the Unicode character set.

The allowed character range excludes the C0 control block `x00-x1F` 
(except for TAB `x09`, LF `x0A`, and CR `x0D` which are allowed),
DEL `x7F`, the C1 control block `x80-x9F` (except for NEL `x85` which is allowed), 
the surrogate block `xD800-xDFFF`, `xFFFE`, and `xFFFF`.
```
c-printable ::=
                         # 8 bit
    x09                  # Tab (\t)
  | x0A                  # Line feed (LF \n)
  | x0D                  # Carriage return (CR \r)
  | [x20-x7E]            # Printable ASCII
                         # 16 bit
  | x85                  # Next Line (NEL)
  | [xA0-xD7FF]          # Basic Multilingual Plane (BMP)
  | [xE000-xFFFD]        # Additional Unicode Areas
  | [x010000-x10FFFF]    # 32 bit
```


## Character Encoding

All characters in this specification are Unicode code points.
AYML files MUST be encoded in UTF-8. No other encodings are supported.

A byte order mark (BOM) MUST NOT appear in an AYML file.

## Line Break Characters

```
b-line-feed ::= x0A
b-carriage-return ::= x0D
b-char ::=
    b-line-feed
  | b-carriage-return
```

All other characters, including the form feed (`x0C`), are considered to be
non-break characters.

```
nb-char ::= c-printable - b-char
```

Line breaks are interpreted differently by different systems and have multiple
widely used formats.

```
b-break ::=
  ( b-carriage-return b-line-feed )    # CR LF
  | b-carriage-return                  # CR
  | b-line-feed                        # LF
```

Line breaks inside scalar content MUST be _normalized_ by the AYML processor.
Each such line break MUST be parsed into a single line feed character.
Outside scalar content, AYML allows any line break to be used to terminate lines.


## White Space Characters

AYML recognizes two _white space_ characters: _space_ and _tab_.

```
s-space ::= x20
s-tab ::= x09
s-white ::= s-space | s-tab
```

The rest of the printable non-break characters are considered to be
non-space characters.

```
ns-char ::= nb-char - s-white
```


## Miscellaneous Characters

A decimal digit is used for numbers:

```
ns-dec-digit ::= [x30-x39]             # 0-9
```

A hexadecimal digit is used for escape sequences:

```
ns-hex-digit ::=
    ns-dec-digit         # 0-9
  | [x41-x46]            # A-F
  | [x61-x66]            # a-f
```



## Indicator Characters

_Indicators_ are characters that have special semantics.

"`-`" (`x2D`, hyphen) denotes a block sequence entry.

```
c-sequence-entry ::= '-'
```

"`:`" (`x3A`, colon) denotes a mapping value.

```
c-mapping-value ::= ':'
```

"`,`" (`x2C`, comma) separates entries in a flow collection.

```
c-collect-entry ::= ','
```

"`[`" (`x5B`, left bracket) starts a flow sequence.

```
c-sequence-start ::= '['
```

"`]`" (`x5D`, right bracket) ends a flow sequence.

```
c-sequence-end ::= ']'
```

"`{`" (`x7B`, left brace) starts a flow mapping.

```
c-mapping-start ::= '{'
```

"`}`" (`x7D`, right brace) ends a flow mapping.

```
c-mapping-end ::= '}'
```

"`#`" (`x23`, octothorpe) denotes a comment.

```
c-comment ::= '#'
```

"`"`" (`x22`, double quote) surrounds a double-quoted scalar.

```
c-double-quote ::= '"'
```

"`\`" (`x5C`, backslash) begins an escape sequence in double-quoted and
triple-quoted scalars.

```
c-escape ::= '\'
```

> **Note:** AYML does not use the following YAML indicators: `?` (explicit
> mapping key), `&` (anchor), `*` (alias), `!` (tag), `>` (folded scalar),
> `%` (directive), `@`, or `` ` `` (reserved).

The union of all indicator characters:

```
c-indicator ::=
    c-sequence-entry     # '-'
  | c-mapping-value      # ':'
  | c-collect-entry      # ','
  | c-sequence-start     # '['
  | c-sequence-end       # ']'
  | c-mapping-start      # '{'
  | c-mapping-end        # '}'
  | c-comment            # '#'
  | c-double-quote       # '"'
  | c-escape             # '\'
```

Flow indicators are the subset that denote structure in flow collections:

```
c-flow-indicator ::=
    c-collect-entry      # ','
  | c-sequence-start     # '['
  | c-sequence-end       # ']'
  | c-mapping-start      # '{'
  | c-mapping-end        # '}'
```


## Escape Sequences

All non-printable characters MUST be _escaped_.
Escape sequences are interpreted in double-quoted and triple-quoted scalars.
In bare strings, the "`\`" character has no special meaning.

```
c-ns-esc-char ::=
    c-escape
    (
        '0'                   # Null (x00)
      | 'a'                   # Bell (x07)
      | 'b'                   # Backspace (x08)
      | 't'                   # Horizontal tab (x09)
      | 'n'                   # Line feed (x0A)
      | 'v'                   # Vertical tab (x0B)
      | 'f'                   # Form feed (x0C)
      | 'r'                   # Carriage return (x0D)
      | 'e'                   # Escape (x1B)
      | x20                   # Space (x20)
      | '"'                   # Double quote (x22)
      | '/'                   # Slash (x2F)
      | '\'                   # Backslash (x5C)
      | ( 'x' ns-hex-digit{2} )    # 8-bit Unicode
      | ( 'u' ns-hex-digit{4} )    # 16-bit Unicode
      | ( 'U' ns-hex-digit{8} )    # 32-bit Unicode
    )
```

Inside triple-quoted strings, a `\` immediately followed by a line break
suppresses that line break (line continuation). The next line's content is
joined directly to the current line.

```
c-escape-break ::= c-escape b-break
```


# Chapter: Indentation and Separation

## Indentation

AYML uses spaces for indentation. Tabs MUST NOT be used for indentation.
The indentation level is tracked as a parameter `n` representing the number
of leading spaces.

```
s-indent(n) ::= s-space{n}
```

```
s-indent-less-than(n) ::=
    s-space{m}    # where m < n
```

```
s-indent-less-or-equal(n) ::=
    s-space{m}    # where m <= n
```

### Indentation Auto-Detection

When a block collection or scalar value appears on a subsequent line (for
example, as the value of a mapping entry), the indentation level `m` is
_auto-detected_ from the first content line. The auto-detected level MUST
satisfy any stated constraint (e.g., `m > n`). All subsequent entries or
content lines within the same block MUST use the same indentation level `m`.

In the grammar, auto-detected indentation is written as a free variable `m`
with a constraint comment.

## Separation

A separation in line is one or more white space characters.

```
s-separate-in-line ::= s-white+
```

In flow collections, whitespace includes line breaks and comments:

```
s-flow-line-comment ::= s-separate-in-line c-nb-comment-text? b-break
```

```
s-flow-white ::= s-white | s-flow-line-comment | b-break
```


# Chapter: Comments

A comment is indicated by a `#` character. When a `#` appears preceded by
whitespace (or at the start of a line) in a non-scalar context, it begins a
comment that extends to the end of the line.

Inside triple-quoted strings (`"""`), all content including `#` characters
is literal string content, not comments.

```
c-nb-comment-text ::= c-comment nb-char*
```

An inline comment (trailing a node on the same line):

```
s-b-comment ::=
    ( s-separate-in-line c-nb-comment-text? )?
    b-break
```

A full-line comment:

```
l-comment(n) ::=
    s-indent-less-or-equal(n)
    c-nb-comment-text
    b-break
```

Consecutive comment lines form a multi-line comment:

```
l-comment-block(n) ::= l-comment(n)+
```


# Chapter: Scalar Productions

## Null

```
ns-null ::= "null"
```

**Example:**

```
optional field: null
```


## Boolean

```
ns-bool ::= "true" | "false"
```

**Example:**

```
is_good: true
is_bad: false
```


## Integer

An AYML decoder MUST support 64-bit signed integers.

```
ns-integer ::=
    ( '-' | '+' )? ns-dec-digit+                    # Decimal
```

**Example:**

```
decimal: 12345
negative: -9876
```


## Float

An AYML decoder MUST support 64-bit (double precision) floating point numbers.

```
ns-float ::=
    ( '-' | '+' )? ns-dec-digit+ '.' ns-dec-digit+
        ( ( 'e' | 'E' ) ( '-' | '+' )? ns-dec-digit+ )?    # Fixed/exponential
  | ( '-' | '+' )? ns-dec-digit+
        ( 'e' | 'E' ) ( '-' | '+' )? ns-dec-digit+         # Pure exponential
  | ( '-' | '+' )? "inf"                                   # Infinity
  | "nan"                                                  # Not a number
```

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
```

```
nb-double-one-line ::= nb-double-char*
```

```
c-double-quoted ::=
    c-double-quote
    nb-double-one-line
    c-double-quote
```

**Example:**

```
unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
```


## Bare String

A bare (unquoted) string has no delimiters. Its content is determined by
context. Bare strings are single-line only. They undergo scalar resolution —
if the content matches `null`, `true`, `false`, an integer, or a float
pattern, it is parsed as that type rather than as a string.

A `#` character is not allowed in bare strings. Use a quoted string for
values containing `#`.

In a **flow context**, bare strings are additionally terminated by flow
indicators (`,`, `]`, `}`).

```
ns-plain-first-char ::=
    ( ns-char - c-indicator )
  | ( ( '-' | ':' ) [ lookahead = ns-char ] )
```

```
ns-plain-char(BLOCK) ::=
    ( ns-char - c-comment - c-mapping-value )
  | ( ':' [ lookahead = ns-char ] )          # ':' not followed by space
```

```
ns-plain-char(FLOW) ::=
    ns-plain-char(BLOCK)
  - c-flow-indicator
```

```
ns-bare-string(c) ::=
    ns-plain-first-char
    ( s-white* ns-plain-char(c) )*
```

**Example:**

```
plain: This unquoted scalar is single-line.
url: https://example.com/path
```


## Triple-Quoted String

A triple-quoted string begins with `"""` followed by a line break, and ends
with `"""` on its own line. The indentation of the closing `"""` determines
the whitespace stripping level — that many leading spaces are removed from
each content line (like Swift's multi-line string literals).

Escape sequences are supported (same as double-quoted strings). A `\` at the
end of a line suppresses the line break, joining the next line directly to
the current one.

All content — including `#` characters, quotes, and other indicator
characters — is literal string content (not comments or structure).

```
c-triple-quote ::= '"""'
```

```
nb-triple-char ::=
    c-ns-esc-char
  | ( nb-char - c-escape )
```

The indentation level `m` is determined by the indentation of the closing
`"""`. Each content line MUST begin with at least `m` leading spaces, which
are stripped. Blank lines (empty or containing only whitespace) are preserved
as empty lines and need not have `m` spaces. A `\` at the end of a line
(before the line break) suppresses the line break; the next line's content
(after stripping `m` leading spaces) is joined directly to the current line.

```
l-triple-content-line(m) ::=
    s-indent(m) nb-triple-char*
    ( c-escape-break s-indent(m) nb-triple-char* )*
```

```
l-triple-blank-line ::= s-space*
```

```
c-triple-quoted ::=
    c-triple-quote b-break
    ( ( l-triple-content-line(m) | l-triple-blank-line ) b-break )*
    s-indent(m) c-triple-quote
```

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

When parsing an unquoted (bare) scalar value, the parser SHOULD attempt to
match in this order:

1. `ns-null`
2. `ns-bool`
3. `ns-integer`
4. `ns-float`
5. `ns-bare-string`

A quoted string (`c-double-quoted`) or triple-quoted string
(`c-triple-quoted`) is always a string regardless of its content.

### Reserved Words

The following bare words are interpreted as non-string types and MUST be
quoted to be used as strings:

| Word    | Type  |
| ------- | ----- |
| `null`  | null  |
| `true`  | bool  |
| `false` | bool  |
| `inf`   | float |
| `+inf`  | float |
| `-inf`  | float |
| `nan`   | float |

There are no other implicit type conversions.
Words like `yes`, `no`, `on`, `off`, and date-like strings
such as `2001-01-23` are parsed as strings.

```
ns-scalar(c) ::=
    c-double-quoted
  | c-triple-quoted
  | ns-null
  | ns-bool
  | ns-integer
  | ns-float
  | ns-bare-string(c)
```


# Chapter: Collection Productions

## Block Gaps

Between block collection entries, blank lines and comment lines are allowed.
These are collectively called _gaps_.

```
l-block-blank-line ::= s-white* b-break
```

```
l-block-gap(n) ::=
    l-block-blank-line
  | l-comment(n)
```

## Block Sequence

A block sequence is a series of entries, each indicated by a `- ` (dash
followed by a space). Each entry begins at the same indentation level.

A block sequence entry indicator `- ` may provide implicit indentation for
the entry. An AYML deserializer MUST accept a block sequence entry with
regular or implicit indentation. An AYML serializer MUST produce a block
sequence entry with regular indentation.

The entry value MUST begin on the same line as `- `. A `- ` followed by
only whitespace and a line break is a parse error (there are no empty nodes
in AYML). To nest block collections inside a sequence, use flow syntax
(`[]`, `{}`) or compact mapping notation.

When a sequence entry contains a mapping, the first mapping entry may appear
on the same line as `- ` (compact notation). The remaining entries appear on
subsequent lines at indentation `n+2`.

```
ns-compact-mapping-entry(n) ::=
    ns-mapping-key(BLOCK)
    s-white*
    c-mapping-value
    (
        s-white+ ns-flow-node(BLOCK)                  # Value on same line
      | s-b-comment                                    # Value on next line(s)
        ( l-block-sequence(m)                          # Sequence, m >= n
        | l-block-mapping(m)                           # Nested mapping, m > n
        | s-indent(m) ns-flow-node(BLOCK)              # Scalar/flow value, m > n
        )
    )
```

```
ns-compact-mapping(n) ::=
    ns-compact-mapping-entry(n)
    ( b-break l-block-gap(n)* l-block-mapping-entry(n) )*
```

```
l-block-seq-entry(n) ::=
    s-indent(n)
    c-sequence-entry s-white
    ( ns-compact-mapping(n+2)                    # Compact mapping
    | ns-flow-node(BLOCK)                        # Scalar or flow collection
    )
```

```
l-block-sequence(n) ::=
    l-block-seq-entry(n)
    ( b-break l-block-gap(n)* l-block-seq-entry(n) )*
```

**Example:**

```
- Mark McGwire
- Sammy Sosa
- Ken Griffey
```


## Block Mapping

A block mapping is a series of key/value pairs.
A mapping MUST NOT have duplicate keys (this is a semantic constraint enforced
by the parser, not expressible in the grammar).

A mapping key is a single-line scalar: a bool, integer, or string.
Float and null keys are not allowed. Triple-quoted strings are not
allowed as mapping keys because they span multiple lines.

```
ns-mapping-key(c) ::=
    c-double-quoted
  | ns-bool
  | ns-integer
  | ns-bare-string(c)
```

A mapping key MUST NOT resolve to null or float. To use the string `"null"` as
a key, quote it.

A mapping entry value may appear on the same line as its key, or on the next line.
If the value is on the next line, its indentation level `m` is auto-detected and
MUST satisfy `m > n`, except for sequences where `m >= n` is permitted due to
implicit indentation.

```
l-block-mapping-entry(n) ::=
    s-indent(n)
    ns-mapping-key(BLOCK)
    s-white*
    c-mapping-value
    (
        s-white+ ns-flow-node(BLOCK)                  # Value on same line
      | s-b-comment                                    # Value on next line(s)
        ( l-block-sequence(m)                          # Sequence, m >= n
        | l-block-mapping(m)                           # Nested mapping, m > n
        | s-indent(m) ns-flow-node(BLOCK)              # Scalar/flow value, m > n
        )
    )
```

```
l-block-mapping(n) ::=
    l-block-mapping-entry(n)
    ( b-break l-block-gap(n)* l-block-mapping-entry(n) )*
```

**Example:**

```
hr:  65
avg: 0.278
rbi: 147
```


## Flow Sequence

A flow sequence is a comma-separated list within square brackets `[]`.
Trailing commas are allowed. Flow sequences may span multiple lines.
Indentation is not significant inside flow collections.

```
ns-flow-seq-entry ::= ns-flow-node(FLOW)
```

```
c-flow-sequence ::=
    c-sequence-start
    s-flow-white*
    (
        ns-flow-seq-entry
        ( s-flow-white* c-collect-entry s-flow-white* ns-flow-seq-entry )*
        ( s-flow-white* c-collect-entry )?     # Optional trailing comma
    )?
    s-flow-white*
    c-sequence-end
```

**Example:**

```
- [name, hr, avg]
- [Mark McGwire, 65, 0.278]
```


## Flow Mapping

A flow mapping is a comma-separated list of key/value pairs within curly
braces `{}`. Trailing commas are allowed. Flow mappings may span multiple
lines.

```
ns-flow-mapping-entry ::=
    ns-mapping-key(FLOW)
    s-flow-white*
    c-mapping-value
    s-flow-white*
    ns-flow-node(FLOW)
```

```
c-flow-mapping ::=
    c-mapping-start
    s-flow-white*
    (
        ns-flow-mapping-entry
        ( s-flow-white* c-collect-entry s-flow-white* ns-flow-mapping-entry )*
        ( s-flow-white* c-collect-entry )?     # Optional trailing comma
    )?
    s-flow-white*
    c-mapping-end
```

**Example:**

```
Mark McGwire: {hr: 65, avg: 0.278}
Sammy Sosa: {
  hr: 63,
  avg: 0.288,
}
```


# Chapter: Node and Document Productions

## Nodes

A flow node is a scalar or flow collection:

```
ns-flow-node(c) ::=
    ns-scalar(c)
  | c-flow-sequence
  | c-flow-mapping
```

A block node is a scalar, flow collection, or block collection:

```
ns-block-node(n) ::=
    l-block-sequence(n)
  | l-block-mapping(n)
  | ns-flow-node(BLOCK)
```

> Note: Block collections are tried before flow nodes so that a mapping
> key followed by `: ` is not consumed as a bare string.


## Document

An AYML file contains exactly one document. A document is optional leading
comments, a single root node, and optional trailing comments or blank lines:

```
l-ayml-document ::=
    l-comment-block(0)?
    ns-block-node(0)
    ( b-break l-block-gap(0)* )?
    <end-of-input>
```

**Example:**

```
# Server configuration
host: localhost
port: 8080
```
