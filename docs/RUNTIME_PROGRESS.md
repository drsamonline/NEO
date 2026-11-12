# NOVA Runtime System — Progress

**Last verified:** 2026-11-12 (`cargo test --workspace`).

The runtime crate (`runtime/`) compiles and is covered by 33 unit tests. It is
**not linked into generated NOVA programs yet**: generated C calls only the shim
ABI declared in `compiler/src/runtime.rs` (`nova_print_*`, `nova_runtime_version`).

## Module status

### Memory (`memory.rs`, 195 lines, 5 tests) ✅
- Allocation trait for primitive types, stack buffers with overflow checks,
  copy/swap helpers, zero-initialisation.

### Types (`types.rs`, 276 lines, 3 tests) ✅
- `NovaString` (concatenation, conversions), `NovaArray<T>` (bounds-checked
  access), `NovaOption<T>` interoperating with Rust's `Option`.

### Errors (`error.rs`, 232 lines, 6 tests) ✅
- `NovaError` with 11 `ErrorKind` categories, source locations, custom handlers,
  `NovaResult<T>`.

### FFI (`ffi.rs`, 193 lines, 4 tests) ✅
- C-compatible booleans, Rust ↔ C string conversion, nullable pointer helpers,
  callback typedefs, malloc/free wrappers.

### Garbage collection (`gc.rs`, 420 lines, 6 tests) ⚠️ reference counting
- `GcPtr<T>` / `GcWeak<T>` over `Rc<RefCell<T>>`, statistics, collection at a
  configurable heap threshold. There is no tracing collector, so reference cycles
  leak; this is documented rather than hidden.

### I/O (`io/mod.rs`, 534 lines, 3 tests) ✅
- File read/write, buffered readers/writers, seeking, directory helpers.

### Threading (`threading/mod.rs`, 314 lines, 2 tests) ✅
- Thread spawn/join, mutex, rwlock and condition-variable wrappers.

### Async (`async_rt/mod.rs`, 114 lines, 2 tests) ⚠️ building blocks only
- Future-like helpers, channels and delay; there is no executor or event loop.

## Remaining work

- [ ] Define a C-facing runtime ABI for strings, arrays and allocation, then link
      it into generated programs (today only printing crosses the boundary).
- [ ] Decide the GC story: tracing collector or documented RC limitation.
- [ ] Build an executor for `async_rt`.
- [ ] Expand I/O and threading tests before relying on either.
- [ ] Bounds-checked arrays in generated C (the compiler currently emits raw
      `int*` indexing with no checks).

## Verification

```bash
cargo test --workspace
```

| Check | Result |
|---|---|
| Workspace tests | 139 passed, 0 failed |
| Runtime tests | 33, all passing |
| Generated C runtime | `nova_runtime.h/.c` ABI v1, print family only |
