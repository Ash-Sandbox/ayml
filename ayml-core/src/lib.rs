//! Reference parser and emitter for AYML.

pub mod directive;
pub mod emitter;
pub mod error;
mod node_reader;
mod parser;
pub mod reader;
pub mod scalar;
pub mod value;

pub use emitter::{Comments, Writer, emit, emit_flow};
pub use error::{Error, ErrorKind, Location, Span};
pub use node_reader::NodeReader;
pub use reader::{DEFAULT_MAX_DEPTH, Reader};
pub use scalar::Scalar;
pub use value::{Key, Mapping, Node, Value};

/// Parse an AYML document from a string.
pub fn parse(input: &str) -> Result<Node, Error> {
    parser::parse(input, DEFAULT_MAX_DEPTH)
}

/// Parse an AYML document with a custom maximum collection nesting depth.
pub fn parse_with_max_depth(input: &str, max_depth: usize) -> Result<Node, Error> {
    parser::parse(input, max_depth)
}
