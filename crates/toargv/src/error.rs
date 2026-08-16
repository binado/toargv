use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A configuration, grammar, rendering, or execution failure.
#[derive(Debug, Error)]
pub enum Error {
    /// A configuration file could not be read.
    #[error("failed to read `{path}`: {source}")]
    Read {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: io::Error,
    },

    /// The configuration path does not have a supported extension.
    #[error("configuration file `{0}` must have a .toml or .json extension")]
    UnsupportedConfigFormat(PathBuf),

    /// A TOML or JSON configuration could not be parsed.
    #[error("failed to parse {format} configuration `{path}`: {message}")]
    ParseConfig {
        /// Path to the invalid configuration.
        path: PathBuf,
        /// Human-readable configuration format.
        format: &'static str,
        /// Parser error message.
        message: String,
    },

    /// Template validation or argument expansion failed.
    #[error(transparent)]
    Template(#[from] toargv_template::Error),

    /// A child process could not be started.
    #[error("failed to execute `{program}`: {source}")]
    Execute {
        /// Display form of the requested program.
        program: String,
        /// Underlying process-spawn error.
        #[source]
        source: io::Error,
    },

    /// Process execution was requested without a program.
    #[error("an executable command is required")]
    MissingCommand,

    /// An argument cannot be represented as safe shell syntax.
    #[error("argument cannot be represented in shell syntax: {0:?}")]
    Unquotable(String),

    /// An argument contains a NUL byte, so it can be neither NUL-separated
    /// nor passed to a child process.
    #[error("argument contains a NUL byte: {0:?}")]
    NulByte(String),

    /// Expansion did not finish within the allowed wall-clock time.
    #[error("filter evaluation did not finish within {seconds} seconds")]
    Timeout {
        /// The deadline that elapsed, in seconds.
        seconds: f64,
    },

    /// The thread running the expansion could not be started or did not finish.
    #[error("filter evaluation failed: {0}")]
    Worker(String),
}
