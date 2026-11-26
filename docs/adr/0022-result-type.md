# ADR 0022 — `Result<T, E>`, and the test that must come first

- **Status:** **proposed** (2026-11-26)
- **Date:** 2026-11-26
- **Related:** [0020](0020-concurrency-model.md), [Errors chapter](../language/12_errors.md), `crates/nova-common/src/types.rs`

## The question

NOVA has one way to say "this may not work": `Option<T>`. It carries **no reason** — it says a thing
did not happen, not why. `docs/language/12_errors.md` states the gap plainly:

> A caller that needs to distinguish "file missing" from "permission denied" has to return something
> wider itself, and there is no standard type for that yet.

Should `Result<T, E>` exist?

## What is true today, measured

| Fact | Command | Result |
|---|---|---|
| stdlib functions that return `Option<T>` | `grep -rn -- "-> Option<" stdlib/*/*.nova \| wc -l` | **16** |
| modules that use it | `grep -rln -- "-> Option<" stdlib/*/*.nova` | **3** — `core`, `io`, `text` |
| `match` parses | `compiler/src/parser/mod.rs` has `parse_match_expr` behind `TokenKind::Match` | **yes** |
| `match` is lowered as | `fn lower_match_as_if` in `compiler/src/lowerer/mod.rs` | an **if-else chain** |
| `Option<T>` is represented as | `option_struct_name` in `crates/nova-common/src/types.rs` | a synthesised struct: `Option_int`, `Option_array_int` |
| the backend knows the type by name | `compiler/src/backend/mod.rs` strips the `Option<` prefix | **yes** |

So `Option<T>` is not a user-definable enum. It is **compiler machinery**: a naming function, two
built-in constructors (`Some`, `None`), a struct the backends emit, and a `match` arm that reads a
field and binds it.

**One thing is not verified.** The chapter shows this, and nothing compiles it:

```nova
match divide(10, 0) {
    Some(value) => print(value),
    None => print(-1),
}
```

It is a **fragment** — no `fn main` — so `every_complete_program_in_the_spec_compiles` skips it, and
this pod has no Rust toolchain to build the compiler and test it by hand. **Whether `match` on an
`Option` works end to end is, as of this ADR, an assumption.**

## Why that matters more than the design

`Result<T, E>` would be built on the same machinery: a synthesised struct, two constructors, and a
`match` that binds a payload. If the existing single-payload path does not actually run, then a
two-payload type is being designed on top of something that was never demonstrated.

**Step 0 of any Result work is therefore a test, not a design**: a complete program that divides,
matches on the `Option`, and prints both branches, run by `cargo test`. It belongs in the repository
whether or not Result is ever built, because the chapter claims the pattern works.

## The extra cost `Result` carries that `Option` does not

`Option<T>` binds **one** type in one arm. `Result<T, E>` binds **two different** types in two arms:

```nova
match parse_int("x") {
    Ok(n) => print(n),        // n: int
    Err(reason) => print(reason),  // reason: String
}
```

Today the arm's binding is derived from the scrutinee's single payload type. With two, the type of
`n` and the type of `reason` differ **within one match**, so the arm's binding type must come from
the arm's pattern rather than from the scrutinee. That is a sema change, not a naming function —
and it is exactly the kind of detail that ADRs 0017, 0018 and 0019 each under-estimated.

## Options

### A. `Result<T, E>` as a second synthesised struct

`result_struct_name(T, E)` beside `option_struct_name`, `Ok`/`Err` constructors, backend support for
the tag and the payload, and per-arm binding types in `match`.

**Cost:** the type is cheap (the Option pattern repeats); the **arm typing** is the real work, plus
16 stdlib functions to consider, plus `?`-style propagation if it is wanted (another keyword).
**Benefit:** the gap the chapter names is closed, and `io.read_file` can say *why*.

### B. `Option<T, E>` — widen the existing type

One type, an optional payload.

**Cost:** cheap. **Benefit:** no second type. **Risk:** every existing `Option<T>` becomes
`Option<T, ()>`, and "no reason" stops being expressible as a type — which is the property the
chapter's design rests on.

### C. Error codes as return values

Return an `int` and define a table. **Rejected:** it is the C convention the chapter explicitly
contrasts itself against, and it loses exhaustiveness — the compiler cannot tell a caller that they
forgot to check.

### D. `panic` with a message, and nothing else

What exists. **Rejected as an answer:** a panic is not a return value; it stops the program, so a
caller cannot recover.

### E. Do nothing, and say so

Keep the chapter's sentence as the answer: there is no standard type for failure-with-reason, and a
caller that needs one defines its own struct.

**Cost:** nothing. **Benefit:** honest, and `Option` stays the one absence type.

## Recommendation

**Step 0 now; A only if step 0 passes.**

Step 0 is a test that compiles and runs the chapter's `Option`/`match` example. It is small, it is
owed regardless, and it converts an assumption into a fact. If it fails, the finding is a bug in
`match` on `Option` — far more valuable than a new type designed on top of it.

If it passes, **A** is the design to build, and its cost is the per-arm binding type, not the struct.
**B is rejected** because it makes "no reason" unrepresentable, which is the one thing `Option`
currently gets right.

## What this ADR does not decide

- Whether `?`-style propagation exists. Without it, every call site is an explicit `match`, which is
  verbose and exhaustiveness-checked — the same trade `Option` already makes.
- Whether the standard library migrates. 16 functions return `Option<T>`; some genuinely have no
  reason to give (`find` returning "not found" needs none), so migration is per-function, not
  wholesale.
- Whether `Result` is spelled `Result<T, E>` or `Result<T>` with a built-in error type. The second
  is cheaper and worse.

## Consequences of choosing E

- `12_errors.md` stays accurate with no edit.
- The 16 `Option`-returning functions keep their signatures.
- Step 0 is still owed, because the chapter's example is still untested.
