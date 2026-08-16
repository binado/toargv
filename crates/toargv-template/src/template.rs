use crate::Error;

/// A validated template string alongside its slot references.
///
/// The template is split into words by whitespace outside quotes. Words map
/// to one argument each, except words consisting of exactly one slot, which
/// may expand to any number of arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Template {
    pub(crate) words: Vec<Word>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Word {
    /// One-based position within the template.
    pub index: usize,
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Part {
    Literal(String),
    Slot(SlotRef),
}

/// An unresolved slot reference as written in the template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SlotRef {
    /// `{}`, consuming the next positional filter in order.
    Next,
    /// `{N}`, referring to the Nth positional filter (one-based).
    Index(usize),
    /// `{name}`, referring to a `name=FILTER` argument.
    Name(String),
}

impl Template {
    /// Parses a template string into words with resolved quoting and slots.
    ///
    /// Quoting governs word splitting only, following POSIX shell rules:
    /// single quotes are literal, double quotes escape `\` and `"` with a
    /// backslash, and a backslash outside quotes escapes the next character.
    ///
    /// Unlike a shell, quotes do not suppress slot interpolation: `{` and `}`
    /// keep their meaning inside quotes of either kind, so `'{}'` is a slot.
    /// Inside quotes `{{` and `}}` are the only way to write a literal brace;
    /// outside them a backslash escape (`\{`) works too. A trailing backslash
    /// with nothing left to escape is kept literally. Positional (`{}`, `{N}`)
    /// and named (`{name}`) slots cannot be mixed within one template.
    ///
    /// An opened quote is itself content: `""` contributes an empty literal,
    /// so `""{}` is a word with a literal and a slot rather than a bare slot,
    /// and its slot is embedded for cardinality purposes.
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut words = Vec::new();
        let mut parts = Vec::new();
        let mut literal = String::new();
        // Set when a quote opened within the current word: the section it
        // begins is a literal even if it turns out to contain nothing.
        let mut literal_opened = false;
        let mut word_started = false;
        let mut quote: Option<char> = None;
        let mut positional_offset: Option<usize> = None;
        let mut named_offset: Option<usize> = None;

        let mut chars = input.char_indices().peekable();
        while let Some((offset, character)) = chars.next() {
            if let Some(closing) = quote
                && character == closing
            {
                quote = None;
                continue;
            }

            match (quote, character) {
                (None, ' ' | '\t' | '\n' | '\r') => {
                    if word_started {
                        end_word(&mut words, &mut parts, &mut literal, &mut literal_opened);
                        word_started = false;
                    }
                }
                (None, '\'' | '"') => {
                    quote = Some(character);
                    literal_opened = true;
                    word_started = true;
                }
                (Some('"'), '\\') => {
                    match chars.peek().copied() {
                        Some((_, '"' | '\\')) => {
                            literal.push(chars.next().expect("peeked").1);
                        }
                        _ => literal.push('\\'),
                    }
                    word_started = true;
                }
                (None, '\\') => {
                    match chars.next() {
                        Some((_, escaped)) => literal.push(escaped),
                        None => literal.push('\\'),
                    }
                    word_started = true;
                }
                (_, '{') => {
                    if matches!(chars.peek(), Some((_, '{'))) {
                        chars.next();
                        literal.push('{');
                        word_started = true;
                        continue;
                    }
                    let slot = parse_slot(&mut chars, offset)?;
                    // The slot that breaks the rule is the one blamed; the
                    // earlier, still-legal slot is named in the message.
                    match &slot {
                        SlotRef::Next | SlotRef::Index(_) => {
                            if let Some(named) = named_offset {
                                return Err(parse_error(
                                    offset,
                                    format!(
                                        "named and positional slots cannot be mixed; \
                                         the named slot at byte {named} comes first"
                                    ),
                                ));
                            }
                            positional_offset.get_or_insert(offset);
                        }
                        SlotRef::Name(_) => {
                            if let Some(positional) = positional_offset {
                                return Err(parse_error(
                                    offset,
                                    format!(
                                        "named and positional slots cannot be mixed; \
                                         the positional slot at byte {positional} comes first"
                                    ),
                                ));
                            }
                            named_offset.get_or_insert(offset);
                        }
                    }
                    push_literal(&mut parts, &mut literal, &mut literal_opened);
                    parts.push(Part::Slot(slot));
                    word_started = true;
                }
                (_, '}') => {
                    if matches!(chars.peek(), Some((_, '}'))) {
                        chars.next();
                        literal.push('}');
                        word_started = true;
                        continue;
                    }
                    return Err(parse_error(
                        offset,
                        "unmatched `}`; write `}}` for a literal brace",
                    ));
                }
                (_, character) => {
                    literal.push(character);
                    word_started = true;
                }
            }
        }

