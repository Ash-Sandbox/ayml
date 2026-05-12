// ── Block Sequences ──────────────────────────────────────────────

use ayml_core::{
    parse,
    value::{MapKey, Value},
};

#[test]
fn sequence_of_scalars() {
    let input = "- Mark McGwire\n- Sammy Sosa\n- Ken Griffey";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq.len(), 3);
    assert_eq!(seq[0].value, Value::Str("Mark McGwire".into()));
    assert_eq!(seq[1].value, Value::Str("Sammy Sosa".into()));
    assert_eq!(seq[2].value, Value::Str("Ken Griffey".into()));
}

#[test]
fn sequence_of_integers() {
    let input = "- 1\n- 2\n- 3";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq.len(), 3);
    assert_eq!(seq[0].value, Value::Int(1));
    assert_eq!(seq[1].value, Value::Int(2));
    assert_eq!(seq[2].value, Value::Int(3));
}

#[test]
fn sequence_of_mixed() {
    let input = "- hello\n- 42\n- true\n- null\n- 3.25";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq.len(), 5);
    assert_eq!(seq[0].value, Value::Str("hello".into()));
    assert_eq!(seq[1].value, Value::Int(42));
    assert_eq!(seq[2].value, Value::Bool(true));
    assert!(seq[3].value.is_null());
    assert_eq!(seq[4].value, Value::Float(3.25));
}

// ── Block Mappings ───────────────────────────────────────────────

#[test]
fn mapping_scalars_to_scalars() {
    let input = "hr: 65\navg: 0.278\nrbi: 147";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("hr".into())].value, Value::Int(65));
    assert_eq!(
        map[&MapKey::String("avg".into())].value,
        Value::Float(0.278)
    );
    assert_eq!(map[&MapKey::String("rbi".into())].value, Value::Int(147));
}

