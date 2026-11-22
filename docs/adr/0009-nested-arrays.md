# ADR 0009 — `[[int]]` is an array of `NovaArray` structs, and that is all it is

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0006](0006-array-element-types.md), [0007](0007-string-indexing.md)

## Context

[ADR 0006](0006-array-element-types.md) gave arrays a header per element type: `NovaArray`
for `int`, `NovaArray_float` for `double`, `NovaArray_string` for `char *`. All three hold
a pointer, a length and a capacity — 24 bytes of element storage each, for elements of 4,
8 and 8 bytes.

`stdlib/text`'s `levenshtein` needs a fourth shape, and this one is not about element size:

```nova
let mut matrix: [[int]] = [];
array_push(matrix, row);      // row is a `[int]`
matrix[i][j] = cost;          // read one row, write into it
```

The element of a `[[int]]` is a `[int]` — a whole `NovaArray`, not a scalar. So the
element type is not "some other 8 bytes", it is 24, and it has structure.

This was the last thing between `text` and the stdlib compile gate: 7 of its 12 remaining
errors, and after ADR 0007 and ADR 0008 the only one.

## Options

**(a) An array of `NovaArray` by value.** `NovaArray_array` holds `NovaArray *data`. A push
copies the struct. The copy shares its `int *data` buffer with the original, so a write
through the copy's pointer lands in the original's storage — which is exactly what
`matrix[i][j] = v` needs, without any pointer chasing in the generated code.

**(b) An array of pointers to `NovaArray`.** `NovaArray **data`. Pushing needs the address
of a `NovaArray` that outlives the call, so a push of a temporary would dangle. It would
have to allocate a copy per push, and the ownership question ADR 0006 left open gets worse
rather than better.

**(c) Reject nested arrays.** Honest, and it keeps the compiler from guessing. But it
leaves `text` uncompilable for a construct the language already parses and type-checks, and
the rejection would have to explain why `[[int]]` is fine to write and impossible to run.

## Decision

**(a), for `[int]` inner arrays only.**

The sharing is the argument. A `NovaArray` is three words, two of which are metadata; the
data lives behind a pointer, and copying the struct copies the pointer. So `matrix[i]`
returns a value that is *not* the slot in the matrix but writes into the same buffer, and
`matrix[i][j] = v` does what it looks like it does. Option (b) would need an allocation per
element to get the same effect, and option (c) would refuse something the standard library
genuinely needs.

**The restriction is real and is stated rather than implied:** only `[int]` inner arrays
are represented. `[[float]]` and `[[String]]` have no header, because their inner arrays
are different structs (`NovaArray_float`, `NovaArray_string`) and `NovaArray_array` holds
`NovaArray`. Nothing in the standard library or the examples declares one.

**The gap that leaves:** a `[[float]]` is not rejected with a diagnostic. It falls back to
the `int` header and fails in the C compiler with an `incompatible type` error — loud, but
not the diagnostic [ADR 0006](0006-array-element-types.md) option (d) would have given.
That is the same gap that ADR closed for scalars and this one does not close for nesting,
and it is recorded here rather than discovered later.

## A defect this uncovered, and a second one it did not

Reaching `text`'s `levenshtein` exposed two things that had nothing to do with arrays:

- **A `while` body whose only item is an expression was dropped entirely.** A block whose
  sole content is an expression parses it as `final_expr`, and the loop lowering read only
  `statements` — so `while c { if ... }` emitted `while (c) { }`. An infinite loop, in a
  module that compiled and passed every gate. Fixed in `lower_loop_body`.
- **`==` on two `String`s compared their pointers.** Two equal strings need not share an
  address, so `substr(s, i, 1) == ","` was always false and `split` never found a delimiter.
  Now `strcmp(a, b) <op> 0`. That is a comparison fix, not a representation decision, and it
  needed no ADR of its own — but it is the reason `text` compiled while being wrong, which
  is worth remembering next time a module joins the compile gate.

## What this ADR does not decide

- **Deeper nesting.** `[[[int]]]` would be an array of `NovaArray_array`, which nothing has
  asked for and which would want its own record.
- **Whether the inner array is owned or borrowed.** A `matrix[i]` shares storage with the
  matrix; a `push` of a local `row` shares storage with that local, which then goes out of
  scope. The runtime never frees arrays, so nothing dangles today — that is luck, not a
  decision, and it is the same ownership question ADR 0006 left open.

## Where this is measured

`nova-compiler emit stdlib/text/text.nova` and a C compiler: 7 errors before, 0 after, and
the whole module joins `core`, `io` and `math` in compiling. `levenshtein("kitten",
"sitting")` returning 3 and `split("a,b,c", ",")` returning three parts are runs, not
readings — and the second of them is what proved the string-comparison defect.
