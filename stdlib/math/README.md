# `math`

76 functions: trigonometry (`sin`, `cos`, `tan`, `asin`), angle conversion
(`radians`, `degrees`), rounding, powers and roots, and the float helpers `abs`,
`min`, `max`, `clamp`.

`float` is a full type but the WASM backend cannot yet hold one in a struct field or
a closure signature, so a `math` program is C-only in practice.
