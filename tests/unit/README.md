# Nothing lives here

Unit tests live beside the code they test, in a `#[cfg(test)]` module at the bottom
of each file — `compiler/src/lexer/mod.rs`, `sema/mod.rs`, `lowerer/mod.rs`, and the
crates under `pkg/`, `lsp/`, `dbg/`. Putting them in a separate tree
means they are not read by the person changing the code.

Kept as a signpost so the path does not silently mislead.
