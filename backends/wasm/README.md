# WebAssembly backend

Emits WebAssembly text (`.wat`) from SIR, assembled with `wat2wasm` and executed
through `host.mjs` under Node.

Runs integers, booleans, string literals, `print`, recursion, `if`/`while`/range
`for`, growable `int` arrays, structs, tuples and `int` closures. Array access is
bounds-checked.

`validate(&SirProgram) -> Result<(), &'static str>` is the documented way to say no:
the CLI calls it before `generate`, so an unsupported signature produces
`the wasm target does not support X yet; use --target c, js or rust` rather than a
placeholder.

CI validates every emitted module with `wasm-tools validate --features=wasm1`, which
is a conformance check independent of this host — `wat2wasm` alone accepts some
malformed modules.
