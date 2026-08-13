use serde_json::json;
use toargv_grammar::{
    Action, ConfigPath, Grammar, GrammarCodec, InlineCodec, NamedAction, OptionToken, Rule,
    generate,
};

fn named(option: &str, source: &str) -> Rule {
    named_with(option, source, Action::Auto, false)
}

fn named_with(option: &str, source: &str, action: impl Into<NamedAction>, required: bool) -> Rule {
    Rule::named(
        OptionToken::parse(option).unwrap(),
        ConfigPath::parse(source).unwrap(),
        action.into(),
        required,
    )
}

fn positional(source: &str) -> Rule {
    positional_with(source, Action::Auto)
}

fn positional_with(source: &str, action: Action) -> Rule {
    Rule::positional(ConfigPath::parse(source).unwrap(), action)
}

#[test]
fn rejects_duplicate_named_options() {
    let error = Grammar::new(vec![named("--seed", "first"), named("--seed", "second")])
        .unwrap_err()
        .to_string();

    assert!(error.contains("duplicate option `--seed`"));
}

#[test]
fn merge_moves_overrides_and_preserves_positionals_when_overlay_has_none() {
    let mut grammar = Grammar::new(vec![
        named("--first", "first"),
        positional("input"),
        named("--second", "old"),
    ])
    .unwrap();
    let overlay = Grammar::new(vec![named("--second", "new"), named("--third", "third")]).unwrap();

    grammar.merge(overlay);

    assert_eq!(
        grammar.rules(),
        [
            named("--first", "first"),
            positional("input"),
            named("--second", "new"),
            named("--third", "third"),
        ]
    );
}

#[test]
fn merge_replaces_all_positionals_when_overlay_has_any() {
    let mut grammar = Grammar::new(vec![
        positional("old.first"),
        named("--keep", "keep"),
        positional("old.second"),
    ])
    .unwrap();
    let overlay = Grammar::new(vec![named("--new", "new"), positional("replacement")]).unwrap();

    grammar.merge(overlay);

    assert_eq!(
        grammar.rules(),
        [
            named("--keep", "keep"),
            named("--new", "new"),
            positional("replacement"),
        ]
    );
}

#[test]
fn emits_rules_in_exact_interleaved_order() {
    let grammar = Grammar::new(vec![
        named("--seed", "seed"),
        positional("input"),
        named_with("--verbose", "verbose", NamedAction::Flag, false),
        named_with("--item", "items", Action::Repeat, false),
        named_with(
            "--features",
            "features",
            Action::Join(",".to_owned()),
            false,
        ),
        named_with("-v", "verbosity", NamedAction::Count, false),
        positional("output"),
    ])
    .unwrap();
    let config = json!({
        "seed": 42,
        "input": "source.txt",
        "verbose": true,
        "items": ["first", "two words"],
        "features": ["a", "b"],
        "verbosity": 2,
        "output": "result.txt",
    });

    assert_eq!(
        generate(&config, &grammar).unwrap(),
        [
            "--seed",
            "42",
            "source.txt",
            "--verbose",
            "--item",
            "first",
            "--item",
            "two words",
            "--features",
            "a,b",
            "-v",
            "-v",
            "result.txt",
        ]
    );
}

#[test]
fn emits_all_positional_modes() {
    let grammar = Grammar::new(vec![
        positional("scalar"),
        positional("auto_array"),
        positional_with("value", Action::Value),
        positional_with("repeat", Action::Repeat),
        positional_with("join", Action::Join(":".to_owned())),
    ])
    .unwrap();

    assert_eq!(
        generate(
            &json!({
                "scalar": true,
                "auto_array": ["a", "b"],
                "value": 42,
                "repeat": ["c", "d"],
                "join": ["e", "f"],
            }),
            &grammar,
        )
        .unwrap(),
        ["true", "a", "b", "42", "c", "d", "e:f"]
    );

    for (action, value) in [
        (Action::Value, json!([1])),
        (Action::Repeat, json!(1)),
        (Action::Join(",".to_owned()), json!(1)),
    ] {
        let grammar = Grammar::new(vec![positional_with("bad", action)]).unwrap();
        assert!(generate(&json!({ "bad": value }), &grammar).is_err());
    }
}

