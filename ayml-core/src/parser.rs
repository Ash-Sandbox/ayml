//! Builds a [`Node`] tree from [`Reader`] events.

use crate::error::{Error, Span};
use crate::reader::{Comment, Event, Reader};
use crate::value::{Key, Mapping, Node, Value};

/// A collection being built, with the key awaiting its value (mappings).
struct Open {
    node: Node,
    key: Option<Key>,
}

fn text(comment: Option<Comment<'_>>) -> Option<String> {
    comment.map(|c| c.text().into_owned())
}

/// Parse a document into a tree with the given maximum nesting depth.
pub(crate) fn parse(input: &str, max_depth: usize) -> Result<Node, Error> {
    let mut reader = Reader::with_max_depth(input, max_depth);
    let mut stack: Vec<Open> = Vec::new();
    let mut root: Option<Node> = None;
    loop {
        let complete = match reader.next_event()? {
            Event::SeqStart { start, leading } => {
                stack.push(Open {
                    node: collection(Value::Seq(Vec::new()), start, leading),
                    key: None,
                });
                continue;
            }
            Event::MapStart { start, leading } => {
                stack.push(Open {
                    node: collection(Value::Map(Mapping::new()), start, leading),
                    key: None,
                });
                continue;
            }
            Event::Key { name, span } => {
                if let Some(open) = stack.last_mut() {
                    open.key = Some(Key {
                        name: name.into_owned(),
                        span,
                    });
                }
                continue;
            }
            Event::Scalar {
                value,
                span,
                leading,
                inline,
            } => Node {
                value: Value::from(value),
                comment: text(leading),
                inline_comment: inline.map(str::to_owned),
                trailing_comment: None,
                span,
            },
            Event::End {
                end,
                inline,
                trailing,
            } => {
                let mut node = stack.pop().expect("open collection").node;
                node.span.end = end;
                node.inline_comment = inline.map(str::to_owned);
                node.trailing_comment = text(trailing);
                node
            }
            Event::DocumentEnd { trailing } => {
                let mut root = root.expect("document has a root");
                if let Some(rest) = text(trailing) {
                    root.trailing_comment = Some(match root.trailing_comment.take() {
                        Some(t) => format!("{t}\n{rest}"),
                        None => rest,
                    });
                }
                return Ok(root);
            }
        };
        match stack.last_mut() {
            None => root = Some(complete),
            Some(open) => match &mut open.node.value {
                Value::Seq(items) => items.push(complete),
                Value::Map(map) => {
                    let key = open.key.take().expect("key before value");
                    map.insert(key, complete);
                }
                _ => unreachable!("collections hold entries"),
            },
        }
    }
}

fn collection(value: Value, start: usize, leading: Option<Comment<'_>>) -> Node {
    Node {
        value,
        comment: text(leading),
        inline_comment: None,
        trailing_comment: None,
        span: Span::new(start, start),
    }
}
