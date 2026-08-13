mod inline;

use thiserror::Error;

use crate::error::Error as GrammarError;
use crate::grammar::Grammar;

pub use inline::InlineCodec;

/// A bidirectional encoding for validated grammars.
pub trait GrammarCodec {
    /// Decodes a grammar from `input`.
    fn decode(&self, input: &str) -> Result<Grammar, CodecError>;
    /// Encodes `grammar` in a canonical, round-trippable representation.
    fn encode(&self, grammar: &Grammar) -> Result<String, CodecError>;
}

/// A grammar codec or validation failure.
#[derive(Debug, Error)]
pub enum CodecError {
    /// Invalid inline grammar syntax at a byte offset.
    #[error("invalid inline grammar at byte {offset}: {message}")]
    Inline {
        /// Byte offset at or near the invalid syntax.
        offset: usize,
        /// Description of the syntax error.
        message: String,
    },

    /// The decoded rules violate a grammar invariant.
    #[error(transparent)]
    Grammar(#[from] GrammarError),
}

impl CodecError {
    pub(super) fn inline(offset: usize, message: impl Into<String>) -> Self {
        Self::Inline {
            offset,
            message: message.into(),
        }
    }
}
