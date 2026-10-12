mod common;

use ayml_core::{Node, Value, parse};
use common::{err_msg, get, items, ok, s};

#[allow(clippy::unnecessary_wraps)]
fn c(text: &str) -> Option<String> {
    Some(text.to_owned())
}

const SPEC_EXAMPLE: &str = "\
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
";

#[test]
fn spec_example_assigns_every_comment() {
    let root = ok(SPEC_EXAMPLE);
    assert_eq!(root.comment, c("leading comment of the root mapping"));
    assert_eq!(root.inline_comment, None);
    assert_eq!(
        root.trailing_comment,
        c("trailing comment of the root mapping")
    );

    let server = get(&root, "server");
    assert_eq!(server.comment, None);
    assert_eq!(server.inline_comment, None);
    assert_eq!(
        server.trailing_comment,
        c("trailing comment of the `server` mapping")
    );

    let host = get(server, "host");
    assert_eq!(host.comment, c("leading comment of `host`"));
    assert_eq!(host.inline_comment, c("inline comment of `localhost`"));
    assert_eq!(host.trailing_comment, None);

    let ports = get(&root, "ports");
    assert_eq!(ports.comment, c("leading comment of `ports`"));
    assert_eq!(
        ports.inline_comment,
        c("inline comment of the `ports` sequence")
    );
    assert_eq!(
        ports.trailing_comment,
        c("trailing comment of the `ports` sequence")
    );

    let entries = items(ports);
    assert_eq!(entries[0].comment, None);
    assert_eq!(entries[0].inline_comment, None);
    assert_eq!(entries[1].inline_comment, c("inline comment of `443`"));
}

#[test]
fn spec_example_round_trips_verbatim() {
    assert_eq!(ayml_core::emit(&ok(SPEC_EXAMPLE)), SPEC_EXAMPLE);
}

#[test]
fn comments_before_root_mapping_belong_to_root() {
    let root = ok("# about a\n# more\na: 1\nb: 2\n");
    assert_eq!(root.comment, c("about a\nmore"));
    assert_eq!(get(&root, "a").comment, None);
    assert_eq!(get(&root, "b").comment, None);
}

#[test]
fn comments_before_root_sequence_belong_to_root() {
    let root = ok("# first\n- a\n# second\n- b\n");
    assert_eq!(root.comment, c("first"));
    assert_eq!(items(&root)[0].comment, None);
    assert_eq!(items(&root)[1].comment, c("second"));
}

#[test]
fn comments_before_root_scalar_belong_to_root() {
    assert_eq!(ok("# about\n42\n").comment, c("about"));
    assert_eq!(ok("# about\n[1, 2]\n").comment, c("about"));
    assert_eq!(ok("# about\n{a: 1}\n").comment, c("about"));
}

#[test]
fn blank_lines_around_leading_comments_are_not_part_of_them() {
    let root = ok("\n\n# header\n\n\na: 1\n");
    assert_eq!(root.comment, c("header"));
}

#[test]
fn leading_comment_of_mapping_entry_is_on_value() {
    let root = ok("a: 1\n# about b\nb: 2\n");
    assert_eq!(get(&root, "b").comment, c("about b"));
}

#[test]
fn leading_comment_may_be_indented_any_amount() {
    let root = ok("a: 1\n      # deep\nb: 2\n");
    assert_eq!(get(&root, "b").comment, c("deep"));
    let root = ok("a:\n  b: 1\n# shallow\n  c: 2\n");
    assert_eq!(get(get(&root, "a"), "c").comment, c("shallow"));
}

#[test]
fn comments_between_key_and_nested_block_belong_to_first_entry() {
    let root = ok("a:\n\n  # about b\n  b: 1\n");
    let a = get(&root, "a");
    assert_eq!(a.comment, None);
    assert_eq!(get(a, "b").comment, c("about b"));
    let root = ok("list:\n# first item\n- x\n");
    assert_eq!(items(get(&root, "list"))[0].comment, c("first item"));
}

#[test]
fn compact_mapping_comments_belong_to_sequence_entry() {
    let root = ok("rules:\n  # allow github\n  - host: github.com\n    port: 22\n");
    let rule = &items(get(&root, "rules"))[0];
    assert_eq!(rule.comment, c("allow github"));
    assert_eq!(get(rule, "host").comment, None);
}

#[test]
fn compact_mapping_later_entries_have_their_own_comments() {
    let root = ok("- a: 1\n  # about b\n  b: 2\n");
    assert_eq!(get(&items(&root)[0], "b").comment, c("about b"));
}

#[test]
fn compact_sequence_comments_belong_to_outer_entry() {
    let root = ok("- x\n# outer\n- - a\n  # second inner\n  - b\n");
    let outer = &items(&root)[1];
    assert_eq!(outer.comment, c("outer"));
    assert_eq!(items(outer)[0].comment, None);
    assert_eq!(items(outer)[1].comment, c("second inner"));
}

