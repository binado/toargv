# toargv

`toargv` expands values from a TOML or JSON configuration into an ordered
argument vector. The template is written as ordinary command-line arguments,
so its literals, ordering, and argument boundaries are visible at the call
site.

## Example

Given `config.toml`:

```toml
output = "results"
verbose = true
files = ["first.csv", "second file.csv"]
```

Expand a template:

```console
$ toargv config.toml -- \
    --output '{output}' \
    '{?verbose:-v}' \
    '{*files:--input}'
--output results -v --input first.csv --input 'second file.csv'
```

Or print an unambiguous JSON array:

```console
$ toargv config.toml --json -- \
    --output '{output}' \
    '{?verbose:-v}' \
    '{*files:--input}'
["--output","results","-v","--input","first.csv","--input","second file.csv"]
```

Quotes around placeholders protect them from the invoking shell. They are not
part of the template argument.

## Installation

```sh
cargo install toargv
```

To install a checkout instead:

```sh
cargo install --path crates/toargv
```

## Usage

```text
toargv <CONFIG> [--check | --json] [-n] [--exec <PROGRAM>] -- [TEMPLATE]...
```

- `--` ends toargv's options. Every following argument is one template
  argument, even when it begins with `-`.
- With no output or execution option, the expanded argv is printed as
  shell-safe syntax.
- `--json` prints the expanded argv as a compact JSON array.
- `--exec PROGRAM` runs `PROGRAM` directly with the expanded arguments. No
  shell is involved.
- `-n`, `--dry-run` prints the full command instead of running it. It requires
  `--exec` and composes with `--json`.
- `--check` parses and expands the template without printing anything. It
  conflicts with `--json`, `-n`, and `--exec`.
- An omitted or empty template is valid and expands to an empty argv.

Configuration format is selected from the case-sensitive `.toml` or `.json`
extension. TOML datetimes are emitted as strings.

## Template syntax

Each argument after `--` is either a literal, an interpolated argument, or a
whole-argument directive:

| Syntax | Requirement | Expansion |
| --- | --- | --- |
| `{path}` | Scalar | The scalar as text |
| `{path:-default}` | Scalar, missing, or null | The scalar, or `default` for missing/null |
| `{path...}` | Array of scalars | One argument per array element |
| `{?path:-v}` | Boolean | `-v` when true, nothing when false |
| `{*path:--file}` | Array of scalars | `--file value` for every element |

Scalar placeholders and defaults can be embedded in a larger argument:

```sh
toargv config.toml -- \
  '--output={directory}/{name}.txt' \
  '--jobs={jobs:-4}'
```

Several scalar placeholders may appear in the same argument. Argument
boundaries never change during scalar interpolation.

Spread, conditional, and repeated-pair placeholders can produce zero or
multiple arguments, so each must occupy its entire template argument:

```sh
# ["a", "b"] becomes: --files a b
toargv config.toml -- --files '{files...}'

# ["a", "b"] becomes: --file a --file b
toargv config.toml -- '{*files:--file}'
```

Conditional and repeated-pair output must be a valid `-x` or `--name` option
token. Empty arrays emit nothing.

### Paths and values

Paths are raw dotted paths through objects and tables. `{foo}` reads the root
key `foo`; `{config.foo}` reads a nested `config.foo` value. Array indexes and
escaping dots in key names are not supported.

Missing and null values are errors except in a scalar placeholder with a
default. A default does not replace `false`, `0`, an empty string, or an empty
array. Defaults are literal strings.

Strings, numbers, and booleans are scalar values. Objects cannot be expanded.
Arrays require spread or repeated-pair syntax.

### Literal braces

Double braces emit literal braces:

```text
{{literal}}       -> {literal}
{{{name}}}        -> {<expanded name>}
```

Unmatched braces, empty paths, malformed operators, and multi-argument
directives embedded in other text are errors. Errors identify the one-based
template argument and byte offset.

## Execution

Use `--exec` before the separator and put all fixed and expanded child
arguments in the template:

```sh
toargv config.toml --exec program -- \
  fixed-argument \
  --output '{output}' \
  '{files...}'
```

The program is spawned directly. Spaces and shell metacharacters in
configuration values remain inside their original argv entries. The default
printed form provides the opposite guarantee: it is deliberately quoted shell
syntax so that copy-paste or `eval` reproduces the same argument vector. Use
`--json` for machine-readable output.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
