# ADR 0017 — what "ownership" would mean, and whether to adopt it now

- **Status:** **accepted** (2026-11-24) — option **C** first, then **D**
- **Date:** 2026-11-24
- **Related:** [0016](0016-debugger-uses-ptrace.md), [Memory chapter](../language/11_memory.md), [Ownership chapter](../language/10_ownership.md) (a stub)

> **Accepted 2026-11-24: option C (move semantics for non-`Copy` types, with an explicit
> `clone`), then option D (reference counting) as a separate decision.** Steps 1–2 of the plan
> below — the `Copy` bit and the move rule in warn mode — are in progress. No chapter describes
> behaviour that is not implemented; chapter 10 stays a stub until step 5.

## Context

NOVA has no ownership model. The measured facts:

- **Nothing is ever freed.** `nova_strdup` leaks by design; there is no `free` in the language and
  no `Drop` to hang one on ([Memory](../language/11_memory.md)).
- **`&mut` is erased by the parser.** `&x` and `&mut x` both compile to a pointer, and the `mut`
  is consumed and dropped, so by the time anything could check aliasing the distinction is gone.
  Two `&mut` references to one value are not detected.
- **Arrays are copied by value.** `NovaArray` is a 12-byte header; passing one copies the header
  and shares the buffer, which is neither a move nor a deep copy.
- **Chapter 10 is a stub** because there is nothing to describe.

This is not one problem but three, and they are usually discussed as one:

1. **Reclamation** — who frees, and when. Today: nobody, and the process exit does it.
2. **Aliasing** — may two references to one value be written through? Today: yes, unchecked.
3. **Linearity** — may a value be used after being given away? Today: yes, and it is a copy.

An ownership system that answers only one of these is a different language from one that answers
all three, and the cost difference is an order of magnitude.

## Options

**A. Do nothing.** Keep leaking; keep `&mut` as documentation.
*Why it loses:* chapter 10 stays a stub and every long-running program leaks. But it costs
nothing today, and the status quo is the only option that has been tested by every program in
`examples/` and `tests/parity/`.

**B. Full borrow checker with lifetimes (Rust-like).**
*Why it loses:* it changes what every existing program means, needs region inference in the
analyser, and would reject most of the standard library as written. The standard library is 249
functions with an assertion each; a change that invalidates them is a rewrite, not a feature.
This is the right answer for a language designed around it, and NOVA was not.

**C. Move semantics for non-`Copy` types, with an explicit `clone`.**
A value is given away by assignment or by passing it to a function; using it afterwards is a
diagnostic. Numbers, `bool` and other scalars stay `Copy`. No lifetimes, no regions, no borrow
checker — just a per-type bit that says "this one moves".
*Cost:* a new rule in the analyser, a `clone` builtin, and a sweep of the standard library for
places that pass a `String` and then reuse it. *Benefit:* it answers question 3, and question 3 is
the one that produces silent wrong answers today — a shared array buffer is exactly how a program
mutates data it did not mean to.

**D. Reference counting at runtime (Rc-like), checked by nothing.**
Every heap value carries a count; `nova_strdup` returns a counted string.
*Cost:* a word per allocation, atomicity or a documented single-threaded assumption, and cycles
leak anyway. *Benefit:* it answers question 1 without touching the type system — the only option
here that fixes leaks without changing what programs mean.

**E. Linear types (must-use handles).**
*Why it loses:* it is C with extra steps for the standard library's string handling, and it makes
`Option<T>` and early `return` awkward — the two constructs the library leans on most.

## Decision

**Accepted 2026-11-24: C then D, in that order.** Option D is a separate decision and is not
implied by accepting C.

The reasoning, unchanged from the proposal:

- **C first**, because a silent wrong answer (a shared buffer mutated through two names) is worse
  than a leak, and move semantics is the smaller change: it adds a rule, it does not add a
  runtime.
- **D second**, because reference counting is invisible to the type system and can land without
  invalidating a single existing program.

**B and E are rejected for this language**, on the evidence above: the standard library and the
parity programs are the specification's test of what NOVA means, and both options require
rewriting them.

## Progress — 2026-11-24

**Step 1 is done and tested.** `SemanticAnalyzer::is_copy` classifies a `nova_common::Type`:
scalars, references and pointers are `Copy`; arrays, functions, structs and enums are not; a tuple
is `Copy` when all its elements are; an unknown type counts as `Copy`, because a missed warning is
cheaper than noise that teaches people to ignore warnings.

