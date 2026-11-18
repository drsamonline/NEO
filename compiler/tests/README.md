# Compiler integration tests

| File | Covers |
|---|---|
| `end_to_end.rs` | Source to a linked binary: every example runs, the runtime shim links, `--target` output is executed, diagnostics fire where they should |
| `stdlib_parse.rs` | Every `stdlib/**/*.nova` parses and emits C |

Unit tests for one layer live in that layer's own file, in a `#[cfg(test)]` module —
the lexer, parser, sema and lowerer each have one. Cross-crate pipeline tests are in
`integration_tests/`.

Diagnostics are not tested here: they have goldens in `tests/diagnostics/`, checked
by `tools/diagnostic-check.py`.
