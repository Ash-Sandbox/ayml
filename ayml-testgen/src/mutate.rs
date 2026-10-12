//! Random edits to source text.

use proptest::prelude::*;
use proptest::sample::{Index, select};

use crate::render::arb_doc;

/// Characters likely to change the meaning of a document.
const INTERESTING: &[char] = &[
    ' ', ' ', '\t', '\n', '\r', '-', ':', '#', ',', '[', ']', '{', '}', '"', '\\', '\'', 'a', '0',
    'x', 'e', '.', '+', '\u{0}', '\u{85}', '\u{FEFF}', 'é',
];

#[derive(Debug, Clone)]
enum Edit {
    Insert(Index, char),
    Delete(Index),
    Replace(Index, char),
    DuplicateLine(Index),
    Indent(Index),
}

fn arb_edit() -> BoxedStrategy<Edit> {
    prop_oneof![
        any::<(Index, char)>().prop_map(|(i, c)| Edit::Insert(i, c)),
        (any::<Index>(), select(INTERESTING)).prop_map(|(i, c)| Edit::Insert(i, c)),
        any::<Index>().prop_map(Edit::Delete),
        (any::<Index>(), select(INTERESTING)).prop_map(|(i, c)| Edit::Replace(i, c)),
        any::<Index>().prop_map(Edit::DuplicateLine),
        any::<Index>().prop_map(Edit::Indent),
    ]
    .boxed()
}

fn apply(text: &str, edit: &Edit) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    match edit {
        Edit::Insert(i, c) => {
            let at = i.index(chars.len() + 1);
            chars.insert(at, *c);
        }
        Edit::Delete(i) if !chars.is_empty() => {
            chars.remove(i.index(chars.len()));
        }
        Edit::Replace(i, c) if !chars.is_empty() => {
            let at = i.index(chars.len());
            chars[at] = *c;
        }
        Edit::DuplicateLine(i) => {
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            if lines.is_empty() {
                return text.to_owned();
            }
            let at = i.index(lines.len());
            let mut out: Vec<&str> = lines.clone();
            out.insert(at, lines[at]);
            return out.concat();
        }
        Edit::Indent(i) => {
            let starts: Vec<usize> = std::iter::once(0)
                .chain(
                    chars
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| **c == '\n')
                        .map(|(p, _)| p + 1),
                )
                .filter(|p| *p <= chars.len())
                .collect();
            let at = starts[i.index(starts.len())];
            chars.insert(at, ' ');
        }
        _ => {}
    }
    chars.into_iter().collect()
}

/// A valid generated document with one to three random edits applied.
/// The result may or may not be valid AYML.
pub fn arb_mutated_doc() -> BoxedStrategy<String> {
    (arb_doc(), prop::collection::vec(arb_edit(), 1..4))
        .prop_map(|(doc, edits)| edits.iter().fold(doc.text, |text, e| apply(&text, e)))
        .boxed()
}
