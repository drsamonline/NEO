# Error Handling

NOVA has two kinds of failure and no third. A program that cannot be compiled gets a
**diagnostic**; a program that fails while running gets whatever **the program itself decided**.
There are no exceptions, no stack unwinding and no `Result` type.

This chapter says what exists, because the interesting part is what a NOVA program can do about
an error, and the answer is narrower than in most languages.

## Compile-time failures are diagnostics

Every diagnostic carries a stable code, a span and, usually, a `help:` line:

```text
prog.nova:2:12: [E0308] function `flag` returns `bool`, but this return value is `int`
  help: return a `bool` value, or change the declared return type
```

The code is what you search for. There are three today — `E0002` (undefined name), `E0061`
(wrong number of arguments) and `E0308` (type mismatch) — and `nova-compiler explain` lists them.
[`docs/errors.md`](../errors.md) is the reference: the codes, the JSON shape `--error-format=json`
produces, and the rule that a code is never repurposed.

A program with a diagnostic does not run. That is the whole of the compile-time story: the
compiler rejects, it does not warn and continue.

## Absence is `Option<T>`, not an error code

There is no `Result`, no `try`, and nothing that propagates a failure implicitly. When a function
can fail, it says so in its return type:

```nova
fn divide(a: int, b: int) -> Option<int> {
    if b == 0 {
        return None;
    }
    Some(a / b)
}
```

The caller must decide, and the compiler will not let it forget — `Option<int>` is not an `int`:

```nova
match divide(10, 0) {
    Some(value) => print(value),
    None => print(-1),
}
```

That prints `-1`. Change the argument to `2` and it prints `5`. The standard library works this
way: `io.read_file`, `parse_int` and the other fallible functions return `Option<T>`, and the
whole library is asserted to be usable, so this is the pattern to copy.

**`Option<T>` carries no reason.** It says a thing did not happen; it cannot say why. A caller
that needs to distinguish "file missing" from "permission denied" has to return something wider
itself, and there is no standard type for that yet.

## `panic` stops the program

`core.panic` takes a message and never returns — its type is `!`, the type with no values:

```nova
use core;

fn main() -> int {
    print(1);
    panic("cannot continue");
    return 0;
}
```

That prints `1`, writes `runtime error: cannot continue` to **stderr**, and exits with status
**1**. Verified: the `return 0` is unreachable and the process status is 1, not 0.

`panic` is for a state the program cannot continue from. It is not an error-reporting mechanism:
there is no way to catch it, no unwinding, and no cleanup — the process simply stops. Anything
the program had allocated is reclaimed by the operating system, which is the only collector NOVA
has (see [Memory](11_memory.md)).

## The exit status is the program's

`main` returns `int`, and that value is the process exit status. `return 0` means success;
anything else means failure, in the sense the shell uses. This is the only channel a NOVA program
has for telling a caller that something went wrong without stopping.

## What is deliberately absent

| Absent | Why |
|---|---|
| `Result<T, E>` | a design decision not yet made: what the error payload is, and whether it is a struct, an enum or a string |
| `try` / `?` | depends on `Result` |
| exceptions, unwinding, `catch` | the runtime has no unwinder; a panic stops the process |
| error codes in `Option` | `Option` is absence, not failure-with-reason |

Each of these is a decision rather than a missing implementation, and
[`DEV_PLAN.md`](../../DEV_PLAN.md) records it as such. This chapter describes what a program can
do today; it does not promise what will arrive.

## Writing a fallible function

The pattern that works now, and that the standard library uses throughout:

1. **Return `Option<T>`** when the only useful fact is whether it worked.
2. **Return a sentinel** — a negative count, an empty string — when the caller can tell from the
   value itself, and document the sentinel where the function is declared.
3. **`panic`** when continuing would produce a wrong answer silently.

What is missing from that list is the fourth option most languages have: return a value that
carries *why* it failed. That is the gap `Result` would fill, and until it exists, a NOVA function
that must explain itself has to invent its own type.
