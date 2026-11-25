# Concurrency

NOVA has **no concurrency**. There are no threads, no `async`, no `await`, no locks, no atomics and
no way to start a second process from a program. A NOVA program is one thread of control from
`main` to exit.

This chapter says that plainly, and lists what a program can do instead, because "there is nothing
here" is a fact a reader needs to know before designing around it.

## What is not in the language

Every word a concurrency model would need is absent from both the grammar and the runtime —
checked, not assumed:

| Word | In `grammar/grammar.ebnf` | In `compiler/src/runtime.rs` |
|---|---|---|
| `async`, `await` | 0 | 0 |
| `thread`, `spawn` | 0 | 0 |
| `mutex`, `lock` | 0 | 0 |
| `atomic`, `sync` | 0 | 0 |

The lexer's keyword list is the same one the [Keywords](03_keywords.md) chapter documents:
`fn let mut if else while for return true false struct enum match use mod pub const type as in
loop break continue impl static`. Nothing in it schedules anything.

## What a program is

One `main`, running on the process's only thread, in the order the statements appear. `for` and
`while` are loops, not parallel iteration: a body runs once per element, one at a time.

## The runtime keeps shared state, and does not lock it

The file API hands back an `int` handle indexing a fixed table:

```c
static FILE *nova_open_files[NOVA_MAX_OPEN_FILES];
```

That table is a single global. In a single-threaded program it is exactly right — an index is
smaller than a `FILE *`, and `-1` means a failed open. In a program with two threads it would be
two writers to one array with no lock, which is why the language does not offer the second thread
rather than offering it unsafely.

`nova_strdup`, the allocator calls and the print functions are ordinary C library calls; they are
not the problem. The table is, and it is the reason this chapter exists rather than a warning
buried in the file module.

## What a program can do instead

- **Do one thing at a time.** This is the intended model, and it is what the standard library is
  written for.
- **Wait, explicitly.** `core.sleep(ms)` suspends the single thread for a duration, and
  `core.now()` reads milliseconds since the epoch. A poll loop is legal; it is just a loop.
- **Run several programs.** The shell starts as many NOVA programs as you like; each is a process
  with its own memory and its own file table. Coordination is the operating system's, through
  files and exit statuses — `main` returns an `int`, and that is what a parent sees.

What a program **cannot** do is start another program itself: there is no `exec`, `fork`, `system`
or `popen` in the runtime (checked: zero occurrences). A NOVA program that needs a child process
needs a shell around it.

## What this rules out

| Because there is one thread | Consequence |
|---|---|
| no data races | the bug class does not exist |
| no deadlocks | there are no locks to order |
| no `Send`/`Sync` annotations | a signature says nothing about sharing, because nothing is shared |
| no parallel speed-up | a CPU-bound program uses one core, whatever the machine has |

## If this changes

A concurrency model is a design decision, not an implementation gap: the shape (threads, tasks,
actors), the sharing rule, and the runtime's locking all have to be chosen together, and each
choice invalidates programs written under the others. Until that decision is taken and recorded in
`docs/adr/`, this chapter is the specification: **one thread, no primitives, no promise of any**.
