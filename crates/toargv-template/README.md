# toargv-template

`toargv-template` is the pure template library behind
[`toargv`](https://crates.io/crates/toargv). It provides:

- a validated, ordered argv template model;
- parsing for scalar/default interpolation, array spread, conditional options,
  and repeated option/value pairs; and
- expansion from `serde_json::Value` configuration trees.

The crate performs no filesystem access and does not spawn processes. See the
[`toargv` repository](https://github.com/binado/toargv) for the complete
template syntax and CLI documentation.

## License

Licensed under the MIT License.
