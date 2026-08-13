use thiserror::Error;

/// An invalid grammar or argument-generation failure.
#[derive(Debug, Error)]
pub enum Error {
    /// The grammar violates a grammar-wide invariant.
    #[error("invalid grammar: {0}")]
    InvalidGrammar(String),

    /// A rule contains an invalid option token or option/action combination.
    #[error("invalid rule for `{option}`: {message}")]
    InvalidRule {
        /// The invalid option token.
        option: String,
        /// Why the rule is invalid.
        message: String,
    },

    /// A required configuration path is missing or resolves to null.
    #[error("configuration path `{path}` was not found for {target}")]
    MissingValue {
        /// The unresolved dotted configuration path.
        path: String,
        /// The option or positional argument requiring the value.
        target: String,
    },

    /// A configuration value cannot be emitted by the rule's action.
    #[error("cannot emit {target} from configuration path `{path}`: {message}")]
    Emission {
        /// The dotted configuration path that resolved to the value.
        path: String,
        /// The option or positional argument receiving the value.
        target: String,
        /// Why the value and action are incompatible.
        message: String,
    },
}