#[test]
fn inline_comments_on_scalars() {
    let root = ok("hr:  65    # Home runs\navg: 0.278 # Batting average\n");
    assert_eq!(get(&root, "hr").inline_comment, c("Home runs"));
    assert_eq!(get(&root, "avg").inline_comment, c("Batting average"));
}

#[test]
fn inline_comment_on_sequence_entry() {
    let root = ok("- 22 # Git (SSH)\n- 443\t# Site\n");
    assert_eq!(items(&root)[0].inline_comment, c("Git (SSH)"));
    assert_eq!(items(&root)[1].inline_comment, c("Site"));
}

#[test]
fn inline_comment_in_compact_mapping_belongs_to_value() {
    let root = ok("- a: 1 # c\n");
    let entry = &items(&root)[0];
    assert_eq!(entry.inline_comment, None);
    assert_eq!(get(entry, "a").inline_comment, c("c"));
}

#[test]
fn inline_comment_after_key_belongs_to_block_value() {
    let root = ok("hr: # 1998 hr ranking\n  - Mark McGwire\n");
    assert_eq!(get(&root, "hr").inline_comment, c("1998 hr ranking"));
    let root = ok("a: # about a\n  b: 1\n");
    assert_eq!(get(&root, "a").inline_comment, c("about a"));
}

#[test]
fn inline_comment_on_root_scalar_and_flow() {
    assert_eq!(ok("42 # c").inline_comment, c("c"));
    assert_eq!(
        ok("[1,2,3,4,] # this is a side comment").inline_comment,
        c("this is a side comment")
    );
}

#[test]
fn inline_comment_on_flow_value() {
    let root = ok("k: {a: 1} # c\n");
    assert_eq!(get(&root, "k").inline_comment, c("c"));
}

#[test]
fn trailing_comment_of_nested_mapping() {
    let root = ok("a:\n  b: 1\n  # end of a\nc: 2\n");
    assert_eq!(get(&root, "a").trailing_comment, c("end of a"));
    assert_eq!(get(&root, "c").comment, None);
}

#[test]
fn dedented_comment_goes_to_next_entry() {
    let root = ok("a:\n  b: 1\n# about c\nc: 2\n");
    assert_eq!(get(&root, "a").trailing_comment, None);
    assert_eq!(get(&root, "c").comment, c("about c"));
}

#[test]
fn trailing_comment_split_between_levels() {
    let root = ok("a:\n  b: 1\n  # end of a\n# about c\nc: 2\n");
    assert_eq!(get(&root, "a").trailing_comment, c("end of a"));
    assert_eq!(get(&root, "c").comment, c("about c"));
}

#[test]
fn split_stops_at_first_shallower_line() {
    let root = ok("a:\n  b: 1\n# about c?\n  # also about c\nc: 2\n");
    assert_eq!(get(&root, "a").trailing_comment, None);
    assert_eq!(get(&root, "c").comment, c("about c?\nalso about c"));
}

#[test]
fn deeper_trailing_comment_belongs_to_collection() {
    let root = ok("a:\n  b: 1\n      # deep\nc: 2\n");
    assert_eq!(get(&root, "a").trailing_comment, c("deep"));
}

#[test]
fn commented_out_entries_at_end_of_block() {
    let root = ok("deps:\n  foo: 1\n  # bar: 2\nother: 3\n");
    assert_eq!(get(&root, "deps").trailing_comment, c("bar: 2"));
}

#[test]
fn trailing_comments_at_end_of_document() {
    let root = ok("a:\n  b: 1\n  # inner\n# outer\n");
    assert_eq!(get(&root, "a").trailing_comment, c("inner"));
    assert_eq!(root.trailing_comment, c("outer"));
}

#[test]
fn trailing_comment_after_nested_sequence_at_end_of_document() {
    let root = ok(
        "# Starting Comment\nthis:\n  - thing\n  - over\n# Trailing Comment\n# With multiple lines\n",
    );
    assert_eq!(
        root.trailing_comment,
        c("Trailing Comment\nWith multiple lines")
    );
    assert_eq!(get(&root, "this").trailing_comment, None);
    assert_eq!(root.comment, c("Starting Comment"));
}

#[test]
fn trailing_comment_of_root_scalar_any_indentation() {
    let root = ok("42\n    # t1\n\n# t2\n");
    assert_eq!(root.trailing_comment, c("t1\n\nt2"));
}

#[test]
fn indented_comment_after_root_mapping() {
    let root = ok("a: 1\n  # trailing");
    assert_eq!(root.trailing_comment, c("trailing"));
    assert_eq!(get(&root, "a").inline_comment, None);
}

#[test]
fn implicit_sequence_keeps_only_deeper_comments() {
    let root = ok("a:\n- 1\n  # deeper\n# about b\nb: 2\n");
    assert_eq!(get(&root, "a").trailing_comment, c("deeper"));
    assert_eq!(get(&root, "b").comment, c("about b"));
}

