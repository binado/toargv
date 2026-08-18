use std::collections::{HashMap, HashSet};

use jaq_json::Val;
use serde_json::Value;

use crate::error::{Error, SlotLabel};
use crate::jq::{self, JqError};
use crate::template::{Filter, Part, Template};

/// Expands a validated template against a JSON-compatible configuration tree.
///
/// Each filter is evaluated once with the configuration root as its input.
/// Words resolve in template order: a word consisting of exactly one slot
/// appends every value its filter yields, while any other word containing a
/// slot requires that filter to yield exactly one scalar value.
pub fn expand(
    config: &Value,
    template: &Template,
    filters: &[Filter],
) -> Result<Vec<String>, Error> {
    let mut positional: Vec<(usize, &Filter)> = Vec::new();
    let mut named: HashMap<&str, (usize, &Filter)> = HashMap::new();

    for (index, filter) in filters.iter().enumerate() {
        let argument = index + 1;
        match filter.name() {
            Some(name) => {
                if named.contains_key(name) {
                    return Err(Error::Binding {
                        argument,
                        message: format!("duplicate binding name `{name}`"),
                    });
                }
                named.insert(name, (argument, filter));
            }
            None => {
                positional.push((argument, filter));
            }
        }
    }

    let mut used_positional = HashSet::new();
    let mut used_named = HashSet::new();
    let mut first_use = Vec::new();
    let mut seen = HashSet::new();

    for word in &template.words {
        for part in &word.parts {
            let Part::Slot(label) = part else {
                continue;
            };
            match label {
                SlotLabel::Positional(index) if *index > positional.len() => {
                    return Err(Error::Expansion {
                        slot: label.clone(),
                        word: word.index,
                        message: format!(
                            "template references slot {index} but only {} positional filter(s) were provided",
                            positional.len()
                        ),
                    });
                }
                SlotLabel::Named(name) if !named.contains_key(name.as_str()) => {
                    let mut available: Vec<_> = named.keys().copied().collect();
                    available.sort_unstable();
                    let available = if available.is_empty() {
                        "none".to_owned()
                    } else {
                        available
                            .iter()
                            .map(|name| format!("`{name}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    return Err(Error::Expansion {
                        slot: label.clone(),
                        word: word.index,
                        message: format!("no filter is bound to `{name}` (available: {available})"),
                    });
                }
                SlotLabel::Positional(index) => {
                    used_positional.insert(*index);
                }
                SlotLabel::Named(name) => {
                    used_named.insert(name.as_str());
                }
            }
            if seen.insert(label) {
                first_use.push((label, word.index));
            }
        }
    }

    for (index, (argument, _)) in positional.iter().enumerate() {
        if !used_positional.contains(&(index + 1)) {
            return Err(Error::Binding {
                argument: *argument,
                message: "positional filter is not referenced by any template slot".to_owned(),
            });
        }
    }
    // Sorted by argument so the earliest unreferenced *binding* is blamed rather
    // than whichever one the hash map happens to yield first. The positional loop
    // above runs to completion first, so an unreferenced positional always wins
    // over an unreferenced binding, even one written earlier.
    let mut unreferenced: Vec<(&str, usize)> = named
        .iter()
        .filter(|(name, _)| !used_named.contains(*name))
        .map(|(name, (argument, _))| (*name, *argument))
        .collect();
    unreferenced.sort_by_key(|(_, argument)| *argument);
    if let Some((name, argument)) = unreferenced.first() {
        return Err(Error::Binding {
            argument: *argument,
            message: format!("filter bound to `{name}` is not referenced by any template slot"),
        });
    }

    // Evaluate each referenced filter once, in first-use order.
    let mut evaluated: HashMap<&SlotLabel, Vec<Val>> = HashMap::new();
    for (label, first_word) in first_use {
        let source = match label {
            SlotLabel::Positional(index) => positional[index - 1].1.source(),
            SlotLabel::Named(name) => named[name.as_str()].1.source(),
        };
        let values = jq::run(source, config).map_err(|error| match error {
            JqError::Compile(message) => Error::Compile {
                slot: label.clone(),
                message,
            },
            JqError::Runtime(message) => Error::Expansion {
                slot: label.clone(),
                word: first_word,
                message,
            },
        })?;
        evaluated.insert(label, values);
    }

    // Emit argv entries in word order.
    let mut output = Vec::new();
    for word in &template.words {
        match word.parts.as_slice() {
            [Part::Slot(label)] => {
                for value in &evaluated[label] {
                    output.push(scalar(value).map_err(|message| Error::Expansion {
                        slot: label.clone(),
                        word: word.index,
                        message: message.to_owned(),
                    })?);
                }
            }
            _ => {
                let mut expanded = String::new();
                for part in &word.parts {
                    match part {
                        Part::Literal(literal) => expanded.push_str(literal),
                        Part::Slot(label) => {
                            let values = &evaluated[label];
                            if values.len() != 1 {
                                return Err(Error::Expansion {
                                    slot: label.clone(),
                                    word: word.index,
                                    message: format!(
                                        "embedded slots require exactly one value, but the filter yielded {}",
                                        values.len()
                                    ),
                                });
                            }
                            expanded.push_str(&scalar(&values[0]).map_err(|message| {
                                Error::Expansion {
                                    slot: label.clone(),
                                    word: word.index,
                                    message: message.to_owned(),
                                }
                            })?);
                        }
                    }
                }
                output.push(expanded);
            }
        }
    }

    Ok(output)
}

fn scalar(value: &Val) -> Result<String, &'static str> {
    match value {
        Val::Bool(boolean) => Ok(boolean.to_string()),
        Val::Num(number) => Ok(number.to_string()),
        Val::TStr(bytes) => std::str::from_utf8(bytes.as_ref() as &[u8])
            .map(str::to_owned)
            .map_err(|_| "filter produced a string that is not valid UTF-8"),
        Val::BStr(_) => Err("byte strings cannot be expanded as argument values"),
        Val::Null => Err(
            "filters must not produce null (missing keys yield null; for defaults, use `// \"default\"`)",
        ),
        Val::Arr(_) => Err("arrays must be iterated inside the filter, e.g. `.field[]`"),
        Val::Obj(_) => Err("objects cannot be expanded as argument values"),
    }
}
