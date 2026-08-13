use serde_json::Value;

use crate::error::Error;
use crate::grammar::{Action, Grammar, NamedAction, Rule};
use crate::path::ConfigPath;

/// Applies `grammar` to `config` and returns the generated argument vector.
///
/// Rules are evaluated in grammar order. Missing required values and values
/// incompatible with their actions are returned as [`Error`] values.
pub fn generate(config: &Value, grammar: &Grammar) -> Result<Vec<String>, Error> {
    let mut output = Vec::new();
    let mut positional_index = 0;

    for rule in grammar.rules() {
        match rule {
            Rule::Named {
                option,
                source,
                action,
                required,
            } => match source.resolve(config) {
                Some(Value::Null) | None if *required => {
                    return Err(missing(source, format!("option `{}`", option.as_str())));
                }
                Some(Value::Null) | None => {}
                Some(value) => {
                    emit_named(option.as_str(), action, value, &mut output).map_err(|message| {
                        Error::Emission {
                            path: source.as_str().to_owned(),
                            target: format!("option `{}`", option.as_str()),
                            message,
                        }
                    })?;
                }
            },
            Rule::Positional { source, action } => {
                positional_index += 1;
                let target = format!("positional argument {positional_index}");
                let value = source
                    .resolve(config)
                    .filter(|value| !value.is_null())
                    .ok_or_else(|| missing(source, target.clone()))?;
                emit_shared(None, action, value, &mut output).map_err(|message| {
                    Error::Emission {
                        path: source.as_str().to_owned(),
                        target,
                        message,
                    }
                })?;
            }
        }
    }

    Ok(output)
}

fn emit_named(
    option: &str,
    action: &NamedAction,
    value: &Value,
    output: &mut Vec<String>,
) -> Result<(), String> {
    match action {
        NamedAction::Shared(action) => emit_shared(Some(option), action, value, output),
        NamedAction::Flag => emit_flag(option, value, output),
        NamedAction::Count => emit_count(option, value, output),
    }
}

fn emit_shared(
    option: Option<&str>,
    action: &Action,
    value: &Value,
    output: &mut Vec<String>,
) -> Result<(), String> {
    match action {
        Action::Auto => emit_auto(option, value, output),
        Action::Value => emit_value(option, value, output),
        Action::Repeat => emit_repeat(option, value, output),
        Action::Join(separator) => emit_join(option, value, separator, output),
    }
}

fn emit_auto(option: Option<&str>, value: &Value, output: &mut Vec<String>) -> Result<(), String> {
    match (value, option) {
        // A boolean means "emit the option token" when there is one to emit, and
        // the string `true` when there is not.
        (Value::Bool(_), Some(option)) => emit_flag(option, value, output),
        (Value::Array(_), _) => emit_repeat(option, value, output),
        (Value::Object(_), _) => Err("automatic action does not support objects".to_owned()),
        (Value::Null, _) => Ok(()),
        _ => emit_value(option, value, output),
    }
}

fn emit_value(option: Option<&str>, value: &Value, output: &mut Vec<String>) -> Result<(), String> {
    push_option(option, output);
    output.push(scalar(value)?);
    Ok(())
}

fn emit_flag(option: &str, value: &Value, output: &mut Vec<String>) -> Result<(), String> {
    match value {
        Value::Bool(true) => {
            output.push(option.to_owned());
            Ok(())
        }
        Value::Bool(false) => Ok(()),
        _ => Err("`flag` action requires a boolean".to_owned()),
    }
}

fn emit_repeat(
    option: Option<&str>,
    value: &Value,
    output: &mut Vec<String>,
) -> Result<(), String> {
    let values = value
        .as_array()
        .ok_or_else(|| "`repeat` action requires an array".to_owned())?;
    for value in values {
        push_option(option, output);
        output.push(scalar(value)?);
    }
    Ok(())
}

fn emit_join(
    option: Option<&str>,
    value: &Value,
    separator: &str,
    output: &mut Vec<String>,
) -> Result<(), String> {
    let values = value
        .as_array()
        .ok_or_else(|| "`join` action requires an array".to_owned())?;
    let joined = values
        .iter()
        .map(scalar)
        .collect::<Result<Vec<_>, _>>()?
        .join(separator);
    push_option(option, output);
    output.push(joined);
    Ok(())
}

fn emit_count(option: &str, value: &Value, output: &mut Vec<String>) -> Result<(), String> {
    let count = value
        .as_u64()
        .ok_or_else(|| "`count` action requires a nonnegative integer".to_owned())?;
    for _ in 0..count {
        output.push(option.to_owned());
    }
    Ok(())
}

fn push_option(option: Option<&str>, output: &mut Vec<String>) {
    if let Some(option) = option {
        output.push(option.to_owned());
    }
}

fn scalar(value: &Value) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Err("null is not a scalar argument value".to_owned()),
        Value::Array(_) => Err("arrays require `repeat` or `join` action".to_owned()),
        Value::Object(_) => Err("objects cannot be emitted as argument values".to_owned()),
    }
}

fn missing(path: &ConfigPath, target: String) -> Error {
    Error::MissingValue {
        path: path.as_str().to_owned(),
        target,
    }
}
