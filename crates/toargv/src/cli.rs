use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};

/// Parsed `toargv` command-line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "toargv",
    version,
    about = "Translate configuration values into command-line arguments",
    group(
        ArgGroup::new("grammars")
            .required(true)
            .multiple(true)
            .args(["grammar_file", "grammar"])
    )
)]
pub struct Cli {
    /// TOML or JSON configuration file
    pub config: PathBuf,

    /// Grammar file in inline syntax; repeat to layer files left to right
    #[arg(short = 'f', long = "grammar-file", value_name = "PATH")]
    pub grammar_file: Vec<PathBuf>,

    /// Inline grammar; repeat to layer inlines left to right, after all files
    #[arg(short = 'g', long = "grammar", value_name = "GRAMMAR")]
    pub grammar: Vec<String>,

    /// Check that the configuration can be translated, printing nothing
    #[arg(long, conflicts_with_all = ["json", "dry_run", "command"])]
    pub check: bool,

    /// Print generated arguments as a compact JSON array
    #[arg(long)]
    pub json: bool,

    /// Print the command instead of running it
    #[arg(short = 'n', long, requires = "command")]
    pub dry_run: bool,

    /// Command and fixed arguments to execute
    #[arg(last = true, num_args = 1.., value_name = "COMMAND")]
    pub command: Vec<OsString>,
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
    /// Validate argument generation without producing output.
    Check,
    /// Render generated arguments without executing a process.
    Print {
        /// Program and fixed arguments to print ahead of the generated ones; empty
        /// unless this is a dry run.
        prefix: &'a [OsString],
        /// Output representation.
        format: Format,
    },
    /// Execute a command with generated arguments appended.
    Exec {
        /// Program and fixed argument prefix.
        command: &'a [OsString],
    },
}

impl Cli {
    /// Resolves parsed flags into one mutually exclusive operating mode.
    pub fn mode(&self) -> Mode<'_> {
        if self.check {
            return Mode::Check;
        }

        if !self.command.is_empty() && !self.dry_run {
            return Mode::Exec {
                command: &self.command,
            };
        }

        Mode::Print {
            prefix: if self.dry_run { &self.command } else { &[] },
            format: if self.json {
                Format::Json
            } else {
                Format::Shell
            },
        }
    }
}
