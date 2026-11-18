# `math`

76 functions: trigonometry (`sin`, `cos`, `tan`, `asin`), angle conversion
(`radians`, `degrees`), rounding, powers and roots, and the float helpers `abs`,
`min`, `max`, `clamp`.

`float` is a full type but the WASM backend cannot yet hold one in a struct field or
a closure signature, so a `math` program is C-only in practice.

**48 of its 76 functions can run**; the other 28 call a builtin that does not exist
(`__builtin_sin`, `__builtin_sqrt`, `__builtin_powf` and the rest). Numbers in
[`../README.md`](../README.md).
