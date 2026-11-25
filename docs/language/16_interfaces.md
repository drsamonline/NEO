# Interfaces

NOVA has **no interfaces and no traits**. There is no `trait` or `interface` keyword — neither
appears in the grammar or in the lexer's keyword list — and there is no way to declare that a type
satisfies a set of methods.

What the language has is `impl`: a block of **inherent methods** attached to one struct or enum.
That is all it is.

## `impl` attaches methods to a type

```nova
struct P {
    x: int,
}

impl P {
    fn twice(self) -> int {
        self.x * 2
    }
}

fn main() -> int {
    let p = P { x: 21 };
    print(p.twice());   // 42
    return 0;
}
```

Verified: that program compiles and prints `42`. The method is called with `p.twice()`, and the
standard library uses the same form once — `impl Range` in `core.nova`.

## What `impl` cannot do

**No `impl Type for OtherType`.** The `for` is a parse error:

```nova
impl Show for P {
    fn show(self) -> int { self.x }
}
```

```text
error: parse errors:
  prog.nova:3:11: Expected '{' after impl type at 'for'
```

So there is no trait to implement, no way to state that two types share an interface, and no
dispatch through one. A method exists on the type it was written for, and nowhere else.

**No bounds on generics.** Generics exist — see [Generics](15_generics.md) — and they are
*instantiated*: a call with a `[int]` produces a `[int]` version of the function. There is no way to
write "`T` must have a `show` method", because there is no method set to require.

## What a program does instead

- **A function per type.** `fn show_p(p: P) -> String` and `fn show_q(q: Q) -> String` — no shared
  name, no shared contract.
- **A tagged enum.** One `enum Shape { Circle(float), Square(float) }` and one `area` function that
  matches on it. This is the idiomatic answer in a language without traits: the set of cases is
  closed and the compiler checks the match.
- **A function-typed value.** `fn(F) -> R` types exist, so a caller can be handed a function to
  call; the closure is the interface, without a name for it.

## If this changes

An interface is the largest of the missing features to add, because it touches the type checker
(does this type satisfy this set?), the lowerer (which method does `x.show()` call when `x`'s type
is not known until instantiation?) and the backend (a vtable, or monomorphisation). It is also the
feature whose absence shapes the standard library most: 249 functions, none of which can be called
through an interface.

Until that is decided and recorded in `docs/adr/`, this chapter is the specification: **inherent
methods, no traits, no dispatch**.