**Step 2 is done, in warn mode.** `let` now records the type of its initialiser — from an
annotation, a literal, a name already in scope, an array's first element, or a declared return
type — in **both** the symbol and the analyser's type environment, which are separate and were the
reason the first attempt changed nothing. A call that passes a non-`Copy` name marks it moved, and
a later use emits `E0010` as a **warning**.

Measured on 2026-11-24, over `examples/` and `tests/parity/`: **28 warnings**. But **26 of those
come from the standard library itself**, re-reported for every program that imports it. The
corpus's own code produces 2. That is the finding that matters for step 3: the count is small, and
the noise is structural — a user cannot act on a warning about `arr` inside `io.nova`.

Two gaps in this first version, both recorded in `docs/errors.md`: a call inside an assignment
(`x = f(y)`) does not mark `y` moved, and imported-module warnings are not suppressed.

**The original blocker, kept because it explains the design:** The rule is "passing a non-`Copy` value
moves it", which needs each binding's type. `check_statement` declares every `let` with
`Symbol::new(name, Variable, span)` — **no type** — so `symbol.typ` is `None` for every binding and
the classification can never say "non-`Copy`". Measured: a program that passes a `String` and then
prints it produced no warning, and instrumentation showed the lookup returning `None`.

So step 2 needs a smaller prerequisite first: **`let` must record the type of its initialiser.**
That is inference, which this analyser does not do — types are strings here, and the lowerer does
its own inference later. Until it lands, a move warning would be dead code that never fires, which
is worse than no warning.

**A real bug fell out of trying.** Warnings were never printed on a successful analysis: the
diagnostics loop ran only inside `if analyzer.analyze(..).is_err()`. Any warning this compiler ever
emitted on a working program was invisible. Fixed — and it is the reason the measurement above
could be made at all.

## What the sweep would touch — measured, 2026-11-24

The cost of option C is not the number of functions but the number of **call sites** that pass a
value and then reuse it. The surface is measurable:

| Module | Functions | taking a `String` | taking an array |
|---|---|---|---|
| `core` | 55 | 12 | 16 |
| `io` | 59 | 41 | 6 |
| `math` | 76 | 0 | 7 |
| `text` | 59 | 59 | 3 |
| **total** | 249 | **112** | **32** |

**The number that matters cannot be counted from the source.** A call site is a problem only when
the caller uses the value again afterwards, and deciding that is the checker's job. Any estimate
of "how many places will break" made by reading is a guess, and this repository has spent a
session replacing those with measurements.

That gives the plan its shape: **build the check before enforcing it.**

## Implementation plan, if C is accepted

1. **A `Copy` bit per type.** Scalars are `Copy`; `String`, arrays, structs and closures are not.
   It belongs in the type representation, next to the arity and the element type.
2. **The rule, in warn mode.** Assigning or passing a non-`Copy` value marks it moved; using it
   afterwards is a diagnostic. Emitted as a warning first, with a code, so the sweep is countable
   rather than estimated.
3. **Measure.** Run it over `stdlib/`, `examples/` and `tests/parity/`. The count of warnings is
   the real cost, and it is the number to put in this ADR before anyone commits to step 4.
4. **A `clone` builtin**, so every warning has an answer that is not "restructure".
5. **Enforce**, and write chapter 10 against what the checker does — not before.

Steps 1–2 are the whole risk. If the warning count from step 3 is large, the honest outcome is to
stop at step 3 and record the number here, which is a better result than a half-enforced rule.

## Consequences

**If C is adopted:** chapter 10 can be written and chapter 11 rewritten — the leak story stays,
the aliasing story changes. Every place the standard library passes a `String` and then uses it
again becomes a diagnostic, and each one is a decision: clone it, or restructure. That sweep is
the real cost, and it is measurable before committing: `grep -c 'fn .*String' stdlib/*/*.nova`
totals 112.

**If D is adopted:** the runtime gains a refcount, `nova_strdup` changes signature, and the
runtime ABI version moves from v3 to v4 — which `COMPATIBILITY.md` says is a change to the
compiler's output contract.

**If nothing is adopted:** chapter 10 stays a stub, and this ADR is the record of why. That is a
respectable outcome; the constitution's first rule is not to invent functionality, and a
half-implemented ownership model would be worse than a documented absence.
