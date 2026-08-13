//! I/O and command execution for the `toargv` configuration-to-arguments CLI.
//!
//! [`build_arguments`] loads a TOML or JSON configuration and one or more
//! inline grammar sources, then generates an ordered argument vector. The
//! remaining helpers render or execute that vector.
#![warn(missing_docs)]

/// Command-line parsing and resolved output modes.
pub mod cli;
/// Argument generation re-exported from [`toargv_grammar`].
pub mod emit {
    pub use toargv_grammar::emit::*;
}
/// Errors returned by loading, rendering, and execution.
pub mod error;
/// Direct process execution without a shell.
pub mod execute;
/// Grammar model types re-exported from [`toargv_grammar`].
pub mod grammar {
    pub use toargv_grammar::grammar::*;
}
/// Configuration and grammar loading.
pub mod load;

use std::borrow::Cow;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use toargv_grammar::Grammar;

pub use emit::generate;
pub use error::Error;
pub use execute::execute;
pub use load::{load_config, load_grammar, load_inline_grammar};

/// Loads and layers grammar sources, then generates arguments from a config.
///
/// Grammar files are merged left to right before inline grammars are merged
/// left to right. At least one grammar source is required.
pub fn build_arguments(
    config_path: &Path,
    grammar_files: &[PathBuf],
    inline_grammars: &[String],
) -> Result<Vec<String>, Error> {
    if grammar_files.is_empty() && inline_grammars.is_empty() {
        return Err(Error::MissingGrammar);
    }

    let config = load_config(config_path)?;
    let mut grammar = Grammar::default();
    for path in grammar_files {
        grammar.merge(load_grammar(path)?);
    }
    for source in inline_grammars {
        grammar.merge(load_inline_grammar(source)?);
    }

    Ok(generate(&config, &grammar)?)
}

/// Concatenates a command prefix with generated arguments, matching the argv
/// `execute` spawns.
pub fn full_argv(prefix: &[OsString], generated: &[String]) -> Vec<String> {
    prefix
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .chain(generated.iter().cloned())
        .collect()
}

/// Renders an argument vector as a compact JSON array.
pub fn render_json(arguments: &[String]) -> String {
    serde_json::to_string(arguments).expect("serializing strings cannot fail")
}

/// Renders an argument vector as shell syntax that preserves argument
/// boundaries.
///
/// Returns [`Error::Unquotable`] if an argument contains a value, such as NUL,
/// that cannot be represented safely.
pub fn render_shell(arguments: &[String]) -> Result<String, Error> {
    arguments
        .iter()
        .map(|argument| {
            shlex::try_quote(argument)
                .map(Cow::into_owned)
                .map_err(|_| Error::Unquotable(argument.clone()))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(" "))
}