        if let Some(character) = quote {
            return Err(parse_error(
                input.len(),
                format!("unterminated `{character}` quote"),
            ));
        }
        if word_started {
            end_word(&mut words, &mut parts, &mut literal, &mut literal_opened);
        }

        Ok(Self { words })
    }

    /// Returns the number of template words.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Returns whether the template has no words.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

/// Closes the word under construction. Every lexer arm that sets
/// `word_started` either pushes a part or opens a literal section, so the
/// finished word is never empty.
fn end_word(
    words: &mut Vec<Word>,
    parts: &mut Vec<Part>,
    literal: &mut String,
    literal_opened: &mut bool,
) {
    push_literal(parts, literal, literal_opened);
    words.push(Word {
        index: words.len() + 1,
        parts: std::mem::take(parts),
    });
}

/// Flushes buffered literal text into `parts`.
///
/// An empty buffer still produces a `Literal` when a quote opened the section:
/// `""` is written content, and dropping it would turn `""{}` into a bare slot
/// and silently give it the flattening cardinality of a whole-word slot.
fn push_literal(parts: &mut Vec<Part>, literal: &mut String, literal_opened: &mut bool) {
    if !literal.is_empty() || *literal_opened {
        parts.push(Part::Literal(std::mem::take(literal)));
    }
    *literal_opened = false;
}

/// Parses the slot body starting after `{` at `offset`, leaving the cursor
/// after the closing `}`.
fn parse_slot(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    offset: usize,
) -> Result<SlotRef, Error> {
    let mut body = String::new();
    for (_, character) in chars.by_ref() {
        if character == '}' {
            return slot_from_body(&body, offset);
        }
        match character {
            '{' => {
                return Err(parse_error(
                    offset,
                    "nested `{` is not allowed inside a slot",
                ));
            }
            _ => body.push(character),
        }
    }
    Err(parse_error(offset, "missing closing `}` for slot"))
}

fn slot_from_body(body: &str, offset: usize) -> Result<SlotRef, Error> {
    if body.is_empty() {
        return Ok(SlotRef::Next);
    }
    if body.bytes().all(|byte| byte.is_ascii_digit()) {
        let index: usize = body
            .parse()
            .map_err(|_| parse_error(offset, format!("slot index `{body}` is too large")))?;
        if index == 0 {
            return Err(parse_error(offset, "slot indexes are one-based"));
        }
        return Ok(SlotRef::Index(index));
    }
    let valid = body
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && body
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    if valid {
        Ok(SlotRef::Name(body.to_owned()))
    } else {
        Err(parse_error(
            offset,
            "slots must be empty, a one-based index, or a name",
        ))
    }
}

fn parse_error(offset: usize, message: impl Into<String>) -> Error {
    Error::Parse {
        offset,
        message: message.into(),
    }
}

/// A jq filter argument, either positional or bound to a name.
///
/// An argument is a binding when it starts with `NAME=` where `NAME` matches
/// `[A-Za-z_][A-Za-z0-9_]*`; otherwise it is a positional filter source. The
/// source is handed to the jq compiler verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    name: Option<String>,
    source: String,
}

impl Filter {
    /// Splits a raw filter argument into an optional binding name and source.
    pub fn parse(input: &str) -> Self {
        let prefix_len: usize = input
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .map(char::len_utf8)
            .sum();
        let is_binding = prefix_len > 0
            && !input
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
            && input[prefix_len..].starts_with('=');

        if is_binding {
            Self {
                name: Some(input[..prefix_len].to_owned()),
                source: input[prefix_len + 1..].to_owned(),
            }
        } else {
            Self {
                name: None,
                source: input.to_owned(),
            }
        }
    }

    /// Returns the binding name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the jq filter source.
    pub fn source(&self) -> &str {
        &self.source
    }
}
