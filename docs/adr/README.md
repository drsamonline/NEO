# Architecture decision records

One file per decision that was hard to make and would be expensive to unmake.

## Why these exist

`DEV_PLAN.md` says what is next. An ADR says what was **decided**, by whom, against
what alternatives, and what it costs. Without that, a decision gets re-litigated
every few months and the reasoning is lost each time — and a plan that quietly
changes its mind is indistinguishable from one that was never followed.

## Format

```
# ADR NNNN — title
- Status: proposed | accepted | superseded by NNNN
- Date: YYYY-MM-DD
- Supersedes: what this replaces, if anything

## Context      — the problem, and what was being treated as one problem but is two
## Decision     — what was chosen, stated so it can be checked
## The invariant / the rule  — what makes it sound, in one sentence
## What this is not          — the nearest wrong reading, refused explicitly
## Precedent                 — who has done this before, with links
## Consequences              — what it buys, what it does not, what it costs
## Known gaps                — what was found while writing it and not fixed
## Next steps                — in order
```

The "what it is not" section is the one that earns its place. Most disagreements
about a decision are about a reading of it that nobody chose.

## Index

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-lossless-syntax-tree.md) | A lossless tree at item boundaries with deferred bodies, rather than a full CST migration or nothing | accepted |
| [0002](0002-traits.md) | Traits are not in the language; the dead `Item::Trait` is removed rather than kept as an aspiration | accepted |
| [0003](0003-option-payload.md) | `Option<T>` holds a primitive, not a struct; a `void*` slot would turn a compile error into a use-after-free | accepted |
| [0004](0004-deferred-global-initialisers.md) | A non-constant global initialiser runs at the top of `main`; a static backing array would make the first `push` undefined behaviour | accepted |
| [0005](0005-references.md) | A borrow is a pointer; borrowing a reference is identity; `&x` used to lower to `!x` | accepted |
| [0006](0006-array-element-types.md) | Arrays hold `int`, so `[float]` and `[String]` were truncated or rejected by C. Monomorphised: one header and six functions per element type | accepted |
| [0007](0007-string-indexing.md) | `s[i]` is a byte, not a codepoint, because `char` is one byte — whatever the spec says. Deciding otherwise means deciding what `char` is first | accepted |
| [0008](0008-casts.md) | `as` converts. It was being dropped, which made a cast a comment and left three type-driven features reading the wrong type | accepted |
| [0009](0009-nested-arrays.md) | `[[int]]` is an array of `NovaArray` structs, copied by value so a write through a row reaches the matrix. Only `[int]` inner arrays are represented | accepted |
| [0010](0010-file-handles.md) | An open file is an index into a fixed table, not a pointer | accepted |
| [0011](0011-int-is-64-bit.md) | `int` is 64-bit in variables and arrays, not only in literals | accepted |
| [0012](0012-entry-point-arguments.md) | The emitted entry point takes `argc`/`argv`, so `core.args()` can answer | accepted |
| [0013](0013-streams-are-file-handles.md) | A `Stream` is a file handle with a label; no separate runtime | accepted |
| [0014](0014-serialize-is-json.md) | `serialize` writes JSON, and rejects types it cannot write honestly | accepted |
| [0015](0015-generic-return-type-inference.md) | A generic used only in its return type takes `T` from the binding | accepted |
| [0016](0016-debugger-uses-ptrace.md) | The debugger attaches with `ptrace`, and `unsafe` lives in one file | accepted |
| [0017](0017-ownership-proposal.md) | Move semantics for non-`Copy` types, then reference counting | accepted |
| [0018](0018-module-qualified-names.md) | Qualified calls; a collision is refused, not resolved | accepted |
| [0019](0019-mangling-for-module-collisions.md) | Mangle only what collides, so two modules can share a name | accepted |
| [0020](0020-concurrency-model.md) | A concurrency model, and why the one-string backend contract cannot carry a thread on two of four targets | accepted (E) |
| [0021](0021-delete-runtime-crate.md) | Delete the Rust runtime crate — 2,480 lines nothing depends on | accepted |
| [0022](0022-result-type.md) | `Result<T, E>` — sugar over the enum that already existed | accepted |
| [0023](0023-print-variable.md) | `print <name>` — the DWARF model is dead weight, and the information is already in the binary | accepted (C) |

## Writing one

Add `NNNN-short-title.md`, add a row above, and link it from
[`../README.md`](../README.md). `tools/docs-check.py` fails if a document under
`docs/` is not reachable from the index, so an unlinked ADR is a red build.
