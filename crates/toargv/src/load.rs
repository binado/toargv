use std::fs;
use std::path::Path;

use serde_json::{Map, Number, Value};
use toargv_grammar::{Grammar, GrammarCodec, InlineCodec};

use crate::Error;

/// Loads a TOML or JSON configuration based on the path's extension.
///
/// TOML data is normalized into [`Value`], including conversion of TOML
/// datetimes to strings.
pub fn load_config(path: &Path) -> Result<Value, Error> {
    let contents = read(path)?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => serde_json::from_str(&contents).map_err(|error| Error::ParseConfig {
            path: path.to_owned(),
            format: "JSON",
            message: error.to_string(),
        }),
        Some("toml") => {
            let value = toml::from_str(&contents).map_err(|error| Error::ParseConfig {
                path: path.to_owned(),
                format: "TOML",
                message: error.to_string(),
            })?;
            toml_to_json(value, path)
        }
        _ => Err(Error::UnsupportedConfigFormat(path.to_owned())),
    }
}

/// Loads and decodes a file containing inline grammar syntax.
pub fn load_grammar(path: &Path) -> Result<Grammar, Error> {
    let contents = read(path)?;
    InlineCodec
        .decode(&contents)
        .map_err(|error| Error::ParseGrammar {
            path: path.to_owned(),
            message: error.to_string(),
        })
}

/// Decodes inline grammar syntax supplied directly by a caller.
pub fn load_inline_grammar(source: &str) -> Result<Grammar, Error> {
    let source = source.trim();
    InlineCodec.decode(source).map_err(|error| {
        let message = if source.starts_with(['[', '<']) {
            error.to_string()
        } else {
            format!("{error}; if this is a grammar file, use -f/--grammar-file")
        };
        Error::ParseInlineGrammar { message }
    })
}

fn read(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })
}

fn toml_to_json(value: toml::Value, path: &Path) -> Result<Value, Error> {
    match value {
        toml::Value::String(value) => Ok(Value::String(value)),
        toml::Value::Integer(value) => Ok(Value::Number(Number::from(value))),
        toml::Value::Float(value) => {
            Number::from_f64(value)
                .map(Value::Number)
                .ok_or_else(|| Error::ParseConfig {
                    path: path.to_owned(),
                    format: "TOML",
                    message: "non-finite floats cannot be represented".to_owned(),
                })
        }
        toml::Value::Boolean(value) => Ok(Value::Bool(value)),
        toml::Value::Datetime(value) => Ok(Value::String(value.to_string())),
        toml::Value::Array(values) => values
            .into_iter()
            .map(|value| toml_to_json(value, path))
            .collect::<Result<_, _>>()
            .map(Value::Array),
        toml::Value::Table(values) => values
            .into_iter()
            .map(|(key, value)| toml_to_json(value, path).map(|value| (key, value)))
            .collect::<Result<Map<_, _>, _>>()
            .map(Value::Object),
    }
}
