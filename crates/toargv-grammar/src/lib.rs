//! Grammar primitives and argument generation for [`toargv`](https://crates.io/crates/toargv).
//!
//! The crate models an ordered mapping from paths in a [`serde_json::Value`]
//! configuration tree to command-line arguments. [`InlineCodec`] decodes and
//! encodes the textual grammar syntax, and [`generate`] applies a validated
//! [`Grammar`] to configuration data.
//!
//! ```
//! use serde_json::json;
//! use toargv_grammar::{generate, GrammarCodec, InlineCodec};
//!
//! let grammar = InlineCodec.decode("[--output output] <input>")?;
//! let arguments = generate(
//!     &json!({"output": "result.txt", "input": "source.txt"}),
//!     &grammar,
//! )?;
//!
//! assert_eq!(arguments, ["--output", "result.txt", "source.txt"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![warn(missing_docs)]

/// Inline grammar codecs.
pub mod codecs;
/// Argument generation from configuration values.
pub mod emit;
/// Errors returned by validated grammar operations and generation.
pub mod error;
/// The validated grammar model.
pub mod grammar;
/// Paths into JSON-compatible configuration trees.
pub mod path;

pub use codecs::{CodecError, GrammarCodec, InlineCodec};
pub use emit::generate;
pub use error::Error;
pub use grammar::{Action, Grammar, NamedAction, OptionToken, Rule};
pub use path::ConfigPath;
