# Rust backend

Emits one `.rs` file from SIR. Compiles with plain `rustc`; CI builds and runs the
output for `examples/hello.nova` and every parity program.

Closures are `Rc<dyn Fn(..) -> ..>` over generated environment structs.

**Globals are `static mut`, and every access is wrapped in `unsafe`.** That is what
makes a program with a `static` compile at all: reading or writing a mutable static
without `unsafe` is E0133. `tests/parity/statics.nova` pins it — a global read and
written across calls, compared across all four backends.

`static mut` rather than an atomic or a `thread_local!` because NOVA has no threads,
the C backend emits a plain mutable global for the same source, and an atomic cannot
hold a `string`. A `RefCell` in a `thread_local!` is the better answer once NOVA has a
memory model to express.

```bash
nova-compiler emit examples/hello.nova --target rust > hello.rs && rustc hello.rs -o hello && ./hello
```
