mod grammar;
pub mod scanner;

use crate::error::Error;
use crate::value::Node;

/// Parse an AYML document from a string, returning the root node.
///
/// # Errors
/// Returns an [`Error`] if the input is not valid AYML.
pub fn parse(input: &str) -> Result<Node, Error> {
    let mut parser = grammar::Parser::new(input);
    parser.parse_document()
}

/// Parse an AYML document with a custom maximum nesting depth.
///
/// # Errors
/// Returns an [`Error`] if the input is not valid AYML or exceeds
/// the given depth limit.
pub fn parse_with_max_depth(input: &str, max_depth: usize) -> Result<Node, Error> {
    let mut parser = grammar::Parser::new(input).with_max_depth(max_depth);
    parser.parse_document()
}
