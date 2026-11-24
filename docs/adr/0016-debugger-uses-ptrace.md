# ADR 0016 — the debugger uses `ptrace`, and only in one file

- **Status:** accepted
- **Date:** 2026-11-24
- **Related:** [0013](0013-file-handles-are-handles.md), [DEV_PLAN](../../DEV_PLAN.md), `dbg/src/target.rs`

## Context

The debugger's `attach` checked that `/proc/<pid>` existed, read the process name, and printed
**"Attached to process N"**. It never called `ptrace`. `connect_remote` printed "Connected" and
opened no socket. Both were removed as false successes (PR #130), which left the honest question:
implement attaching, or drop it in writing?

Attaching is the whole difference between a REPL that knows about breakpoints and a debugger that
knows where a program is. It also needs `unsafe`: `ptrace` is a variadic C function whose
arguments' meaning depends on the request, and reading registers means handing the kernel a
pointer to a struct it fills in.

This workspace has one crate with `unsafe` in it (`runtime`, which says why in its own source)
and every other crate avoids it. Adding a second is a decision, not a detail.

## Decision

**The debugger attaches, and `unsafe` lives in exactly one file: `dbg/src/target.rs`.**

Four functions there call `ptrace`, and each carries a `SAFETY` comment naming why its pointers
are valid:

| Function | Request | Why the pointers are sound |
|---|---|---|
| `attach` | `PTRACE_ATTACH` | both ignored by the kernel for this request; null is correct |
| `attach` | `waitpid` | writes one `c_int` through a pointer to a live local of that type |
| `detach` | `PTRACE_DETACH` | both ignored, as above |
| `instruction_pointer` | `PTRACE_GETREGS` | writes `user_regs_struct` through a pointer to a live local of that type |

Nothing else in the crate may contain `unsafe`. `dbg/src/main.rs` carries
`#![deny(unsafe_code)]` with an `#[allow]` on the `target` module, so a future `unsafe` block
anywhere else fails the build rather than arriving in review.

**`libc` is the crate's only C-level dependency**, and the reason is this ADR: `ptrace` has no
stable Rust binding, and wrapping it would mean writing the same `unsafe` elsewhere.

## What this buys, measured

```
$ cargo test -p nova-dbg test_attach_reads_a_real_child
test session::tests::test_attach_reads_a_real_child ... ok
```

The test spawns `sleep 30`, attaches to it, reads the instruction pointer, asserts it is a real
address, and detaches. That is the whole path exercised against a live process.

## The limit, which is the environment's and not the code's

`ptrace` is gated by `yama/ptrace_scope`. A container commonly sets it to `1`, which permits
tracing **children only**:

```
$ nova-dbg /tmp/spin
(nova-dbg) attach 13687
Error: cannot attach to process 13687 (spin): Operation not permitted (os error 1).
Nothing was attached
```

That message is correct and is what a user sees. The test therefore traces a **child** of its own,
because that is the case a sandbox allows — and it asserts the `EPERM` message when even that is
refused, so a restrictive host reports a limit rather than looking like a defect.

## What this does not do

- **No `PTRACE_CONT`, no single-stepping, no breakpoint insertion.** Attaching stops a process;
  resuming it under the debugger's control is the next step and is not claimed here.
- **x86-64 Linux only.** The register read refuses on every other target with a message saying
  so, rather than reading the wrong struct.
- **`connect_remote` still refuses.** The remote GDB stub protocol is a different project.

## Consequences

A user can attach, see where a program is (`where` symbolizes the real `rip` through the line
table), and detach. `backtrace` and `print` still refuse, because both need the stack and the
symbol table, not just the instruction pointer.

The alternative — leaving `attach` refusing — would have kept the crate free of `unsafe` at the
cost of a debugger that cannot look at a running program. That trade is worth making once, in one
file, with the boundary enforced by the compiler.
