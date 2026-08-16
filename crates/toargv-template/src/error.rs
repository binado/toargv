use std::fmt;

use thiserror::Error;

/// Identifies a template slot in diagnostics.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SlotLabel {
    /// The Nth positional filter, one-based.
    Positional(usize),
    /// A named filter binding.
    Named(String),
}

impl fmt::Display for SlotLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Positional(index) => write!(formatter, "slot {index}"),
            Self::Named(name) => write!(formatter, "slot `{name}`"),
        }
    }
}

/// An invalid template, filter argument, or expansion failure.
#[derive(Debug, Error)]
pub enum Error {
    /// The template string contains invalid syntax.
    #[error("invalid template at byte {offset}: {message}")]
    Parse {
        /// Byte offset within the template string.
        offset: usize,
        /// Why parsing failed.
        message: String,
    },

    /// A filter argument is malformed or redundant.
    #[error("invalid filter argument {argument}: {message}")]
    Binding {
        /// One-based position in the filter argument vector.
        argument: usize,
        /// Why the filter argument is invalid.
        message: String,
    },

    /// A filter failed to compile.
    #[error("cannot compile filter in {slot}: {message}")]
    Compile {
        /// The slot whose filter failed.
        slot: SlotLabel,
        /// Compiler error message.
        message: String,
    },

    /// A filter produced values incompatible with its slot.
    #[error("cannot expand {slot} in template word {word}: {message}")]
    Expansion {
        /// The slot whose filter is incompatible.
        slot: SlotLabel,
        /// One-based position of the template word.
        word: usize,
        /// Why the values cannot be expanded.
        message: String,
    },
}
