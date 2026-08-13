use std::fs;

use tempfile::tempdir;
use toargv::grammar::Rule;
use toargv::load::{load_config, load_grammar, load_inline_grammar};
use toargv::{Error, render_shell};

fn shell(arguments: &[&str]) -> String {
    let owned: Vec<String> = arguments.iter().map(|value| value.to_string()).collect();
    render_shell(&owned).unwrap()
}

#[test]
fn loads_inline_grammar_file() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("grammar.grm");
    fs::write(&path, "[-o output]\n<input>\n").unwrap();

    let grammar = load_grammar(&path).unwrap();

    assert_eq!(grammar.rules().len(), 2);
    assert!(matches!(grammar.rules()[0], Rule::Named { .. }));
    assert!(matches!(grammar.rules()[1], Rule::Positional { .. }));
}

#[test]
fn loads_inline_grammar() {
    let grammar = load_inline_grammar("  [-o output] <input>  ").unwrap();

    assert_eq!(grammar.rules().len(), 2);
}

#[test]
fn inline_grammar_path_errors_hint_at_grammar_file() {
    let error = load_inline_grammar("grammar.toml").unwrap_err().to_string();

    assert!(error.contains("-f/--grammar-file"));
}

#[test]
fn grammar_file_parse_errors_include_the_file_path() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("broken.grm");
    fs::write(&path, "[--output\n").unwrap();

    let error = load_grammar(&path).unwrap_err().to_string();

    assert!(error.contains(path.to_str().unwrap()));
}

#[test]
fn loads_json_and_normalizes_toml_datetimes() {
    let directory = tempdir().unwrap();
    let json_path = directory.path().join("config.json");
    let toml_path = directory.path().join("config.toml");
    fs::write(&json_path, r#"{"nested":{"value":3}}"#).unwrap();
    fs::write(
        &toml_path,
        "date = 2026-08-13\ntime = 10:30:00\nwhen = 2026-08-13T10:30:00Z\n",
    )
    .unwrap();

    assert_eq!(load_config(&json_path).unwrap()["nested"]["value"], 3);
    assert_eq!(load_config(&toml_path).unwrap()["date"], "2026-08-13");
    assert_eq!(load_config(&toml_path).unwrap()["time"], "10:30:00");
    assert_eq!(
        load_config(&toml_path).unwrap()["when"],
        "2026-08-13T10:30:00Z"
    );
}

#[test]
fn shell_rendering_leaves_safe_tokens_unquoted() {
    assert_eq!(
        shell(&["--seed", "42", "path/to/file.txt"]),
        "--seed 42 path/to/file.txt"
    );
}

#[test]
fn shell_rendering_quotes_empty_and_unsafe_arguments() {
    assert_eq!(shell(&[""]), "''");
    assert_eq!(shell(&["two words"]), "'two words'");
    assert_eq!(shell(&["$HOME"]), "'$HOME'");
    assert_eq!(shell(&["it's"]), r#""it's""#);
    assert_eq!(shell(&["line\nbreak"]), "'line\nbreak'");
}

/// An argument needing both quoting styles is spliced, relying on adjacent
/// quoted segments concatenating into one shell word.
#[test]
fn shell_rendering_splices_mixed_quoting_styles() {
    assert_eq!(shell(&["it's $HOME"]), r#""it's "'$HOME'"#);
}

#[test]
fn shell_rendering_rejects_an_embedded_nul() {
    let arguments = vec!["safe".to_string(), "with\0nul".to_string()];

    let error = render_shell(&arguments).unwrap_err();

    assert!(matches!(&error, Error::Unquotable(argument) if argument == "with\0nul"));
    assert!(error.to_string().contains(r"with\0nul"));
}

#[test]
fn rejects_unknown_config_extensions() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("config.yaml");
    fs::write(&path, "value: 1").unwrap();

    assert!(
        load_config(&path)
            .unwrap_err()
            .to_string()
            .contains(".toml")
    );
}
