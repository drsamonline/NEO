# ADR 0010 — An open file is an index into a fixed table, not a pointer

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0003](0003-option-payload.md)

## Context

`stdlib/io` declares its handle as an `int`:

```nova
struct File {
    path: String,
    mode: String,
    handle: int,  // Opaque handle to runtime file descriptor
}
```

Ten builtins sit behind that field — `__builtin_fopen`, `fclose`, `fread_line`, `fread_all`,
`fwrite`, `feof`, `fseek`, `ftell`, `fflush`, `fread_bytes`, `fwrite_bytes` — and none of
them had an arm in the lowerer. Every one fell through to `nova_unsupported`, which compiles
and traps. So every function in `io.nova` that touches a file was a runtime error waiting to
happen, and the module still passed the stdlib compile gate, because the gate asks whether C
compiles.

The obstacle is the field. An open file in C is a `FILE *`, which is a machine word. An
`int` is not guaranteed to hold one, and on the platforms this compiler targets it does not.

## Options

**1. Widen `File.handle` to a pointer type.** Add `ptr` or `handle` to the language, make
`File.handle: ptr`, and store the `FILE *` directly.

Rejected: it changes a language-visible field to hold a C type, which ADR 0007 and the
backend have both worked to keep out of NOVA's surface. It also makes every `File` a word
wider for a value no NOVA program can inspect, and `File.handle >= 0` — the check `fopen`
already writes — stops being expressible.

**2. Change `File` to hold no handle and keep the file open in the runtime, keyed by path.**
Rejected: two `File` values for the same path would collide, and `fopen` on the same path
twice is legal and common.

**3. Index a fixed table.** `handle` stays an `int`; `1..NOVA_MAX_OPEN_FILES` are open files
and `-1` is "not open". The runtime keeps `static FILE *nova_open_files[64]` and hands back
`slot + 1`.

Accepted.

## Decision

A handle is an index into a fixed table of `NOVA_MAX_OPEN_FILES` (64) slots.

- `-1` means "not open". A failed `fopen` returns it, and `fclose` leaves it in the field —
  which is what `io.nova` already did, so no source changed.
- Slots are reused: `fclose` clears its slot, so a program that opens and closes in a loop
  does not exhaust the table.
- Opening past 64 live files closes the newly opened descriptor and traps, rather than
  leaking it. A program that holds 64 files open at once has a design problem worth naming
  loudly.
- Every shim looks the handle up and treats an unknown one as a closed file, so a stale
  handle reads as empty or `-1` rather than dereferencing freed memory.

## Consequences

`fread_line` returns a line without its terminator, which is the contract `fgets` already
has. That makes it impossible to tell an empty line from the end of the file — so
`fread_all` could not be built from it, and the version that tried returned `"ab"` for a file
containing `"a\nb\n"` and stopped at the first blank line. `nova_file_read_all` reads from the
current position to EOF in one call instead. That also removes a quadratic rebuild: the old
loop re-allocated the whole string once per line.

The table is fixed, so the 65th simultaneously open file traps. That is a deliberate limit
rather than a silent failure, and it is one `#define` to raise.

The ten builtins are now typeable — `__builtin_fopen` returns an `int` handle that compares
with `>= 0`, which is how `fopen` was already written.
