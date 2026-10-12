//! Reads a [`Node`] tree as a stream of [`Event`]s.

use std::borrow::Cow;

use crate::reader::{Comment, Event};
use crate::value::{Key, Node, Value};

/// Reads a [`Node`] tree.
pub struct NodeReader<'a> {
    root: &'a Node,
    state: State<'a>,
    stack: Vec<Open<'a>>,
}

enum State<'a> {
    /// Starts a node
    Start(&'a Node),
    /// Continue the innermost open collection
    Resume,
    /// End the document.
    Done,
}

enum Open<'a> {
    Seq(&'a Node, std::slice::Iter<'a, Node>),
    Map(&'a Node, indexmap::map::Iter<'a, Key, Node>),
}

impl<'a> NodeReader<'a> {
    #[must_use]
    pub const fn new(root: &'a Node) -> Self {
        Self {
            root,
            state: State::Start(root),
            stack: Vec::new(),
        }
    }

    /// Read the next event.
    pub fn next_event(&mut self) -> Event<'a> {
        match std::mem::replace(&mut self.state, State::Resume) {
            State::Start(node) => self.start(node),
            State::Resume => self.resume(),
            State::Done => {
                self.state = State::Done;
                Event::DocumentEnd { trailing: None }
            }
        }
    }

    fn start(&mut self, node: &'a Node) -> Event<'a> {
        let leading = comment(node.comment.as_deref());
        let start = node.span.start;
        match &node.value {
            Value::Seq(items) => {
                self.stack.push(Open::Seq(node, items.iter()));
                Event::SeqStart { start, leading }
            }
            Value::Map(map) => {
                self.stack.push(Open::Map(node, map.iter()));
                Event::MapStart { start, leading }
            }
            scalar => Event::Scalar {
                value: scalar.as_scalar().expect("scalar value"),
                span: node.span,
                leading,
                inline: node.inline_comment.as_deref(),
            },
        }
    }

    fn resume(&mut self) -> Event<'a> {
        let Some(open) = self.stack.last_mut() else {
            self.state = State::Done;
            let trailing = match self.root.value {
                Value::Seq(_) | Value::Map(_) => None,
                _ => comment(self.root.trailing_comment.as_deref()),
            };
            return Event::DocumentEnd { trailing };
        };
        match open {
            Open::Seq(_, items) => {
                if let Some(item) = items.next() {
                    return self.start(item);
                }
            }
            Open::Map(_, entries) => {
                if let Some((key, value)) = entries.next() {
                    self.state = State::Start(value);
                    return Event::Key {
                        name: Cow::Borrowed(&key.name),
                        span: key.span,
                    };
                }
            }
        }
        let Some(Open::Seq(node, _) | Open::Map(node, _)) = self.stack.pop() else {
            unreachable!("open collection")
        };
        Event::End {
            end: node.span.end,
            inline: node.inline_comment.as_deref(),
            trailing: comment(node.trailing_comment.as_deref()),
        }
    }
}

fn comment(text: Option<&str>) -> Option<Comment<'_>> {
    text.map(Comment::from_text)
}
