use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A configuration, grammar, rendering, or execution failure.
#[derive(Debug, Error)]
pub enum Error {
    /// A configuration or grammar file could not be read.
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

    /// A grammar file could not be decoded.
    #[error("failed to parse grammar `{path}`: {message}")]
    ParseGrammar {
        /// Path to the invalid grammar.
        path: PathBuf,
        /// Codec error message.
        message: String,
    },

    /// An inline grammar argument could not be decoded.
    #[error("failed to parse inline grammar: {message}")]
    ParseInlineGrammar {
        /// Codec error message.
        message: String,
    },

    /// No grammar file or inline grammar was supplied.
    #[error("at least one -f/--grammar-file or -g/--grammar source is required")]
    MissingGrammar,

    /// Grammar validation or argument generation failed.
    #[error(transparent)]
    Grammar(#[from] toargv_grammar::Error),

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
}
