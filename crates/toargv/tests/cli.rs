use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;

fn toargv(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_toargv"))
        .args(arguments)
        .output()
        .unwrap()
}

fn fixture(config: &str) -> (TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    fs::write(&config_path, config).unwrap();
    (directory, config_path.to_string_lossy().into_owned())
}

const CONFIG: &str = "\
seed = 42
name = \"two words\"
input = \"source.txt\"
verbose = true
files = [\"one.txt\", \"two words.txt\"]
";

#[test]
fn renders_interpolated_arguments() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[
        &config,
        "--",
        "--seed",
        "{seed}",
        "{input}",
        "--name={name}",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--seed 42 source.txt '--name=two words'\n"
    );
}

#[test]
fn renders_arguments_as_shell_words() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[&config, "--", "--name", "{name}"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--name 'two words'\n"
    );
}

#[test]
fn expands_defaults_conditionals_spreads_and_repeated_pairs() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[
        &config,
        "--",
        "--output={output:-result.txt}",
        "{?verbose:-v}",
        "{files...}",
        "{*files:--file}",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "'--output=result.txt' -v one.txt 'two words.txt' --file one.txt --file 'two words.txt'\n"
    );
}

#[test]
fn target_flags_after_separator_are_not_consumed() {
    let (_directory, config) = fixture("");

    let output = toargv(&[&config, "--", "--json", "--check", "-n"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--json --check -n\n"
    );
}

#[test]
fn malformed_and_missing_placeholders_fail() {
    let (_directory, config) = fixture("");

    let malformed = toargv(&[&config, "--", "{missing"]);
    assert!(!malformed.status.success());
    assert!(
        String::from_utf8(malformed.stderr)
            .unwrap()
            .contains("template argument 1")
    );

    let missing = toargv(&[&config, "--", "{missing}"]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("configuration path `missing`")
    );
}

#[test]
fn check_is_silent_on_success() {
    let (_directory, config) = fixture("seed = 42\n");

    let output = toargv(&[&config, "--check", "--", "--seed", "{seed}"]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn check_conflicts_with_output_and_execution_modes() {
    let (_directory, config) = fixture("");

    for incompatible in [
        vec![&*config, "--check", "--exec", "true"],
        vec![&*config, "--check", "-n", "--exec", "true"],
    ] {
        let output = toargv(&incompatible);
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("--check")
        );
    }
}

#[test]
fn dry_run_requires_exec() {
    let (_directory, config) = fixture("");

    let output = toargv(&[&config, "-n"]);

    assert!(!output.status.success());
}

#[test]
fn empty_template_prints_an_empty_line() {
    let (_directory, config) = fixture("");

    let output = toargv(&[&config]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "\n");
}

#[cfg(unix)]
#[test]
fn exec_passes_expanded_arguments_without_shell_splitting() {
    let (_directory, config) = fixture("name = \"two words\"\nenabled = true\n");

    let output = toargv(&[
        &config,
        "--exec",
        "sh",
        "--",
        "-c",
        r#"printf '%s\n' "$@""#,
        "argv",
        "--name",
        "{name}",
        "{?enabled:--enabled}",
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
    let (_directory, config) = fixture("");

    let output = toargv(&[&config, "--exec", "sh", "--", "-c", "exit 7"]);

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn dry_run_prints_the_expanded_command() {
    let (_directory, config) = fixture("name = \"two words\"\n");

    let output = toargv(&[&config, "-n", "--exec", "program", "--", "--name", "{name}"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "program --name 'two words'\n"
    );
}
