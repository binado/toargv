use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;

fn toargv(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_toargv"))
        .args(arguments)
        .output()
        .unwrap()
}

fn fixture(config: &str, grammar: &str) -> (TempDir, String, String) {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let grammar_path = directory.path().join("grammar");
    fs::write(&config_path, config).unwrap();
    fs::write(&grammar_path, grammar).unwrap();
    (
        directory,
        config_path.to_string_lossy().into_owned(),
        grammar_path.to_string_lossy().into_owned(),
    )
}

const INTERLEAVED_CONFIG: &str = "seed = 42\nname = \"two words\"\ninput = \"source.txt\"\n";

const INTERLEAVED_GRAMMAR: &str = "[--seed seed] <input> [--name name]\n";

#[test]
fn renders_interleaved_arguments_as_json() {
    let (_directory, config, grammar) = fixture(INTERLEAVED_CONFIG, INTERLEAVED_GRAMMAR);

    let output = toargv(&[&config, "-f", &grammar, "--json"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--seed\",\"42\",\"source.txt\",\"--name\",\"two words\"]\n"
    );
}

#[test]
fn renders_interleaved_arguments_as_shell_words() {
    let (_directory, config, grammar) = fixture(INTERLEAVED_CONFIG, INTERLEAVED_GRAMMAR);

    let output = toargv(&[&config, "-f", &grammar]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--seed 42 source.txt --name 'two words'\n"
    );
}

#[test]
fn supports_inline_grammar() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.json");
    fs::write(&config, r#"{"output":"result.txt","input":"source.txt"}"#).unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-g",
        "[-o output] <input>",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"-o\",\"result.txt\",\"source.txt\"]\n"
    );
}

#[test]
fn supports_inline_grammar_file() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.json");
    let grammar = directory.path().join("grammar.grm");
    fs::write(&config, r#"{"output":"result.txt","input":"source.txt"}"#).unwrap();
    fs::write(&grammar, "[-o output]\n<input>\n").unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-f",
        grammar.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"-o\",\"result.txt\",\"source.txt\"]\n"
    );
}

#[test]
fn supports_complete_inline_actions() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.json");
    fs::write(
        &config,
        r#"{
            "output": "result.txt",
            "items": ["first", "second"],
            "features": ["fast", "safe"],
            "files": ["one.txt", "two.txt"]
        }"#,
    )
    .unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-g",
        r"[--output !v output] [--item r items] [--features j=\  features] <r files>",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--output\",\"result.txt\",\"--item\",\"first\",\"--item\",\"second\",\"--features\",\"fast safe\",\"one.txt\",\"two.txt\"]\n"
    );
}

#[test]
fn layers_repeated_grammar_sources_in_order() {
    let (_directory, config, grammar) = fixture(
        "first = 1\nold_second = 2\nnew_second = 20\ninput = \"source\"\nthird = 3\n",
        "[--first first] <input> [--second old_second]\n",
    );

    let output = toargv(&[
        &config,
        "-f",
        &grammar,
        "-g",
        "[--second new_second] [--third third]",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--first\",\"1\",\"source\",\"--second\",\"20\",\"--third\",\"3\"]\n"
    );
}

#[test]
fn later_positionals_replace_all_earlier_positionals() {
    let (_directory, config, grammar) = fixture(
        "old = \"old\"\nnew = \"new\"\nvalue = 1\n",
        "<old> [--value value]\n",
    );

    let output = toargv(&[&config, "-f", &grammar, "-g", "<new>", "--json"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--value\",\"1\",\"new\"]\n"
    );
}

#[test]
fn removed_rule_option_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.json");
    fs::write(&config, r#"{"seed":1}"#).unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-g",
        "[--seed seed]",
        "--rule=--seed=seed",
    ]);

    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("--rule"));
}

#[test]
fn grammar_is_required() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.json");
    fs::write(&config, "{}").unwrap();

    let output = toargv(&[config.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--grammar")
    );
}

