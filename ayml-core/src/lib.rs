#![warn(clippy::all, clippy::pedantic)]

pub mod directive;
pub mod emitter;
pub mod error;
pub mod parser;
pub mod value;

use error::Error;
use value::Node;

/// Parse an AYML document from a string.
///
/// Uses [`DEFAULT_MAX_DEPTH`] (128) as the nesting limit.
///
/// # Errors
/// Returns an [`Error`] if the input is not valid AYML.
pub fn parse(input: &str) -> Result<Node, Error> {
    parser::parse(input)
}

/// Parse an AYML document with a custom maximum nesting depth.
///
/// # Errors
/// Returns an [`Error`] if the input is not valid AYML or exceeds
/// the given depth limit.
pub fn parse_with_max_depth(input: &str, max_depth: usize) -> Result<Node, Error> {
    parser::parse_with_max_depth(input, max_depth)
}

/// Emit an AYML document to a string.
#[must_use]
pub fn emit(node: &Node) -> String {
    emitter::emit(node)
}
