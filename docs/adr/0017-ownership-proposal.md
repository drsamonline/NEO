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

Measured on 2026-11-24, over `examples/` and `tests/parity/`: **28 warnings**, of which **26 came
from the standard library** and were re-reported for every program that imported it.

**Fixed the same day, and the fix needed two parts.** Warnings are now filtered by the same bound
the C backend uses for `#line` — a line past the end of the file the user named is not from that
file — and **the filter only works if the warning has a span**. It did not: 25 of the 28 arrived
with no span, so they could not be attributed and were all shown. Falling back to the enclosing
expression's span brought the count to **3 shown, 25 suppressed**, which is the number the
suppression was written for.

**Step 4 is done.** `core.clone(s: String) -> String` copies the string through
`__builtin_strdup`, which the runtime already had and nothing could reach. Passing to `clone` does
not count as a move: a clone that consumed its argument would make the advice produce the same
warning one line later, which is how a rule gets turned off. Arrays and structs are not cloneable
yet — calling `clone` on one is a type error rather than a shallow copy that shares the buffer the
rule exists to protect.

**The assignment gap is closed.** `x = f(y)` and `other = greeting` now mark the value moved,
the same way a call argument does — so the rule is as wide as its own description. Re-measured
after closing it: still **3 shown, 25 suppressed** over the corpus, because the corpus assigns
strings nowhere.

Nothing in the rule is knowingly narrower than its statement now, which is the condition step 5
was waiting for.

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

## Step 5 attempted, and not taken — measured, 2026-11-24

Enforcing the rule was a one-line change and it failed on evidence, in two stages.

**First: the shipped example `array_ops.nova` stopped compiling**, with three move warnings about an
array it passes and reuses. Arrays are not cloneable, so the advice was impossible to follow. The
rule now applies only where `clone` exists — `String` today — and the corpus's own code trips it
**0** times.

**Second: the standard library stopped compiling.** With `E0010` as an error, a program that
imports `core`, `io` and `text` failed with "semantic analysis failed inside an imported module" —
our own library violates the rule in places, and a rule that rejects the library it ships with is
not enforceable.

So step 5 is **blocked on the standard library complying**, and the order is:

1. ~~clone for arrays~~ — **done**: `clone` is now a compiler builtin dispatching on the
   argument's type (`String` → `nova_strdup`, `[int]` → `nova_array_clone`), and it refuses
   `[String]` at compile time because a shallow copy of an array of pointers shares exactly what
   the rule protects. It had to become a builtin: two `fn clone` overloads are a duplicate
   definition, which the check from #140 correctly rejects;
2. ~~clone for structs~~ — **done** for a struct whose fields are all scalars, field by field.
   A struct holding a `String` is refused: copying it would share the string, which is the
   distinction that made `[String]` refuse in the first place;
3. fix the library's own sites, counted by running the checker over it with enforcement on;
4. then enforce.

**A finding worth keeping**: the suppression of imported-module diagnostics had to be extended to
errors as well, because an error inside `io.nova` would otherwise fail every program that imports
the standard library. That is now done, and it is what made the second measurement visible at all.

## The enforcement measurement — 2026-11-24

Run with `E0010` temporarily raised to an error and a probe program importing all four modules.

| Where | `E0010` |
|---|---|
| `examples/` + `tests/parity/` (own code) | 3 |
| **the standard library** | **214** |

**214 is not a number you fix with 214 `clone` calls.** It says something about the rule: NOVA
strings are shared pointers, so passing one does not invalidate the caller's variable — the
callee gets the same pointer. The rule says "passing moves it", which is what a language with
real move semantics means, and is not what this language does at runtime.

So step 5 is not merely expensive, it is **wrong as specified**: enforcing it would make every
function that reuses a `String` parameter clone it, allocating and copying on a hot path, to
protect against a mutation that the runtime does not permit anyway (there is no `free`, and
strings are immutable in practice).

**What this means for the decision:** the honest options are

1. **keep the warning, never enforce** — it still catches the case that is real today, a value
   used after being given away by assignment, at the cost of noise on every pass-then-use;
2. **narrow the rule to assignment only** — `other = greeting` really does look like giving a
   value away, and the corpus trips that zero times;
3. **implement real move semantics at runtime** — the callee takes ownership, which is a change
   to the calling convention, not to the checker.

Option 2 is the one this measurement argues for, and it is the opposite of what the ADR proposed
when the rule was designed.

## The decision the measurement forced — 2026-11-24

Of the three options the 214 measurement left, the **second was taken: the rule is narrowed to
assignment**, and passing no longer counts.

| Construct | Marked? | Why |
|---|---|---|
| `other = greeting` | **yes** | two names for one value; the reader cannot tell which was meant |
| `f(greeting)` | no | the callee gets the same pointer and the caller's variable stays valid |

Verified both ways by hand: a `String` passed and used again builds with no warning; a `String`
assigned and then used warns, with a message that says which of the two happened. The corpus trips
it **0** times, and so does the standard library, because neither assigns a `String` away.

**This is the opposite of what this ADR proposed.** It said a value is moved when passed *or*
assigned. The measurement — 214 sites in the library, every one of them a pass-then-use — showed
that the passing half of the rule was describing a language that does not exist, and it was
dropped rather than enforced.

What the rule now catches is small and real. What it no longer catches was never a bug in this
language.

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
