# Compiler error codes

Every diagnostic the compiler emits carries a stable code. The code is what you
search for; the message and the `help:` line are what you act on.

There are three today. This page lists what exists, not what is planned — a
compiler that documents codes it does not emit teaches you to distrust the
documentation.

## Reading them from a program

```bash
nova-compiler emit prog.nova -o prog.c --error-format=json
```

One JSON object per diagnostic on **stdout**, one per line, nothing on stderr:

```json
{"level":"error","code":"E0308","message":"function `flag` returns `bool`, but this return value is `int`","file":"prog.nova","span":{"line_start":2,"column_start":12,"line_end":2,"column_end":13},"notes":[],"helps":["return a `bool` value, or change the declared return type"]}
```

`code` and `span` are `null` rather than absent, so a consumer does not have to
tell "missing" from "no value". A failure that is not a diagnostic — a file that
cannot be read — is still one object, with `level: error` and `code: null`. The
exit status remains the signal for whether the run succeeded.

This is what the language server should consume instead of re-deriving
diagnostics from a second compiler pass.

`nova-compiler explain` lists the codes; `nova-compiler explain E0308` prints the
long form below. The three lists — emitted, described, documented — are checked
against each other in CI by `tools/error-code-check.py`.

## Where codes come from

| Layer | Codes |
|---|---|
| lexer | none — lexical errors carry a message and a span |
| parser | none — parse errors carry a message and a span |
| semantic analysis | **E0002**, **E0061**, **E0308**, **E0400** |
| backends | none — an unsupported feature is rejected with a sentence, not a code |

The codes are attached in `compiler/src/sema/mod.rs` with
`Diagnostic::with_code(...)`. To check this page has not drifted:

```bash
rg -o 'E0[0-9]{3}' compiler/src/sema/ | sort -u
```

---

## E0400 — an `Option` payload must be a primitive

`Option<T>` is one C struct with an `int`, a `double` and a `char*` payload slot, so
`T` can be `int`, `float`, `bool`, `char` or `string` and nothing else. A struct
payload would need one `Option` per payload type, which the compiler does not emit yet.

```
error [E0400]: `Option<File>` cannot be represented: an `Option` payload is `int`,
              `float`, `bool`, `char` or `string`
```

The rule exists so this is a diagnostic at the source line rather than a C error inside
generated code. [`adr/0003-option-payload.md`](adr/0003-option-payload.md) has the
reasoning, including why adding a `void*` slot would be worse than either alternative.

## E0002 — undefined name

A name was used that nothing has declared in scope.

```
error [E0002]: undefined name `total`
    help: declare `total` first (`let total = ...` or `fn total(...)`), or check the spelling
```

Causes, in the order they happen:

- a typo in the name;
- a `use`/`mod` that did not pull in the module holding it;
- a function declared *after* its use in a module whose declarations were not
  merged — NOVA resolves across a file, so this is usually the `use` line.

---

## E0061 — wrong number of arguments

A call passes a different number of arguments than the callee declares.

```
error [E0061]: function `add` takes 2 arguments, but 3 were given
    help: `add` expects (int, int)
```

The `help:` line prints the full parameter list, which is the fastest way to see
whether you are missing an argument or calling the wrong function.

Generic calls are checked against the instantiation at the call site, so a
mismatch here can also mean the type arguments picked a different signature than
you expected.

---

## E0308 — mismatched types

The single most common error. One code covers four situations, distinguished by
the message:

| Message | Means |
|---|---|
| ``argument 2 of `add` is `float`, but the parameter is `int` `` | argument type |
| ``function `flag` returns `bool`, but this return value is `int` `` | `return expr` type |
| ``function `flag` returns `bool`, but this `return` has no value`` | bare `return` in a function that must produce a value |
| ``function `flag` returns `bool`, but its final expression is `int` `` | block-value return |

```
error [E0308]: function `flag` returns `bool`, but this return value is `int`
    help: return a `bool` value, or change the declared return type
```

`int` → `float` is the only implicit conversion. Everything else has to be
written out:

```nova
let x: float = 1;        // fine, implicit
let y: int = 1.5;        // E0308
let z: int = 1.5 as int; // explicit cast
```

---

## Not a NOVA code

`E0560` appears in a comment in `lsp/src/main.rs`. That is a **rustc** code, quoted
because it is what the Rust build said when the language server constructed an
LSP struct with a field that does not exist on it. It is not emitted by NOVA.

## Adding a code

1. Pick the next free number. The three in use are `E0002`, `E0061` and `E0308`;
   the gaps are deliberate, so a code never changes meaning.
2. Attach it with `.with_code("Exxxx")` and add a `.with_help(...)` line that says
   what to do, not what went wrong.
3. Add a row to this page and a case to the sema tests.
4. If the code has more than one message shape, table the shapes as E0308 does.
