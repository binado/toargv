//! I/O and command execution for the `toargv` configuration-to-arguments CLI.
//!
//! [`build_arguments`] loads a TOML or JSON configuration, parses an argv
//! template, and expands it into an ordered argument vector. The remaining
//! helpers render or execute that vector.
#![warn(missing_docs)]

/// Command-line parsing and resolved output modes.
pub mod cli;
/// Errors returned by loading, rendering, and execution.
pub mod error;
/// Direct process execution without a shell.
pub mod execute;
/// Configuration loading.
pub mod load;
/// Template parsing and expansion re-exported from [`toargv_template`].
pub mod template {
    pub use toargv_template::*;
}

use std::borrow::Cow;
use std::ffi::OsString;
use std::path::Path;

pub use error::Error;
pub use execute::execute;
pub use load::load_config;
pub use template::{Filter, Template, expand};

/// Loads a configuration, parses a template string, and expands its slots
/// with the given jq filters.
pub fn build_arguments(
    config_path: &Path,
    template: &str,
    filters: &[String],
) -> Result<Vec<String>, Error> {
    let config = load_config(config_path)?;
    let template = Template::parse(template)?;
    let filters: Vec<Filter> = filters.iter().map(|source| Filter::parse(source)).collect();
    Ok(expand(&config, &template, &filters)?)
}

/// Concatenates a command prefix with expanded arguments, matching the argv
/// `execute` spawns.
pub fn full_argv(prefix: &[OsString], expanded: &[String]) -> Vec<String> {
    prefix
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .chain(expanded.iter().cloned())
        .collect()
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

/// Renders an argument vector NUL-separated, in the format `xargs -0`
/// expects: every argument terminated by a NUL byte.
///
/// NUL cannot occur inside an operating-system argument, so this encoding
/// preserves boundaries without any quoting grammar. Returns
/// [`Error::NulByte`] if an argument itself contains a NUL byte.
pub fn render_nul(arguments: &[String]) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    for argument in arguments {
        if argument.contains('\0') {
            return Err(Error::NulByte(argument.clone()));
        }
        bytes.extend_from_slice(argument.as_bytes());
        bytes.push(0);
    }
    Ok(bytes)
}
