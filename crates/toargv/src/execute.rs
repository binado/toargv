use std::ffi::OsString;
use std::process::{Command, ExitStatus};

use crate::Error;

/// Appends expanded arguments to a command prefix and executes it directly.
///
/// No shell is involved. The returned status is the child process's status.
///
/// Returns [`Error::NulByte`] if an expanded argument contains a NUL byte.
/// The spawn would reject it anyway, but as an opaque `InvalidInput` blamed on
/// the program; checking first names the offending argument, as the print
/// modes do. Only `expanded` needs checking: arguments arriving in `command`
/// came from the operating system and are NUL-terminated by definition.
pub fn execute(command: &[OsString], expanded: &[String]) -> Result<ExitStatus, Error> {
    let Some(program) = command.first() else {
        return Err(Error::MissingCommand);
    };

    for argument in expanded {
        if argument.contains('\0') {
            return Err(Error::NulByte(argument.clone()));
        }
    }

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
