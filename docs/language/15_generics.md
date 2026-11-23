# Generics

A generic function is one definition used at several types. NOVA has **no constraints and no
trait bounds**: a generic parameter is a placeholder for a type and nothing more, so a body
can only do to a `T` what every type supports — hold it, pass it, return it.

The grammar:

```
generic_params = "<" identifier { "," identifier } ">" ;
generic_call   = identifier "<" type_expr { "," type_expr } ">" "(" [ arguments ] ")" ;
```

## Declaring

```nova
fn identity<T>(value: T) -> T {
    value
}

fn pair<A, B>(first: A, second: B) -> (A, B) {
    (first, second)
}
```

The standard library uses this for its own surface — `print<T>`, `tostring<T>`,
`array_pop<T>`, `compose<A, B, C>`.

## How it is implemented

**Monomorphisation.** Each concrete use produces its own C function, named from the base name
and the mangled type arguments — `array_pop_int`, `compose_int_int_int`. There is no runtime
dispatch and no boxing: a generic function at `int` is a function that takes an `int`.

Two consequences worth knowing:

- **Code size grows with the number of instantiations**, not with the number of generic
  definitions. A generic used at ten types is ten functions.
- **Two instantiations are two functions.** `identity<int>` and `identity<String>` share no
  code at run time.

## Inference

Type arguments are inferred from the arguments at the call:

```nova
fn first_of<T>(items: [T]) -> T { ... }

let a = first_of([1, 2, 3]);        // T is int
let b = first_of(["x", "y"]);       // T is String
```

Explicit arguments are also accepted: `first_of<int>(items)`.

### The limitation: a parameter that appears only in the return type

**Inference reads the arguments. It cannot read the result.** When a type parameter appears
only in the return type, there is nothing at the call site to infer it from, and it falls back
to `int`:

```nova
fn make<T>() -> T { ... }

let x: String = make();   // T is inferred as int, not String
```

The failure is visible in the emitted C rather than at the NOVA level — the program is
accepted, and the C compiler then refuses it:

```c
char* x = make_int();
```

This is not a hypothetical: it is why `io.deserialize<T>(s: String) -> Option<T>` does not
work, and why that function is the one blocked entry in the standard library. The declaration
receiving the result knows the type — `let x: Option<float> = ...` — but the inference does
not consult it.

**Until this is fixed, do not write a generic whose only use of a type parameter is its return
type.** Pass the type through an argument instead:

```nova
fn make<T>(witness: T) -> T { ... }   // T is inferred from `witness`
```

## What does not exist

- **No bounds.** `fn f<T: Ord>(...)` does not parse. There is no way to say "T must support
  `<`", because there is no trait or interface system to say it with — see
  [16_interfaces.md](16_interfaces.md), which is unwritten for that reason.
- **No specialization.** Two definitions of one generic name for different types do not
  coexist.
- **No generic structs in practice.** The compiler carries the machinery for `struct Name<T>`
  and `Option<T>` is built in, but **no program in this repository declares a generic struct
  of its own**, so that path is unverified. Treat it as unsupported until a test says
  otherwise.

## `Option<T>`

`Option<T>` is the built-in generic: `Some(value)` and `None`, read with `match`. Its payload
is monomorphised like any other instantiation, so `Option<int>` and `Option<String>` are
distinct C structs.

`None` has no type of its own and takes its payload from context — the binding's declared type,
or the enclosing function's return type:

```nova
let empty: Option<File> = None;   // Option<File>, not Option<int>
```

Without an annotation in reach, `None` is `Option<int>`.

## Why this chapter says what it cannot do

The rest of this book describes what the compiler accepts. This section describes a limit that
a reader would otherwise discover by writing code that compiles and then behaves wrongly, which
is the failure mode the project's constitution names first. A documented gap is a smaller
problem than a silent one.
