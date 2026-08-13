use thiserror::Error;

/// An invalid template or argument-expansion failure.
#[derive(Debug, Error)]
pub enum Error {
    /// A dotted configuration path is invalid.
    #[error("invalid configuration path `{path}`: {message}")]
    InvalidPath {
        /// The invalid path.
        path: String,
        /// Why the path is invalid.
        message: String,
    },

    /// A template argument contains invalid syntax.
    #[error("invalid template argument {argument} at byte {offset}: {message}")]
    Parse {
        /// One-based position in the template argument vector.
        argument: usize,
        /// Byte offset within the argument.
        offset: usize,
        /// Why parsing failed.
        message: String,
    },

    /// A strict placeholder's configuration path is missing or null.
    #[error("configuration path `{path}` was not found")]
    MissingValue {
        /// The unresolved dotted configuration path.
        path: String,
        /// One-based position in the template argument vector.
        argument: usize,
    },

    /// A configuration value is incompatible with its placeholder.
    #[error("cannot expand configuration path `{path}`: {message}")]
    Expansion {
        /// One-based position in the template argument vector.
        argument: usize,
        /// The dotted configuration path that resolved to the value.
        path: String,
        /// Why the value and placeholder are incompatible.
        message: String,
    },
}
