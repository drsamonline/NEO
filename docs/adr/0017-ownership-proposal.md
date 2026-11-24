# ADR 0017 — what "ownership" would mean, and whether to adopt it now

- **Status:** proposed
- **Date:** 2026-11-24
- **Related:** [0016](0016-debugger-uses-ptrace.md), [Memory chapter](../language/11_memory.md), [Ownership chapter](../language/10_ownership.md) (a stub)

> **This ADR is proposed, not accepted.** It lays out a decision that belongs to the maintainer.
> Nothing in it has been implemented, and no chapter describes behaviour it does not have.

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

**None taken.** The maintainer decides; this ADR exists so the decision is made once, with the
options on the table, rather than rediscovered in a pull request comment.

If a decision is wanted now, the recommended pair is **C then D**, in that order:

- **C first**, because a silent wrong answer (a shared buffer mutated through two names) is worse
  than a leak, and move semantics is the smaller change: it adds a rule, it does not add a
  runtime.
- **D second**, because reference counting is invisible to the type system and can land without
  invalidating a single existing program.

**B and E are rejected for this language**, on the evidence above: the standard library and the
parity programs are the specification's test of what NOVA means, and both options require
rewriting them.

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
