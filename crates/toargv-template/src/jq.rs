use jaq_core::data::JustLut;
use jaq_core::{Compiler, Ctx, Vars, compile, load};
use jaq_json::Val;
use serde_json::Value;

/// The maximum number of values a single filter may yield.
///
/// Bounds filters that produce output without end, such as `repeat(.)`. It
/// says nothing about filters that loop or recurse without yielding; see
/// [`MAX_FILTER_DEPTH`] and the caller-side deadline in the `toargv` crate.
pub const MAX_OUTPUT_VALUES: usize = 100_000;

/// The maximum `(`/`[`/`{` nesting depth accepted in a filter source.
///
/// jaq parses with recursive descent, so a deeply nested source overflows the
/// stack — an abort the process cannot catch. The limit sits far above any
/// filter written by hand and far below where jaq runs out of stack.
pub const MAX_FILTER_DEPTH: usize = 256;

/// A jq compile or runtime failure, distinguished for diagnostics.
#[derive(Debug)]
pub(crate) enum JqError {
    Compile(String),
    Runtime(String),
}

/// Renders load (lex/parse) failures as `at byte N: expected X` messages.
///
/// jaq reports the unconsumed remainder of the input rather than a position,
/// so the offset is recovered by locating that remainder inside `source`.
fn format_load_errors(source: &str, errors: &load::Errors<&str, ()>) -> String {
    let mut messages = Vec::new();
    for (_file, error) in errors {
        match error {
            load::Error::Io(failures) => messages.extend(
                failures
                    .iter()
                    .map(|(path, message)| format!("cannot load `{path}`: {message}")),
            ),
            load::Error::Lex(failures) => {
                messages.extend(failures.iter().map(|(expect, remainder)| {
                    format!(
                        "{}: expected {}",
                        position(source, remainder),
                        lex_expect(expect)
                    )
                }));
            }
            load::Error::Parse(failures) => {
                messages.extend(failures.iter().map(|(expect, remainder)| {
                    format!(
                        "{}: expected {}",
                        position(source, remainder),
                        expect.as_str()
                    )
                }));
            }
        }
    }
    join(messages)
}

/// Renders undefined-symbol failures as `undefined filter \`foo\`` messages.
fn format_compile_errors(errors: &compile::Errors<&str, ()>) -> String {
    let messages = errors
        .iter()
        .flat_map(|(_file, failures)| failures)
        .map(|(symbol, undefined)| format!("undefined {} `{symbol}`", undefined.as_str()))
        .collect();
    join(messages)
}

fn join(messages: Vec<String>) -> String {
    if messages.is_empty() {
        "invalid filter".to_owned()
    } else {
        messages.join("; ")
    }
}

/// Describes a lexer expectation without going through `Expect::as_str`,
/// which panics on delimiters outside its known set.
fn lex_expect(expect: &load::lex::Expect<&str>) -> String {
    match expect {
        load::lex::Expect::Delim(delim) => match *delim {
            "(" => "closing parenthesis".to_owned(),
            "[" => "closing bracket".to_owned(),
            "{" => "closing brace".to_owned(),
            "\"" => "closing quote".to_owned(),
            other => format!("closing delimiter for `{other}`"),
        },
        other => other.as_str().to_owned(),
    }
}

/// Locates `remainder` within `source` to describe where an error occurred.
fn position(source: &str, remainder: &str) -> String {
    let base = source.as_ptr() as usize;
    let start = remainder.as_ptr() as usize;
    if start < base || start > base + source.len() {
        return "in filter".to_owned();
    }
    let offset = start - base;
    if offset >= source.len() {
        "at end of filter".to_owned()
    } else {
        format!("at byte {offset}")
    }
}

/// Returns the deepest `(`/`[`/`{` nesting reached anywhere in `source`.
///
/// jq strings (with `\` escapes) and `#` comments are skipped, since brackets
/// inside them are ordinary text. String interpolation `\(..)` is treated as a
/// plain escape, so its contents are counted as string bytes rather than as
/// nesting. That under-counts, which is the safe direction: the scan can miss
/// depth, but can never reject a filter that jaq would have parsed happily.
fn nesting_depth(source: &str) -> usize {
    let mut characters = source.chars();
    let mut depth = 0usize;
    let mut deepest = 0usize;
    let mut in_string = false;
    let mut in_comment = false;

    while let Some(character) = characters.next() {
        if in_comment {
            in_comment = character != '\n';
        } else if in_string {
            match character {
                '\\' => {
                    characters.next();
                }
                '"' => in_string = false,
                _ => {}
            }
        } else {
            match character {
                '"' => in_string = true,
                '#' => in_comment = true,
                '(' | '[' | '{' => {
                    depth += 1;
                    deepest = deepest.max(depth);
                }
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }

    deepest
}

/// Compiles `source` as a jq filter and runs it with `config` as the input.
///
/// The returned vector contains every value the filter yields, in order.
/// Two bounds apply. Sources nesting deeper than [`MAX_FILTER_DEPTH`] are
/// rejected before jaq parses them, and a filter yielding more than
/// [`MAX_OUTPUT_VALUES`] values fails once the value past the bound arrives.
/// Neither bounds a filter that recurses at run time without yielding: jaq
/// exposes no fuel or depth hook for its evaluator, so that failure mode is
/// handled by the wall-clock deadline in the `toargv` crate, and unbounded
/// *runtime* recursion can still overflow the stack and abort the process.
pub(crate) fn run(source: &str, config: &Value) -> Result<Vec<Val>, JqError> {
    let depth = nesting_depth(source);
    if depth > MAX_FILTER_DEPTH {
        return Err(JqError::Compile(format!(
            "filter nests brackets {depth} deep, exceeding the limit of {MAX_FILTER_DEPTH}"
        )));
    }

    let arena = load::Arena::default();
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let loader = load::Loader::new(defs);
    let modules = loader
        .load(
            &arena,
            load::File {
                path: (),
                code: source,
            },
        )
        .map_err(|errors| JqError::Compile(format_load_errors(source, &errors)))?;
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let filter = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errors| JqError::Compile(format_compile_errors(&errors)))?;

    let input: Val = serde_json::from_value(config.clone()).map_err(|error| {
        JqError::Runtime(format!("cannot convert configuration value: {error}"))
    })?;
    let context = Ctx::<JustLut<Val>>::new(&filter.lut, Vars::new([]));

    let mut values = Vec::new();
    for output in filter.id.run((context, input)) {
        match output {
            Ok(value) => {
                values.push(value);
                if values.len() > MAX_OUTPUT_VALUES {
                    return Err(JqError::Runtime(format!(
                        "filter yielded more than {MAX_OUTPUT_VALUES} values"
                    )));
                }
            }
            Err(exception) => {
                let exception = match exception.get_err() {
                    Ok(error) => return Err(JqError::Runtime(error.to_string())),
                    Err(exception) => exception,
                };
                return match exception.get_halt() {
                    Ok(code) => Err(JqError::Runtime(format!(
                        "filter halted with exit code {code}"
                    ))),
                    Err(_) => Err(JqError::Runtime(
                        "filter aborted with `break` outside of a label".to_owned(),
                    )),
                };
            }
        }
    }
    Ok(values)
}
