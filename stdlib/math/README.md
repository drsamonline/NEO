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

`nova-compiler build` links `-lm` for these; `emit` does not need to.## ⚠️ The float-array functions are wrong

`sum_float`, `mean`, `variance`, `stddev` and `median` take `[float]`, and `NovaArray`
holds `int` elements. A `[float]` literal is truncated before anything reads it:

```nova
print(sum_float([1.5, 2.5]));   // prints 3, not 4
```

The module compiles and the compile gate passes, which is why this is written here rather
than discovered by a user. See [ADR 0006](../../docs/adr/0006-array-element-types.md);
`a_float_array_sum_is_not_truncated` in `compiler/tests/end_to_end.rs` is the runnable
proof.

The scalar functions — `sqrt`, `floor`, `round`, `powf` and the rest — are correct and
tested end to end.


