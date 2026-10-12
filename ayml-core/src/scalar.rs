//! Lexical rules for scalars, shared by the parser and the emitter.

use std::borrow::Cow;

/// Block or flow context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Block,
    Flow,
}

/// `c-printable`.
#[must_use]
pub const fn is_printable(ch: char) -> bool {
    matches!(ch as u32,
        0x09 | 0x0A | 0x0D
        | 0x20..=0x7E
        | 0x85
        | 0xA0..=0xD7FF
        | 0xE000..=0xFFFD
        | 0x1_0000..=0x10_FFFF
    )
}

/// `c-indicator`.
#[must_use]
pub const fn is_indicator(ch: char) -> bool {
    matches!(
        ch,
        '-' | ':' | ',' | '[' | ']' | '{' | '}' | '#' | '"' | '\\'
    )
}

/// `c-flow-indicator`.
#[must_use]
pub const fn is_flow_indicator(ch: char) -> bool {
    matches!(ch, ',' | '[' | ']' | '{' | '}')
}

/// `ns-plain-safe(c)`.
#[must_use]
pub const fn is_plain_safe(ch: char, ctx: Context) -> bool {
    let ns_char = is_printable(ch) && !matches!(ch, ' ' | '\t' | '\n' | '\r');
    ns_char && ch != '#' && !(matches!(ctx, Context::Flow) && is_flow_indicator(ch))
}

/// `ns-plain-first-char(c)`; `next` is the character after `ch`.
#[must_use]
pub fn is_plain_first(ch: char, next: Option<char>, ctx: Context) -> bool {
    if ch == '-' || ch == ':' {
        next.is_some_and(|n| is_plain_safe(n, ctx))
    } else {
        is_plain_safe(ch, ctx) && !is_indicator(ch) && ch != '\''
    }
}

/// `ns-plain-char(c)`; `next` is the character after `ch`.
#[must_use]
pub fn is_plain_char(ch: char, next: Option<char>, ctx: Context) -> bool {
    if ch == ':' {
        next.is_some_and(|n| is_plain_safe(n, ctx))
    } else {
        is_plain_safe(ch, ctx)
    }
}

/// Byte classes for scanning bare strings. Bytes at or above 0x80 belong to
/// non-ASCII characters, which are plain-safe whenever they are printable.
const PLAIN_SAFE_BLOCK: u8 = 1;
const PLAIN_SAFE_FLOW: u8 = 2;

const fn plain_class_table() -> [u8; 256] {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        #[allow(clippy::cast_possible_truncation)] // i < 256
        let b = i as u8;
        let safe_block = b >= 0x80 || (b >= 0x21 && b <= 0x7E && b != b'#');
        if safe_block {
            table[i] |= PLAIN_SAFE_BLOCK;
            if !matches!(b, b',' | b'[' | b']' | b'{' | b'}') {
                table[i] |= PLAIN_SAFE_FLOW;
            }
        }
        i += 1;
    }
    table
}

static PLAIN_CLASS: [u8; 256] = plain_class_table();

/// Scan a bare string (`ns-bare-string(c)`) in `input` starting at byte
/// offset `start`. Returns the end offset.
#[must_use]
pub fn scan_bare(input: &str, start: usize, ctx: Context) -> Option<usize> {
    let b = input.as_bytes();
    let mask = match ctx {
        Context::Block => PLAIN_SAFE_BLOCK,
        Context::Flow => PLAIN_SAFE_FLOW,
    };
    let safe = |i: usize| {
        b.get(i)
            .is_some_and(|&c| PLAIN_CLASS[c as usize] & mask != 0)
    };
    let first = *b.get(start)?;
    let first_ok = match first {
        b'-' | b':' => safe(start + 1),
        b'\'' => false,
        c if c < 0x80 => safe(start) && !is_indicator(c as char),
        _ => true,
    };
    if !first_ok {
        return None;
    }
    let mut end = start + 1;
    let mut i = end;
    loop {
        while i < b.len() && b[i] != b':' && PLAIN_CLASS[b[i] as usize] & mask != 0 {
            i += 1;
        }
        if i > end {
            end = i;
        }
        match b.get(i) {
            Some(b':') if safe(i + 1) => {
                i += 1;
                end = i;
            }
            Some(b' ' | b'\t') => {
                let mut j = i + 1;
                while matches!(b.get(j), Some(b' ' | b'\t')) {
                    j += 1;
                }
                match b.get(j) {
                    Some(b':') if safe(j + 1) => {
                        i = j + 1;
                        end = i;
                    }
                    Some(&c) if c != b':' && PLAIN_CLASS[c as usize] & mask != 0 => i = j,
                    _ => return Some(end),
                }
            }
            _ => return Some(end),
        }
    }
}

