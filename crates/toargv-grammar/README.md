# toargv-grammar

`toargv-grammar` is the pure grammar library behind
[`toargv`](https://crates.io/crates/toargv). It provides:

- a validated, ordered grammar model;
- a bidirectional codec for the inline grammar syntax; and
- argument generation from `serde_json::Value` configuration trees.

The crate performs no filesystem access and does not spawn processes. See the
[`toargv` repository](https://github.com/binado/toargv) for the complete syntax
reference and CLI documentation.

## License

Licensed under the MIT License.
