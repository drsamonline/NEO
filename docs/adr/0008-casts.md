# ADR 0008 — `as` converts; it is not a comment

- **Status:** accepted
- **Date:** 2026-11-22
- **Related:** [0006](0006-array-element-types.md), [0007](0007-string-indexing.md)

## Context

`x as T` parses to `Expr::Cast`, and the lowerer throws the cast away:

```rust
Expr::Cast(inner, _type_expr) => {
    // For now, just pass through the inner expression
    self.lower_expr(inner)
}
```

There is no `Cast` arm in `expr_type` either, so a cast expression reports **no type at
all**. Meanwhile `SirExpr::Cast { value, typ }` exists in the SIR and the C backend already
emits it correctly:

```rust
SirExpr::Cast { value, typ } => format!("(({})({}))", self.c_type(typ), self.expr_to_c(value))
```

So the machinery is built and nothing ever feeds it.

**It went unnoticed because C converts implicitly.** Dropping `as float` from a function
argument still produces correct C — the prototype converts it. The cast only matters where
the *NOVA* type is consulted rather than the C one, and there are exactly three such
places: generic instantiation, `tostring`, and the choice of `print` function.

**Measured, not inferred.** `tostring((c as int - 32) as char)` in `stdlib/text`'s
`to_upper` produces `"69"` instead of `"E"`. The inner expression is an `int` with the cast
erased, so `tostring` instantiates `nova_tostring_int`. [ADR 0007](0007-string-indexing.md)
fixed the `char` arm of `tostring`; it could not help, because nothing ever became a `char`.

The blast radius is known and small: **26 casts in the standard library and examples, every
one to `int`, `float` or `char`.** No cast to a struct, an array, an `Option` or a tuple
exists anywhere in the repository.

## Options

**(a) Keep dropping them.** Cheapest, and the status quo compiles. But a cast that does
nothing is a lie in the surface language: `x as char` says "treat this as a character" and
does not. It also makes three features silently wrong rather than loudly unsupported, which
is the failure the constitution's rule 3 exists to prevent.

**(b) Lower to `SirExpr::Cast` and type the expression as the target.** The machinery is
already there and correct. An explicit C cast performs the same conversion C would have
performed implicitly, so code that relied on the no-op keeps its behaviour — the difference
is that the NOVA type is now the one the source asked for.

**(c) (b), but only for types a C cast can express**, and reject the rest. `((NovaArray)(x))`
is not valid C, and `c_type` falls back to `int` for anything it does not recognise — so an
unrecognised target would silently become an `int` cast, which is worse than either
compiling or failing.

## Decision

**(b) guarded by (c).**

A cast to `int`, `float`, `char` or `bool` lowers to `SirExpr::Cast`, wrapped in a `Typed`
carrying the target type, and `expr_type` reports that target. A cast to anything else
lowers to `nova_unsupported("cast to T")`, which names the construct and exits — the same
treatment `s[i] = c` got in ADR 0007, and for the same reason: the compiler should not emit
C whose meaning nobody decided.

The whitelist is not a judgement about the language. It is the set of targets that exist in
this repository today, and adding a target means deciding what its C cast means first —
which for a struct or an array is not obvious, because neither is a scalar.

**What this costs:** every `as` now emits a parenthesised C cast, so the generated C gets
noisier. Nothing else changes, because C's explicit cast is the same conversion it was
performing implicitly.

## What this ADR does not decide

- **Whether a lossy cast should warn.** `3.7 as int` is 3, silently. A warning would be
  reasonable and is a separate decision.
- **Whether `as` should be the only conversion.** There is an implicit `int` → `float`
  coercion in the analyser (`can_coerce`); this ADR does not touch it.
- **The two `expected expression` errors in `from_hex`.** Measured: **independent.** They
  survive this change unchanged, and the C names the cause — `? (...) : )` with an empty
  else, so an `if`/`else if` chain used as a value loses its final `else`. They belong to
  the same defect as `format_number`'s `int digits = ...`, not to this one.

## Where this is measured

`tostring((c as int - 32) as char)` printing `"69"` before and `"E"` after is a run. The
26-cast inventory is
`grep -rn " as " stdlib examples tests/parity`, every target a scalar. The C emission path
is `SirExpr::Cast` in `compiler/src/backend/mod.rs`, which existed before this ADR and was
never reached.
