use crate::{ConfigPath, Error};

/// A validated, ordered argv template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Template {
    pub(crate) arguments: Vec<Argument>,
}

impl Template {
    /// Parses template arguments while preserving their order and boundaries.
    pub fn parse(arguments: &[String]) -> Result<Self, Error> {
        arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| parse_argument(argument, index + 1))
            .collect::<Result<_, _>>()
            .map(|arguments| Self { arguments })
    }

    /// Returns the number of template arguments.
    pub fn len(&self) -> usize {
        self.arguments.len()
    }

    /// Returns whether the template has no arguments.
    pub fn is_empty(&self) -> bool {
        self.arguments.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Argument {
    Interpolated(Vec<Part>),
    Spread { source: ConfigPath },
    Conditional { source: ConfigPath, option: String },
    Repeat { source: ConfigPath, option: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Part {
    Literal(String),
    Value {
        source: ConfigPath,
        default: Option<String>,
    },
}

enum Expression {
    Value {
        source: ConfigPath,
        default: Option<String>,
    },
    Spread {
        source: ConfigPath,
    },
    Conditional {
        source: ConfigPath,
        option: String,
    },
    Repeat {
        source: ConfigPath,
        option: String,
    },
}

fn parse_argument(input: &str, argument: usize) -> Result<Argument, Error> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut offset = 0;

    while offset < input.len() {
        let rest = &input[offset..];
        if rest.starts_with("{{") {
            literal.push('{');
            offset += 2;
            continue;
        }
        if rest.starts_with("}}") {
            literal.push('}');
            offset += 2;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("offset remains within the argument");
        match character {
            '{' => {
                push_literal(&mut parts, &mut literal);
                let body_start = offset + 1;
                let closing = input[body_start..]
                    .find('}')
                    .map(|relative| body_start + relative)
                    .ok_or_else(|| parse_error(argument, offset, "missing closing `}`"))?;
                let body = &input[body_start..closing];
                if body.contains('{') {
                    return Err(parse_error(
                        argument,
                        offset,
                        "nested `{` is not allowed inside a placeholder",
                    ));
                }
                let expression = parse_expression(body, argument, offset)?;
                parts.push(match expression {
                    Expression::Value { source, default } => Part::Value { source, default },
                    directive => {
                        if !parts.is_empty() || closing + 1 != input.len() {
                            return Err(parse_error(
                                argument,
                                offset,
                                "spread, conditional, and repeat placeholders must occupy the entire argument",
                            ));
                        }
                        return Ok(match directive {
                            Expression::Spread { source } => Argument::Spread { source },
                            Expression::Conditional { source, option } => {
                                Argument::Conditional { source, option }
                            }
                            Expression::Repeat { source, option } => {
                                Argument::Repeat { source, option }
                            }
                            Expression::Value { .. } => unreachable!(),
                        });
                    }
                });
                offset = closing + 1;
            }
            '}' => return Err(parse_error(argument, offset, "unmatched `}`")),
            _ => {
                literal.push(character);
                offset += character.len_utf8();
            }
        }
    }

    push_literal(&mut parts, &mut literal);
    Ok(Argument::Interpolated(parts))
}

fn parse_expression(body: &str, argument: usize, offset: usize) -> Result<Expression, Error> {
    if body.is_empty() {
        return Err(parse_error(
            argument,
            offset,
            "placeholders cannot be empty",
        ));
    }

    if let Some(directive) = body.strip_prefix('?') {
        let (path, option) = split_directive(directive, argument, offset, "conditional")?;
        validate_option(option, argument, offset)?;
        return Ok(Expression::Conditional {
            source: parse_path(path, argument, offset)?,
            option: option.to_owned(),
        });
    }

    if let Some(directive) = body.strip_prefix('*') {
        let (path, option) = split_directive(directive, argument, offset, "repeat")?;
        validate_option(option, argument, offset)?;
        return Ok(Expression::Repeat {
            source: parse_path(path, argument, offset)?,
            option: option.to_owned(),
        });
    }

    if let Some(path) = body.strip_suffix("...") {
        if path.contains(":-") {
            return Err(parse_error(
                argument,
                offset,
                "spread placeholders do not support defaults",
            ));
        }
        return Ok(Expression::Spread {
            source: parse_path(path, argument, offset)?,
        });
    }

    let (path, default) = match body.split_once(":-") {
        Some((path, default)) => (path, Some(default.to_owned())),
        None => (body, None),
    };
    Ok(Expression::Value {
        source: parse_path(path, argument, offset)?,
        default,
    })
}

fn split_directive<'a>(
    directive: &'a str,
    argument: usize,
    offset: usize,
    name: &str,
) -> Result<(&'a str, &'a str), Error> {
    let Some((path, option)) = directive.split_once(':') else {
        return Err(parse_error(
            argument,
            offset,
            format!("{name} placeholders require `:OPTION`"),
        ));
    };
    if path.is_empty() {
        return Err(parse_error(
            argument,
            offset,
            format!("{name} placeholders require a configuration path"),
        ));
    }
    if option.is_empty() {
        return Err(parse_error(
            argument,
            offset,
            format!("{name} placeholders require an option token"),
        ));
    }
    Ok((path, option))
}

fn validate_option(option: &str, argument: usize, offset: usize) -> Result<(), Error> {
    let valid = option.starts_with('-')
        && option != "-"
        && option != "--"
        && !option.contains(char::is_whitespace)
        && !option.contains('=');
    if valid {
        Ok(())
    } else {
        Err(parse_error(
            argument,
            offset,
            "directive output must be a `-x` or `--name` option without whitespace or `=`",
        ))
    }
}

fn parse_path(path: &str, argument: usize, offset: usize) -> Result<ConfigPath, Error> {
    ConfigPath::parse(path).map_err(|error| parse_error(argument, offset, error.to_string()))
}

fn push_literal(parts: &mut Vec<Part>, literal: &mut String) {
    if !literal.is_empty() {
        parts.push(Part::Literal(std::mem::take(literal)));
    }
}

fn parse_error(argument: usize, offset: usize, message: impl Into<String>) -> Error {
    Error::Parse {
        argument,
        offset,
        message: message.into(),
    }
}