#[test]
fn omits_missing_optional_rules_and_reports_required_values() {
    let optional = Grammar::new(vec![
        named("--missing", "absent"),
        named("--null", "nothing"),
    ])
    .unwrap();
    assert_eq!(
        generate(&json!({ "nothing": null }), &optional).unwrap(),
        Vec::<String>::new()
    );

    let required = Grammar::new(vec![named_with(
        "--seed",
        "nested.seed",
        Action::Auto,
        true,
    )])
    .unwrap();
    let error = generate(&json!({}), &required).unwrap_err().to_string();
    assert!(error.contains("nested.seed"));
    assert!(error.contains("--seed"));

    let positional = Grammar::new(vec![positional("missing")]).unwrap();
    assert!(generate(&json!({}), &positional).is_err());
}

#[test]
fn inline_codec_decodes_all_specifiers_and_requiredness() {
    let grammar = InlineCodec
        .decode(
            "[--auto config.auto] [--value value config.value] \
             [--flag !f config.flag] [--repeat r config.repeat] \
             [--join join=, config.join] [--count !count config.count] \
             [--required ! config.required] \
             <auto config.scalar> <v config.value> <repeat config.repeat> <j=: config.join>",
        )
        .unwrap();

    assert_eq!(
        grammar.rules(),
        [
            named("--auto", "config.auto"),
            named_with("--value", "config.value", Action::Value, false),
            named_with("--flag", "config.flag", NamedAction::Flag, true),
            named_with("--repeat", "config.repeat", Action::Repeat, false),
            named_with("--join", "config.join", Action::Join(",".to_owned()), false,),
            named_with("--count", "config.count", NamedAction::Count, true),
            named_with("--required", "config.required", Action::Auto, true,),
            positional("config.scalar"),
            positional_with("config.value", Action::Value),
            positional_with("config.repeat", Action::Repeat),
            positional_with("config.join", Action::Join(":".to_owned()),),
        ]
    );
}

#[test]
fn inline_codec_encodes_canonically_and_round_trips_escapes() {
    let grammar = Grammar::new(vec![
        named("--auto", "config.auto"),
        named_with("--value", "config.value", Action::Value, true),
        named_with(
            "--close]",
            "config.some key",
            Action::Join(" ]\\\n".to_owned()),
            false,
        ),
        positional_with("files", Action::Repeat),
        positional_with("joined", Action::Join(String::new())),
    ])
    .unwrap();

    let encoded = InlineCodec.encode(&grammar).unwrap();

    assert_eq!(
        encoded,
        "[--auto config.auto] [--value !v config.value] \
         [--close\\] j=\\ \\]\\\\\\n config.some\\ key] <r files> <j= joined>"
    );
    assert_eq!(InlineCodec.decode(&encoded).unwrap(), grammar);
}

#[test]
fn inline_codec_round_trips_unicode_and_control_escapes() {
    let grammar = Grammar::new(vec![named_with(
        "--join",
        "config.value",
        Action::Join(" \u{00a0}\u{0007}".to_owned()),
        false,
    )])
    .unwrap();

    let encoded = InlineCodec.encode(&grammar).unwrap();

    assert!(encoded.contains(r"\ \u{A0}\u{7}"));
    assert_eq!(InlineCodec.decode(&encoded).unwrap(), grammar);
}

#[test]
fn inline_codec_rejects_invalid_rules_and_escapes() {
    for input in [
        "[-o config.output",
        "[-o]",
        "<input extra words>",
        "plain",
        "[-o first] [-o second]",
        "[--x unknown path]",
        "<f path>",
        "<c path>",
        "<!r path>",
        "[--x value path\\q]",
        "[--x value path\\]",
        r"[--x value path\u{}]",
        r"[--x value path\u{D800}]",
    ] {
        assert!(InlineCodec.decode(input).is_err(), "{input} should fail");
    }
}
