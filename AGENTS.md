# CLAUDE.md

This file provides guidance to AI agents when working with code in this repository.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings

# single test / single file
cargo test -p toargv --test cli renders_interleaved_arguments_as_shell_words
cargo test -p toargv-grammar --test grammar

# run the CLI without installing
cargo run -p toargv -- config.toml -f grammar
```

Pre-commit hooks are managed by [prek](https://github.com/j178/prek) (`prek.toml`, not
`.pre-commit-config.yaml`): `prek run --all-files` runs the fmt and clippy hooks above.

Rust edition 2024 with an MSRV of Rust 1.85.

## Architecture

Two crates, split by whether they touch the outside world:

- **`crates/toargv-grammar`** — pure library. No filesystem, no process spawning. Owns the
  grammar model, the config-path resolver, the argv emitter, and the codecs.
- **`crates/toargv`** — CLI plus the I/O layer: file loading, format detection, `Command`
  spawning, exit-code mapping. Re-exports the library's `emit`/`grammar` modules so callers
  of `toargv` as a library see one surface (`lib.rs:1-21`).

### The pipeline

`build_arguments` (`crates/toargv/src/lib.rs:23`) is the whole program:

```
config file ──load_config──▶ serde_json::Value
-f files ──load_grammar────────┐
                               ├──merge files, then inlines──▶ Grammar ──generate──▶ argv
-g inlines ──load_inline_grammar┘
```

Everything downstream of loading operates on `serde_json::Value`, including TOML input:
`load.rs:59 toml_to_json` converts TOML into JSON values (datetimes become strings, non-finite
floats are an error). This is why `ConfigPath::resolve` and every `emit_*` function only ever
handle one value type. Config format is chosen by case-sensitive `.toml`/`.json` extension.

### The grammar model

One ordered `Vec<Rule>`; argv is emitted in exactly that order, interleaving named and
positional arguments. A `Rule` is orthogonal by design (`grammar.rs:53`):

- `source: ConfigPath` — dotted path, no array indexing, no dot escaping
- `action: Action` — `Auto | Value | Repeat | Join(sep) | Flag | Count`
- `binding: Binding` — `Named { option, required }` or `Positional`

Named and positional rules share one `Action` enum; the illegal combinations
(`Flag`/`Count` on a positional) are rejected in `Rule::new`, and duplicate option tokens in
`Grammar::new`. **Both structs keep private fields with `Result`-returning constructors** —
an invalid `Grammar` cannot be constructed, so `generate` and the codecs never re-validate
structure. Preserve that invariant when adding fields.

`Grammar::merge` implements layering with two different rules: named rules are keyed by
their exact option token, and a later rule *replaces and relocates* an earlier one; positionals
are all-or-nothing — any positional in the overlay drops every earlier positional.
The CLI applies every `-f` file first, then every `-g` inline, so inline sources always
override files regardless of argv order.

### Codecs

`GrammarCodec` (`codecs/mod.rs`) is bidirectional: `decode` and `encode` for the same model.
`InlineCodec` is the only implementation. Its `encode` is canonical and round-trippable:
`escape` re-emits whitespace, control chars, and delimiters, so `decode(encode(g)) == g`.
The round-trip tests in `tests/grammar.rs` enforce this.

`-f` loads a grammar file as inline (`load.rs`). `-g` is the same syntax as an
argument string. There is no extension or prefix heuristic: a path that starts
with `[` or `<` is still a file under `-f`.

**Adding an `Action` variant means touching four places:** `grammar.rs` (the enum + validity
rules), `emit.rs` (`emit_action` and a new `emit_*`), and the inline codec's decode *and*
encode paths. The compiler catches all of them — the matches are exhaustive with no `_`
arms; keep them that way.

### The CLI surface

One flat command, no subcommands (`cli.rs`):

```
toargv <CONFIG> [-f <PATH>]... [-g <GRAMMAR>]... [--check | --json] [-n] [-- <COMMAND>...]
```

All four behaviours run the same `build_arguments` and differ only in what they do with the
resulting `Vec<String>`, so `main.rs:20 run` calls it once and then matches. Flag combinations
are resolved in exactly one place — `Cli::mode` — and `main.rs` matches the resulting `Mode`
rather than re-reading the booleans. `Mode::Print` carries a `prefix: &[OsString]` that is
empty for a plain print and the full command for `-n`, because the program prefix is dry-run's
*only* unique contribution; keep that as one arm rather than splitting it.

**`--exec` is deliberately absent.** A greedy option cannot express a trailing command safely:
without `allow_hyphen_values` the child can take no flags, and with it `--exec ruff --json`
silently swallows toargv's own `--json`. `--` is the only unambiguous separator, which
`trailing_command_flags_are_not_consumed` pins.

A trailing `--` with nothing after it leaves clap's `command` *absent*, not empty, so it
degrades to a plain print. That is what keeps `Error::MissingCommand` unreachable from the
binary — it is dead code reached only by library callers of `execute`. Dropping `last = true`'s
companion `required = true` is what allows this; don't add it back expecting a rejection.

### Output modes and the shell

Two opposite guarantees live here, and they are easy to conflate:

- **exec never goes through a shell.** `execute.rs` appends generated arguments to the
  `--`-separated command and spawns it directly, so spaces and shell metacharacters stay
  inside their own argv entries. `main.rs:40 exit_code` propagates the child's status, mapping
  signal termination to `128 + signal` on unix.
- **the default print is deliberately shell syntax.** `render_shell` (`lib.rs:58`) quotes each
  argument with `shlex::try_quote` so the line survives `eval` and copy-paste. Only the `try_*`
  API is used — the plain `quote`/`join` are deprecated and CVE-affected. Quoting is per
  argument rather than via `try_join` so `Error::Unquotable` can name the offending argument;
  it is reachable because a NUL is valid UTF-8 and can come from a config value.
  `render_json` (`lib.rs:54`) is the unambiguous machine path.

`full_argv` (`lib.rs:46`) builds the printed argv for `-n` and must stay in step with the argv
`execute` spawns; both sides carry a comment saying so.

## Testing conventions

All tests are integration tests under `tests/` — there are no `#[cfg(test)]` modules in `src/`,
which keeps them against the public API. CLI tests spawn the real binary via
`env!("CARGO_BIN_EXE_toargv")` with `tempfile` fixtures (`crates/toargv/tests/cli.rs`), so they
cover exit codes and stderr text, not just return values.

## Documentation

`README.md` is the user-facing grammar spec (action table, inline syntax, escaping rules,
layering semantics). Behaviour changes to actions, inline syntax, or merge rules need it
updated in the same change.
