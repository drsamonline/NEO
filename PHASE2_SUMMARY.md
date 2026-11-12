# Phase 2: Runtime System — Status

**Last verified:** 2026-11-12 against the code and `cargo test --workspace`.

Phase 2 built the Rust runtime crate under `runtime/`. It compiles and its unit
tests pass, but it is **not linked into generated NOVA programs**: the C backend
targets the small C shim in `compiler/src/runtime.rs` instead. The crate is the
foundation for a future richer runtime, not a description of what compiled NOVA
programs currently use.

## Modules

| Module | Lines | Tests | What it actually provides |
|---|---|---|---|
| `memory.rs` | 195 | 5 | Allocation traits, stack buffers, copy/swap helpers, zero-initialisation |
| `types.rs` | 276 | 3 | `NovaString`, `NovaArray<T>`, `NovaOption<T>` and their traits |
| `error.rs` | 232 | 6 | `NovaError`, `ErrorKind`, locations, custom handlers |
| `ffi.rs` | 193 | 4 | C string conversion, nullable pointers, callbacks, malloc/free wrappers |
| `gc.rs` | 420 | 6 | Reference counting (`GcPtr`/`GcWeak`), stats, threshold-triggered collection |
| `lib.rs` | 81 | 2 | `init`/`shutdown`, unified exports, runtime stats |
| `io/mod.rs` | 534 | 3 | File read/write, buffered streams, seeking, directory listing |
| `threading/mod.rs` | 314 | 2 | Thread spawning, mutex, rwlock, condition-variable wrappers |
| `async_rt/mod.rs` | 114 | 2 | Minimal future/channel/delay helpers |
| **Total** | **2,359** | **33** | |

## What changed against the earlier draft of this file

- The per-module line and test counts were stale (they described an earlier
  snapshot: 1,220 lines and 21 tests, with no I/O, threading or async module).
- `gc.rs` is reference counting, not a mark-and-sweep collector.
- I/O, threading and async helpers now exist; they were listed as "next steps".

## Remaining Phase 2 work

1. **Link the runtime to generated code.** The C backend only knows the print
   ABI in `compiler/src/runtime.rs`; strings, arrays, bounds checks and allocation
   would need a C-facing ABI first.
2. **GC:** replace reference counting with a tracing collector, or document the
   cycle limitation and keep RC.
3. **Async:** the current `async_rt` helpers are building blocks, not an executor.
4. **I/O and threading coverage:** both have three and two tests respectively;
   grow them before treating either as production-ready.

## Verification

```bash
cargo test --workspace   # includes the 33 runtime tests
```

The workspace total is 139 passing tests, 0 failing (compiler, runtime, tooling
and end-to-end tests together).
