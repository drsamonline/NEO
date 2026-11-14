# Compiler Backends

All four generators consume the same SIR from `crates/nova-common` and are selected
with `emit --target <c|js|rust|wasm>`. `build` and `run` link C only.

| Directory | Target | Current coverage |
|---|---|---|
| `c/` | C | Reference backend, linked with `compiler/src/runtime.rs` (ABI v3). Statements, expressions, structs, enums/`match`, generics, arrays, closures, tuples. Arrays are bounds-checked and growable. |
| `javascript/` | JavaScript | Emits executable ES modules-style source. Only functions reachable from `main` are emitted. Arrays use native JS arrays; closures take `__env` first and are bound with `fn.bind(null, env)`. |
| `rust/` | Rust | Emits executable Rust. Only reachable functions are emitted. Arrays are `Vec<i32>`; closures are `Rc<dyn Fn(..) -> ..>` over generated environment structs. Structs are emitted as Rust structs. |
| `wasm/` | WebAssembly Text | Emits WAT assembled with `wat2wasm` and run by `host.mjs`. Ints, bools, strings, `print`, recursion, loops, growable int arrays, structs, tuples and `int` closures. Bounds checks trap; unsupported signatures are rejected by `validate`. |

## Verification

- `backends/*/src/lib.rs` unit tests cover emission and validation.
- `compiler/tests/end_to_end.rs` assembles/runs JS, Rust and WASM output.
- `nova-compiler test --parity <program>` compares C, JS, Rust and WASM stdout
  and exit status. WASM needs `node` and `wat2wasm`; otherwise it is reported as
  `held` and the run fails.
- CI runs the parity harness over every `examples/*.nova` and
  `tests/parity/*.nova` program.

The parity corpus includes growable arrays, structs/tuples and closures. Known
limits: WASM has no float struct fields or non-`int` closure signatures; WASM
out-of-bounds indexing traps while C prints a runtime error.