#[test]
fn check_is_silent_on_success() {
    let (_directory, config, grammar) = fixture("seed = 42\n", "[--unused seed]\n");

    let output = toargv(&[&config, "-f", &grammar, "--check"]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn check_conflicts_with_json() {
    let (_directory, config, grammar) = fixture("seed = 42\n", "");

    let output = toargv(&[&config, "-f", &grammar, "--check", "--json"]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--check")
    );
}

#[test]
fn check_conflicts_with_a_trailing_command() {
    let (_directory, config, grammar) = fixture("seed = 42\n", "");

    let output = toargv(&[&config, "-f", &grammar, "--check", "--", "true"]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--check")
    );
}

#[test]
fn dry_run_requires_a_command() {
    let (_directory, config, grammar) = fixture("seed = 42\n", "");

    let output = toargv(&[&config, "-f", &grammar, "-n"]);

    assert!(!output.status.success());
}

/// A trailing `--` with nothing after it leaves `command` absent rather than
/// empty, so it degrades to a plain print. This is what keeps
/// `Error::MissingCommand` unreachable from the binary.
#[test]
fn a_bare_separator_prints_instead_of_executing() {
    let (_directory, config, grammar) = fixture("name = \"two words\"\n", "[--name name]\n");

    let output = toargv(&[&config, "-f", &grammar, "--"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--name 'two words'\n"
    );
}

#[cfg(unix)]
#[test]
fn exec_passes_generated_arguments_without_shell_splitting() {
    let (_directory, config, grammar) = fixture(
        "name = \"two words\"\nenabled = true\n",
        "[--name name] [--enabled enabled]\n",
    );

    let output = toargv(&[
        &config,
        "-f",
        &grammar,
        "--",
        "sh",
        "-c",
        r#"printf '%s\n' "$@""#,
        "argv",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--name\ntwo words\n--enabled\n"
    );
}

#[cfg(unix)]
#[test]
fn exec_propagates_the_child_exit_code() {
    let (_directory, config, grammar) = fixture("", "");

    let output = toargv(&[&config, "-f", &grammar, "--", "sh", "-c", "exit 7"]);

    assert_eq!(output.status.code(), Some(7));
}

#[cfg(unix)]
#[test]
fn trailing_command_flags_are_not_consumed() {
    let (_directory, config, grammar) = fixture("name = \"two words\"\n", "[--name name]\n");

    let output = toargv(&[
        &config,
        "-f",
        &grammar,
        "--",
        "sh",
        "-c",
        r#"printf '%s\n' "$@""#,
        "argv",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--json\n--name\ntwo words\n"
    );
}

#[test]
fn dry_run_prints_the_command_without_running_it() {
    let (_directory, config, grammar) = fixture("", "");

    let output = toargv(&[&config, "-f", &grammar, "-n", "--", "sh", "-c", "exit 7"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "sh -c 'exit 7'\n"
    );
}

#[test]
fn dry_run_composes_with_json() {
    let (_directory, config, grammar) = fixture("name = \"two words\"\n", "[--name name]\n");

    let output = toargv(&[
        &config,
        "-f",
        &grammar,
        "-n",
        "--json",
        "--",
        "program",
        "fixed-argument",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"program\",\"fixed-argument\",\"--name\",\"two words\"]\n"
    );
}

#[test]
fn inline_grammar_overrides_files_regardless_of_argv_order() {
    let (_directory, config, grammar) = fixture(
        "first = 1\nold_second = 2\nnew_second = 20\ninput = \"source\"\nthird = 3\n",
        "[--first first] <input> [--second old_second]\n",
    );

    let output = toargv(&[
        &config,
        "-g",
        "[--second new_second] [--third third]",
        "-f",
        &grammar,
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--first\",\"1\",\"source\",\"--second\",\"20\",\"--third\",\"3\"]\n"
    );
}

#[test]
fn grammar_files_layer_left_to_right() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.toml");
    let base = directory.path().join("base");
    let overlay = directory.path().join("overlay");
    fs::write(
        &config,
        "first = 1\nold_second = 2\nnew_second = 20\ninput = \"source\"\nthird = 3\n",
    )
    .unwrap();
    fs::write(&base, "[--first first] <input> [--second old_second]\n").unwrap();
    fs::write(&overlay, "[--second new_second] [--third third]\n").unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-f",
        base.to_str().unwrap(),
        "-f",
        overlay.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--first\",\"1\",\"source\",\"--second\",\"20\",\"--third\",\"3\"]\n"
    );
}

#[test]
fn inline_grammars_layer_left_to_right() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.toml");
    fs::write(
        &config,
        "first = 1\nold_second = 2\nnew_second = 20\ninput = \"source\"\nthird = 3\n",
    )
    .unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-g",
        "[--first first] <input> [--second old_second]",
        "-g",
        "[--second new_second] [--third third]",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--first\",\"1\",\"source\",\"--second\",\"20\",\"--third\",\"3\"]\n"
    );
}

#[cfg(unix)]
#[test]
fn grammar_file_names_may_start_with_inline_delimiters() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.toml");
    let grammar = directory.path().join("<foo>.toml");
    fs::write(&config, "value = 1\n").unwrap();
    fs::write(&grammar, "[--value value]\n").unwrap();

    let output = toargv(&[
        config.to_str().unwrap(),
        "-f",
        grammar.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[\"--value\",\"1\"]\n"
    );
}

#[test]
fn treating_a_grammar_file_as_inline_hints_at_grammar_file() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.toml");
    let grammar = directory.path().join("grammar.toml");
    fs::write(&config, "value = 1\n").unwrap();
    fs::write(&grammar, "").unwrap();

    let output = toargv(&[config.to_str().unwrap(), "-g", grammar.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("-f/--grammar-file"));
}
