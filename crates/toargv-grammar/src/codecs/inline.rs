use crate::codecs::{CodecError, GrammarCodec};
use crate::grammar::{Action, Grammar, NamedAction, OptionToken, Rule};
use crate::path::ConfigPath;

/// Codec for toargv's bracket-and-angle-bracket inline grammar syntax.
///
/// Encoding is canonical and preserves every valid grammar when decoded
/// again.
#[derive(Clone, Copy, Debug, Default)]
pub struct InlineCodec;

impl GrammarCodec for InlineCodec {
    fn decode(&self, input: &str) -> Result<Grammar, CodecError> {
        let mut rules = Vec::new();
        let mut offset = 0;

        while offset < input.len() {
            offset = skip_whitespace(input, offset);
            if offset == input.len() {
                break;
            }

            let opening = input[offset..]
                .chars()
                .next()
                .expect("offset is before the end of input");
            let (closing, kind) = match opening {
                '[' => (']', RuleKind::Named),
                '<' => ('>', RuleKind::Positional),
                _ => {
                    return Err(CodecError::inline(
                        offset,
                        "expected a named rule beginning with `[` or a positional rule beginning with `<`",
                    ));
                }
            };
            let body_start = offset + opening.len_utf8();
            let closing_offset = find_closing(input, body_start, closing).ok_or_else(|| {
                CodecError::inline(offset, format!("missing closing delimiter `{closing}`"))
            })?;
            let body = &input[body_start..closing_offset];

            let rule = match kind {
                RuleKind::Named => parse_named(body, offset)?,
                RuleKind::Positional => parse_positional(body, offset)?,
            };
            rules.push(rule);
            offset = closing_offset + closing.len_utf8();
        }

        Grammar::new(rules).map_err(CodecError::from)
    }

    fn encode(&self, grammar: &Grammar) -> Result<String, CodecError> {
        Ok(grammar
            .rules()
            .iter()
            .map(format_rule)
            .collect::<Vec<_>>()
            .join(" "))
    }
}

#[derive(Clone, Copy)]
enum RuleKind {
    Named,
    Positional,
}

fn parse_named(body: &str, offset: usize) -> Result<Rule, CodecError> {
    let tokens = tokenize(body, offset)?;
    let (option, specifier, path) = match tokens.as_slice() {
        [option, path] => (option, None, path),
        [option, specifier, path] => (option, Some(specifier.as_str()), path),
        _ => {
            return Err(CodecError::inline(
                offset,
                "named rules must have the form `[OPTION PATH]` or `[OPTION SPEC PATH]`",
            ));
        }
    };
    let (action, required) = parse_named_action(specifier, offset)?;
    let source = ConfigPath::parse(path)?;
    let option = OptionToken::parse(option)?;
    Ok(Rule::named(option, source, action, required))
}

fn parse_positional(body: &str, offset: usize) -> Result<Rule, CodecError> {
    let tokens = tokenize(body, offset)?;
    let (specifier, path) = match tokens.as_slice() {
        [path] => (None, path),
        [specifier, path] => (Some(specifier.as_str()), path),
        _ => {
            return Err(CodecError::inline(
                offset,
                "positional rules must have the form `<PATH>` or `<SPEC PATH>`",
            ));
        }
    };
    let action = parse_positional_action(specifier, offset)?;
    let source = ConfigPath::parse(path)?;
    Ok(Rule::positional(source, action))
}

fn parse_named_action(
    specifier: Option<&str>,
    offset: usize,
) -> Result<(NamedAction, bool), CodecError> {
    let Some(specifier) = specifier else {
        return Ok((Action::Auto.into(), false));
    };
    let (required, mode) = match specifier.strip_prefix('!') {
        Some(mode) => (true, mode),
        None => (false, specifier),
    };
    let action = named_only_action(mode)
        .or_else(|| parse_shared_action(mode).map(NamedAction::from))
        .ok_or_else(|| invalid_specifier(offset, mode))?;
    Ok((action, required))
}

fn parse_positional_action(specifier: Option<&str>, offset: usize) -> Result<Action, CodecError> {
    let Some(specifier) = specifier else {
        return Ok(Action::Auto);
    };
    if specifier.starts_with('!') {
        return Err(CodecError::inline(
            offset,
            "`!` requiredness is only valid for named rules",
        ));
    }
    if named_only_action(specifier).is_some() {
        return Err(CodecError::inline(
            offset,
            format!("`{specifier}` action requires a named option"),
        ));
    }
    parse_shared_action(specifier).ok_or_else(|| invalid_specifier(offset, specifier))
}

fn parse_shared_action(specifier: &str) -> Option<Action> {
    match specifier {
        "" | "a" | "auto" => Some(Action::Auto),
        "v" | "value" => Some(Action::Value),
        "r" | "repeat" => Some(Action::Repeat),
        _ => join_separator(specifier).map(|separator| Action::Join(separator.to_owned())),
    }
}

