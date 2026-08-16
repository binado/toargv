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
            if nul_terminated {
                use std::io::Write;
                std::io::stdout()
                    .write_all(&render_nul(&argv)?)
                    .expect("failed to write to stdout");
            } else {
                println!("{}", render_shell(&argv)?);
            }
            Ok(None)
        }
        Mode::Exec { program } => execute(std::slice::from_ref(program), &arguments).map(Some),
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
