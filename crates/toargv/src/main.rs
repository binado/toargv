use std::process::{self, ExitStatus};

use clap::Parser;
use toargv::cli::{Cli, Mode};
use toargv::{build_arguments, execute, full_argv, render_nul, render_shell};

fn main() {
    let cli = Cli::parse();

    match run(cli) {
        Ok(Some(status)) => process::exit(exit_code(status)),
        Ok(None) => {}
        Err(error) => {
            eprintln!("error: {error}");
            process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<Option<ExitStatus>, toargv::Error> {
    let template = cli.template.as_deref().unwrap_or("");
    let arguments = build_arguments(&cli.config, template, &cli.filters)?;

    match cli.mode() {
        Mode::Check => Ok(None),
        Mode::Print {
            program,
            nul_terminated,
        } => {
            let prefix = program.map_or(&[][..], std::slice::from_ref);
            let argv = full_argv(prefix, &arguments);
            let bytes = if nul_terminated {
                render_nul(&argv)?
            } else {
                let mut line = render_shell(&argv)?.into_bytes();
                line.push(b'\n');
                line
            };
            write_stdout(&bytes);
            Ok(None)
        }
        Mode::Exec { program } => execute(std::slice::from_ref(program), &arguments).map(Some),
    }
}

/// Writes rendered output to stdout, exiting quietly when the reader is gone.
///
/// A consumer that stops reading — `| head`, or an `xargs -0` that aborts — is
/// normal for a filter, so a broken pipe ends the process silently rather than
/// panicking or reporting an error.
fn write_stdout(bytes: &[u8]) {
    use std::io::{ErrorKind, Write};

    let mut stdout = std::io::stdout();
    match stdout.write_all(bytes).and_then(|()| stdout.flush()) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::BrokenPipe => process::exit(0),
        Err(error) => {
            eprintln!("error: cannot write to stdout: {error}");
            process::exit(1);
        }
    }
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }

    signal_exit_code(status).unwrap_or(1)
}

#[cfg(unix)]
fn signal_exit_code(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;

    status.signal().map(|signal| 128 + signal)
}

#[cfg(not(unix))]
fn signal_exit_code(_status: ExitStatus) -> Option<i32> {
    None
}
