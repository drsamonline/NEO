# ADR 0023 — `print <name>`: the DWARF model is dead weight, and the information is already in the binary

- **Status:** **proposed** (2026-11-27)
- **Date:** 2026-11-27
- **Related:** [0021](0021-delete-runtime-crate.md) (the same shape), `dbg/src/dwarf.rs`, `compiler/src/main.rs:682`

## The question

The debugger can set breakpoints, walk the frame chain and resolve `.nova` line numbers, but it
cannot answer `print x`. Every name is refused:

> cannot evaluate `x`: reading a variable needs its stack offset and its type, and this compiler
> emits neither — only line information. `print 0xADDRESS` reads memory directly

Should that change, and at what cost?

## What is true today, measured

| Fact | Command | Result |
|---|---|---|
| the C compiler is invoked with debug info | `compiler/src/main.rs:682` | **`link.arg("-g")`** |
| a DWARF model exists in the debugger | `dbg/src/dwarf.rs` | **364 lines**: `DwarfInfo`, `CompileUnit`, `DebugEntry`, `AttributeValue`, `VariableSymbol`, `find_variable`, `extract_symbols` |
| that model can ever be populated | `parse_buffer` in the same file | **no** — it returns `Err(NotImplemented)` for every input |
| anything calls it | `grep -rn "parse_file\|parse_buffer" dbg/src` outside `dwarf.rs` | **nothing** |
| what does answer line numbers | the debugger, per ADR 0016 | `addr2line`, not this module |

**So the information `print x` needs is already in every binary this compiler builds**, because it
links with `-g`. What is missing is a reader. And the file that looks like a reader is a data model
with no parser, re-exported from `dbg/src/main.rs` and called by nobody.

That is the `runtime/` shape from [ADR 0021](0021-delete-runtime-crate.md): a scaffold whose presence
suggests a capability the code does not have. It is also why the refusal message blames the compiler —
"this compiler emits neither" — when the compiler emits line information *and* the C compiler emits
the rest. **The message is wrong about its own reason**, which is worse than a plain "not implemented".

## Options

### A. Parse `.debug_info` with `gimli`, and make `print x` work

Add `gimli` and `object` — the standard read-only, pure-Rust DWARF crates — delete the hand-rolled
model, and implement variable lookup: find the DIE for the current function, its children of kind
`DW_TAG_variable`, and evaluate the location expression for the current PC.

**Cost:** two dependencies in a workspace that has deliberately few; a real implementation of
location expressions (register, stack offset, and the composite forms gcc emits).
**Benefit:** `print x` works wherever the C compiler emits locals — gcc and clang, which is every
target the project builds on. It does **not** work with `zig cc`, which emits line information and
no locals; the debugger would say so rather than refuse everything.

### B. Hand-roll the `.debug_info` parser

**Rejected.** DWARF is a format where partial support produces plausible wrong answers — a variable
read from the wrong slot looks like a number, not like an error. Writing it by hand is months, and
the result would be a worse `gimli`.

### C. Delete `dbg/src/dwarf.rs` and keep the refusal

364 lines that can never be populated go away, and the refusal message is corrected to name the real
reason.

**Cost:** `print x` stays impossible. **Benefit:** the debugger stops implying a capability it lacks,
and the codebase loses a second scaffold of the same shape as the first.

### D. Shell out to `readelf` / `addr2line`

**Rejected.** They answer line questions and symbol addresses. Neither evaluates a location
expression, which is the actual problem.

## Recommendation

**C now, A when someone wants `print x`.**

Not because A is wrong — A is the only design that works — but because it is a decision with a
dependency cost, and the repository currently pays for a *fake* version of the same feature. Deleting
the dead model and fixing the message is one commit and makes the repository honest. A is then a
feature someone chooses, with a measurement in front of it.

If A is chosen, the shape is known: `gimli` reads `.debug_info`, the existing `addr2line` path keeps
answering line questions, and `print x` says which compiler it needs when the binary has no locals.

## What this ADR does not decide

- Whether `print x` should work on `zig cc` builds. It cannot: that compiler emits no local-variable
  DWARF. The honest behaviour is a message naming the compiler, not a refusal of all names.
- Whether a NOVA-side debug map (a sidecar the compiler writes) is worth building instead of DWARF.
  It would work on every toolchain, but it means designing a format and keeping it in step with the
  generated C — a second copy of information DWARF already carries.

## Consequences of choosing C

- `print x` keeps refusing, with a message that names the real reason.
- 364 lines leave `dbg/`, and the debugger's remaining 2,000-odd lines all do something.
- A is still available, and this ADR is where its cost is written down.