/// A scalar value, borrowing its string where possible.
#[derive(Debug, Clone, PartialEq)]
pub enum Scalar<'a> {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Cow<'a, str>),
}

/// An error resolving a bare scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveError {
    IntegerOverflow,
    FloatOverflow,
}

/// Resolve the text of a bare scalar in value position: `null`, `true`,
/// `false`, an integer, a float, or else a string.
pub fn resolve(text: &str) -> Result<Scalar<'_>, ResolveError> {
    Ok(match text {
        "null" => Scalar::Null,
        "true" => Scalar::Bool(true),
        "false" => Scalar::Bool(false),
        // Numbers start with a digit or sign, or are `inf` or `nan`.
        _ if text
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'i' | b'n')) =>
        {
            if let Some(int) = parse_int(text) {
                Scalar::Int(int.map_err(|()| ResolveError::IntegerOverflow)?)
            } else if let Some(float) = parse_float(text) {
                Scalar::Float(float.map_err(|()| ResolveError::FloatOverflow)?)
            } else {
                Scalar::Str(Cow::Borrowed(text))
            }
        }
        _ => Scalar::Str(Cow::Borrowed(text)),
    })
}

/// Parse `text` as an AYML integer (`ns-integer`).
#[must_use]
pub fn parse_int(text: &str) -> Option<Result<i64, ()>> {
    let (negative, unsigned) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(d) = unsigned.strip_prefix("0x") {
        (16, d)
    } else if let Some(d) = unsigned.strip_prefix("0o") {
        (8, d)
    } else if let Some(d) = unsigned.strip_prefix("0b") {
        (2, d)
    } else {
        // Decimal: no leading zeros.
        if unsigned.len() > 1 && unsigned.starts_with('0') {
            return None;
        }
        (10, unsigned)
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let mut magnitude: u64 = 0;
    for c in digits.chars() {
        let d = u64::from(c.to_digit(radix).unwrap_or(0));
        let Some(next) = magnitude
            .checked_mul(u64::from(radix))
            .and_then(|m| m.checked_add(d))
        else {
            return Some(Err(()));
        };
        magnitude = next;
    }
    let value = if negative {
        0i64.checked_sub_unsigned(magnitude)
    } else {
        i64::try_from(magnitude).ok()
    };
    Some(value.ok_or(()))
}

/// Parse `text` as an AYML float (`ns-float`).
#[must_use]
pub fn parse_float(text: &str) -> Option<Result<f64, ()>> {
    match text {
        "inf" | "+inf" => return Some(Ok(f64::INFINITY)),
        "-inf" => return Some(Ok(f64::NEG_INFINITY)),
        "nan" => return Some(Ok(f64::NAN)),
        _ => {}
    }
    let b = text.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'-' | b'+')));
    // ns-dec-int
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_len = i - int_start;
    if int_len == 0 || (int_len > 1 && b[int_start] == b'0') {
        return None;
    }
    let mut has_fraction_or_exponent = false;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return None;
        }
        has_fraction_or_exponent = true;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return None;
        }
        has_fraction_or_exponent = true;
    }
    if i != b.len() || !has_fraction_or_exponent {
        return None;
    }
    let value: f64 = text.parse().ok()?;
    Some(if value.is_finite() {
        Ok(value)
    } else {
        Err(())
    })
}

/// Append a float in AYML syntax to `out`.
pub fn write_float(out: &mut String, f: f64) {
    if f.is_nan() {
        out.push_str("nan");
    } else if f.is_infinite() {
        out.push_str(if f.is_sign_negative() { "-inf" } else { "inf" });
    } else {
        out.push_str(ryu::Buffer::new().format_finite(f));
    }
}

/// An error decoding an escape sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapeError {
    pub message: String,
    /// Byte offset just past the malformed escape.
    pub end: usize,
}

