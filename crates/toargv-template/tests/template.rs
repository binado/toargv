use serde_json::json;
use toargv_template::{Error, Template, expand};

fn template(arguments: &[&str]) -> Template {
    Template::parse(
        &arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn preserves_literals_and_expands_embedded_scalars_in_order() {
    let template = template(&[
        "--seed",
        "{seed}",
        "--output={directory}/{name}.txt",
        "{enabled}",
    ]);

    assert_eq!(
        expand(
            &json!({
                "seed": 42,
                "directory": "build output",
                "name": "server",
                "enabled": true,
            }),
            &template,
        )
        .unwrap(),
        ["--seed", "42", "--output=build output/server.txt", "true"]
    );
}

#[test]
fn resolves_raw_dotted_paths() {
    let template = template(&["{foo}", "{config.foo}"]);

    assert_eq!(
        expand(
            &json!({"foo": "root", "config": {"foo": "nested"}}),
            &template,
        )
        .unwrap(),
        ["root", "nested"]
    );
}

#[test]
fn defaults_only_replace_missing_and_null_values() {
    let template = template(&[
        "{missing:-fallback}",
        "{nothing:-fallback}",
        "{zero:-fallback}",
        "{disabled:-fallback}",
        "value={empty:-fallback}",
    ]);

    assert_eq!(
        expand(
            &json!({
                "nothing": null,
                "zero": 0,
                "disabled": false,
                "empty": "",
            }),
            &template,
        )
        .unwrap(),
        ["fallback", "fallback", "0", "false", "value="]
    );
}

#[test]
fn expands_spreads_conditionals_and_repeated_pairs() {
    let template = template(&[
        "command",
        "{files...}",
        "{?verbose:-v}",
        "{?quiet:--quiet}",
        "{*includes:--include}",
    ]);

    assert_eq!(
        expand(
            &json!({
                "files": ["one.txt", "two words.txt"],
                "verbose": true,
                "quiet": false,
                "includes": ["src", "tests"],
            }),
            &template,
        )
        .unwrap(),
        [
            "command",
            "one.txt",
            "two words.txt",
            "-v",
            "--include",
            "src",
            "--include",
            "tests",
        ]
    );
}

#[test]
fn empty_arrays_emit_no_arguments() {
    let template = template(&["before", "{values...}", "{*values:--value}", "after"]);

    assert_eq!(
        expand(&json!({"values": []}), &template).unwrap(),
        ["before", "after"]
    );
}

#[test]
fn doubled_braces_emit_literal_braces() {
    let template = template(&["{{literal}}", "{{{value}}}"]);

    assert_eq!(
        expand(&json!({"value": "inside"}), &template).unwrap(),
        ["{literal}", "{inside}"]
    );
}

#[test]
fn missing_values_are_strict_for_every_non_default_form() {
    for input in [
        "{missing}",
        "{missing...}",
        "{?missing:-v}",
        "{*missing:--file}",
    ] {
        let error = expand(&json!({}), &template(&[input])).unwrap_err();

        assert!(matches!(
            error,
            Error::MissingValue {
                argument: 1,
                ref path
            } if path == "missing"
        ));
    }
}

#[test]
fn rejects_values_incompatible_with_placeholder_forms() {
    for (input, config, expected) in [
        ("{value}", json!({"value": [1]}), "arrays require"),
        ("{value}", json!({"value": {}}), "objects cannot"),
        ("{value...}", json!({"value": 1}), "spread requires"),
        ("{?value:-v}", json!({"value": 1}), "conditional requires"),
        ("{*value:--item}", json!({"value": 1}), "repeat requires"),
        ("{*value:--item}", json!({"value": [{}]}), "objects cannot"),
    ] {
        let error = expand(&config, &template(&[input]))
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{input}: {error}");
    }
}

#[test]
fn rejects_malformed_templates_with_argument_locations() {
    for input in [
        "{",
        "}",
        "{}",
        "{foo{bar}",
        "{.foo}",
        "{foo.}",
        "{foo...:-fallback}",
        "{?foo}",
        "{?foo:value}",
        "{*foo}",
        "{*foo:--}",
        "prefix{foo...}",
        "{?foo:-v}suffix",
        "prefix{*foo:--foo}",
    ] {
        let error = Template::parse(&[input.to_owned()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("template argument 1"), "{input}: {error}");
        assert!(error.contains("byte"), "{input}: {error}");
    }
}

#[test]
fn empty_template_and_empty_literal_argument_are_distinct() {
    let empty = Template::parse(&[]).unwrap();
    let empty_argument = template(&[""]);

    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(expand(&json!({}), &empty).unwrap(), Vec::<String>::new());
    assert_eq!(expand(&json!({}), &empty_argument).unwrap(), [""]);
}
