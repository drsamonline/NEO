# ADR 0012 — the emitted entry point takes `argc` and `argv`

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0010](0010-file-handles.md)

## Context

`core.args()` returns the command-line arguments. It was the last function in `core` that
could not run, and it could not run because there was nothing to return: the backend emitted

```c
int main(void) { ... }
```

so the process never received its arguments in a form the runtime could reach.

The value is small — a compiled language that cannot read its command line cannot be used for
a command-line tool — but the change is not: `main` is emitted once per program, so its
signature is a property of every executable this compiler produces.

## Options

**1. Leave `main(void)` and drop `args()` from the library.** Rejected: `args()` is a
documented part of `core`, and removing it to avoid a signature change is choosing the
smaller fix over the right one.

**2. Read `/proc/self/cmdline`.** Rejected: it is Linux-only, and the runtime already carries
`#ifdef _WIN32` branches for the functions that need them. Adding a third platform story for
something the language already has in its hands would be a worse trade.

**3. Emit `int main(int argc, char **argv)`.** Accepted.

## Decision

The emitted entry point is `int main(int argc, char **argv)`, and its first statement is
`nova_set_args(argc, argv)`, before any global initialiser runs — a global whose initialiser
calls `args()` must see them.

`nova_args()` builds a `[String]` from **`argv[1..]`**: `argv[0]` is the program's own path,
which is not an argument the program was given. A program that wants its own name cannot get
it from `args()`, and that is a deliberate limit rather than an oversight — every caller that
asks for "the arguments" means the ones after the path.

## Consequences

- Every program this compiler emits has a different `main` signature than before. Two
  integration tests asserted `int main(void)` and now assert the new signature — the assertion
  is tighter than a search for `int main`, so dropping the arguments again fails a test.
- A program that never calls `args()` pays one function call at startup and nothing else.
- `args()` answers a fresh array each call, built from pointers into `argv` that the runtime
  does not own and does not free.
- There is no way to reach `argv[0]` from NOVA. If a program needs to know how it was invoked,
  that is a new decision and a new function.
