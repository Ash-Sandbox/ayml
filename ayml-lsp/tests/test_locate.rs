use ayml_core::parse;

use ayml_lsp::locate;

/// Path at the first occurrence of `needle` in `input`, plus `delta` bytes.
fn path_at(input: &str, needle: &str, delta: usize) -> Vec<String> {
    let node = parse(input).unwrap();
    let offset = input.find(needle).expect("needle") + delta;
    locate::path_at_offset(&node, offset)
}

#[test]
fn top_level_key_and_value() {
    let input = "name: Alice\nage: 30";
    assert_eq!(path_at(input, "name", 0), ["name"]);
    assert_eq!(path_at(input, "name", 3), ["name"]);
    assert_eq!(path_at(input, ":", 0), ["name"]); // the colon after the key
    assert_eq!(path_at(input, "Alice", 0), ["name"]);
    assert_eq!(path_at(input, "age", 0), ["age"]);
    assert_eq!(path_at(input, "30", 1), ["age"]);
}

#[test]
fn nested_key() {
    let input = "network:\n  rules: ok";
    assert_eq!(path_at(input, "network", 0), ["network"]);
    assert_eq!(path_at(input, "rules", 0), ["network", "rules"]);
    assert_eq!(path_at(input, "ok", 0), ["network", "rules"]);
}

#[test]
fn deeply_nested_key() {
    let input = "network:\n  rules:\n    host: x";
    assert_eq!(path_at(input, "rules", 2), ["network", "rules"]);
    assert_eq!(path_at(input, "host", 0), ["network", "rules", "host"]);
    assert_eq!(path_at(input, "x", 0), ["network", "rules", "host"]);
}

#[test]
fn sequence_element() {
    let input = "items:\n  - foo\n  - bar";
    assert_eq!(path_at(input, "foo", 0), ["items", "0"]);
    assert_eq!(path_at(input, "bar", 2), ["items", "1"]);
}

#[test]
fn compact_mapping_in_sequence() {
    let input = "rules:\n  - host: a\n    port: 1\n  - host: b\n";
    assert_eq!(path_at(input, "port", 0), ["rules", "0", "port"]);
    assert_eq!(path_at(input, "host: b", 0), ["rules", "1", "host"]);
    assert_eq!(path_at(input, "b\n", 0), ["rules", "1", "host"]);
}

#[test]
fn quoted_key_and_flow_mapping() {
    let input = "\"a key\": {x: 1, \"y\": [2, 3]}";
    assert_eq!(path_at(input, "a key", 0), ["a key"]);
    assert_eq!(path_at(input, "x", 0), ["a key", "x"]);
    assert_eq!(path_at(input, "\"y\"", 0), ["a key", "y"]);
    assert_eq!(path_at(input, "3", 0), ["a key", "y", "1"]);
}

#[test]
fn non_ascii_text_before_cursor() {
    let input = "ключ: значение\nимя:\n  фамилия: Иванов\n";
    assert_eq!(path_at(input, "значение", 0), ["ключ"]);
    assert_eq!(path_at(input, "фамилия", 0), ["имя", "фамилия"]);
    assert_eq!(path_at(input, "Иванов", 2), ["имя", "фамилия"]);
}

#[test]
fn whitespace_and_comments_resolve_to_enclosing_collection() {
    let input = "a:\n  b: 1\n  # note\n  c: 2\n";
    assert_eq!(path_at(input, "# note", 0), ["a"]);
    assert_eq!(path_at(input, "a:", 0), ["a"]);
}
