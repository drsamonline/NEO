# ADR 0021 — delete the Rust runtime crate

- **Status:** **accepted** (2026-11-26)
- **Date:** 2026-11-26
- **Related:** [0020](0020-concurrency-model.md) (a decision about what not to build), [Memory chapter](../language/11_memory.md)

## The question

`runtime/` was a workspace crate of **2,480 lines** across nine modules — `gc.rs`, `threading/`,
`async_rt/`, `ffi.rs`, `io/`, `memory.rs`, `error.rs`, `types.rs` — carrying thirty-three unit tests
of its own, all passing.

**Nothing depended on it.** No crate listed `nova-runtime` in `[dependencies]`, and generated C links
only the shim in `compiler/src/runtime.rs`, which contains no collector, no arena, no FFI layer and
no threading. The crate had been in that state since Phase 2.

Delete it, wire it, or keep it and say why.

## What was true, measured before the deletion

| Fact | Command | Result |
|---|---|---|
| lines of Rust | `find runtime -name '*.rs' \| xargs wc -l` | **2,480** |
| unit tests of its own | `grep -rh '#\[test\]' runtime/src \| wc -l` | **33** |
| crates that depend on it | `grep -rn "nova-runtime" --include=Cargo.toml .` outside `runtime/` | **0** |
| documents describing it | `grep -rln "runtime/" --include=*.md .` | **8** |

## Options

### A. Delete the crate

Remove `runtime/`, its workspace member entry, `docs/RUNTIME_PROGRESS.md`, `runtime/README.md`, and
the references in eight documents. Its own thirty-three tests go with it.

**Cost:** one commit, and the loss of tests that test nothing a program runs.
**Benefit:** 2,480 lines that describe a runtime nobody runs stop existing, and the repository's
description of itself becomes true without a disclaimer.

### B. Wire it up

Link `nova-runtime` into generated programs. **Not a small change**: generated C would have to call
Rust, which means a C ABI over `nova-runtime`, which means `cdylib`, a build step and a linker
contract for every user. The C shim exists precisely because that is expensive.

**Cost:** months. **Benefit:** a collector, real threading and async — but ADR 0020 has just decided
that concurrency is out of scope, which removes most of the reason to wire it.

### C. Keep it, documented as unused

What the repository did until now: `runtime/src/lib.rs` opened with a table of what the crate *has*
versus what a program *gets*, and six documents explained that it is not linked.

**Cost:** every reader who opened `gc.rs` still saw 23 functions of collector infrastructure and had
to read a header to learn it was a sketch. **Benefit:** the FFI module recorded how to wrap C safely
in a workspace that is otherwise `#![forbid(unsafe_code)]`.

### D. Keep the FFI module, delete the rest

Extract `ffi.rs` as documentation of the unsafe-boundary pattern, delete the other eight modules.

**Cost:** a smaller edit than A, plus a decision about where a 262-line file with no caller lives.
**Benefit:** keeps the one part with transferable value.

## Recommendation

**A, with D's content preserved as prose.**

The reasoning is the one that decided ADR 0020: a claim the code does not support is the defect this
repository spent the week removing. `runtime/` was the largest remaining one — 2,480 lines whose
presence makes "NOVA has a garbage collector" a reading a person can reach from the file tree alone,
whatever the header says.

D's content is real but small: the pattern is "every `unsafe` block carries a `// SAFETY:` note
stating the obligation it relies on", which belongs in `CONTRIBUTING.md` as a sentence, not in a
module. It is there now.

## What was done — 2026-11-26

| Removed | Also removed |
|---|---|
| `runtime/` (nine modules, 2,480 lines) | its workspace member entry in `Cargo.toml` |
| `docs/RUNTIME_PROGRESS.md` | two rows in `docs/README.md` |
| `runtime/README.md` | the tree entry and two sentences in `ARCHITECTURE.md` |

`README.md`'s Phase 2 section now says the crate is gone and why, and the workspace test count
moved from 346 to **313** — the count gate required every document that states it to change in the
same commit, which is the check working as designed.

## What this costs

- **Thirty-three tests.** They tested code no program ran. A test for a function nothing calls is
  not coverage; it is a second copy of the function's assumptions.
- **The option to wire it later.** Git history keeps every line, and `git show` is how it comes back
  if a future ADR decides the C shim should be replaced.

## Consequences

- `cargo test --workspace` runs 313 tests instead of 346.
- Coverage falls: the crate's lines were partly covered by its own tests. The floor is lowered in the
  same commit with this reason written beside it.
- `ARCHITECTURE.md`'s "Future Architecture" loses its first item ("link the Rust runtime crate or
  replace it"), because the second half of that sentence is now the answer: the C shim **is** the
  documented runtime.
