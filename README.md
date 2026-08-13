# toargv

`toargv` translates values from a TOML or JSON configuration file into an
ordered command-line argument vector. Grammars use one inline syntax, from a
file (`-f`), an argument string (`-g`), or several layered sources.

## Example

Given `config.toml`:

```toml
output_dir = "results"
inputs = ["first.csv", "second file.csv"]
source = "data.csv"

[config]
num_nodes = 4

[config.sampler.random]
seed = 42
```

And `grammar`:

```text
[--output output_dir]
[--num-nodes config.num_nodes]
[--seed config.sampler.random.seed]
<source>
[--input r inputs]
```

Inspect the generated arguments as a shell-quoted line:

```console
$ toargv config.toml -f grammar
--output results --num-nodes 4 --seed 42 data.csv --input first.csv --input 'second file.csv'
```

Or as a compact JSON array:

```console
$ toargv config.toml -f grammar --json
["--output","results","--num-nodes","4","--seed","42","data.csv","--input","first.csv","--input","second file.csv"]
```

Or execute a command with those arguments appended:

```sh
toargv config.toml -f grammar -- program fixed-argument
```

`--` never goes through a shell: the command is spawned directly, so spaces and
shell characters in generated values remain part of their original arguments.
The default output is the other side of that coin — it is deliberately shell
syntax, quoted so that `eval` or copy-and-paste reproduces the same argument
vector. Use `--json` for an unambiguous machine-readable form.

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
toargv <CONFIG> [-f <PATH>]... [-g <GRAMMAR>]... [--check | --json] [-n] [-- <COMMAND>...]
```

- At least one `-f/--grammar-file` or `-g/--grammar` is required; each may be
  repeated to layer sources of that kind.
- `-f` loads a grammar file in the inline format. `-g` parses the same syntax
  from the argument string. All files are merged left to right, then all
  inlines, so inline sources always override files.
- With no other flags, the generated argv is printed as a shell-quoted line.
- `--json` prints the generated argv as a compact JSON array instead.
- `-- <COMMAND>...` appends the generated arguments to the command, runs it, and
  propagates its exit status.
- `-n`, `--dry-run` prints the full command instead of running it. It requires a
  trailing command and composes with `--json`.
- `--check` validates grammar rules, configuration paths, and value types,
  printing nothing and exiting 0 or 1. It cannot be combined with `--json`,
  `-n`, or a trailing command.

Configuration format is selected from a case-sensitive `.toml` or `.json`
extension.

## Ordered grammar rules

A grammar is one ordered list containing named and positional rules. Arguments
are emitted in this exact order.

Pass it as a `-g` string, or put the same syntax in a file (one rule per line
is fine) and load it with `-f`:

```sh
toargv config.toml \
  -g '[-o !v config.output_file] [-s f config.save] [--input r inputs] <input.data> <r files>'
```

```text
[OPTION PATH]          optional named rule with auto action
[OPTION SPEC PATH]     optional named rule with explicit action
[OPTION ! PATH]        required named rule with auto action
[OPTION !SPEC PATH]    required named rule with explicit action
<PATH>                 positional rule with auto action
<SPEC PATH>            positional rule with explicit action
```

Action specifiers have short and long spellings:

| Short | Long |
| --- | --- |
| `a` | `auto` |
| `v` | `value` |
| `f` | `flag` |
| `r` | `repeat` |
| `j=SEP` | `join=SEP` |
| `c` | `count` |

| Action | Named result | Positional result |
| --- | --- | --- |
| `auto` | Selects `value`, `flag`, or `repeat` by input type | Emits one scalar or each array item |
| `value` | Emits `--option value` | Emits one scalar |
| `flag` | Emits the option when a boolean is true | Not supported |
| `repeat` | Repeats `--option value` for each array item | Emits each array item |
| `join` | Emits the option and the joined array | Emits the joined array |
| `count` | Repeats the option by a nonnegative integer | Not supported |

`f` and `c` are valid only for named rules. Prefixing a named specifier with
`!` makes the value required. Rules retain their textual order. Missing and
null named values are omitted unless the rule is required. Positionals are
always required. Objects and incompatible value types are errors.

Dotted paths traverse objects and tables. Array indexes and escaping dots in key
names are not supported.

Tokens use backslash escaping rather than quoting. Escape whitespace,
backslashes, and rule delimiters as `\ `, `\\`, `\t`, `\n`, `\r`, `\[`, `\]`,
`\<`, or `\>`. Any Unicode scalar can be written as `\u{HEX}`. For example,
`j=\ ` joins values with a space. Unknown and incomplete escapes are errors.

## Layering grammars

`-f` files are merged left to right, then `-g` inlines left to right. Inline
sources always take precedence, including when they appear first on the command
line:

```sh
toargv config.toml \
  -f base \
  -g '[--seed config.alternate_seed] <input.alternate>'
```

Named rules use their exact option token as a key. A later rule replaces an
earlier rule with the same key and moves to the later grammar's position.
Unaffected earlier rules keep their relative order.

If a later grammar contains any positional rules, they replace all earlier
positionals. A grammar without positionals preserves the earlier positionals.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
