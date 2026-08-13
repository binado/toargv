use serde_json::Value;

use crate::template::{Argument, Part};
use crate::{ConfigPath, Error, Template};

/// Expands a validated template against a JSON-compatible configuration tree.
///
/// Template order and argument boundaries are preserved. Missing strict values
/// and values incompatible with their placeholder forms are returned as
/// [`Error`] values.
pub fn expand(config: &Value, template: &Template) -> Result<Vec<String>, Error> {
    let mut output = Vec::new();

    for (index, argument) in template.arguments.iter().enumerate() {
        let argument_number = index + 1;
        match argument {
            Argument::Interpolated(parts) => {
                let mut expanded = String::new();
                for part in parts {
                    match part {
                        Part::Literal(literal) => expanded.push_str(literal),
                        Part::Value { source, default } => {
                            expanded.push_str(&expand_value(
                                config,
                                source,
                                default.as_deref(),
                                argument_number,
                            )?);
                        }
                    }
                }
                output.push(expanded);
            }
            Argument::Spread { source } => {
                let values = resolve(config, source, argument_number)?;
                let values = values.as_array().ok_or_else(|| {
                    expansion_error(argument_number, source, "spread requires an array")
                })?;
                for value in values {
                    output
                        .push(scalar(value).map_err(|message| {
                            expansion_error(argument_number, source, message)
                        })?);
                }
            }
            Argument::Conditional { source, option } => {
                let value = resolve(config, source, argument_number)?;
                match value {
                    Value::Bool(true) => output.push(option.clone()),
                    Value::Bool(false) => {}
                    _ => {
                        return Err(expansion_error(
                            argument_number,
                            source,
                            "conditional requires a boolean",
                        ));
                    }
                }
            }
            Argument::Repeat { source, option } => {
                let values = resolve(config, source, argument_number)?;
                let values = values.as_array().ok_or_else(|| {
                    expansion_error(argument_number, source, "repeat requires an array")
                })?;
                for value in values {
                    output.push(option.clone());
                    output
                        .push(scalar(value).map_err(|message| {
                            expansion_error(argument_number, source, message)
                        })?);
                }
            }
        }
    }

    Ok(output)
}

fn expand_value(
    config: &Value,
    source: &ConfigPath,
    default: Option<&str>,
    argument: usize,
) -> Result<String, Error> {
    match source.resolve(config) {
        Some(Value::Null) | None => default
            .map(str::to_owned)
            .ok_or_else(|| missing(source, argument)),
        Some(value) => scalar(value).map_err(|message| expansion_error(argument, source, message)),
    }
}

fn resolve<'a>(
    config: &'a Value,
    source: &ConfigPath,
    argument: usize,
) -> Result<&'a Value, Error> {
    source
        .resolve(config)
        .filter(|value| !value.is_null())
        .ok_or_else(|| missing(source, argument))
}

fn scalar(value: &Value) -> Result<String, &'static str> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Err("null is not a scalar argument value"),
        Value::Array(_) => Err("arrays require a spread or repeat placeholder"),
        Value::Object(_) => Err("objects cannot be expanded as argument values"),
    }
}

fn missing(source: &ConfigPath, argument: usize) -> Error {
    Error::MissingValue {
        path: source.as_str().to_owned(),
        argument,
    }
}

fn expansion_error(argument: usize, source: &ConfigPath, message: impl Into<String>) -> Error {
    Error::Expansion {
        argument,
        path: source.as_str().to_owned(),
        message: message.into(),
    }
}
