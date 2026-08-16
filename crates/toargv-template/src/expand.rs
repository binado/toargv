use std::collections::{BTreeMap, HashMap};

use jaq_json::Val;
use serde_json::Value;

use crate::error::{Error, SlotLabel};
use crate::jq::{self, JqError};
use crate::template::{Filter, Part, SlotRef, Template};

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
    let mut positional: Vec<&Filter> = Vec::new();
    let mut positional_arguments: Vec<usize> = Vec::new();
    let mut named: HashMap<&str, &Filter> = HashMap::new();
    // Ordered so that diagnostics list names and blame arguments deterministically.
    let mut named_arguments: BTreeMap<&str, usize> = BTreeMap::new();

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
                named.insert(name, filter);
                named_arguments.insert(name, argument);
            }
            None => {
                positional.push(filter);
                positional_arguments.push(argument);
            }
        }
    }

    // Resolve slot references to labels in template order, assigning `{}`
    // occurrences successive positional indexes.
    let mut words: Vec<(usize, Vec<ResolvedPart>)> = Vec::new();
    let mut next_index = 1usize;
    let mut referenced: Vec<(SlotLabel, usize)> = Vec::new();
    for word in &template.words {
        let mut resolved_parts = Vec::new();
        for part in &word.parts {
            let part = match part {
                Part::Literal(literal) => ResolvedPart::Literal(literal.clone()),
                Part::Slot(reference) => {
                    let label = match reference {
                        SlotRef::Next => {
                            let label = SlotLabel::Positional(next_index);
                            next_index += 1;
                            label
                        }
                        SlotRef::Index(index) => SlotLabel::Positional(*index),
                        SlotRef::Name(name) => SlotLabel::Named(name.clone()),
                    };
                    match &label {
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
                            let available = named_arguments
                                .keys()
                                .map(|name| format!("`{name}`"))
                                .collect::<Vec<_>>();
                            return Err(Error::Expansion {
                                slot: label.clone(),
                                word: word.index,
                                message: format!(
                                    "no filter is bound to `{name}` (available: {})",
                                    if available.is_empty() {
                                        "none".to_owned()
                                    } else {
                                        available.join(", ")
                                    }
                                ),
                            });
                        }
                        _ => {}
                    }
                    referenced.push((label.clone(), word.index));
                    ResolvedPart::Slot(label)
                }
            };
            resolved_parts.push(part);
        }
        words.push((word.index, resolved_parts));
    }

    // Reject filter arguments the template never references.
    let referenced_labels: std::collections::HashSet<&SlotLabel> =
        referenced.iter().map(|(label, _)| label).collect();
    for (index, _filter) in positional.iter().enumerate() {
        let label = SlotLabel::Positional(index + 1);
        if !referenced_labels.contains(&label) {
            return Err(Error::Binding {
                argument: positional_arguments[index],
                message: "positional filter is not referenced by any template slot".to_owned(),
            });
        }
    }
    // Sorted by argument so the earliest unreferenced *binding* is blamed rather
    // than whichever one the hash map happens to yield first. The positional loop
    // above runs to completion first, so an unreferenced positional always wins
    // over an unreferenced binding, even one written earlier.
    let mut unreferenced: Vec<(&str, usize)> = named_arguments
        .iter()
        .map(|(name, argument)| (*name, *argument))
        .collect();
    unreferenced.sort_by_key(|(_, argument)| *argument);
    for (name, argument) in unreferenced {
        let label = SlotLabel::Named(name.to_owned());
        if !referenced_labels.contains(&label) {
            return Err(Error::Binding {
                argument,
                message: format!("filter bound to `{name}` is not referenced by any template slot"),
            });
        }
    }

    // Evaluate each referenced filter once, in first-use order.
    let mut evaluated: HashMap<SlotLabel, Vec<Val>> = HashMap::new();
    for (label, first_word) in &referenced {
        if evaluated.contains_key(label) {
            continue;
        }
        let source = match label {
            SlotLabel::Positional(index) => positional[index - 1].source(),
            SlotLabel::Named(name) => named[name.as_str()].source(),
        };
        let values = jq::run(source, config).map_err(|error| match error {
            JqError::Compile(message) => Error::Compile {
                slot: label.clone(),
                message,
            },
            JqError::Runtime(message) => Error::Expansion {
                slot: label.clone(),
                word: *first_word,
                message,
            },
        })?;
        evaluated.insert(label.clone(), values);
    }

    // Emit argv entries in word order.
    let mut output = Vec::new();
    for (word_index, parts) in &words {
        match parts.as_slice() {
            [ResolvedPart::Slot(label)] => {
                for value in &evaluated[label] {
                    output.push(scalar(value).map_err(|message| Error::Expansion {
                        slot: label.clone(),
                        word: *word_index,
                        message: message.to_owned(),
                    })?);
                }
            }
            _ => {
                let mut expanded = String::new();
                for part in parts {
                    match part {
                        ResolvedPart::Literal(literal) => expanded.push_str(literal),
                        ResolvedPart::Slot(label) => {
                            let values = &evaluated[label];
                            if values.len() != 1 {
                                return Err(Error::Expansion {
                                    slot: label.clone(),
                                    word: *word_index,
                                    message: format!(
                                        "embedded slots require exactly one value, but the filter yielded {}",
                                        values.len()
                                    ),
                                });
                            }
                            expanded.push_str(&scalar(&values[0]).map_err(|message| {
                                Error::Expansion {
                                    slot: label.clone(),
                                    word: *word_index,
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

enum ResolvedPart {
    Literal(String),
    Slot(SlotLabel),
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
