//! Argument templates for [`toargv`](https://crates.io/crates/toargv).
//!
//! The crate parses an ordered argv template and expands paths from a
//! [`serde_json::Value`] configuration tree into command-line arguments.
//!
//! ```
//! use serde_json::json;
//! use toargv_template::{Template, expand};
//!
//! let template = Template::parse(&[
//!     "--output".to_owned(),
//!     "{output}".to_owned(),
//!     "{input}".to_owned(),
//! ])?;
//! let arguments = expand(
//!     &json!({"output": "result.txt", "input": "source.txt"}),
//!     &template,
//! )?;
//!
//! assert_eq!(arguments, ["--output", "result.txt", "source.txt"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![warn(missing_docs)]

/// Errors returned by template parsing and expansion.
pub mod error;
/// Argument expansion from configuration values.
pub mod expand;
/// Paths into JSON-compatible configuration trees.
pub mod path;
/// The validated argv template model and parser.
pub mod template;

pub use error::Error;
pub use expand::expand;
pub use path::ConfigPath;
pub use template::Template;
