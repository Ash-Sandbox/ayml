use ayml_core::{MapKey, parse};

#[test]
fn star_glob_value_span() {
    let input = "host: *.github.com";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let host = &map[&MapKey::String("host".into())];
    let span_text = &input[host.span.start..host.span.end];
    assert_eq!(span_text, "*.github.com", "span was: {:?} -> {:?}", host.span, span_text);
}

#[test]
fn star_glob_in_compact_mapping() {
    let input = "- host: *.github.com";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    let map = seq[0].value.as_mapping().unwrap();
    let host = &map[&MapKey::String("host".into())];
    let span_text = &input[host.span.start..host.span.end];
    assert_eq!(span_text, "*.github.com", "span was: {:?} -> {:?}", host.span, span_text);
}

#[test]
fn star_glob_with_inline_comment() {
    let input = "- host: *.github.com # primary domain";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    let map = seq[0].value.as_mapping().unwrap();
    let host = &map[&MapKey::String("host".into())];
    let span_text = &input[host.span.start..host.span.end];
    assert_eq!(span_text, "*.github.com", "span was: {:?} -> {:?}", host.span, span_text);
}
