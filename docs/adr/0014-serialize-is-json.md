# ADR 0014 — `serialize` writes JSON, and only for types it can write honestly

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0006](0006-array-representation.md), [0013](0013-streams-are-file-handles.md)

## Context

`stdlib/io` declares:

```nova
/// Serialize value to JSON-like string
fn serialize<T>(value: T) -> String

/// Deserialize string to value
fn deserialize<T>(s: String) -> Option<T>
```

Both called builtins nothing implemented, and they were the last two blocked functions in the
library.

The format was an open question — a length-prefixed binary framing would round-trip NOVA
values exactly, and JSON would interoperate. The module had already answered it in its own
doc comment: **"JSON-like"**. Implementing anything else would have meant the code and its
documentation disagreeing, which is the failure `docs/specs/constitution.md` rule 1 exists to
prevent.

The harder question was the generic `T`. A C runtime cannot be generic, and NOVA has no
tagged union to pass a value through.

## Options

**1. A tagged union at runtime.** Give the runtime a `NovaValue { tag, payload }` and
serialize that.

Rejected: it is a second type system, and every call site would have to build the union —
which means the compiler already knows the type and is throwing the knowledge away.

**2. Type-directed codegen.** The call site knows `T`, so it picks the function.

Accepted.

**3. Serialize every type by reflection over its fields.** Structs emit as objects.

Accepted, and implemented after the rest: the encoder is generated in SIR, because only the
lowerer knows a struct's field names and types. `Point { x: 1, y: 2 }` becomes
`{"x":1,"y":2}`, and a nested record keeps its nesting.

## Decision

`__builtin_serialize` is dispatched in the lowerer on the argument's type, one runtime
function per type:

| NOVA type | Runtime function | Output |
|---|---|---|
| `int` | `nova_json_int` | `42` |
| `float` | `nova_json_float` | `1.5` |
| `bool` | `nova_json_bool` | `true` |
| `String` | `nova_json_string` | `"hi"`, quoted and escaped |
| `[int]` | `nova_json_array_int` | `[1,2,3]` |
| `[float]` | `nova_json_array_float` | `[1.5,2.5]` |
| `[String]` | `nova_json_array_string` | `["x","y"]` |

**A type with no function is rejected.** An enum, an `Option`, or an array of structs compiles
and traps at the call with `serialize<Point>` — naming the type it could not write. It does not
serialise as its first field, as its pointer, or as `null`. Records used to be in that list and
are not any more.
Rule 3 of the constitution is that a construct the compiler cannot express gets a diagnostic
rather than C that misbehaves, and a struct silently becoming `{}` or `0` is exactly that.

**The type match is on the lowercased form.** The same type reaches the lowerer as `string`
from a literal and `String` from a declaration; matching one spelling would have made
`serialize("hi")` a trap while `serialize(some_string_variable)` worked.

## Consequences

- `serialize` covers scalars, arrays and records. It does **not** cover enums, `Option`, or an
  array of records — those still trap, naming the type.
- Floats are written with `%.17g`, which round-trips a `double` exactly. NaN and infinity are
  written as `null`, because JSON has no tokens for them and a parser that accepts `nan` is
  not parsing JSON.
- Strings pass bytes above `0x1F` through unchanged, which is correct for UTF-8 and wrong for
  nothing this library can produce.
- `deserialize` is **not implemented**. It needs the same type-directed dispatch plus a
  parser, and a parser written in a hurry is worse than an absent one.
