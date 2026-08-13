use std::process::{self, ExitStatus};

use clap::Parser;
use toargv::cli::{Cli, Format, Mode};
use toargv::{build_arguments, execute, full_argv, render_json, render_shell};

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
    let arguments = build_arguments(&cli.config, &cli.grammar_file, &cli.grammar)?;

    match cli.mode() {
        Mode::Check => Ok(None),
        Mode::Print { prefix, format } => {
            let argv = full_argv(prefix, &arguments);
            println!(
                "{}",
                match format {
                    Format::Shell => render_shell(&argv)?,
                    Format::Json => render_json(&argv),
                }
            );
            Ok(None)
        }
        Mode::Exec { command } => execute(command, &arguments).map(Some),
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
