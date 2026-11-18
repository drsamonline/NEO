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

`nova-compiler build` links `-lm` for these; `emit` does not need to.
