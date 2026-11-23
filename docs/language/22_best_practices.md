# Best practices

Advice that follows from what this language actually does. Every rule here exists because the
alternative fails in a way the compiler will not warn you about — which is why it belongs in a
book rather than in a style guide.

## Handle an `Option`, never assume it

```nova
match parse_int(text) {
    Some(value) => print(value),
    None => print("not a number"),
}
```

`Option<T>` is how the standard library reports "no value". `parse_int`, `parse_float`,
`parse_bool`, `stat`, `array_get` and `first_char` all answer it. Reaching for the value without
matching means reaching for a trap.

## Compare strings with `==`

```nova
let a = "ab";
let b = strcat("a", "b");
print(a == b);   // true
```

`==` on two `String`s compares their contents, not their addresses. Verified, because this is
exactly the kind of thing a compiled language gets wrong once and then forever.

## Do not rely on `&mut` for safety

`&mut` is documentation. The parser discards the difference between `&` and `&mut` before
anything could check it, so two mutable references to one value are not detected — and a
reference held across a reallocation is a dangling pointer with no diagnostic.

Treat `&mut` as a note to the reader, and treat aliasing as your problem.

## Return from every path of a non-unit function

```nova
fn bad(x: int) -> int {
    if x > 0 { return 1; }
}                       // compiles; returns garbage when x <= 0
```

**This is not caught.** The analyser has a `check_missing_return`, but it does not fire for this
shape, and the C compiler accepts it. Write the final expression or a `return` on every path.

## Do not write a generic whose only use of `T` is its return type

```nova
fn make<T>() -> T { ... }

let x: String = make();   // T binds to int; the emitted C is `char* x = make_int();`
```

Inference reads arguments, not results. Pass a witness instead:

```nova
fn make<T>(witness: T) -> T { ... }   // T inferred from the argument
```

## Mind the leak

Every string, array and closure environment is allocated and never freed — see
[Memory](11_memory.md). Fine for a program that exits; a leak for one that loops for days.

If a long-running loop builds strings, it grows without bound. There is no `free` to reach for,
so the fix is to allocate less: build once, reuse a buffer, or bound the loop.

## Annotate an empty array when nothing else gives it a type

```nova
let empty: [String] = [];      // explicit
format("x", []);               // fine — the parameter supplies the type
let mystery = [];              // an `int` array, because there was nothing else to ask
```

An empty literal takes its element type from a parameter when it is passed to one. With no
parameter and no annotation, it is `[int]`.

## Keep `main` simple and `int`-returning

```nova
fn main() -> int {
    // ...
    return 0;
}
```

`main` is the entry point the runtime looks for. The value it returns becomes the process exit
code, so a program that reports failure should return non-zero rather than print and return 0.

## Prefer `match` over sentinels

`-1` from a search, `0` from a parse and `""` from a lookup are all ambiguous: `-1` is a valid
index to nobody but a valid number to everybody. The standard library answers `Option<T>` where
absence is possible; use it, and answer `Option<T>` in your own functions.

## Run the gates

```bash
python3 tools/docs-check.py && python3 tools/stdlib-check.py && \
python3 tools/grammar-check.py && python3 tools/example-check.py && \
python3 tools/type-name-check.py && python3 tools/extension-check.py && \
python3 tools/diagnostic-check.py && python3 tools/error-code-check.py && \
python3 tools/status.py --check
```

None of them needs a C toolchain, so they run anywhere. They catch documentation that has
drifted from the code, which is the failure this project has spent the most time on.

## What is not advice but a warning

Three things in this book are gaps rather than rules — falling off the end of a function,
`&mut` exclusivity, and the return-type inference limit. They are listed here because a reader
meets them as surprises. Each has a chapter that says more.
