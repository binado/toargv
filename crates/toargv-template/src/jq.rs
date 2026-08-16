use jaq_core::data::JustLut;
use jaq_core::{Compiler, Ctx, Vars, load};
use jaq_json::Val;
use serde_json::Value;

/// The maximum number of values a single filter may yield.
///
/// Bounds runaway filters such as unbounded recursion.
pub const MAX_OUTPUT_VALUES: usize = 100_000;

/// A jq compile or runtime failure, distinguished for diagnostics.
#[derive(Debug)]
pub(crate) enum JqError {
    Compile(String),
    Runtime(String),
}

/// Compiles `source` as a jq filter and runs it with `config` as the input.
///
/// The returned vector contains every value the filter yields, in order. At
/// most [`MAX_OUTPUT_VALUES`] values are collected before failing.
pub(crate) fn run(source: &str, config: &Value) -> Result<Vec<Val>, JqError> {
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
        .map_err(|errors| {
            JqError::Compile(
                errors
                    .iter()
                    .map(|error| format!("{error:?}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let filter = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errors| {
            JqError::Compile(
                errors
                    .into_iter()
                    .map(|error| format!("{error:?}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;

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
