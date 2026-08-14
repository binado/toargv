use std::ffi::OsString;
use std::process::{Command, ExitStatus};

use crate::Error;

/// Appends expanded arguments to a command prefix and executes it directly.
///
/// No shell is involved. The returned status is the child process's status.
pub fn execute(command: &[OsString], expanded: &[String]) -> Result<ExitStatus, Error> {
    let Some(program) = command.first() else {
        return Err(Error::MissingCommand);
    };

    // This argv shape is what `full_argv` prints for a dry run; keep them in step.
    Command::new(program)
        .args(&command[1..])
        .args(expanded)
        .status()
        .map_err(|source| Error::Execute {
            program: program.to_string_lossy().into_owned(),
            source,
        })
}