/// Decode the escape sequence whose `\` is at byte offset `pos` in `input`.
/// Returns the decoded character and the offset just past the escape.
pub fn decode_escape(input: &str, pos: usize) -> Result<(char, usize), EscapeError> {
    let mut chars = input[pos + 1..].chars();
    let Some(ch) = chars.next() else {
        return Err(EscapeError {
            message: "`\\` at end of input".into(),
            end: pos + 1,
        });
    };
    let after = pos + 1 + ch.len_utf8();
    let simple = match ch {
        '0' => '\0',
        'a' => '\x07',
        'b' => '\x08',
        't' => '\t',
        'n' => '\n',
        'v' => '\x0B',
        'f' => '\x0C',
        'r' => '\r',
        'e' => '\x1B',
        ' ' => ' ',
        '"' => '"',
        '/' => '/',
        '\\' => '\\',
        'x' | 'u' | 'U' => {
            let digits = match ch {
                'x' => 2,
                'u' => 4,
                _ => 8,
            };
            let hex = input
                .get(after..after + digits)
                .filter(|h| h.len() == digits && h.bytes().all(|b| b.is_ascii_hexdigit()));
            let Some(hex) = hex else {
                return Err(EscapeError {
                    message: format!("`\\{ch}` must be followed by {digits} hex digits"),
                    end: after,
                });
            };
            let code = u32::from_str_radix(hex, 16).unwrap_or(u32::MAX);
            let Some(decoded) = char::from_u32(code) else {
                return Err(EscapeError {
                    message: format!(
                        "`\\{ch}{hex}` is not a Unicode scalar value (surrogates and values above 10FFFF are not allowed)"
                    ),
                    end: after + digits,
                });
            };
            return Ok((decoded, after + digits));
        }
        other => {
            let shown = if other == '\n' || other == '\r' {
                "a line break".to_owned()
            } else {
                format!("`\\{other}`")
            };
            return Err(EscapeError {
                message: format!("unknown escape {shown}"),
                end: after,
            });
        }
    };
    Ok((simple, after))
}

/// Characters that are escaped even though they may appear literally.
#[must_use]
pub const fn is_confusable(ch: char) -> bool {
    matches!(ch, '\u{85}' | '\u{FEFF}' | '\u{2028}' | '\u{2029}')
}

/// True if `s` can be written as a bare mapping key in context `ctx` and
/// read back as `s`. Keys are never resolved, so `true` and `42` are valid
/// bare keys.
///
/// Beyond what the grammar requires, this rejects tabs and invisible or
/// confusable characters, which an emitter should make visible by quoting.
#[must_use]
pub fn is_bare_key(s: &str, ctx: Context) -> bool {
    let b = s.as_bytes();
    let mask = match ctx {
        Context::Block => PLAIN_SAFE_BLOCK,
        Context::Flow => PLAIN_SAFE_FLOW,
    };
    let safe = |c: u8| c < 0x80 && PLAIN_CLASS[c as usize] & mask != 0;
    let (Some(&first), Some(&last)) = (b.first(), b.last()) else {
        return false;
    };
    if first >= 0x80 || last >= 0x80 {
        return is_bare_unicode(s, ctx);
    }
    let first_ok = match first {
        b'-' | b':' => b.get(1).is_some_and(|&n| n >= 0x80 || safe(n)),
        c => safe(c) && !is_indicator(c as char) && c != b'\'',
    };
    if !first_ok || last == b' ' {
        return false;
    }
    let mut i = 1;
    while i < b.len() {
        // Runs of plain characters other than `:` and space.
        while i < b.len() && b[i] != b':' && PLAIN_CLASS[b[i] as usize] & mask != 0 && b[i] < 0x80 {
            i += 1;
        }
        if i == b.len() {
            break;
        }
        match b[i] {
            b' ' => {}
            b':' => {
                if !b.get(i + 1).is_some_and(|&n| n >= 0x80 || safe(n)) {
                    return false;
                }
            }
            c if c >= 0x80 => return is_bare_unicode(s, ctx),
            c => {
                if !safe(c) {
                    return false;
                }
            }
        }
        i += 1;
    }
    true
}

/// True if `s` can be written as a bare scalar in value position in context
/// `ctx` and read back as the string `s`.
#[must_use]
pub fn is_bare_value(s: &str, ctx: Context) -> bool {
    is_bare_key(s, ctx) && matches!(resolve(s), Ok(Scalar::Str(_)))
}

/// [`is_bare_key`] for strings with non-ASCII characters, which
/// [`scan_bare`] does not inspect.
fn is_bare_unicode(s: &str, ctx: Context) -> bool {
    let visible_ends = |c: Option<char>| c.is_some_and(|c| !c.is_whitespace());
    visible_ends(s.chars().next())
        && visible_ends(s.chars().next_back())
        && !s.contains(['\t', '\n', '\r'])
        && s.chars().all(|c| is_printable(c) && !is_confusable(c))
        && scan_bare(s, 0, ctx) == Some(s.len())
}
