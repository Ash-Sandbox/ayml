#![warn(clippy::all, clippy::pedantic)]

pub mod commented;
pub mod commented_value;
pub mod de;
pub mod error;
mod fmt_helpers;
mod read;
pub mod ser;
pub mod value;

pub use de::from_reader;
pub use de::from_slice;
pub use de::from_str;
pub use ser::to_string;
pub use ser::to_vec;
pub use ser::to_writer;
