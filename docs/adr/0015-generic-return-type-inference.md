# ADR 0015 — a generic that appears only in its return type takes `T` from the binding

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0014](0014-serialize-is-json.md), [the Generics chapter](../language/15_generics.md)

## Context

`io.deserialize<T>(s: String) -> Option<T>` is the last function in the standard library that
cannot run. `T` appears **only in the return type**, so there is no argument to infer it from,
and the inference fell straight through to `int`:

```nova
let f: Option<float> = deserialize("1.5");
```

emitted

```c
Option_float f = deserialize_int("1.5");
```

Every call instantiated the same function, whatever the binding said.

## Decision

When a generic has no binding from the arguments, `resolve_generic_call` now consults
`expected_type` — the declared type of the binding receiving the result — and takes the payload
of an `Option<...>`. That is the only place `T` is written down.

The fallback to `int` remains for the case where neither source knows, because a program with no
annotation has nothing better to offer.

## Consequences

- `deserialize` now instantiates per type: the example above emits `deserialize_float`.
- **The feature is still not implemented**, and this ADR does not claim it is. See below.

## What still blocks `deserialize`

Three things had to be true, and two now are:

| Requirement | Status |
|---|---|
| `T` reaches the instantiation | **fixed** — this ADR |
| the runtime can parse JSON into a typed slot | written, works standalone |
| the lowerer can build `Option<T>` in the body | **blocked** |

The third is the hard one. `Option_int` and friends are structs the backend emits per program,
so the runtime cannot return one; the lowerer must build it from a `let`, an `if` and a
`StructLiteral`. That part works — it was written and produced correct C — but it needs to know
`T` **inside the instantiated body**, and it does not:

`lower_function` sets `expected_type` to the substituted return type, and `lower_instance` —
which lowers monomorphised bodies — does not. The body of `deserialize_float` is lowered by
`lower_instance`, so `expected_type` is empty there and the dispatch sees no payload:
`deserialize<>`.

The fix was small and structural, and it is made: **`lower_instance` now sets `expected_type`
the way `lower_function` does**, from `instance.function.return_type` with the instance's
substitution applied. `deserialize` works for `int`, `float`, `bool` and `String`, and
`io.serialize` and `io.deserialize` round-trip.

Arrays are still refused, deliberately: an `Option<[T]>` payload is lost between the binding and
the `match` that reads it, and `option_struct_name` mangles `[int]` to the same `Option_int` as
`int`. Both are recorded in the [Generics chapter](../language/15_generics.md) as gaps rather
than papered over.

## Why the inference fix is worth landing alone

It is not dead code: it corrects the instantiation of **any** generic whose type parameter
appears only in its return type, and it is verified — `deserialize_float` is emitted where
`deserialize_int` was. It changes no existing behaviour, because no existing generic in the
standard library has that shape.
