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
fn escaped_characters_start_a_word_on_their_own() {
    let config = json!({});
    // An escape is word-forming: a word built only from escapes must survive
    // splitting instead of merging into the next word or vanishing.
    assert_eq!(run(r"\a foo", &config, &[]).unwrap(), ["a", "foo"]);
    assert_eq!(run(r"\a \b", &config, &[]).unwrap(), ["a", "b"]);
    assert_eq!(run(r"x \{ y", &config, &[]).unwrap(), ["x", "{", "y"]);
    assert_eq!(run(r#""\\" z"#, &config, &[]).unwrap(), ["\\", "z"]);
}

#[test]
fn quotes_do_not_suppress_slot_interpolation() {
    let config = json!({"a": "value"});
    // Quoting governs word splitting only; braces keep their meaning inside
    // quotes of either kind, where doubling is the only way to write a literal
    // brace. Outside quotes a backslash escape works too, as
    // `escaped_characters_start_a_word_on_their_own` shows.
    assert_eq!(run("'{}'", &config, &[".a"]).unwrap(), ["value"]);
    assert_eq!(run(r#""{}""#, &config, &[".a"]).unwrap(), ["value"]);
    assert_eq!(run("'{{}}'", &config, &[]).unwrap(), ["{}"]);
}

#[test]
fn quoted_empty_literal_makes_a_slot_embedded() {
    let config = json!({"files": ["a", "b"], "one": "x"});

    // An opened quote is content even when it encloses nothing, so the word is
    // a literal plus a slot rather than a bare slot, and the flattening path
    // does not apply.
    for template in [r#"""{}"#, r#"{}"""#, "''{}", "{}''"] {
        let Err(Error::Expansion { message, .. }) = run(template, &config, &[".files[]"]) else {
            panic!("template {template:?} must require exactly one value");
        };
        assert!(message.contains("exactly one"), "{template:?}: {message}");

        assert_eq!(run(template, &config, &[".one"]).unwrap(), ["x"]);
    }

    // A quoted empty word on its own is still one empty argument.
    assert_eq!(run("''", &config, &[]).unwrap(), [""]);
    assert_eq!(run(r#""""#, &config, &[]).unwrap(), [""]);
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
fn mixing_slot_styles_blames_the_offending_slot() {
    let config = json!({});

    // The offset points at the slot that broke the rule, not at the earlier
    // slot that was legal when it was read.
    for (template, offending, earlier) in [
        ("{} {name}", 3, "positional slot at byte 0"),
        ("{name} {}", 7, "named slot at byte 0"),
    ] {
        let Err(Error::Parse { offset, message }) = run(template, &config, &[]) else {
            panic!("template {template:?} must fail");
        };
        assert_eq!(offset, offending, "template {template:?}");
        assert!(
            message.contains(earlier),
            "template {template:?}: {message}"
        );
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
            // braces, strings, and comments are all inert here: the filter is
            // handed to jq verbatim, with no template escaping grammar
            &[r#". | {nested: {braces: "%} {{"}} # a comment with } and %}
              | "done""#],
        )
        .unwrap(),
        ["done"]
    );
}

#[test]
fn deeply_nested_filter_sources_are_rejected() {
    let config = json!({});

    // jaq parses by recursive descent, so an over-nested source would overflow
    // the stack and abort. The pre-scan turns that into an ordinary error.
    let Err(Error::Compile { message, .. }) = run("{}", &config, &["[".repeat(5000).as_str()])
    else {
        panic!("a deeply nested filter must be rejected");
    };
    assert!(message.contains("exceeding the limit"), "got: {message}");

    // Nesting a hand-written filter could plausibly reach is still accepted.
    let nested = format!("{}1{}", "(".repeat(64), ")".repeat(64));
    assert_eq!(run("{}", &config, &[nested.as_str()]).unwrap(), ["1"]);

    // Brackets inside jq strings and comments are text, not nesting.
    let quoted = format!(r#""{}" # {}"#, "[".repeat(1000), "[".repeat(1000));
    assert_eq!(run("{}", &config, &[quoted.as_str()]).unwrap().len(), 1);
}

#[test]
fn filters_at_and_beyond_the_output_bound() {
    let config = json!({});

    // 100_000 values are the most a single filter may yield.
    assert_eq!(
        run("{}", &config, &["range(100000)"]).unwrap().len(),
        100_000
    );

    let Err(Error::Expansion { message, .. }) = run("{}", &config, &["range(100001)"]) else {
        panic!("a filter past the output bound must fail");
    };
    assert!(message.contains("more than 100000"), "got: {message}");
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
fn binding_diagnostics_are_deterministic() {
    let config = json!({});

    // The earliest unreferenced binding is blamed, matching the positional
    // branch, rather than whichever one a hash map happens to yield first.
    for _ in 0..32 {
        let Err(Error::Binding { argument, .. }) =
            run("{a}", &config, &["a=.a", "b=.b", "c=.c", "d=.d"])
        else {
            panic!("expected unused-binding error");
        };
        assert_eq!(argument, 2);
    }

    // The "available" hint is sorted by name.
    for _ in 0..32 {
        let Err(Error::Expansion { message, .. }) = run("{zz}", &config, &["a=.a", "m=.m", "b=.b"])
        else {
            panic!("expected unknown-binding error");
        };
        assert!(message.contains("`a`, `b`, `m`"), "got: {message}");
    }
}

#[test]
fn jq_compile_errors_are_readable() {
    let config = json!({});

    let Err(Error::Compile { message, .. }) = run("{}", &config, &["nosuchfn"]) else {
        panic!("expected compile error");
    };
    assert_eq!(message, "undefined filter `nosuchfn`");

    let Err(Error::Compile { message, .. }) = run("{}", &config, &[".foo ("]) else {
        panic!("expected compile error");
    };
    assert!(
        message.contains("expected closing parenthesis"),
        "got: {message}"
    );
    // Diagnostics carry a position and must not leak jaq's internal types.
    assert!(
        !message.contains("File {") && !message.contains("Parse(["),
        "message leaks jaq internals: {message}"
    );
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
