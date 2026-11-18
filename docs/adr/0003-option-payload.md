# ADR 0003 — `Option<T>` holds a primitive, not a struct

- **Status:** accepted, with a follow-up
- **Date:** 2026-11-18
- **Related:** [0001](0001-lossless-syntax-tree.md), [0002](0002-traits.md)

## Context

`stdlib/io` declares `fn fopen(path: String, mode: String) -> Option<File>`. The module
has never compiled, and the reason is not in the module:

```c
typedef struct Option { int tag; int payload_i; double payload_f; char* payload_s; } Option;
...
return (Option){ .tag = 0, .payload_i = (File){ .path = path, .mode = mode, .handle = handle } };
```

`Option` is one C struct with **three fixed payload slots** — an `int`, a `double` and a
`char*`. There is no slot for a struct, so `Some(File { ... })` is assigned to
`payload_i` and C says `incompatible types when initializing type 'int' using type
'File'`.

`Option<String>` works because `payload_s` exists. `Option<int>` and `Option<float>`
work for the same reason. `Option<File>` cannot be expressed at all.

## Decision

**`Option<T>` is a primitive-only type today, and that is now a documented limit rather
than an accident.** The three slots stay as they are.

Two things follow, and only the first is in scope now:

1. **Sema rejects `Option<Struct>` with a diagnostic.** The compiler knows the payload
   cannot be represented, so it should say so at the source line rather than emit C that
   a C compiler then refuses. This is the same rule as `backends/wasm`'s `validate`:
   reject what you cannot express, in your own words, before someone else's tool
   produces a worse error.
2. **The stdlib stops promising it.** `fopen`, `stat` and `deserialize` return
   `Option<File>` and `Option<FileMetadata>` today. Those signatures are wrong until the
   representation changes.

## Why not add a `void*` slot

Because `Some(File { ... })` would have to store the **address** of a compound literal,
and that literal's lifetime is the enclosing block. A function returning
`Option<File>` would hand back a pointer to its own stack frame. Adding a slot turns a
compile error into a use-after-free, which is worse than either alternative.

## The real fix, when it is wanted

Monomorphise: emit `Option_int`, `Option_string`, `Option_File` — one struct per payload
type, sized correctly, copied by value. The lowerer already monomorphises generic
structs (`Box<int>` becomes `Box_int`), so the machinery exists; what does not exist is
a reason to spend it on `Option` while the stdlib's own `array_pop<T>` and
`array_get<T>` also return `Option<T>` with a `T` that is usually `int`.

## Consequences

- `stdlib/io` and `stdlib/text` stay in `stdlib/not-compiling.txt` with the real reason
  named, instead of a guess.
- A program that writes `Option<File>` gets a NOVA diagnostic with a line and a column
  instead of a C error with a column in generated code.
- The stdlib's fallible constructors are marked as aspirational in
  `PACKAGE_ARCHITECTURE.md`'s spirit: what exists is what runs.

## Follow-up

✅ **Sema rejects `Option<Struct>` as `E0400`.** The compiler no longer emits C it knows
cannot be compiled; a program writing `Option<File>` gets a line, a column and a
sentence in its own language. `tests/diagnostics/E0400_option_of_struct.nova` pins it.

⬜ **Monomorphisation** — and the cost is not where it looks.

The struct half is small. `Option<T>`'s payload mechanism is two functions in
`compiler/src/lowerer/mod.rs` — `payload_struct_fields()` and `payload_field(typ)` —
and the C backend has **no** references to the payload field names at all. Emitting
`Option_File { int tag; File payload; }` instead of one three-slot struct was prototyped
in C and behaves: a struct payload held by value, returned by value, matched, and copied
correctly on assignment.

**The real cost is that `None` has no type.** It lowers to

    SirExpr::Typed { typ: "Option<int>", ... }

unconditionally — the type is hard-coded. So `let f: Option<File> = None;` is already
wrong today, and no amount of struct monomorphisation fixes it: `None` needs to be typed
by its context, which means threading an expected type through expression lowering.
That is a bidirectional-inference change, not a payload change, and it is the reason this
is an ADR-sized job rather than an afternoon's edit.

Sequence when it is done: `None` becomes contextually typed, then the payload struct is
monomorphised, then `E0400` is removed. Doing them in the other order leaves `None`
lying about its type.
