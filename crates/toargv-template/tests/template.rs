use serde_json::{Value, json};
use toargv_template::{Error, Filter, Template, expand};

fn run(template: &str, config: &Value, filters: &[&str]) -> Result<Vec<String>, Error> {
    let template = Template::parse(template)?;
    let filters: Vec<Filter> = filters.iter().map(|source| Filter::parse(source)).collect();
    expand(config, &template, &filters)
}

#[test]
fn splits_words_on_whitespace() {
    let config = json!({});
    assert_eq!(
        run("--flag  value\t'more words'", &config, &[]).unwrap(),
        ["--flag", "value", "more words"]
    );
}

#[test]
fn quotes_preserve_inner_whitespace_and_remove_themselves() {
    let config = json!({});
    assert_eq!(
        run("--output 'result dir' \"quoted value\" plain", &config, &[]).unwrap(),
        ["--output", "result dir", "quoted value", "plain"]
    );
}

#[test]
fn quoted_empty_word_expands_to_an_empty_argument() {
    let config = json!({});
    assert_eq!(run("a '' b", &config, &[]).unwrap(), ["a", "", "b"]);
}

#[test]
fn backslash_escapes_the_next_character_outside_quotes() {
    let config = json!({});
    assert_eq!(
        run(r#"escaped\ space 'don'"'"'t'"#, &config, &[]).unwrap(),
        ["escaped space", "don't"]
    );
}

#[test]
fn double_quotes_only_escape_quotes_and_backslashes() {
    let config = json!({});
    assert_eq!(
        run(r#""a\\b" "c\"d" "keep\t""#, &config, &[]).unwrap(),
        ["a\\b", "c\"d", "keep\\t"]
    );
}

#[test]
fn doubled_braces_emit_literal_braces() {
    let config = json!({});
    assert_eq!(
        run("--glob '{{literal}}' }}x{{", &config, &[]).unwrap(),
        ["--glob", "{literal}", "}x{"]
    );
}

#[test]
fn empty_template_is_valid() {
    let config = json!({});
    assert_eq!(run("", &config, &[]).unwrap(), Vec::<String>::new());
    assert!(Template::parse("").unwrap().is_empty());
}

#[test]
fn rejects_malformed_words_with_offsets() {
    let config = json!({});
    for (template, message) in [
        ("}", "unmatched `}`"),
        ("{", "missing closing `}`"),
        ("{name", "missing closing `}`"),
        ("{a{b}", "nested `{`"),
        ("{0}", "one-based"),
        ("{not a slot}", "slots must be empty"),
        ("'open", "unterminated `'`"),
    ] {
        let Err(Error::Parse {
            message: actual, ..
        }) = run(template, &config, &[])
        else {
            panic!("template {template:?} must fail");
        };
        assert!(
            actual.contains(message),
            "template {template:?}: expected {message:?} in {actual:?}"
        );
    }
}

#[test]
fn positional_slots_consume_filters_in_order() {
    let config = json!({"user": {"name": "ada"}, "count": 3});
    assert_eq!(
        run("--user {} --count {}", &config, &[".user.name", ".count"]).unwrap(),
        ["--user", "ada", "--count", "3"]
    );
}

#[test]
fn indexed_slots_reorder_and_reuse_positional_filters() {
    let config = json!({"a": 1, "b": 2});
    assert_eq!(
        run("{2} {1} {2}", &config, &[".a", ".b"]).unwrap(),
        ["2", "1", "2"]
    );
}

#[test]
fn indexed_slots_do_not_advance_the_next_counter() {
    let config = json!({"a": 1, "b": 2});
    assert_eq!(
        run("{1} {} {}", &config, &[".a", ".b"]).unwrap(),
        ["1", "1", "2"]
    );
}

#[test]
fn named_slots_bind_filters_by_name_and_are_reusable() {
    let config = json!({"output": "result.txt", "jobs": 4});
    assert_eq!(
        run(
            "--output {output} --jobs {jobs} again-{output}",
            &config,
            &["jobs=.jobs", "output=.output"],
        )
        .unwrap(),
        ["--output", "result.txt", "--jobs", "4", "again-result.txt"]
    );
}

#[test]
fn mixing_positional_and_named_slots_is_rejected() {
    let config = json!({});
    for template in ["{} {name}", "{name} {}", "{1} {name}"] {
        let Err(Error::Parse { message, .. }) = run(template, &config, &[]) else {
            panic!("template {template:?} must fail");
        };
        assert!(message.contains("cannot be mixed"), "got: {message}");
    }
}

#[test]
fn whole_word_slots_flatten_filter_streams() {
    let config = json!({"files": ["first.csv", "second file.csv"], "verbose": true});
    assert_eq!(
        run(
            "--files {} {}",
            &config,
            &[".files[]", r#"if .verbose then "--verbose" else empty end"#],
        )
        .unwrap(),
        ["--files", "first.csv", "second file.csv", "--verbose"]
    );
}

#[test]
fn filters_emitting_pairs_replace_repeated_options() {
    let config = json!({"files": ["a", "b"]});
    assert_eq!(
        run("{}", &config, &[".files[] | \"--file\", ."]).unwrap(),
        ["--file", "a", "--file", "b"]
    );
}

#[test]
fn empty_streams_emit_no_arguments() {
    let config = json!({"files": [], "verbose": false});
    assert_eq!(
        run(
            "before {} between {} after",
            &config,
            &[".files[]", "if .verbose then \"--verbose\" else empty end"],
        )
        .unwrap(),
        ["before", "between", "after"]
    );
}

#[test]
fn booleans_and_numbers_become_text() {
    let config = json!({"enabled": true, "ratio": 1.5, "count": 4});
    assert_eq!(
        run("{} {} {}", &config, &[".enabled", ".ratio", ".count"]).unwrap(),
        ["true", "1.5", "4"]
    );
}

#[test]
fn missing_keys_fail_through_the_scalar_gate() {
    let config = json!({});
    let Err(Error::Expansion { slot, message, .. }) = run("{}", &config, &[".missing"]) else {
        panic!("expected expansion error");
    };
    assert_eq!(slot.to_string(), "slot 1");
    assert!(message.contains("null"), "got: {message}");
}

#[test]
fn arrays_objects_and_null_are_not_scalars() {
    for (source, message) in [
        (".array", "iterated inside the filter"),
        (".object", "objects cannot be expanded"),
        (".nothing", "must not produce null"),
    ] {
        let config = json!({"array": [1], "object": {}});
        let Err(Error::Expansion {
            message: actual, ..
        }) = run("{}", &config, &[source])
        else {
            panic!("filter {source:?} must fail");
        };
        assert!(actual.contains(message), "{source:?}: got {actual:?}");
    }
}

#[test]
fn embedded_slots_require_exactly_one_value() {
    let config = json!({"files": ["a", "b"], "none": []});
    let Err(Error::Expansion {
        slot,
        word,
        message,
    }) = run("--tag={}", &config, &[".files[]"])
    else {
        panic!("expected expansion error");
    };
    assert_eq!((slot.to_string(), word), ("slot 1".to_owned(), 1));
    assert!(message.contains("exactly one"), "got: {message}");

    let config = json!({});
    assert!(
        run("pre-{}-post", &config, &["empty"])
            .unwrap_err()
            .to_string()
            .contains("exactly one")
    );
}

#[test]
fn embedded_slots_can_repeat_within_a_word() {
    let config = json!({"first": "a", "second": "b"});
    assert_eq!(
        run("{1}-{}-{}", &config, &[".first", ".second"]).unwrap(),
        ["a-a-b"]
    );
}

#[test]
fn rejects_filter_errors_with_slot_labels() {
    let config = json!({});

    let Err(Error::Compile { slot, .. }) = run("{}", &config, &["def broken: ["]) else {
        panic!("expected compile error");
    };
    assert_eq!(slot.to_string(), "slot 1");

    let Err(Error::Expansion { slot, message, .. }) = run("{}", &config, &[".a + .b"]) else {
        panic!("expected runtime error");
    };
    assert_eq!(slot.to_string(), "slot 1");
    assert!(!message.is_empty());
}

#[test]
fn filter_sources_may_contain_any_bytes() {
    let config = json!({});
    assert_eq!(
        run(
            "{}",
            &config,
            // braces, strings, comments, and percent signs are all inert here
            &[r#". | {nested: {braces: "%} {{"}} # a comment with } and %}
              | "done""#],
        )
        .unwrap(),
        ["done"]
    );
}

#[test]
fn binding_syntax_splitting() {
    let binding = Filter::parse("name=.user.name");
    assert_eq!(binding.name(), Some("name"));
    assert_eq!(binding.source(), ".user.name");

    for positional in [".a = 1", "1x=.a", "=.a", ".a"] {
        assert_eq!(Filter::parse(positional).name(), None, "{positional:?}");
    }
}

#[test]
fn unknown_and_unused_filters_are_rejected() {
    let config = json!({});

    let Err(Error::Expansion { message, .. }) = run("{name}", &config, &["other=.a"]) else {
        panic!("expected unknown-binding error");
    };
    assert!(message.contains("`other`"), "got: {message}");

    let Err(Error::Binding { argument, .. }) = run("{}", &config, &[".a", ".b"]) else {
        panic!("expected unused-filter error");
    };
    assert_eq!(argument, 2);
}

#[test]
fn duplicate_binding_names_are_rejected() {
    let config = json!({});
    let Err(Error::Binding { argument, message }) = run("{name}", &config, &["name=.a", "name=.b"])
    else {
        panic!("expected duplicate-binding error");
    };
    assert_eq!(argument, 2);
    assert!(message.contains("duplicate"), "got: {message}");
}

#[test]
fn out_of_range_positional_slots_are_rejected() {
    let config = json!({});
    let Err(Error::Expansion { message, .. }) = run("{2}", &config, &[".a"]) else {
        panic!("expected out-of-range error");
    };
    assert!(message.contains("only 1"), "got: {message}");
}
