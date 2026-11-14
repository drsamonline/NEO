# Runtime

The `runtime/` crate is the Rust runtime system: memory utilities, runtime types,
errors, FFI, reference-counted GC, file and stream I/O, threading primitives and
async helpers. It compiles and its unit tests run as part of
`cargo test --workspace`.

It is **not linked into generated NOVA programs**. Generated C includes the small
`nova_runtime.h`/`nova_runtime.c` shim emitted by `compiler/src/runtime.rs`
(ABI v3), which provides printing, strings, bounds-checked growable arrays,
closures and panic/exit. The crate is the foundation for a future native runtime,
not the current execution path.

Current overall project stage: Phase 1 complete, Phase 3 partial, Phase 4 in
progress; see `DEV_PLAN.md`.
