# Functions

A function is the only unit of behaviour. There are no lambdas at the syntax level
beyond closures, no methods outside `impl`, and no top-level statements — a bare
expression at the top of a file becomes a synthetic `fn __main()`.

The grammar:

```
function       = "fn" identifier [ generic_params ] "(" [ param { "," param } ] ")"
                 [ "->" type_expr ] ( block | ";" ) ;
generic_params = "<" identifier { "," identifier } ">" ;
param          = [ "&" ] [ "mut" ] identifier [ ":" type_expr ] ;
```

## Declaring

```nova
fn add(a: int, b: int) -> int {
    return a + b;
}
```

- **The return type is optional.** Without `-> T` the function returns `unit`, and a
  `return` inside it must be bare. `return 0;` in a `unit` function is `E0308`.
- **A body may be `;` instead of a block.** `fn f(x: int) -> int;` declares without
  defining — the form the standard library uses to introduce a builtin.
- **A block's final expression is its value** when it has no semicolon. So `add` above
  can also be written `fn add(a: int, b: int) -> int { a + b }`, and mixing the two is
  an `E0308` if the final expression's type disagrees with the declared one.
- **`pub`** may prefix a function; it is accepted and does not yet change visibility.

## Parameters

| Form | Meaning | Status |
|---|---|---|
| `x: int` | by value | works |
| `arr: &mut [T]` | a reference to an array header, so a push inside is visible outside | works |
| `file: &mut File` | a reference to a struct — emitted as `File*` | works |
| `x` with no type | inferred | accepted, typed `int` |
| `self` in an `impl` block | the receiver, emitted as a pointer | works |

## Generics

```nova
fn id<T>(value: T) -> T {
    return value;
}
```

Generics are **monomorphised from the call site**: `id(3)` produces the `int`
instantiation and `id("x")` the `string` one. A type parameter with no concrete call
site falls back to `int`, which is a documented limitation rather than a design.

`Option<T>` returned from a generic function is checked at the instantiation, not at
the declaration — see [types](06_types.md).

## Recursion and the entry point

`main` is the entry point and is emitted as C `main`. It may return `int` or `unit`.
Recursion is unrestricted; there is no tail-call guarantee.

## Calling

```nova
let total = add(1, 2);
```

Arguments are checked for count (`E0061`) and type (`E0308`). `int` → `float` is the
only implicit conversion.

**A function whose name belongs to the C library is emitted with a `nova_fn_` prefix.**
Without that, a NOVA `fn floor(x: float) -> int` becomes a definition of libm's `floor`,
and the runtime's call to `floor` recurses into it until the stack runs out.

## What is missing

- **No default arguments, no overloading, no named arguments.**
- **No `async`, no `await`.** The runtime crate has futures; the language does not.
- **No visibility enforcement.** `pub` is parsed and ignored.
- **No `impl` for generic types beyond monomorphisation from concrete call sites.**

## Where this is tested

`compiler/tests/end_to_end.rs` compiles, links and runs programs that use every form
above; `tests/diagnostics/` covers `E0061` and the four `E0308` shapes.
