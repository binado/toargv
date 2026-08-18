# toargv-template

`toargv-template` is the pure template library behind
[`toargv`](https://crates.io/crates/toargv). A template is a single string that
expands into an ordered argument vector, with each slot filled by a
[jq](https://jqlang.org/) filter evaluated against a configuration tree.

```rust
use serde_json::json;
use toargv_template::{Filter, Template, expand};

let template = Template::parse("--output {} {}").unwrap();
let arguments = expand(
    &json!({"output": "result dir", "files": ["a.txt", "b.txt"]}),
    &template,
    &[Filter::parse(".output"), Filter::parse(".files[]")],
)
.unwrap();

assert_eq!(arguments, ["--output", "result dir", "a.txt", "b.txt"]);
```

It provides:

- a POSIX-shaped lexer that splits the template string into words, with
  `'..'`/`".."` quoting and backslash escapes governing word splitting only;
- three slot forms — `{}` for the next positional filter, `{N}` for the Nth,
  and `{name}` for a `NAME=FILTER` binding;
- jq evaluation through [jaq](https://github.com/01mf02/jaq), including its
  standard library, with each filter compiled and run once regardless of how
  often it is referenced;
- a cardinality rule tied to word structure: a slot embedded in a word requires
  exactly one scalar value, while a word that is exactly one slot flattens
  however many values the filter yields into argv; and
- validation ahead of evaluation, rejecting unknown slot references, unused
  filter arguments, duplicate binding names, and out-of-range indexes, with
  diagnostics that name the slot, template word, or filter argument at fault.

The crate performs no filesystem access, spawns no processes, and starts no
threads. Filters therefore run to completion: one that never terminates hangs
the calling thread, and the wall-clock guard for that case lives in the
`toargv` CLI. See the [`toargv` repository](https://github.com/binado/toargv)
for the complete template syntax and CLI documentation.

## License

Licensed under the MIT License.
