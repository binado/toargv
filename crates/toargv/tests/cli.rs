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
fn mixing_slot_styles_fails() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[
        &config,
        "--seed {} {} --name={name}",
        ".seed",
        ".input",
        "name=.name",
    ]);

    assert!(
        !output.status.success(),
        "named and positional slots must not mix: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn renders_positional_slots_in_template_order() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[
        &config,
        "--seed {} {} --name={}",
        ".seed",
        ".input",
        ".name",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--seed 42 source.txt '--name=two words'\n"
    );
}

#[test]
fn renders_named_slots_and_reuses_bindings() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[
        &config,
        "--seed {seed} --name={name} again-{name}",
        "seed=.seed",
        "name=.name",
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--seed 42 '--name=two words' 'again-two words'\n"
    );
}

#[test]
fn whole_word_slots_flatten_streams() {
    let (_directory, config) = fixture(CONFIG);

    let output = toargv(&[
        &config,
        "--files {} {}",
        ".files[]",
        r#"if .verbose then "--verbose" else empty end"#,
    ]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--files one.txt 'two words.txt' --verbose\n"
    );
}

#[test]
fn template_flag_words_after_separator_are_not_consumed_by_clap() {
    let (_directory, config) = fixture("");

    let output = toargv(&[&config, "--", "--json --check -n"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "--json --check -n\n"
    );
}

#[test]
fn malformed_templates_and_bad_slots_fail() {
    let (_directory, config) = fixture("");

    let malformed = toargv(&[&config, "{missing"]);
    assert!(!malformed.status.success());
    assert!(
        String::from_utf8(malformed.stderr)
            .unwrap()
            .contains("invalid template")
    );

    let missing = toargv(&[&config, "{}", ".missing"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8(missing.stderr).unwrap().contains("null"));

    let unbound = toargv(&[&config, "{name}", "other=.input"]);
    assert!(!unbound.status.success());
    assert!(
        String::from_utf8(unbound.stderr)
            .unwrap()
            .contains("slot `name`")
    );
}

#[test]
fn check_is_silent_on_success() {
    let (_directory, config) = fixture("seed = 42\n");

    let output = toargv(&[&config, "--check", "--seed {}", ".seed"]);

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
        vec![&*config, "--check", "-0"],
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

#[test]
fn print0_terminates_every_argument_with_nul() {
    let (_directory, config) = fixture("name = \"two words\"\n");

    let output = toargv(&[&config, "-0", "--name {} \"line1\nline2\"", ".name"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"--name\0two words\0line1\nline2\0");
}

#[test]
fn print0_conflicts_with_execution_modes() {
    let (_directory, config) = fixture("");

    for incompatible in [
        vec![&*config, "-0", "--exec", "true"],
        vec![&*config, "-0", "-n", "--exec", "true"],
    ] {
        assert!(!toargv(&incompatible).status.success());
    }
}

#[cfg(unix)]
#[test]
fn exec_passes_expanded_arguments_without_shell_splitting() {
    let (_directory, config) = fixture("name = \"two words\"\nenabled = true\n");

    let output = toargv(&[
        &config,
        "--exec",
        "sh",
        r#"-c 'printf "%s\n" "$@"' argv --name {} {}"#,
        ".name",
        r#"if .enabled then "--enabled" else empty end"#,
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

    let output = toargv(&[&config, "--exec", "sh", "-c 'exit 7'"]);

    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn dry_run_prints_the_expanded_command() {
    let (_directory, config) = fixture("name = \"two words\"\n");

    let output = toargv(&[&config, "-n", "--exec", "program", "--name {}", ".name"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "program --name 'two words'\n"
    );
}
