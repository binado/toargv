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
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

pub use error::Error;
pub use execute::execute;
pub use load::load_config;
pub use template::{Filter, Template, expand};

/// The wall-clock budget the CLI gives a full expansion.
///
/// A jq filter can loop without ever yielding a value (`def f: f; f`), which
/// no output bound can catch. The deadline turns that hang into a diagnostic.
pub const MAX_FILTER_DURATION: Duration = Duration::from_secs(10);

/// Stack size for the expansion worker.
///
/// jaq evaluates recursively, so a filter that recurses at run time consumes
/// the thread's stack. A generous stack raises the depth reached before that
/// happens; it does not make the failure recoverable, because a Rust stack
/// overflow aborts the process rather than unwinding.
const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

/// Loads a configuration, parses a template string, and expands its slots
/// with the given jq filters.
///
/// Filters run to completion: a filter that never terminates hangs this call.
/// Use [`build_arguments_within`] to bound it.
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

/// Runs [`build_arguments`] under a wall-clock deadline, returning
/// [`Error::Timeout`] if it does not finish in time.
///
/// The work happens on a worker thread while this thread waits on a channel.
/// Only the finished `Vec<String>` crosses the boundary, so no jq value has to
/// be `Send` and the jq engine stays confined to one thread.
///
/// A timed-out worker is abandoned, not stopped — jaq offers no way to
/// interrupt an evaluation — so it keeps running until the process exits. That
/// is why this guard lives in the CLI, whose next act is to exit, and not in
/// the library.
pub fn build_arguments_within(
    timeout: Duration,
    config_path: &Path,
    template: &str,
    filters: &[String],
) -> Result<Vec<String>, Error> {
    let config_path = config_path.to_path_buf();
    let template = template.to_owned();
    let filters = filters.to_vec();
    let (sender, receiver) = mpsc::channel();

    std::thread::Builder::new()
        .stack_size(WORKER_STACK_SIZE)
        .spawn(move || {
            // A closed receiver means the deadline already elapsed; the result
            // is no longer wanted, so a failed send is not an error.
            let _ = sender.send(build_arguments(&config_path, &template, &filters));
        })
        .map_err(|source| Error::Worker(format!("cannot start the evaluation thread: {source}")))?;

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(Error::Timeout {
            seconds: timeout.as_secs_f64(),
        }),
        Err(RecvTimeoutError::Disconnected) => Err(Error::Worker(
            "the evaluation thread ended without a result".to_owned(),
        )),
    }
}

/// Concatenates a command prefix with expanded arguments, matching the argv
/// `execute` spawns.
///
/// Returns [`Error::Unquotable`] if a prefix argument is not valid UTF-8.
/// Replacing the invalid bytes would print a command that differs from the one
/// `execute` spawns, which is exactly what a dry run must not do.
pub fn full_argv(prefix: &[OsString], expanded: &[String]) -> Result<Vec<String>, Error> {
    let mut argv = Vec::with_capacity(prefix.len() + expanded.len());
    for argument in prefix {
        let text = argument
            .to_str()
            .ok_or_else(|| Error::Unquotable(argument.to_string_lossy().into_owned()))?;
        argv.push(text.to_owned());
    }
    argv.extend(expanded.iter().cloned());
    Ok(argv)
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
