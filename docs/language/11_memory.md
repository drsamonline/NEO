# Memory

NOVA has no garbage collector, no destructors and no ownership checker. A program allocates
and never frees, and the process exits when it is done. This chapter says what that means in
practice, because a reader who assumes otherwise will write a program that behaves in ways they
cannot explain.

The short version: **memory management is the runtime's problem, and the runtime's answer is
"leak it".**

## What the runtime allocates and never frees

- **Every string.** `nova_strdup` copies a string onto the heap and nothing ever releases it.
  The runtime's own comment says so: *"the runtime already leaks every `nova_strdup`"*.
  Concatenating two strings allocates a third; formatting allocates; reading a line allocates.
- **Every closure environment.** A closure that captures anything gets a heap-allocated struct
  holding its captures, passed to the lifted function as `void *__env`.
- **Every array buffer.** `NovaArray` owns a heap buffer sized by its capacity.

For a program that runs and exits — a command-line tool, a compiler pass, a script — this is
fine and is a deliberate trade: no collector means no pauses and no tracing metadata.

For a program that runs for days, it is a leak. There is no `free` in the language, and no
`Drop` to hang one on.

## Arrays are headers, not handles

```nova
struct NovaArray {
    long long *data;
    long long len;
    long long cap;
};
```

`NovaArray` is **copied by value**, so passing an array to a function copies twelve bytes of
header — the buffer is shared, the length and capacity are not. Two consequences:

- **`push` takes a pointer.** `nova_array_push(NovaArray *array, ...)` updates the caller's
  header, because a reallocation changes `data` and `cap` in the header and the callee's copy
  cannot tell the caller.
- **A by-value parameter that reallocates changes only its own copy.** The buffer may be moved
  while the caller still holds the old pointer.

Float and string arrays are separate types (`NovaArray_float`, `NovaArray_string`) with the same
shape, because a `double` and a `char *` do not fit in an `int` slot. Since
[ADR 0011](../adr/0011-int-is-64-bit.md) an `int` element is 8 bytes.

## References are pointers, and nothing checks them

`&x` and `&mut x` both compile to a pointer. **The parser erases the difference** — it consumes
`mut` and drops it — so by the time anything could check aliasing, the distinction is gone.

That means:

- `&mut` is documentation, not a guarantee. Two `&mut` references to one value are not detected,
  because there is nothing left to detect them with.
- There is no borrow checker, no lifetime, and no way to express one.
- A reference held across a mutation that reallocates is a dangling pointer, and the language
  will not tell you.

This is why the [Ownership](10_ownership.md) chapter is unwritten: there is no ownership model
to document, only the absence of one. Writing it as though there were would be inventing.

## The exception: file handles

File and stream handles are the one resource that is tracked. `File.handle` is an index into a
64-slot table; `-1` means not open; closing a file releases its slot for reuse; opening a 65th
file **traps** rather than leaking a descriptor.

That is deliberate — a leaked descriptor is a failure the operating system will eventually
punish, while a leaked string only costs address space.

## What a program can rely on

| Property | Holds? |
|---|---|
| A string value stays valid for the life of the process | Yes — nothing is freed |
| Two variables holding one array share its buffer | Yes |
| Mutating through `&mut` is exclusive | **No** — nothing checks |
| Memory is reclaimed | **No** — except by exiting |

## Why this is written down rather than fixed

A memory model is a language design decision with consequences for every program, and NOVA has
made the simple choice deliberately. What it had not done was say so where a reader would find
it — and a reader who assumes a collector will write code that leaks for weeks before anyone
notices.

The honest description of a limit is worth more than a chapter describing a model that does not
exist.
