# Rust backend

Emits one `.rs` file from SIR. Compiles with plain `rustc`; CI builds and runs the
output for `examples/hello.nova` and every parity program.

Closures are `Rc<dyn Fn(..) -> ..>` over generated environment structs.

**Known defect, unfixed:** a NOVA `static` global is emitted as Rust `static mut`,
whose access requires `unsafe` in edition 2021 — so a program with a global does not
compile. Nothing in `examples/` or `tests/parity/` uses one, so no test catches it.
Recorded in `DEV_PLAN.md`'s limitations rather than hidden.

```bash
nova-compiler emit examples/hello.nova --target rust > hello.rs && rustc hello.rs -o hello && ./hello
```