#[test]
fn mapping_scalars_to_sequences() {
    let input = "\
american:
- Boston Red Sox
- Detroit Tigers
- New York Yankees
national:
- New York Mets
- Chicago Cubs
- Atlanta Braves";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    let american = map[&MapKey::String("american".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(american.len(), 3);
    assert_eq!(american[0].value, Value::Str("Boston Red Sox".into()));

    let national = map[&MapKey::String("national".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(national.len(), 3);
    assert_eq!(national[0].value, Value::Str("New York Mets".into()));
}

#[test]
fn sequence_of_mappings() {
    let input = "\
- name: Mark McGwire
  hr:   65
  avg:  0.278
- name: Sammy Sosa
  hr:   63
  avg:  0.288";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq.len(), 2);

    let m0 = seq[0].value.as_mapping().unwrap();
    assert_eq!(
        m0[&MapKey::String("name".into())].value,
        Value::Str("Mark McGwire".into())
    );
    assert_eq!(m0[&MapKey::String("hr".into())].value, Value::Int(65));
    assert_eq!(m0[&MapKey::String("avg".into())].value, Value::Float(0.278));

    let m1 = seq[1].value.as_mapping().unwrap();
    assert_eq!(
        m1[&MapKey::String("name".into())].value,
        Value::Str("Sammy Sosa".into())
    );
    assert_eq!(m1[&MapKey::String("hr".into())].value, Value::Int(63));
}

#[test]
fn nested_mapping() {
    let input = "\
bill-to:
  given: Chris
  family: Dumars
  address:
    city: Royal Oak
    state: MI";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let bill_to = map[&MapKey::String("bill-to".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(
        bill_to[&MapKey::String("given".into())].value,
        Value::Str("Chris".into())
    );

    let address = bill_to[&MapKey::String("address".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(
        address[&MapKey::String("city".into())].value,
        Value::Str("Royal Oak".into())
    );
}

#[test]
fn compact_nested_mapping() {
    let input = "\
- item: Super Hoop
  quantity: 1
- item: Basketball
  quantity: 4
- item: Big Shoes
  quantity: 1";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq.len(), 3);

    let m0 = seq[0].value.as_mapping().unwrap();
    assert_eq!(
        m0[&MapKey::String("item".into())].value,
        Value::Str("Super Hoop".into())
    );
    assert_eq!(m0[&MapKey::String("quantity".into())].value, Value::Int(1));
}

// ── Flow Sequences ───────────────────────────────────────────────

#[test]
fn flow_sequence_simple() {
    let input = "items: [name, hr, avg]";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let seq = map[&MapKey::String("items".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(seq.len(), 3);
    assert_eq!(seq[0].value, Value::Str("name".into()));
    assert_eq!(seq[1].value, Value::Str("hr".into()));
    assert_eq!(seq[2].value, Value::Str("avg".into()));
}

#[test]
fn flow_sequence_with_types() {
    let input = "items: [Mark McGwire, 65, 0.278]";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let seq = map[&MapKey::String("items".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(seq[0].value, Value::Str("Mark McGwire".into()));
    assert_eq!(seq[1].value, Value::Int(65));
    assert_eq!(seq[2].value, Value::Float(0.278));
}

#[test]
fn flow_sequence_trailing_comma() {
    let input = "items: [a, b, c,]";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let seq = map[&MapKey::String("items".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(seq.len(), 3);
}

#[test]
fn flow_sequence_empty() {
    let input = "items: []";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let seq = map[&MapKey::String("items".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(seq.len(), 0);
}

#[test]
fn flow_sequence_nested() {
    let input = "\
- [name, hr, avg]
- [Mark McGwire, 65, 0.278]
- [Sammy Sosa, 63, 0.288]";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq.len(), 3);

    let row0 = seq[0].value.as_sequence().unwrap();
    assert_eq!(row0.len(), 3);
    assert_eq!(row0[0].value, Value::Str("name".into()));
}

// ── Flow Mappings ────────────────────────────────────────────────

#[test]
fn flow_mapping_simple() {
    let input = "player: {hr: 65, avg: 0.278}";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let inner = map[&MapKey::String("player".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(inner[&MapKey::String("hr".into())].value, Value::Int(65));
    assert_eq!(
        inner[&MapKey::String("avg".into())].value,
        Value::Float(0.278)
    );
}

#[test]
fn flow_mapping_trailing_comma() {
    let input = "player: {hr: 65, avg: 0.278,}";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let inner = map[&MapKey::String("player".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(inner.len(), 2);
}

#[test]
fn flow_mapping_multiline() {
    let input = "player: {\n  hr: 63,\n  avg: 0.288,\n}";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let inner = map[&MapKey::String("player".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(inner[&MapKey::String("hr".into())].value, Value::Int(63));
}

#[test]
fn flow_mapping_empty() {
    let input = "player: {}";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let inner = map[&MapKey::String("player".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(inner.len(), 0);
}

// ── Mapping Key Types ────────────────────────────────────────────

#[test]
fn bool_mapping_key() {
    let input = "true: yes\nfalse: no";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::Bool(true)].value, Value::Str("yes".into()));
    assert_eq!(map[&MapKey::Bool(false)].value, Value::Str("no".into()));
}

#[test]
fn int_mapping_key() {
    let input = "1: first\n2: second";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::Int(1)].value, Value::Str("first".into()));
    assert_eq!(map[&MapKey::Int(2)].value, Value::Str("second".into()));
}

#[test]
fn quoted_null_key() {
    let input = "\"null\": a value";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("null".into())].value,
        Value::Str("a value".into())
    );
}

// ── CRLF and CR line endings ─────────────────────────────────────

#[test]
fn crlf_line_endings_in_mapping() {
    let input = "a: 1\r\nb: 2\r\nc: 3";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("a".into())].value, Value::Int(1));
    assert_eq!(map[&MapKey::String("b".into())].value, Value::Int(2));
    assert_eq!(map[&MapKey::String("c".into())].value, Value::Int(3));
}

#[test]
fn cr_line_endings_in_mapping() {
    let input = "a: 1\rb: 2\rc: 3";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("a".into())].value, Value::Int(1));
    assert_eq!(map[&MapKey::String("b".into())].value, Value::Int(2));
    assert_eq!(map[&MapKey::String("c".into())].value, Value::Int(3));
}

#[test]
fn crlf_line_endings_in_sequence() {
    let input = "- one\r\n- two\r\n- three";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq[0].value, Value::Str("one".into()));
    assert_eq!(seq[1].value, Value::Str("two".into()));
    assert_eq!(seq[2].value, Value::Str("three".into()));
}

#[test]
fn crlf_in_nested_mapping() {
    let input = "outer:\r\n  inner: val";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let inner = map[&MapKey::String("outer".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(
        inner[&MapKey::String("inner".into())].value,
        Value::Str("val".into())
    );
}
