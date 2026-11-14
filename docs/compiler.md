# Compiler

The compiler pipeline is:

```
source .nova → lexer → parser → semantic analyser → SIR → lowerer → c | js | rust | wasm
```

C is the reference backend and the only one `build`/`run` link. `emit --target
js|rust|wasm` produces the other three. The SIR lives in
`crates/nova-common`; backend crates live in `backends/`.

Current stage: Phase 1 complete, Phase 3 partial, Phase 4 in progress. The suite
has 187 passing tests, and `nova-compiler test --parity` compares stdout and exit
status across all four backends. CI runs the parity harness over every
`examples/*.nova` and `tests/parity/*.nova` program.

Known backend gaps are explicit: WASM float struct fields and non-`int` closure
signatures are rejected by `validate`; WASM out-of-bounds indexing traps while C
reports a runtime error; Rust struct-focused parity programs are not in the corpus
yet.

See [../compiler/README.md](../compiler/README.md) for the C runtime shim,
lowering details and backend-specific behaviour, and
[../DEV_PLAN.md](../DEV_PLAN.md) for authoritative status.
