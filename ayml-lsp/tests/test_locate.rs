use ayml_core::parse;

#[path = "../src/locate.rs"]
#[allow(dead_code)]
mod locate;

fn path_at(input: &str, line: usize, col: usize) -> Vec<String> {
    let node = parse(input).unwrap();
    let offset = line_col_to_offset(input, line, col);
    locate::path_at_offset(&node, offset)
}

fn line_col_to_offset(text: &str, line: usize, col: usize) -> usize {
    let mut l = 0;
    let mut c = 0;
    for (i, ch) in text.char_indices() {
        if l == line && c == col {
            return i;
        }
        if ch == '\n' {
            l += 1;
            c = 0;
        } else {
            c += 1;
        }
    }
    text.len()
}

#[test]
fn top_level_key() {
    let input = "name: Alice\nage: 30";
    assert_eq!(path_at(input, 0, 0), vec!["name"]); // cursor on "n" of "name"
    assert_eq!(path_at(input, 0, 6), vec!["name"]); // cursor on "A" of "Alice"
    assert_eq!(path_at(input, 1, 0), vec!["age"]); // cursor on "a" of "age"
    assert_eq!(path_at(input, 1, 5), vec!["age"]); // cursor on "3" of "30"
}

#[test]
fn nested_key() {
    let input = "network:\n  rules: ok";
    assert_eq!(path_at(input, 0, 0), vec!["network"]); // "network" key
    assert_eq!(path_at(input, 1, 2), vec!["network", "rules"]); // "rules" key
    assert_eq!(path_at(input, 1, 10), vec!["network", "rules"]); // "ok" value
}

#[test]
fn deeply_nested_key() {
    let input = "network:\n  rules:\n    host: x";
    assert_eq!(path_at(input, 2, 4), vec!["network", "rules", "host"]); // "host" key
    assert_eq!(path_at(input, 2, 10), vec!["network", "rules", "host"]); // "x" value
}

#[test]
fn sequence_element() {
    let input = "items:\n  - foo\n  - bar";
    assert_eq!(path_at(input, 1, 4), vec!["items", "0"]);
    assert_eq!(path_at(input, 2, 4), vec!["items", "1"]);
}
