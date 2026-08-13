use std::collections::HashSet;

use crate::error::Error;
use crate::path::ConfigPath;

/// A validated, ordered collection of argument-generation rules.
///
/// Named options are unique within a grammar. Use [`Grammar::new`] to enforce
/// that invariant.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Grammar {
    rules: Vec<Rule>,
}

impl Grammar {
    /// Builds a grammar, rejecting duplicate named option tokens.
    pub fn new(rules: Vec<Rule>) -> Result<Self, Error> {
        let mut options = HashSet::new();
        for rule in &rules {
            if let Rule::Named { option, .. } = rule {
                if !options.insert(option) {
                    return Err(Error::InvalidGrammar(format!(
                        "duplicate option `{}`",
                        option.as_str()
                    )));
                }
            }
        }

        Ok(Self { rules })
    }

    /// Returns the rules in argument-emission order.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Layers `overlay` onto this grammar.
    ///
    /// An overlay named rule replaces the existing rule with the same exact
    /// option token and moves it to the overlay's position. If the overlay
    /// contains any positional rules, all existing positional rules are
    /// removed before the overlay is appended.
    pub fn merge(&mut self, overlay: Self) {
        let overlay_options: HashSet<_> = overlay
            .rules
            .iter()
            .filter_map(|rule| match rule {
                Rule::Named { option, .. } => Some(option),
                Rule::Positional { .. } => None,
            })
            .collect();
        let replaces_positionals = overlay
            .rules
            .iter()
            .any(|rule| matches!(rule, Rule::Positional { .. }));

        self.rules.retain(|rule| match rule {
            Rule::Named { option, .. } => !overlay_options.contains(option),
            Rule::Positional { .. } => !replaces_positionals,
        });
        self.rules.extend(overlay.rules);
    }
}

/// One named or positional argument-generation rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rule {
    /// A rule that emits an option token, optionally followed by values.
    Named {
        /// The option token to emit.
        option: OptionToken,
        /// The configuration path providing the input value.
        source: ConfigPath,
        /// How to translate the value for a named option.
        action: NamedAction,
        /// Whether a missing or null value is an error.
        required: bool,
    },
    /// A rule that emits one or more positional argument values.
    Positional {
        /// The configuration path providing the input value.
        source: ConfigPath,
        /// How to translate the value.
        action: Action,
    },
}

impl Rule {
    /// Builds a named rule from validated components.
    pub fn named(
        option: OptionToken,
        source: ConfigPath,
        action: NamedAction,
        required: bool,
    ) -> Self {
        Self::Named {
            option,
            source,
            action,
            required,
        }
    }

    /// Builds a positional rule from validated components.
    pub fn positional(source: ConfigPath, action: Action) -> Self {
        Self::Positional { source, action }
    }

    /// Returns the configuration path that supplies this rule's value.
    pub fn source(&self) -> &ConfigPath {
        let (Self::Named { source, .. } | Self::Positional { source, .. }) = self;
        source
    }
}

/// An action available to every rule, whether or not it has an option token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Chooses value emission for scalars and repeat emission for arrays.
    Auto,
    /// Emits exactly one scalar value.
    Value,
    /// Emits every scalar in an array as a separate argument.
    Repeat,
    /// Joins scalar array elements with the contained separator.
    Join(String),
}

/// A named rule's action: any shared action, plus the two that emit the option
/// token itself and so cannot appear on a positional rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedAction {
    /// An action also available to positional rules.
    Shared(Action),
    /// Emits the option token when the input boolean is true.
    Flag,
    /// Repeats the option token according to a nonnegative integer.
    Count,
}

impl From<Action> for NamedAction {
    fn from(action: Action) -> Self {
        Self::Shared(action)
    }
}

/// A validated option token, such as `-v` or `--seed`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OptionToken(String);

impl OptionToken {
    /// Parses and validates an option token.
    ///
    /// Tokens must begin with `-`, contain no whitespace or `=`, and must not
    /// be exactly `-` or `--`.
    pub fn parse(option: &str) -> Result<Self, Error> {
        let valid = option.starts_with('-')
            && option != "-"
            && option != "--"
            && !option.contains(char::is_whitespace)
            && !option.contains('=');

        if valid {
            Ok(Self(option.to_owned()))
        } else {
            Err(Error::InvalidRule {
                option: option.to_owned(),
                message:
                    "option must be a nonempty `-x` or `--name` token without whitespace or `=`"
                        .to_owned(),
            })
        }
    }

    /// Returns the validated option token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
