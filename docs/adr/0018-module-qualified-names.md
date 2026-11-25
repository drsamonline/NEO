# ADR 0018 — module-qualified names

- **Status:** **accepted** (2026-11-25) — option **C**, of which the qualified form is implemented
- **Date:** 2026-11-25
- **Related:** [Modules chapter](../language/08_modules.md), [0017](0017-ownership-proposal.md)

> **Accepted 2026-11-25: option C.** `math.log(x)` now resolves. The parts not implemented are
> named in "What was built, and what the plan got wrong" below.

## Context

`use name;` merges a module's declarations into **one flat namespace**. Two modules that declare the
same name cannot be imported together, and until 2026-11-24 the second definition was dropped
silently, so the program resolved the name to whichever module happened to load first.

Measured facts:

- `io.log(level, message)` and `math.log(x)` collided; a program with `use math; use io;` reported
  a wrong-arity call **inside the library**, naming neither module. Fixed by renaming `io.log` to
  `log_message`; the loader now refuses a collision and names both modules; `stdlib-check` fails if
  the library ever reintroduces one.
- A qualified call does not work at all today:

  ```nova
  use math;
  let x = math.log(2.0);   // error [E0002]: undefined name `math`
  ```

- The parser already produces `Expr::Field(Identifier("math"), "log")` for `math.log`, and the
  lowerer already special-cases one meaning for that shape — `Color.Red` is an enum variant. So
  the syntax exists and has a competing interpretation.

## What was built, and what the plan got wrong — 2026-11-25

**Implemented: step 3, the qualified form.** `math.log(x)` resolves to `log`, in three places that
each had to learn about it:

| Place | What it does now |
|---|---|
| `sema` | a call whose callee is `Field(module, name)` checks `name`, not the module |
| `lowerer` | lowers it to the function itself, before the method-call path |
| `lowerer`'s `expr_type` | answers the function's return type |

**The third one was not in the plan and it mattered.** With only the first two, `math.log(2.0)`
built and printed `0`: the type was unknown, `print` fell back to `int`, and the answer was
truncated. A qualified call that compiles and answers wrong is worse than one that refuses — the
same lesson as `✓ Published`, in a different place.

**And the plan's order was wrong.** It said steps 1–2 (diagnose an ambiguous use, naming both
modules) would remove the failure on their own. They cannot: to keep *both* definitions in the
merged program, the backend must emit two C functions with the same name, which needs mangling —
work the plan did not account for. So a collision is still refused at import, and `use math; use
io;` still fails until the library renames one. What C bought is the **syntax** for when that is
not the case.

Verified by hand and by a test: `log(2.0)` and `math.log(2.0)` both answer `0.693147`, and a
`String` argument works (`text.trim(s)`).

## What is actually being decided

Not "should modules be namespaced" — they already are files with their own declarations. The
question is **how a name from another module is written at the use site**, and there are three
separate things a decision could cover:

1. **Disambiguation** — writing `math.log` to pick one of two `log`s.
2. **Hygiene** — whether an unqualified `log` resolves at all when both are imported.
3. **Re-export** — whether a module can say "my `log` is `math`'s `log`".

Only the first is needed to fix the defect. The other two are scope creep and are marked as such
below.

## Options

**A. Status quo, with the refusal.** Keep the flat namespace; a collision is an error naming both
modules, and the library is kept clean by a gate.
*Cost:* zero. *Why it loses:* a user still cannot import two third-party modules that share a name,
and the only remedy is editing code they may not own.

**B. Qualified only.** `use math;` brings in nothing unqualified; every use is `math.log(x)`.
*Cost:* every existing program and all 249 standard-library functions change at every use site.
*Why it loses:* it is the largest possible break for the smallest possible gain, and it makes the
common case (one module, no collision) verbose.

**C. Qualified when ambiguous, unqualified otherwise.** `use math;` keeps `log` usable unqualified
while nothing else declares it; when two modules declare the same name, an unqualified use is an
error that names both and suggests the qualified forms.
*Cost:* the resolver must know which module each declaration came from — the loader already records
this since 2026-11-24 — and the lowerer must resolve a `Field` whose object is a module.
*Benefit:* no existing program changes. The collision becomes solvable instead of fatal.

**D. Prefixed names.** The library exports `math_log`, `io_log`.
*Why it loses:* it is a naming convention with no enforcement, which is what the gate already does,
and it reads badly for anything with two segments.

**E. Explicit import lists.** `use math::{log, sqrt};` — only the named items enter scope.
*Cost:* a parser change and a new grammar production. *Benefit:* precise, and it composes with C.
*Why it loses for now:* it solves hygiene, which is problem 2, not the defect, which is problem 1.
C is a strict subset of what E gives and does not need new syntax.

## Decision

**None taken.** The recommendation is **C**, then **E** if hygiene is ever wanted:

- **C fixes the defect** — a program can import two modules that share a name — without changing a
  single existing program, because unqualified names keep working wherever they are unambiguous.
- **E is the better long-term design** but adds grammar for a problem nobody has reported. It can
  be layered on top of C later without undoing it.

B is rejected as a break with no matching benefit. D is rejected because it is a convention the
gate already enforces. A is what exists, and it is what the defect costs.

## Consequences, if C is accepted

**The hard part is not the resolver, it is the syntax.** `math.log(x)` parses as
`Field(Identifier("math"), "log")`, and `Color.Red` already means an enum variant. So the lowerer
must decide, for a `Field` whose object is a bare identifier: is it a module, an enum, or a struct
value? That is one lookup in three tables, and the rule must be stated in the
[Modules](../language/08_modules.md) and [Types](../language/06_types.md) chapters or the next
reader will guess wrong.

The order of work, if accepted:

1. record each declaration's module in the symbol table (the loader has it; sema does not);
2. an unqualified use of an ambiguous name becomes a diagnostic naming both modules and the
   qualified forms — **the error message is the deliverable here**;
3. resolve `Field(module, name)` in the lowerer, after the enum-variant case and before struct
   field access;
4. chapter 08 gains a section, and the ADR's rejected options go with it.

Steps 1–2 alone remove the "cannot import two modules" failure; step 3 makes the qualified form
work.

**What this does not fix:** a module cannot re-export another's name, and two modules still cannot
both be imported *and* both used unqualified. C makes the second one diagnosable and solvable by
the author, not silent.
