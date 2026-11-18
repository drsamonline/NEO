# ADR 0004 — a non-constant global initialiser runs at the top of `main`

- **Status:** accepted
- **Date:** 2026-11-18
- **Related:** [0003](0003-option-payload.md)

## Context

`stdlib/io` declares:

```nova
static SPINNER_FRAMES: [char] = ['|', '/', '-', '\\'];
```

The C backend emitted it at file scope as a call:

```c
NovaArray SPINNER_FRAMES = nova_array_from((int[]){ '|', '/', '-', '\\' }, 4);
```

C requires a file-scope initialiser to be a constant expression, so this was rejected:
`error: initializer element is not constant`. The module could not be compiled, and
neither could `stdlib/text`, which reaches it through `use io;`.

A string constant had the same shape once and was solved differently: it lowered to
`nova_strdup("...")`, and the fix was to emit the **literal** instead, because a string
literal has static storage. An array literal has no such fallback.

## The three options, and why two of them fail

**(a) A static backing array plus a compound initialiser.**

```c
static const int SPINNER_FRAMES__data[] = { '|', '/', '-', '\\' };
NovaArray SPINNER_FRAMES = { (int *)SPINNER_FRAMES__data, 4, 4 };
```

This compiles and is the tidiest-looking answer. It is also wrong, and the failure is
not theoretical. `nova_array_push` grows the buffer:

```c
int *grown = (int *)realloc(array->data, (size_t)cap * sizeof(int));
```

`realloc` on a pointer that did not come from `malloc`/`calloc`/`realloc` is undefined
behaviour. `SPINNER_FRAMES` starts with `len == cap == 4`, so the **first push**
reallocates static storage. `NovaArray` is three fields — `data`, `len`, `cap` — with no
ownership flag, and a runtime test pins that layout, so `push` cannot tell a static
buffer from a heap one.

**(c) Reject the declaration with a diagnostic.** Defensible in isolation, but the
language accepts `static [T] = [...]`, the stdlib uses it, and an array literal is a
natural way to write a constant table. Rejecting it removes a construct rather than
fixing the emission.

**(b) Declare it zeroed and assign it at the top of `main`.** Chosen.

```c
NovaArray SPINNER_FRAMES;

int main(void) {
    SPINNER_FRAMES = nova_array_from((int[]){ '|', '/', '-', '\\' }, 4);
    /* the program's own statements */
}
```

## Decision

A global whose initialiser is not a C constant expression is declared zero-initialised at
file scope and assigned at the **top of the entry point**, before any statement, in
declaration order.

Constant initialisers — literals, and a `nova_strdup` of a string literal, which
`global_init_to_c` renders as the literal — stay at file scope, unchanged.

## What this costs, stated plainly

- **Initialisation order is declaration order**, which is the order the source was
  written in. A global whose initialiser reads another deferred global works; a cycle
  would read a zeroed header.
- **Nothing can read a deferred global before `main`.** NOVA has no pre-`main` code, so
  nothing does. A read of a zeroed `NovaArray` is a clean runtime error
  (`nova_array_get` bounds-checks `len` and `data != NULL`), not a segfault.
- **A module emitted on its own, with no `main`, never runs its initialisers.** That is
  correct for a library that is never executed; when the module is imported into a
  program, its globals merge into that program and its `main` runs them.

## Why not give `NovaArray` an ownership flag

Because that changes the ABI every backend constructs arrays through, to fix one
emission problem. The deferral keeps the invariant the whole runtime relies on — *every
`NovaArray.data` came from `malloc`* — and costs one function call at start-up.

## Where this is tested

`stdlib/io` and `stdlib/text` are in `stdlib/not-compiling.txt` for two *other* reasons
now, and `core` and `math` compile through the same gate. The behaviour itself is pinned
by the stdlib compile step in `ci.yml`, which runs `cc -std=c11 -c` on every module.
