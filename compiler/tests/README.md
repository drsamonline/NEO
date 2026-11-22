# Compiler integration tests

| File | Covers |
|---|---|
| `end_to_end.rs` | Source to a linked binary: every example runs, the runtime shim links, `--target` output is executed, diagnostics fire where they should |
| `stdlib.rs` | The standard library *computes* what it promises: a program per module, asserting values that are checkable by hand. Compiling is a different question and a weaker one |
| `stdlib_parse.rs` | Every `stdlib/**/*.nova` parses and emits C |
| `format.rs` | `nova-compiler fmt` round-trips, and `--check` fails on unformatted input |
| `item_tree.rs` | The lossless syntax tree survives a round trip, which is what ADR 0001 promised |

Unit tests for one layer live in that layer's own file, in a `#[cfg(test)]` module —
the lexer, parser, sema and lowerer each have one. Cross-crate pipeline tests are in
`integration_tests/`.

Diagnostics are not tested here: they have goldens in `tests/diagnostics/`, checked
by `tools/diagnostic-check.py`.
