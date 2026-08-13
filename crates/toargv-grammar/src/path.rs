use serde_json::Value;

use crate::error::Error;

/// A validated dotted path into a JSON-compatible configuration tree.
///
/// Each segment traverses an object key. Array indexes and escaping literal
/// dots in keys are not supported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPath {
    raw: String,
    segments: Vec<String>,
}

impl ConfigPath {
    /// Parses a nonempty dotted path with no empty segments.
    pub fn parse(path: &str) -> Result<Self, Error> {
        if path.is_empty() {
            return Err(Error::InvalidGrammar(
                "configuration paths cannot be empty".to_owned(),
            ));
        }

        let segments: Vec<_> = path.split('.').map(str::to_owned).collect();
        if segments.iter().any(String::is_empty) {
            return Err(Error::InvalidGrammar(format!(
                "configuration path `{path}` contains an empty segment"
            )));
        }

        Ok(Self {
            raw: path.to_owned(),
            segments,
        })
    }

    /// Resolves this path against `root`.
    ///
    /// Returns `None` if a segment is absent or an intermediate value is not
    /// an object.
    pub fn resolve<'a>(&self, root: &'a Value) -> Option<&'a Value> {
        let mut current = root;
        for segment in &self.segments {
            current = current.as_object()?.get(segment)?;
        }
        Some(current)
    }

    /// Returns the original dotted path.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}
