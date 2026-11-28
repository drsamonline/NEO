# ADR 0024 — closure parameter types are not inferred, and the default is `int`

- **Status:** **proposed** (2026-11-28)
- **Date:** 2026-11-28
- **Related:** [0023](0023-print-variable.md), `tests/parity/float_closure.nova`, `compiler/src/lowerer/mod.rs`

## The question

`|x| x * 2.5` and `|x: float| x * 2.5` are the same program to a reader. They are not the same
program to this compiler, and the difference is invisible until the output is wrong.

What should happen to a closure parameter with no annotation?

## What is true today, measured

An unannotated parameter is typed `int`:

```rust
let param_types: Vec<String> = params.iter().enumerate().map(|(index, (_, typ))| match typ {
    Some(typ) => self.substitute(&type_display(typ)),
    None => contextual.get(index).cloned().unwrap_or_else(|| "int".to_string()),
})
```

The body is typed separately, and the binary rule says a float operand makes the result float — so
`x * 2.5` is `float` and the closure's *return* type is `float` while its *parameter* stays `int`.

A parity program then gives four answers to one program:

| Backend | `|x| x * 2.5` called with `2.0`, `0.5`, `1.5` |
|---|---|
| **C** (the reference) | `5`, `0`, `25` — the float argument truncates to the `int` parameter |
| **JavaScript** | `5`, `1.25`, `3.75` — right by accident: one number type |
| **Rust** | does not compile |
| **WebAssembly** | refuses the module: `i32.mul` on `f64` operands |

With the annotation, all four agree — `tests/parity/float_closure.nova` checks that on every push.

**Nothing checks the call site either.** `sema::check_call_arguments` returns early unless the callee
is an `Identifier`, so a closure call — whose callee is an expression — has its arguments unchecked.
That is why the C backend can truncate without a diagnostic: no pass compares the argument's type
with the parameter's.

## Options

### A. Infer the parameter type from its use in the body

If a parameter appears as an operand of an arithmetic expression whose other operand is a float, it is
a float. A small constraint pass over the closure body, solved before the body is lowered.

**Cost:** a real inference step, with the usual questions — what if the uses disagree, what if a
parameter is unused, how does it interact with generics.
**Benefit:** `|x| x * 2.5` means what it looks like, and the annotation becomes optional.

### B. Require the annotation

Refuse an unannotated closure parameter, naming it.

**Cost:** more typing at every closure, including the common `|x| x + 1`.
**Benefit:** one rule, no inference, and nothing silent. It also breaks every existing closure in
`tests/parity/closures.nova` and the standard library, so it is a breaking change to the language.

### C. Check the call site and refuse a mismatch

Leave the default as `int`, and make a float argument to an `int` parameter an error — the same
treatment a named function gets.

**Cost:** needs the closure's signature at the call site, which means sema must infer closure types
anyway, or the check moves into the lowerer, which has no diagnostics.
**Benefit:** the silent truncation becomes a refusal, which is this project's rule. `|x| x * 2.5`
still does not *work*, but it stops producing four answers.

### D. Leave it, and keep it documented

`COMPATIBILITY.md` already shows the four answers.

**Cost:** the trap stays. **Benefit:** nothing breaks, and nobody is misled who reads the table.

## Correction — C was attempted alone, and it does not work

An attempt to do **C** by itself was made and closed unmerged (PR #233). It added the call-site check,
and the check never fired: `check_call_arguments` looks the callee's type up, and sema never records
one for a closure.

```rust
nova_common::Expr::Closure(params, body) => {
    for (name, _typ) in params {          // the type is discarded
        let symbol = Symbol::new(name.clone(), SymbolKind::Parameter, Span::default());
        let _ = self.symbol_table.insert(symbol);
    }
```

`infer_expr_type` has no `Closure` arm either, so `let twice = |x: float| ...` leaves `twice` with no
type at all.

**The estimate below was right and the plan was wrong.** C is not a smaller first step; it is the same
work with nothing to check against. A guard that compiles, passes narrow tests and silently never fires
is the false-success shape this project keeps removing, so the PR was closed rather than merged.

The single piece of work is: sema learns `Type::Function` for a closure — parameter types from the
annotations, return type from the body. Then C is a dozen lines and A falls out of the same pass.

## Recommendation

**C, then A.**

C is the smallest change that removes the *silent* part: a program that means four different things
should not compile to any of them. It needs a home for the check, and the honest home is sema — which
means sema needs the closure's signature, and that is most of A's work already.

So the order is: give sema closure signatures (which it needs to check any call through a value),
refuse the mismatch, and then infer the parameter type from the body so the refusal rarely fires.

Doing A first would hide the call-site hole: an inferred parameter would agree with its body, and the
same silence would remain for a closure stored in a variable and called with the wrong type.

## What this ADR does not decide

- Whether a closure parameter may be inferred from a *contextual* type — `let f: fn(float) -> float = |x| x * 2.0;`. The lowerer already consults `contextual` when it has one; this ADR does not extend it.
- Whether numeric literals should be polymorphic (`2.0` as an `int` where an `int` is wanted). They are not, and that is a separate question with a much wider blast radius.

## Consequences of doing nothing

The four answers stay, `COMPATIBILITY.md` keeps showing them, and every reader of a closure with a
float in it has to know to annotate. That is a documented trap, not an unknown — which is better than
it was this morning, and not the same as fixed.