fn named_only_action(specifier: &str) -> Option<NamedAction> {
    match specifier {
        "f" | "flag" => Some(NamedAction::Flag),
        "c" | "count" => Some(NamedAction::Count),
        _ => None,
    }
}

fn join_separator(specifier: &str) -> Option<&str> {
    specifier
        .strip_prefix("j=")
        .or_else(|| specifier.strip_prefix("join="))
}

fn invalid_specifier(offset: usize, specifier: &str) -> CodecError {
    CodecError::inline(offset, format!("unknown action specifier `{specifier}`"))
}

fn tokenize(body: &str, offset: usize) -> Result<Vec<String>, CodecError> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut chars = body.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        match character {
            '\\' => {
                let Some((_, escaped)) = chars.next() else {
                    return Err(CodecError::inline(
                        offset + index,
                        "dangling escape at end of rule",
                    ));
                };
                if escaped == 'u' {
                    token.push(decode_unicode_escape(&mut chars, offset + index)?);
                } else {
                    token.push(decode_escape(escaped).ok_or_else(|| {
                        CodecError::inline(
                            offset + index,
                            format!("unknown escape sequence `\\{escaped}`"),
                        )
                    })?);
                }
            }
            character if character.is_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            _ => token.push(character),
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn decode_unicode_escape(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    offset: usize,
) -> Result<char, CodecError> {
    if !matches!(chars.next(), Some((_, '{'))) {
        return Err(CodecError::inline(
            offset,
            "Unicode escapes must have the form `\\u{HEX}`",
        ));
    }

    let mut digits = String::new();
    loop {
        match chars.next() {
            Some((_, '}')) => break,
            Some((_, character)) if character.is_ascii_hexdigit() && digits.len() < 6 => {
                digits.push(character);
            }
            _ => {
                return Err(CodecError::inline(
                    offset,
                    "Unicode escapes must contain one to six hexadecimal digits",
                ));
            }
        }
    }
    if digits.is_empty() {
        return Err(CodecError::inline(
            offset,
            "Unicode escapes must contain one to six hexadecimal digits",
        ));
    }
    let value = u32::from_str_radix(&digits, 16).expect("digits were validated as hexadecimal");
    char::from_u32(value)
        .ok_or_else(|| CodecError::inline(offset, "Unicode escape is not a valid scalar value"))
}

fn decode_escape(character: char) -> Option<char> {
    match character {
        '\\' => Some('\\'),
        ' ' => Some(' '),
        't' => Some('\t'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        '[' => Some('['),
        ']' => Some(']'),
        '<' => Some('<'),
        '>' => Some('>'),
        _ => None,
    }
}

fn find_closing(input: &str, start: usize, closing: char) -> Option<usize> {
    let mut escaped = false;
    for (relative, character) in input[start..].char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == closing {
            return Some(start + relative);
        }
    }
    None
}

fn format_rule(rule: &Rule) -> String {
    let path = escape(rule.source().as_str());
    match rule {
        Rule::Named {
            option,
            action,
            required,
            ..
        } => {
            let option = escape(option.as_str());
            match named_specifier(action, *required) {
                Some(specifier) => format!("[{option} {specifier} {path}]"),
                None => format!("[{option} {path}]"),
            }
        }
        Rule::Positional { action, .. } => match shared_mode(action) {
            Some(mode) => format!("<{mode} {path}>"),
            None => format!("<{path}>"),
        },
    }
}

fn named_specifier(action: &NamedAction, required: bool) -> Option<String> {
    let mode = match action {
        NamedAction::Shared(action) => match shared_mode(action) {
            Some(mode) => mode,
            // `Auto` has no mode of its own, so a bare `!` is the whole specifier.
            None => return required.then(|| "!".to_owned()),
        },
        NamedAction::Flag => "f".to_owned(),
        NamedAction::Count => "c".to_owned(),
    };
    Some(if required { format!("!{mode}") } else { mode })
}

fn shared_mode(action: &Action) -> Option<String> {
    match action {
        Action::Auto => None,
        Action::Value => Some("v".to_owned()),
        Action::Repeat => Some("r".to_owned()),
        Action::Join(separator) => Some(format!("j={}", escape(separator))),
    }
}

fn escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            ' ' => escaped.push_str("\\ "),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '[' => escaped.push_str("\\["),
            ']' => escaped.push_str("\\]"),
            '<' => escaped.push_str("\\<"),
            '>' => escaped.push_str("\\>"),
            character if character.is_whitespace() || character.is_control() => {
                escaped.push_str(&format!("\\u{{{:X}}}", character as u32));
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn skip_whitespace(input: &str, mut offset: usize) -> usize {
    while let Some(character) = input[offset..].chars().next() {
        if !character.is_whitespace() {
            break;
        }
        offset += character.len_utf8();
    }
    offset
}
