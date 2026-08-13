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
cargo test -p toargv --test cli renders_interpolated_arguments_as_json
cargo test -p toargv-template --test template

# run the CLI without installing
cargo run -p toargv -- config.toml -- --output '{output}'
```

Pre-commit hooks are managed by [prek](https://github.com/j178/prek) (`prek.toml`, not
`.pre-commit-config.yaml`): `prek run --all-files` runs the fmt and clippy hooks above.

Rust edition 2024 with an MSRV of Rust 1.85.

## Architecture

Two crates, split by whether they touch the outside world:

- **`crates/toargv-template`** — pure library. No filesystem or process spawning. Owns
  the template model/parser, config-path resolver, and argv expander.
- **`crates/toargv`** — CLI plus I/O: config loading, format detection, `Command`
  spawning, exit-code mapping, and shell/JSON rendering. Re-exports the template
  library so callers see one surface.

### The pipeline

`build_arguments` (`crates/toargv/src/lib.rs`) is the whole program:

```text
config file ──load_config──▶ serde_json::Value ─┐
                                                ├──expand──▶ argv
args after -- ──Template::parse──▶ Template ────┘
```

Everything downstream of loading operates on `serde_json::Value`, including TOML
input: `load.rs` converts TOML into JSON values (datetimes become strings,
non-finite floats are errors). Config format is selected by case-sensitive
`.toml`/`.json` extension.

### The template model

`Template` is an ordered vector with private parsed argument nodes. Parse every
template argument before expansion so malformed templates cannot be constructed
through the public API. Each argument is one of:

- interpolated literal/scalar parts (`{path}`, `{path:-default}`);
- array spread (`{path...}`);
- conditional option (`{?path:-v}`); or
- repeated option/value pairs (`{*path:--file}`).

Scalar/default placeholders may be embedded and never change the containing
argument's boundary. Spread, conditional, and repeat placeholders must occupy a
whole argument because they can emit zero or multiple argv entries. `{{` and `}}`
emit literal braces.

Paths are raw dotted paths with no array indexing or dot escaping. Missing/null
values are strict except for scalar defaults. Defaults replace only missing/null,
not false, zero, or empty values. Spread/repeat require arrays of scalars;
conditionals require booleans; objects are never emitted.

Parsing errors carry a one-based template argument and byte offset. Expansion
errors carry the argument and config path. Preserve those diagnostics when adding
syntax.

**Adding a placeholder form means touching four places:** `template.rs` (model and
parser), `expand.rs`, integration tests, and README syntax documentation. Keep
model fields private and expansion matches exhaustive.

### The CLI surface

One flat command, no subcommands (`cli.rs`):

```text
toargv <CONFIG> [--check | --json] [-n] [--exec <PROGRAM>] -- [TEMPLATE]...
```

`--` is the only template separator. Clap must not consume child/template flags
after it. `--exec` consumes exactly one program; fixed child arguments belong to
the template. This avoids a greedy executable option swallowing toargv's flags.

All behaviors call `build_arguments` once, then use `Cli::mode`:

- plain and `--json` print expanded argv;
- `--check` expands silently;
- `--exec PROGRAM` spawns the program with expanded argv;
- `-n --exec PROGRAM` prints the full command instead.

Do not re-read raw CLI booleans in `main.rs`; resolve combinations in `Cli::mode`.
An empty template is valid.

### Output modes and the shell

- **exec never goes through a shell.** `execute.rs` appends expanded arguments and
  spawns the selected program directly. Spaces and metacharacters remain inside
  their argv entries. Unix signal termination maps to `128 + signal`.
- **default print is deliberately shell syntax.** `render_shell` quotes each
  argument with `shlex::try_quote`. Use only the `try_*` API; deprecated plain
  quoting APIs have security issues. Per-argument quoting lets
  `Error::Unquotable` identify values containing NUL. `render_json` is the
  unambiguous machine path.

`full_argv` and `execute` must remain in step for dry-run output.

## Testing conventions

Tests are integration tests under `tests/`, not `#[cfg(test)]` modules in `src/`.
Pure tests cover parsing and expansion through public APIs. CLI tests spawn the
real binary with `env!(\"CARGO_BIN_EXE_toargv\")` and tempfile configs, covering
argv boundaries, output, errors, exit statuses, and clap separation.

## Documentation

`README.md` is the user-facing template and CLI specification. Any syntax,
missing-value, type, argv-boundary, or execution behavior change requires a
matching README update.
</coding_guidelines>
