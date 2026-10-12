//! Proptest generators for AYML, shared by `ayml-core` and `ayml` test suites.
//!
//! * [`strings`] — adversarial characters, strings, keys, numbers, comments.
//! * [`tree`] — arbitrary [`Node`](ayml_core::Node) trees and a precise tree diff.
//! * [`render`] — renders a canonical tree as AYML source text with randomized formatting.
//! * [`mutate`] — small random edits to source text.

pub mod mutate;
pub mod render;
pub mod strings;
pub mod tree;

use proptest::test_runner::Config;

/// A proptest config with `default` cases, overridable with the
/// `PROPTEST_CASES` environment variable.
pub fn config(default: u32) -> Config {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default);
    Config {
        cases,
        ..Config::default()
    }
}
