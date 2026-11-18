# Backend API

What a code generator must provide, and the rules it must follow. There are four:
`compiler/src/backend/` (C, the reference) and `backends/{javascript,rust,wasm}/`.

## The contract

Every backend takes SIR and returns source text:

```rust
fn generate(&mut self, program: &SirProgram) -> String
```

That is the whole interface. The compiler never asks a backend to run anything, and
a backend never reads the filesystem, the environment or the clock — the same SIR
must always produce the same text, which
`integration_tests/lib.rs::lowering_is_deterministic` checks for the C backend.

The C backend has one extra duty: `emit -o file.c` writes `nova_runtime.h` and
`nova_runtime.c` beside its output (`compiler/src/runtime.rs`), because generated C
calls that ABI rather than libc directly.

## The rules

1. **Reject what you cannot express; never emit a placeholder.** A backend that
   cannot compile a program returns an error naming the feature, so the user gets
   `the wasm target does not support X yet; use --target c, js or rust`. It must
   never emit code that compiles and misbehaves — that is the one failure a user
   cannot see.
2. **`validate` is how you say no.** `backends/wasm/src/lib.rs` exposes
   `validate(&SirProgram) -> Result<(), &'static str>`, and the CLI calls it before
   `generate`. A new backend that supports a subset should do the same.
3. **Match the C reference, or say why not.** `nova-compiler test --parity` compares
   stdout and exit status across all four. A backend that disagrees on a program in
   `examples/` or `tests/parity/` fails CI.
4. **Do not re-infer types.** SIR expressions are typed (`SirExpr::Typed`) precisely
   so a backend never has to guess. If a type is missing, that is a lowering bug,
   not something to paper over in the emitter.

## What each backend actually supports

The honest table is `DEV_PLAN.md` → "Known limitations". In short: C is the
reference and the only target `build`/`run` can link; JavaScript and Rust run
arrays and closures; WebAssembly runs integers, strings, growable int arrays,
structs, tuples and `int` closures, and rejects the rest.

## Adding one

1. A crate under `backends/`, depending only on `nova-common`.
2. `generate`, and a `validate` if the supported subset is not everything.
3. A `--target` value in `compiler/src/main.rs`.
4. A program in `tests/parity/` if the feature should agree across targets.
5. A row in `DEV_PLAN.md`'s status table saying what it does and does not do.
