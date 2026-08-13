use std::ffi::OsString;
use std::path::PathBuf;

use clap::Parser;

/// Parsed `toargv` command-line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "toargv",
    version,
    about = "Expand configuration values into an argument template"
)]
pub struct Cli {
    /// TOML or JSON configuration file
    pub config: PathBuf,

    /// Check that the template can be expanded, printing nothing
    #[arg(long, conflicts_with_all = ["json", "dry_run", "exec"])]
    pub check: bool,

    /// Print expanded arguments as a compact JSON array
    #[arg(long)]
    pub json: bool,

    /// Program to execute with the expanded arguments
    #[arg(long, value_name = "PROGRAM")]
    pub exec: Option<OsString>,

    /// Print the expanded command instead of running it
    #[arg(short = 'n', long, requires = "exec")]
    pub dry_run: bool,

    /// Literal arguments and configuration placeholders to expand
    #[arg(last = true, value_name = "TEMPLATE")]
    pub template: Vec<String>,
}

/// How printed arguments are quoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// POSIX-like shell syntax with safely quoted arguments.
    Shell,
    /// A compact JSON string array.
    Json,
}

/// What a parsed invocation asks for, with the flag combinations already resolved.
#[derive(Debug)]
pub enum Mode<'a> {
    /// Validate template expansion without producing output.
    Check,
    /// Render expanded arguments without executing a process.
    Print {
        /// Program to print ahead of the expanded arguments for a dry run.
        program: Option<&'a OsString>,
        /// Output representation.
        format: Format,
    },
    /// Execute a command with expanded arguments appended.
    Exec {
        /// Program receiving the expanded arguments.
        program: &'a OsString,
    },
}

impl Cli {
    /// Resolves parsed flags into one mutually exclusive operating mode.
    pub fn mode(&self) -> Mode<'_> {
        if self.check {
            return Mode::Check;
        }

        if let Some(program) = &self.exec
            && !self.dry_run
        {
            return Mode::Exec { program };
        }

        Mode::Print {
            program: if self.dry_run {
                Some(
                    self.exec
                        .as_ref()
                        .expect("clap requires --exec when --dry-run is present"),
                )
            } else {
                None
            },
            format: if self.json {
                Format::Json
            } else {
                Format::Shell
            },
        }
    }
}
