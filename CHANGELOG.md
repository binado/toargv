# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Drop template-argument indexes from missing-value and type-expansion error
  messages. The configuration path already identifies the placeholder.
- Replace grammar files, inline grammars, and layering with passthrough argv
  templates supporting interpolation, defaults, array spread, conditional
  options, and repeated option/value pairs.
- Replace trailing-command execution with `--exec PROGRAM -- TEMPLATE...`.
- Rename the pure `toargv-grammar` crate to `toargv-template`.

### Removed

- Remove `--json` output and `render_json`. Expanded argv is printed as shell
  syntax; programmatic callers use the library or `--exec`.
- Remove the bracket grammar, grammar codecs, `-f/--grammar-file`,
  `-g/--grammar`, join/count actions, and TextMate grammar.

## [0.1.0] - 2026-08-13

### Added

- Translate TOML and JSON configuration values into ordered argument vectors.
- Load and layer inline grammars from files and command-line arguments.
- Support named and positional `auto`, `value`, `repeat`, and `join` actions.
- Support named `flag` and `count` actions and required named values.
- Print shell-quoted or JSON output, validate inputs, dry-run commands, or
  execute commands directly.
- Publish the pure grammar model, codec, and emitter as `toargv-grammar`.

[Unreleased]: https://github.com/binado/toargv/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/binado/toargv/releases/tag/v0.1.0
