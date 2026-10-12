//! Serde support for AYML.
//!
//! ```
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Debug, PartialEq)]
//! struct Config {
//!     host: String,
//!     port: u16,
//! }
//!
//! let config: Config = ayml::from_str("host: localhost\nport: 8080\n").unwrap();
//! assert_eq!(config, Config { host: "localhost".into(), port: 8080 });
//! assert_eq!(ayml::to_string(&config).unwrap(), "host: localhost\nport: 8080\n");
//! ```

pub mod commented;
pub mod de;
pub mod error;
pub mod ser;
pub mod value;

pub use ayml_core::Node;
pub use commented::Commented;
pub use de::{from_node, from_reader, from_slice, from_str};
pub use error::{Error, Result};
pub use ser::{to_node, to_string, to_vec, to_writer};
pub use value::Value;
