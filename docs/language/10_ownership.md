# Ownership

NOVA has **no ownership model**. There is no borrow checker, no lifetimes, no destructors and no
`free`. A program allocates and never releases, and the process exit is the only cleanup.

What it does have is one rule about **values**, and this chapter is mostly about how narrow that
rule is — because a reader arriving from Rust will assume far more than exists.

## Two kinds of value

Every type is either **`Copy`** or not. Assigning a `Copy` value duplicates it; assigning anything
else gives the same buffer a second name.

| `Copy` | Not `Copy` |
|---|---|
| `int`, `float`, `bool`, `char`, `unit`, `!` | `String` |
| `&T`, `&mut T`, `*T` | arrays, slices |
| a tuple whose elements are all `Copy` | functions, closures, structs, enums |

The classification is `SemanticAnalyzer::is_copy`, a function over the type, and it is tested both
ways round.

## The one rule

Assigning a non-`Copy` value to another binding gives it away, and using the original afterwards is
**`E0010`**:

```nova
let greeting = "hello";
let mut other = "x";
other = greeting;
print(other);      // ok
print(greeting);   // warning [E0010]: `greeting` was assigned to another binding
```

It is a **warning**, not an error, and it is the whole of the ownership checking this language
does.

`let t = s` is the same rule: a `let` initialised from another binding is also a second name.

## Passing does not count, and that is deliberate

```nova
let a = shout(greeting);
print(greeting);   // fine — no warning
```

A NOVA `String` is a `char *`. The callee receives the same address, the caller's variable stays
valid, and nothing frees the buffer. Calling that a move would be a fiction.

This was measured rather than assumed: reporting a move on every pass produced **214** warnings in
the standard library alone, every one of them a function reusing a `String` parameter that was
still perfectly valid. The rule was narrowed to assignment on that evidence, and
[ADR 0017](../adr/0017-ownership-proposal.md) records the reversal.

## `clone` is the way out

`clone` is a compiler builtin — what it does depends on the argument's type, and the language has no
overloading:

| Argument | Result |
|---|---|
| `String` | a fresh buffer with the same bytes |
| `[int]` | a fresh buffer with the same elements |
| a struct whose fields are all scalars | a field-by-field copy |
| `[String]`, a struct holding a `String`, a closure | **a compile-time error** |

The refusals are the point. A shallow copy of an array of pointers shares exactly what the rule
exists to protect, so `clone` refuses rather than quietly sharing.

## Arrays are headers, and passing one copies the header

`NovaArray` is twelve bytes — a pointer, a length, a capacity — and it is **copied by value**. So
passing an array to a function copies the header and shares the buffer:

```nova
fn fill(xs: [int]) {
    xs[0] = 99;        // writes through the caller's buffer
}
```

Two names, one buffer, no diagnostic. This is not a bug being described; it is the language.

## `&mut` is erased

`&x` and `&mut x` both compile to a pointer. The parser consumes `mut` and drops it, so by the time
anything could check aliasing the distinction is gone. Two `&mut` references to one value are not
detected.

See [Memory](11_memory.md) for what that means in practice.

## What this rules out, and what it costs

| Because nothing is freed | Consequence |
|---|---|
| no use-after-free | the bug class does not exist |
| no double-free | likewise |
| no ownership annotations | a function signature says nothing about who keeps a value |
| memory grows monotonically | a long-running program leaks, by design |

There is no way to say "this function takes ownership" or "this value outlives that one". Those are
the things a real ownership model is for, and this language does not have them.

## Known gaps in the rule itself

Stated here rather than discovered later:

- **A struct assigned to another binding is not reported.** `let b = a` for a struct is silent,
  because the analyser does not infer a struct literal's type at the `let`. A `String` and a
  `[int]` are covered; a struct is not.
- **The rule does not look across functions.** A value returned from a function and then used is
  never reported, because there is nothing to track it against.

Both are omissions in a warning, not in safety: nothing here can corrupt memory, because nothing
frees.
