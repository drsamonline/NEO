# ADR 0023 — `print <name>`: the DWARF model is dead weight, and the information is already in the binary

- **Status:** **accepted** (2026-11-27) — option **C**, then **A** (2026-11-28): `print x` works
- **Date:** 2026-11-27
- **Related:** [0021](0021-delete-runtime-crate.md) (the same shape), `compiler/src/main.rs:682`

## The question

The debugger could set breakpoints, walk the frame chain and resolve `.nova` line numbers, but it
could not answer `print x`. Every name was refused:

> cannot evaluate `x`: reading a variable needs its stack offset and its type, and this compiler
> emits neither — only line information. `print 0xADDRESS` reads memory directly

Should that change, and at what cost?

## What was true, measured

| Fact | Command | Result |
|---|---|---|
| the C compiler is invoked with debug info | `compiler/src/main.rs:682` | **`link.arg("-g")`** |
| a DWARF model existed in the debugger | `dbg/src/dwarf.rs` | **364 lines**: `DwarfInfo`, `CompileUnit`, `DebugEntry`, `AttributeValue`, `VariableSymbol`, `find_variable`, `extract_symbols` |
| that model could ever be populated | `parse_buffer` in the same file | **no** — it returned `Err(NotImplemented)` for every input |
| anything called it | `grep -rn "parse_file\|parse_buffer" dbg/src` outside `dwarf.rs` | **nothing** |
| what answered line numbers | the debugger, per ADR 0016 | `addr2line`, not this module |
| the session's `variables` map | `session.rs` | **never filled** — `info variables` could only answer "No variables in scope" |

**So the information `print x` needs was already in every binary this compiler builds**, because it
links with `-g`. What was missing was a reader — and the file that looked like a reader was a data
model with no parser, re-exported from `dbg/src/main.rs` and called by nobody.

That is the `runtime/` shape from [ADR 0021](0021-delete-runtime-crate.md): a scaffold whose presence
suggests a capability the code does not have. The refusal message also blamed the compiler — "this
compiler emits neither" — which was false, and worse than a plain "not implemented".

## Options

### A. Parse `.debug_info` with `gimli`, and make `print x` work

Add `gimli` and `object` — the standard read-only, pure-Rust DWARF crates — delete the hand-rolled
model, and implement variable lookup: find the DIE for the current function, its children of kind
`DW_TAG_variable`, and evaluate the location expression for the current PC.

**Cost:** two dependencies in a workspace that has deliberately few; a real implementation of
location expressions (register, stack offset, and the composite forms gcc emits).
**Benefit:** `print x` works wherever the C compiler emits locals — gcc and clang. It does **not**
work with `zig cc`, which emits line information and no locals; the debugger would say so.

### B. Hand-roll the `.debug_info` parser

**Rejected.** DWARF is a format where partial support produces plausible wrong answers — a variable
read from the wrong slot looks like a number, not like an error.

### C. Delete the model and keep the refusal

**Chosen.** 364 lines that could never be populated go away, the `info variables` command goes with
them, and the refusal is corrected to name the real reason.

### D. Shell out to `readelf` / `addr2line`

**Rejected.** They answer line questions and symbol addresses. Neither evaluates a location
expression, which is the actual problem.

## What was done — 2026-11-27

Option **C**, and it was larger than the model itself:

| Removed | Why |
|---|---|
| `dbg/src/dwarf.rs` — 364 lines | a complete DWARF data model whose parser refused every input and which nothing called |
| its `mod` and re-export in `dbg/src/main.rs` | the only references that existed |
| `session.rs`'s `variables` map, its `Variable` type and the `info variables` arm | the map was **never filled**, so the command could only ever answer "No variables in scope" — a false affordance, not a limit |
| the usage line's `variables` option | it named a command that no longer exists |

And the refusal was corrected. It used to say *"this compiler emits neither — only line information"*,
which was false: the compiler links with `-g`. It now says what is true — the debugger reads only the
line table, the information is in the binary, and nothing here parses it yet.

**Two scaffolds of the same shape are now gone from this repository**: `runtime/` (ADR 0021) and this
one. Both looked like capability and were not.

## A was built after all — 2026-11-28

The recommendation was "C now, A when someone wants the feature". A was wanted, and it is in:
`gimli` and `object` read `.debug_info`, `evaluate_location` runs the location expression against the
stopped frame, and `print x` returns the value — **7**, in a test that runs a real process and stops at
a real breakpoint.

Two things the work taught that this ADR could not have:

- **`DW_OP_fbreg` is relative to `DW_AT_frame_base`, not to `RBP`.** The first version passed the frame
  pointer as the base, so `fbreg -20` landed 16 bytes below the slot and read a neighbouring value —
  which is a number, so nothing looked broken. gcc emits `DW_AT_frame_base = DW_OP_call_frame_cfa`,
  so the base has to be *resolved*. It was found by making the failing test print the compiler's own
  numbers, not by trying offsets until the value matched.
- **the crate had to become a library.** A binary-only crate cannot be driven by a test, which is why
  every part of this feature was testable in isolation and the join was not.

Deleting the dead model first (option C) made the real one easier to see: the 364 lines that could
never be populated were replaced by a reader that is used on every `print`.

## What is still open

It does not decide:

- whether `print x` should work on `zig cc` builds. It cannot: that compiler emits no local-variable
  DWARF. The honest behaviour is a message naming the compiler, not a refusal of all names.
- whether a NOVA-side debug map — a sidecar the compiler writes — is worth building instead of DWARF.
  It would work on every toolchain, but it means designing a format and keeping it in step with the
  generated C: a second copy of information DWARF already carries.