#[test]
fn implicit_sequence_at_end_of_document() {
    let root = ok("a:\n- 1\n# root trailing\n");
    assert_eq!(get(&root, "a").trailing_comment, None);
    assert_eq!(root.trailing_comment, c("root trailing"));
}

#[test]
fn compact_mapping_trailing_comment() {
    let root = ok("- a: 1\n  # end of item\n- b\n");
    assert_eq!(items(&root)[0].trailing_comment, c("end of item"));
    assert_eq!(items(&root)[1].comment, None);
}

#[test]
fn comment_between_sequence_entries_is_leading() {
    let root = ok("- 1\n  # deeper but continuing\n- 2\n");
    assert_eq!(items(&root)[1].comment, c("deeper but continuing"));
}

#[test]
fn flow_leading_inline_and_trailing_comments() {
    let root = ok("a: {\n  x: 1, # one\n  # about y\n  y: 2,\n  # trailing\n}\n");
    let a = get(&root, "a");
    assert_eq!(get(a, "x").inline_comment, c("one"));
    assert_eq!(get(a, "y").comment, c("about y"));
    assert_eq!(a.trailing_comment, c("trailing"));
}

#[test]
fn flow_inline_comment_before_comma() {
    let root = ok("[1 # one\n, 2]");
    assert_eq!(items(&root)[0].inline_comment, c("one"));
    assert_eq!(items(&root)[1].comment, None);
}

#[test]
fn flow_comment_after_opening_bracket_is_leading() {
    let root = ok("[ # first\n  1]");
    assert_eq!(items(&root)[0].comment, c("first"));
}

#[test]
fn flow_comment_between_key_and_value_is_leading() {
    let root = ok("{a: # about a\n  1}");
    assert_eq!(get(&root, "a").comment, c("about a"));
}

#[test]
fn empty_flow_collection_with_trailing_comment() {
    let root = ok("a: [\n  # nothing yet\n]\n");
    assert_eq!(get(&root, "a").trailing_comment, c("nothing yet"));
    assert_eq!(get(&root, "a").value, Value::Seq(vec![]));
}

#[test]
fn flow_comment_needs_whitespace() {
    assert!(err_msg("[1,# c\n]").contains("`#` must be preceded by white space"));
    assert!(err_msg("[# c\n1]").contains("`#` must be preceded by white space"));
}

#[test]
fn blank_lines_inside_comment_group_are_empty_lines() {
    let root = ok("a: 0\n# a\n\n\n# b\nx: 1\n");
    assert_eq!(get(&root, "x").comment, c("a\n\n\nb"));
}

#[test]
fn bare_hash_line_is_an_empty_line() {
    let root = ok("a: 0\n# Title\n#\n# Paragraph\nx: 1\n");
    assert_eq!(get(&root, "x").comment, c("Title\n\nParagraph"));
}

#[test]
fn empty_comments() {
    assert_eq!(get(&ok("x: 1 #\n"), "x").inline_comment, c(""));
    assert_eq!(get(&ok("a: 0\n#\nx: 1\n"), "x").comment, c(""));
}

#[test]
fn one_space_after_hash_is_stripped() {
    let root = ok("a: 0\n#no space\n#  two spaces\nx: 1 #\ttab\n");
    assert_eq!(get(&root, "x").comment, c("no space\n two spaces"));
    assert_eq!(get(&root, "x").inline_comment, c("\ttab"));
}

#[test]
fn trailing_whitespace_in_comments_is_trimmed() {
    let root = ok("a: 0\n# a  \t\nx: 1 # b   \n");
    assert_eq!(get(&root, "x").comment, c("a"));
    assert_eq!(get(&root, "x").inline_comment, c("b"));
}

#[test]
fn comment_text_keeps_hashes_and_unicode() {
    let root = ok("a: 0\n# ## heading # ☺\nx: 1\n");
    assert_eq!(get(&root, "x").comment, c("## heading # ☺"));
}

#[test]
fn hash_in_triple_quoted_string_is_not_a_comment() {
    let root = ok("code: \"\"\"\n  # thread-safe\n  done() # done\n  \"\"\"\n");
    let code = get(&root, "code");
    assert_eq!(code.value, s("# thread-safe\ndone() # done"));
    assert_eq!(code.comment, None);
    assert_eq!(code.inline_comment, None);
}

#[test]
fn tab_indented_comment_line_is_an_error() {
    assert!(parse("a: 1\n\t# comment\nb: 2").is_err());
}

#[test]
fn crlf_comments() {
    let root = ok("a: 0\r\n# a\r\n\r\n# b\r\nx: 1 # c\r\n");
    assert_eq!(get(&root, "x").comment, c("a\n\nb"));
    assert_eq!(get(&root, "x").inline_comment, c("c"));
}

#[test]
fn comments_round_trip_through_emitter() {
    let input = "# a\n#\n# b\nx: 1 # c\nlist: # l\n  # first\n  - 1\n  # end\n# root end\n";
    let root = ok(input);
    let again: Node = ok(&ayml_core::emit(&root));
    assert_eq!(root, again);
}
