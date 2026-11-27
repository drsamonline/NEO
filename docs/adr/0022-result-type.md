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

## Step 1, measured — 2026-11-26

A test was written before the feature, as this ADR asks: a complete program whose `Ok(n)` binds an
`int` and whose `Err(reason)` binds a `String`, in one match. It fails, in the compiler's own words:

```
result_match.nova:5:16: [E0002] undefined name `Err`
```

Two things that measurement settles:

- **`Result<int, String>` as a return type is accepted** — types are strings, so the annotation
  passes without `Result` existing. The failure is at the *constructor*, not the annotation.
- **The gap is exactly where this ADR predicted**: there is no `Ok`, no `Err`, and no struct for a
  match arm to read. Nothing about the arm-typing problem can be measured until those exist.

The test was not merged: a repository whose checks are all green cannot carry a test that fails. It
is reproduced in this section so the next attempt starts from the same program.

**Building it needs a toolchain.** The pod that wrote this section has no `cargo`, `rustc`, `zig` or
`gcc`, so the compiler cannot be built locally and the four layers below cannot be verified without a
CI round trip each. That is a fact about the environment, not about the design.

## Found while building it — `Result` already exists as a user enum

**2026-11-27.** A step-3 attempt (constructors, struct emission, backend mapping) was written and CI
failed on a test that already existed:

```
generic_enum.c:18:11: error: unknown type name 'Result_int_String'
```

That test declares this, and it **passed before the change**:

```nova
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn show(r: Result<int, String>) {
    match r {
        Result.Ok(v) => { print(v); },
        Result.Err(e) => { print(e); },
    }
}
```

So the language **already has** a two-payload generic enum, and its `match` **already binds a
different type per arm** — `Result.Ok(v)` binds an `int`, `Result.Err(e)` binds a `String`, in one
match, today, in a passing test.

**This ADR's central claim was wrong.** "The cost `Result` carries that `Option` does not is arm
typing" is false: arm typing for two payloads is the *enum* path, and it works. The attempt to give
`Result` its own struct was not just unnecessary, it **broke the existing enum** by hijacking the
type name.

What is actually missing is smaller and different: **the `Result.` prefix**. A user must write
`Result.Ok(7)`; the chapter and this ADR assume `Ok(7)`. The work is sugar — recognise a bare `Ok`/`Err`
call and a bare `Ok(x)` pattern as `Result.Ok`/`Result.Err`, and inject the enum definition when a
program uses them without declaring it.

That is a much smaller change than the map below describes, and it reuses machinery that is already
proven rather than building a second way to express the same thing.

The step-3 attempt was reverted in full: `backend/mod.rs`, `lowerer/mod.rs` and `sema/mod.rs` are
back at `origin/master`. `result_struct_name` and `result_payloads_of` remain in `nova-common` with
their tests — they are correct functions that the enum path does not need, and removing them is a
separate decision rather than a side effect of this one.

## The implementation map — 2026-11-27

Step 2 is merged: `result_struct_name(ok, error)` in `crates/nova-common/src/types.rs`, sharing one
`mangle_payload` with `option_struct_name`, with four unit tests.

What remains, located exactly. **`Option` is the template for every one of these**, which is why the
list is short:

| Layer | `Option` does this | `Result` needs |
|---|---|---|
| sema, `check_identifier` | `compiler/src/sema/mod.rs:815` admits `Some`/`None` | admit `Ok`/`Err` |
| sema, call check | `:940` skips signature checking for `Some` | skip for `Ok`/`Err` |
| sema, `infer_expr_type` | `:1182` returns `None` for a `Some` call | `None` for `Ok`/`Err` |
| lowerer, field | `option_payloads: BTreeSet<String>` (line 66) | `result_payloads: BTreeSet<(String, String)>` |
| lowerer, registration | `note_option_payload` from declared types (826, 871) | the same, parsing `Result<A, B>` |
| lowerer, struct emission | one `Option_<T>` per payload, fields `tag`/`payload` (625) | one `Result_A_B` per pair, fields `tag`/`ok`/`error` |
| lowerer, `Ok(x)` | `Some(x)` builds a struct literal with tag 0 | tag 0, field `ok` |
| lowerer, `Err(e)` | — | tag 1, field `error` |
| lowerer, match arm | `Pattern::EnumVariant` at 3468: `Some` → tag 0, payload from `option_payload_type` | `Ok`/`Err` → tag, and **the field and type differ per arm** |
| backend | `compiler/src/backend/mod.rs:388` maps a payload to `option_struct_name` | map a pair to `result_struct_name` |

**The last two rows are the work.** Everything above them is mechanical repetition of a pattern that
already exists; the arm binding is the one place where `Result` genuinely differs, because the
binding's type must come from the arm's pattern rather than from the scrutinee's single payload.

**It was not written in the same session as this map, on purpose.** Each of those rows needs the
compiler to build, and this pod cannot build Rust — `static.rust-lang.org` is not on the egress
allowlist. Merging `Ok`/`Err` half-wired would mean a compiler that accepts a constructor and
mis-lowers it, which is the one failure this repository treats as worse than a refusal.

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
