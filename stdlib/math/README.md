# `math`

76 functions: trigonometry (`sin`, `cos`, `tan`, `asin`), angle conversion
(`radians`, `degrees`), rounding, powers and roots, and the float helpers `abs`,
`min`, `max`, `clamp`.

`float` is a full type but the WASM backend cannot yet hold one in a struct field or
a closure signature, so a `math` program is C-only in practice.

**75 of its 76 functions can run.** The floating-point builtins landed —
`__builtin_sqrt`, `__builtin_sin`, `__builtin_powf` and twenty-six more — so this is
now the most complete module in the library. Numbers in
[`../README.md`](../README.md).

`nova-compiler build` links `-lm` for these; `emit` does not need to.## The float-array functions are correct

`sum_float`, `mean`, `variance`, `stddev` and `median` take `[float]`, and a `[float]`
array is now a real array of `double` rather than an `int` array with the fractional part
thrown away:

```nova
print(sum_float([1.5, 2.5]));   // prints 4
```

This was wrong — it printed `3` — and the compile gate could not see it, because the
emitted C compiled. Two end-to-end tests now assert the numbers instead of the C:
`a_float_array_sum_is_not_truncated` and `float_array_statistics_are_not_truncated` in
`compiler/tests/end_to_end.rs`. See
[ADR 0006](../../docs/adr/0006-array-element-types.md).

The scalar functions — `sqrt`, `floor`, `round`, `powf` and the rest — are correct and
tested end to end.


