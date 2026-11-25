# ADR 0019 — mangling, so two modules can declare one name

- **Status:** proposed
- **Date:** 2026-11-25
- **Related:** [0018](0018-module-qualified-names.md), [Modules chapter](../language/08_modules.md)

> **Proposed, not accepted.** Nothing here is implemented.

## Context

ADR 0018 gave NOVA the *syntax* for a qualified call and left the collision fatal, because keeping
two functions called `log` in one program needs the backend to emit two C functions under one name.
This ADR is that piece, and it exists because the last two ADRs were both wrong about their own
cost — 0017 proposed an enforcement that would have made the library uncompilable, 0018 proposed a
step order that cannot work. So this one is written **after** measuring, not before.

## What the measurement says

Four facts, from the code:

1. **`emitted_name` is the only place a NOVA function name becomes a C name.** It is called at
   three sites: the declaration (twice, entry and ordinary) and the call
   (`compiler/src/backend/mod.rs:210`, `:262`, `:980`). Nothing else writes a function name into
   the output.
2. **The SIR callee is a `String`**, not a struct with fields. So a *qualified* name needs no
   change to `SirExpr::Call` — `math::log` fits in the string that is already there.
3. **The loader already knows which module each declaration came from** — it records
   `name → module` to detect collisions, since 2026-11-24.
4. **`Item::Function` carries a `name: String`**, so a module can be encoded there without a new
   field, exactly as in (2).

That made the change look smaller than 0018 feared: no new struct fields, one function to extend,
and the information needed already exists.

## Correction — measured the same day, and fact 4 is wrong

Writing the plan turned up the thing that decides the whole shape: **a call site has to know which
module it is in**, or `io`'s internal `log(x)` still resolves to `math`'s. Nothing carries that:

- `Item::Function` has no module field — fact 4 said one would fit, but it is not there;
- the merged program is a flat `Vec<Item>` with no origin;
- **spans cannot stand in for it**: a span is line and column with no file, and **24 line:column
  positions are shared by two or more standard-library functions** (measured: `91:1` is both
  `core.array_pop` and `math.log2`; `95:1` is `core.array_get`, `io.feof` and `text.trim`).

So the minimal honest change is **one new field**, `module: Option<String>`, on `Item::Function`,
set by the loader. Its ripple is measurable rather than guessed:

| | count |
|---|---|
| places that construct a `Function` | 8 |
| files that match on `Item::Function` | 5 |
| reads of `func.name` in the C backend alone | 20 |

That is a real change to the AST shared by four backends, and it is the piece this ADR's first
draft claimed did not exist. **The estimate was wrong a third time**, and the pattern is now clear
enough to state: every one of these ADRs under-estimated because it counted the *emission* point and
not the *information* the emission needs.

The rest of this document is the original plan; read it with that correction in mind. Steps 1–3 are
larger than written: step 1 must also attach the module to every declaration, and step 3 needs the
caller's module to resolve an unqualified name.

## Options

**A. Rename in the library, forever.** What exists: `io.log` → `log_message`, a gate against
reintroducing a collision, a refusal naming both modules.
*Why it loses:* it does not scale to third-party modules, and it makes a legitimate name choice
(the same word for the same idea in two domains) a defect.

**B. Mangle every function by module, always.** Every C function becomes `module__name`.
*Cost:* the output no longer resembles the source, every debugging session reads `io__log_message`,
and `nm` output in the debugger stops matching what the user wrote. *Why it loses:* it pays a
permanent readability cost to solve a rare problem.

**C. Mangle only what collides.** A name declared by two modules is emitted as `module__name`; a
name declared once keeps its plain name. Qualified calls emit the mangled form; unqualified calls
are rewritten by the lowerer to whichever single candidate exists.
*Cost:* the loader must keep both declarations instead of dropping one; sema must stop reporting a
duplicate for a name that came from two different modules; the lowerer must rewrite an unqualified
callee to its mangled form.
*Benefit:* the common case is unchanged — the output for a program with no collision is byte-for-
byte what it is today — and a collision becomes legal.

**D. Mangle only when the user asks** (`use math as m;`).
*Why it loses:* it puts the burden on the person who cannot change a library they depend on.

## Decision

**None taken.** The recommendation is **C**, because it is the only option that leaves today's
output untouched for every program that does not have the problem.

## The plan, with the four places that change

1. **loader** — keep both declarations; record `name → [modules]` rather than `name → module`, and
   refuse only when the same module declares a name twice (which is a genuine duplicate).
2. **sema** — a name with two candidates is not a duplicate; it is *ambiguous*, and an unqualified
   use of it is a diagnostic naming both modules and both qualified forms.
3. **lowerer** — resolve an unqualified callee against the candidate map and emit the mangled name
   in `SirExpr::Call.callee`; a qualified call emits it directly.
4. **backend** — `emitted_name` maps `a::b` to `a__b` and leaves every other name alone.

Steps 1–3 are where the risk is; step 4 is a match arm.

## Consequences

**What gets worse.** A name that collides produces a C name nobody wrote, so a breakpoint or an
`nm` lookup on it reads `math__log`. The debugger resolves `break <fn>` through `nm`, so it will
find the mangled name and report the `.nova` line — but the name in the transcript will be mangled,
and that is a small cost worth stating in the chapter.

**What stays the same.** A program with no collision emits exactly what it emits today, because
`emitted_name` only changes names containing `::`.

**What this does not fix.** Re-export (`use a::b as c`), visibility, and `use ... as ...`. Those
are ADR 0018's rejected option E, still unwritten.

## Why this is worth doing at all

The defect is narrow: two modules cannot share a word. But the workaround has already cost one
rename in the library, a gate, a loader refusal, and a chapter paragraph explaining all three — and
it will cost the same again the next time two modules want the same word. Mangling is the thing
that ends the sequence rather than extending it.
