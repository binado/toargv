# toargv

`toargv` expands values from a TOML or JSON configuration into an ordered
argument vector. The template is a single quoted string with `{}` slots, and
each slot is filled by a [jq](https://jqlang.org/) filter evaluated against the
configuration. Argument order and boundaries are always visible at the call
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
$ toargv config.toml \
    '--output {} {} {}' \
    '.output' \
    '.files[]' \
    'if .verbose then "--verbose" else empty end'
--output results first.csv 'second file.csv' --verbose
```

The template string is one shell word, so its quoting is settled once by the
invoking shell. The jq filters are separate arguments, each passed to the jq
compiler verbatim — braces, strings, and comments inside them need no
escaping.

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
toargv <CONFIG> [TEMPLATE] [FILTER]... [-0 | -c | -e PROGRAM [-n]]
```

- With no execution option, the expanded argv is printed as shell-safe syntax.
- `-0`, `--print0` prints the expanded argv NUL-separated instead, ready for
  `xargs -0`. It conflicts with `-c`, `-n`, and `-e`.
- `-e`, `--exec PROGRAM` runs `PROGRAM` directly with the expanded arguments.
  No shell is involved.
- `-n`, `--dry-run` prints the full command instead of running it. It requires
  `--exec`.
- `-c`, `--check` parses and expands the template without printing anything.
- A template beginning with `-` needs no separator, but any word that exactly
  matches a defined flag is still claimed by the argument parser: `-0` and
  `--print0`, `-c` and `--check`, `-e` and `--exec`, `-n` and `--dry-run`,
  `-h` and `--help`, `-V` and `--version`. Use `--` to pass a template or
  filter that collides with one of them.
- An omitted or empty template expands to an empty argv.

Configuration format is selected from the case-sensitive `.toml` or `.json`
extension; TOML datetimes become strings.

## Template syntax

The template is split into **words** by whitespace. Quoting controls word
splitting and follows POSIX shell rules: `'single quotes'` are literal,
`"double quotes"` are literal except `\"` and `\\`, and a backslash outside
quotes escapes the next character. A trailing backslash, with nothing left to
escape, stands for itself: `'ab\'` is the argument `ab\`.

Unlike a shell, quoting does **not** turn off slot interpolation — `{` and `}`
keep their meaning everywhere, so `'{}'` is a slot, not a literal pair of
braces. Write `{{` and `}}` for literal braces; inside quotes that is the only
form, while outside them a backslash escape works too, so `\{` is a literal
`{`. The slot forms are:

| Slot | Refers to |
| --- | --- |
| `{}` | The next positional filter, in order |
| `{1}`, `{2}`, ... | The Nth positional filter (`{1}` does not advance the `{}` counter) |
| `{name}` | A filter passed as `name=FILTER` |

A filter argument is a **binding** when it starts with `NAME=` where `NAME`
matches `[A-Za-z_][A-Za-z0-9_]*`; otherwise it is positional. Positional and
named slots cannot be mixed in one template.

**Cardinality.** Slots know their place:

- A slot *embedded* in a word (`--tag={}`) requires its filter to yield
  **exactly one** scalar value.
- A word consisting of **only** a slot (`{}`) expands to **0, 1, or many**
  arguments — one per value the filter yields. This is how arrays, conditionals,
  and repeated option/value pairs are expressed.

A quote counts as text even when it encloses nothing, so `""{}` is a word with
a literal and a slot, not a bare slot: its slot is embedded and requires
exactly one value. Write `{}` unquoted to flatten a stream.

Every value must be a scalar (string, number, or boolean). Strings are used
verbatim, including whitespace; numbers and booleans are stringified. `null`,
arrays, and objects are rejected: missing keys yield `null`, so strictness on
absent values is automatic. Byte strings, and strings that are not valid
UTF-8, are rejected as well. Non-finite jq numbers are *not* rejected: a filter
yielding `infinite` or `nan` produces the literal arguments `Infinity` and
`NaN` — unlike a non-finite float in a TOML configuration, which fails to
load.

`toargv` rejects — before running anything — templates referencing a filter
that does not exist, and filter arguments no slot references.

## jq recipes

Filters are ordinary jq programs run with the configuration root as `.` (via
the [jaq](https://github.com/01mf02/jaq) implementation, including its standard
library). Frequent patterns:

| You want | Filter |
| --- | --- |
| A value | `.user.name` |
| A value with a default | `.output // "result.txt"` |
| Default on missing/null only | `.output \| if . == null then "result.txt" else . end` |
| An array, one argument per element | `.files[]` |
| Repeated option/value pairs | `.files[] \| "--file", .` |
| A conditional flag | `if .verbose then "--verbose" else empty end` |
| Transformed values | `.files[] \| "--file=" + .`, `.targets[].name` |

jq's `//` operator substitutes for `null` **and** `false`; use the longer
`if` form when `false` is a real value in the configuration.

Note that every value must reach a slot as a scalar, so a filter ends in an
iteration (`.targets[].name`) rather than an array (`.targets | map(.name)`),
which the scalar gate rejects.

**Bounds on runaway filters.** A filter is an arbitrary program, so `toargv`
constrains three ways one can fail to finish:

- a filter may yield at most **100,000 values**, which stops filters that
  produce output without end, such as `repeat(.)`;
- expansion as a whole gets **10 seconds** of wall-clock time, which stops
  filters that loop without yielding, such as `def f: f; f`;
- a filter source may nest brackets at most **256 deep**, which is checked
  before jq parses it, since a deeply nested source would otherwise overflow
  the parser's stack.

One case is not covered: a filter recursing at run time without bound, such as
`def f: [f]; f`, exhausts the stack, and a stack overflow aborts the process
rather than raising an error that could be reported.

## NUL-separated output

Piping arguments through text loses boundaries unless the separator can never
appear inside an argument. NUL is that separator — the operating system
defines every argument as NUL-terminated, so it cannot contain one. Pipe with
`-0` into `xargs -0`:

```console
$ toargv -0 config.json '--name {}' '.name' | xargs -0 some-program
```

This is exact for arguments containing spaces, quotes, or newlines. `--exec`
is even stronger: it passes argv in memory, with no serialization at all.

If the reader closes the pipe early — `| head`, or an `xargs -0` that stops on
an error — `toargv` exits 0 without an error, like any other pipeline tool.

## Migration from 0.1.x

The 0.1 per-argument template syntax was replaced by the single template
string with jq filters:

In 0.1:

```console
$ toargv config.toml -- --output {output} {?verbose:-v} {*files:--file}
```

In 0.2:

```console
$ toargv config.toml \
    '--output {} {} {}' \
    '.output' \
    'if .verbose then "-v" else empty end' \
    '.files[] | "--file", .'
```

- `{path}` → `{}` with filter `.path`; missing keys now fail with a null error
  from the scalar gate.
- `{path:-default}` → filter `.path // "default"` (mind the `false` caveat
  above).
- `{path...}` → `{}` with `.path[]`.
- `{?flag:-v}` → `{}` with `if .flag then "-v" else empty end`.
- `{*path:--file}` → `{}` with `.path[] | "--file", .`.
- `--` is optional; the template is a single string argument.

## Execution

Under `--exec`, `toargv` exits with the child's own exit code. If the child is
terminated by a signal instead of exiting, `toargv` exits with `128 + signal`
on Unix, and with `1` elsewhere. Errors raised by `toargv` itself — a bad
configuration, an invalid template, a filter that fails or runs too long —
always exit `1`.

## License

MIT
