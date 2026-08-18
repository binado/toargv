//! Argument templates for [`toargv`](https://crates.io/crates/toargv).
//!
//! The crate parses a template string containing `{}`, `{N}`, and `{name}`
//! slots, and expands it into command-line arguments by evaluating jq
//! filters against a [`serde_json::Value`] configuration tree.
//!
//! ```
//! use serde_json::json;
//! use toargv_template::{Filter, Template, expand};
//!
//! let template = Template::parse("--output {} {}")?;
//! let arguments = expand(
//!     &json!({"output": "result dir", "files": ["a.txt", "b.txt"]}),
//!     &template,
//!     &[Filter::parse(".output"), Filter::parse(".files[]")],
//! )?;
//!
//! assert_eq!(arguments, ["--output", "result dir", "a.txt", "b.txt"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![warn(missing_docs)]

/// Errors returned by template parsing, filter validation, and expansion.
pub mod error;
/// Argument expansion from configuration values.
pub mod expand;
mod jq;
/// The validated template model, lexer, and filter bindings.
pub mod template;

pub use error::{Error, SlotLabel};
pub use expand::expand;
pub use template::{Filter, Template};
