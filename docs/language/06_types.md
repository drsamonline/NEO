# Types

The types a program can name, and what the compiler does with them.

## The source-level names

`compiler/src/sema/mod.rs` maps a written type name to a type. That mapping is the
whole surface:

| Written | Type | Notes |
|---|---|---|
| `int`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `int` | **all of them collapse to one type** |
| `float`, `f32`, `f64` | `float` | likewise |
| `bool` | `bool` | |
| `string`, `str` | `string` | |
| `char` | `char` | |
| `unit`, `void`, `()` | `unit` | |

**The widths are accepted and ignored.** `i8` and `u64` are both `int`, so writing one
does not get you its range — it gets you a 64-bit signed value. The names are there so
source reads naturally and so the widths can be given meaning later without breaking
it.

None of these names is a keyword: they are ordinary identifiers recognised in type
position, which is why `string` can also be a variable name.

## Composites

| Form | Written as | Backed by |
|---|---|---|
| Array | `[int]`, `Array` | `NovaArray`, `int` elements only |
| Tuple | `(int, string)` | a generated `NovaTuple_*` struct |
| Struct | a declared `struct` | a C struct / Rust struct / WASM struct |
| Enum | a declared `enum` | a tagged struct; a variant carries at most one payload |
| Function | `fn(A) -> B` | a function pointer, or a lifted closure |
| Reference | `&T`, `&mut T` | lowered **only** for array parameters |
| Generic | `T` in `<A, B>` | monomorphised from concrete call sites |

## References

`&T` and `&mut T` are parsed as one type: `parse_unary` consumes the `mut` token and
drops it, so nothing downstream can tell them apart. A reference to a struct is a `T*`
in C; a reference to an array is a pointer to the array header, so a push inside the
callee is visible to the caller.

There is no borrow checker and no `&&T`. See [ADR 0005](../../docs/adr/0005-references.md).

## What is deliberately absent

- **No ownership or borrow checking.** `&T` exists for array parameters and nothing
  else; there is no lifetime syntax and no borrow checker.
- **No traits or interfaces.** `trait` is not a keyword — see
  [ADR 0002](../adr/0002-traits.md).
- **No `impl Trait`, no operator overloading, no macros.**
- **No signed/unsigned distinction at the type level.** One `int`.
- **No nullable reference.** Absence is `Option<T>`.
- **No `never`-typed expression a program can write.** `PrimitiveType::Never` exists in
  `crates/nova-common/src/types.rs` for functions that do not return; the source
  surface for it is not settled.

## Conversions

`int` → `float` is the **only** implicit conversion. Everything else needs `as`, which
the grammar allows anywhere a type name can follow an expression:

```nova
let whole: int = 1.5 as int;
```

## Where this is tested

`compiler/src/sema/mod.rs` unit tests for the checks, `tests/diagnostics/` for the
`E0308` family (a mismatched argument, return value, bare `return` and final
expression), and the parity corpus for the same program producing the same values on
all four backends.

The honest summary of what is *implemented* is `DEV_PLAN.md` → "Known limitations".
This chapter says what the type system is; that list says what the compiler currently
does with it.
