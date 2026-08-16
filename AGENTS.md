<coding_guidelines>
# CLAUDE.md

This file provides guidance to AI agents when working with code in this repository.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings

# single test / single file
cargo test -p toargv --test cli renders_positional_slots_in_template_order
cargo test -p toargv-template --test template

# run the CLI without installing
cargo run -p toargv -- config.toml '--output {}' '.output'
```

Pre-commit hooks are managed by [prek](https://github.com/j178/prek) (`prek.toml`, not
`.pre-commit-config.yaml`): `prek run --all-files` runs the fmt and clippy hooks above.

Rust edition 2024 with an MSRV of Rust 1.88.

## Architecture

Two crates, split by whether they touch the outside world:

- **`crates/toargv-template`** — pure library. No filesystem or process spawning. Owns
  the template lexer/model, the jq engine adapter, and the argv expander. Its jq
  evaluation runs through [jaq](https://github.com/01mf02/jaq) crates
  (`jaq-core`, `jaq-std`, `jaq-json` with the `serde` feature); pin `jaq-json`
  exactly and keep `jaq-core`/`jaq-std` within the versions it depends on.
- **`crates/toargv`** — CLI plus I/O: config loading, format detection, `Command`
  spawning, exit-code mapping, and shell/NUL rendering. Re-exports the template
  library so callers see one surface.

### The pipeline

`build_arguments` (`crates/toargv/src/lib.rs`) is the whole program:

```text
config file ──load_config──▶ serde_json::Value ──────────┐
                                                         ├──expand──▶ argv
TEMPLATE string ──Template::parse──▶ Template            │
FILTER args ──Filter::parse──▶ [Filter] ─────────────────┘
```

Everything downstream of loading operates on `serde_json::Value`, including TOML
input: `load.rs` converts TOML into JSON values (datetimes become strings,
non-finite floats are errors). Config format is selected by case-sensitive
`.toml`/`.json` extension.

### The template model

The template is **a single string**, split into words by a POSIX-shaped lexer
(`'..'`/`".."` quoting, backslash escapes, `{{`/`}}` literal braces). Quoting
governs **word splitting only**: braces keep their slot meaning inside quotes
of either kind, so `{{`/`}}` is the only way to write a literal brace. Every
content-producing lexer arm must set `word_started`, including the backslash
arms — a word built solely from escapes is still a word. Words
contain literal text and slots: `{}` (next positional filter), `{N}` (Nth
positional), or `{name}` (a `NAME=FILTER` binding argument). Positional and
named slot styles cannot be mixed (parse error). A filter argument is a
binding iff it matches `^[A-Za-z_][A-Za-z0-9_]*=`; everything else is a
positional jq program handed to jaq verbatim — jq source never appears inside
the template string, so filter bytes (braces, strings, comments) need no
escaping grammar.

Cardinality rules mirror the word structure:

- a slot *embedded* in a word requires its filter to yield exactly one scalar;
- a word that is exactly one slot flattens 0..N filter values into argv.

Every emitted value passes the `scalar()` gate in `expand.rs`: strings
verbatim, numbers/bools stringified, null/array/object rejected — strictness
on missing keys emerges from jq producing `null`, not from a special case.
Each filter is compiled and evaluated **once** regardless of reuse, via
`jq.rs`; a filter may yield at most `MAX_OUTPUT_VALUES` (100k) values, which
bounds runaway programs like unbounded recursion.

Validation happens before evaluation: unknown slot references, unused filter
arguments, duplicate binding names, and out-of-range indexes are all hard
errors. Diagnostics identify slots (`slot 2`, `slot {output}`), template words
(one-based), byte offsets (parse errors), or filter argument positions
(binding errors) — preserve that specificity when adding syntax.

### The CLI surface

One flat command, no subcommands (`cli.rs`):

```text
toargv <CONFIG> [TEMPLATE] [FILTER]... [-0 | -c | -e PROGRAM [-n]]
```

`template` uses clap's `allow_hyphen_values` so templates beginning with `-x`
words need no separator; defined flags (`-0/-c/-e/-n`) still win, so `--`
remains the escape for colliding values. `--exec` consumes exactly one
program; fixed child arguments (e.g. `-c 'script'`) belong to the template
string.

All behaviors call `build_arguments` once, then use `Cli::mode`:

- default print expands argv as shell syntax;
- `-0 --print0` expands argv as NUL-separated bytes;
- `--check` expands silently;
- `--exec PROGRAM` spawns the program with expanded argv;
- `-n --exec PROGRAM` prints the full command instead.

Do not re-read raw CLI booleans in `main.rs`; resolve combinations in `Cli::mode`.
An empty or omitted template is valid.

### Output modes and the shell

- **exec never goes through a shell.** `execute.rs` appends expanded arguments and
  spawns the selected program directly. Spaces and metacharacters remain inside
  their argv entries. Unix signal termination maps to `128 + signal`.
- **default print is deliberately shell syntax.** `render_shell` quotes each
  argument with `shlex::try_quote`. Use only the `try_*` API; deprecated plain
  quoting APIs have security issues. Per-argument quoting lets
  `Error::Unquotable` identify values containing NUL.
- **print0 is the machine format.** `render_nul` terminates every argument with
  a NUL byte. NUL cannot occur inside an OS argument, so the encoding is
  lossless without a quoting grammar; arguments containing NUL themselves fail
  with `Error::NulByte`.
- **Both print modes write through `write_stdout`.** A closed reader (`| head`,
  an aborting `xargs -0`) is routine for a pipeline tool, so `BrokenPipe` exits
  0 silently. Never write to stdout with `println!` or `.expect(..)`, which
  turn a normal broken pipe into a panic and exit 101.

`filters` must not gain `allow_hyphen_values`. On a trailing variadic
positional it swallows `-0`, `-c`, `-e`, and `-n` written after the template;
filters starting with `-` go after `--`.

`full_argv` and `execute` must remain in step for dry-run output.

## Testing conventions

Tests are integration tests under `tests/`, not `#[cfg(test)]` modules in `src/`.
Pure tests cover lexing, slot resolution, cardinality, scalar gating, and jq
error propagation through public APIs. CLI tests spawn the real binary with
`env!(\"CARGO_BIN_EXE_toargv\")` and tempfile configs, covering argv boundaries,
output bytes, errors, exit statuses, and clap separation.

## Documentation

`README.md` is the user-facing template and CLI specification. Any syntax,
cardinality, type, argv-boundary, or execution behavior change requires a
matching README update.
</coding_guidelines>
