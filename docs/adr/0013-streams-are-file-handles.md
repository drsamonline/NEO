# ADR 0013 — a `Stream` is a file handle with a label

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0010](0010-file-handles.md)

## Context

`stdlib/io` declares a `Stream` for "generic" I/O:

```nova
enum StreamType {
    InputStream,
    OutputStream,
    BidirectionalStream,
}

struct Stream {
    kind: StreamType,
    handle: int,
}

fn stream_read(stream: &mut Stream, count: int) -> [int]
fn stream_write(stream: &mut Stream, data: [int]) -> int
fn stream_flush(stream: &mut Stream) -> bool
fn stream_close(stream: &mut Stream)
```

All four called a `__builtin_stream_*` that nothing implemented, so the whole type was
unusable. The question was what a stream *is*: a new abstraction over pipes, sockets and
files, or something narrower.

## Options

**1. A new abstraction.** Give `Stream` its own runtime, its own handle table and its own
notion of what can be read and written.

Rejected. There is nothing a stream needs that a file handle does not already provide, and
inventing a parallel mechanism would mean two things to keep working instead of one. A pipe
or a socket, when NOVA has them, is a handle — the same shape `File` already holds.

**2. Streams over the file handle, and nothing else.** Accepted.

## Decision

`Stream.handle` is a `File` handle — an index into the table [ADR 0010](0010-file-handles.md)
defined. The four builtins are the file builtins under another name, and there is no separate
runtime:

| Stream builtin | Answers |
|---|---|
| `__builtin_stream_read` | `nova_file_read_bytes` |
| `__builtin_stream_write` | `nova_file_write_bytes` |
| `__builtin_stream_flush` | `nova_file_flush` |
| `__builtin_stream_close` | `nova_file_close` |

`kind` is carried and not consulted. It records what the caller intends, not what the handle
can do — the same descriptor can be read and written if it was opened `r+`.

**`stream_flush` compares its result to zero.** The runtime answers C's `fflush`, which is
zero on success; returning it directly made a successful flush report `false`. `fseek` and
`fclose` already compared, and this did not.

**`stream_close` clears the handle to `-1`**, as `fclose` does, so a second close fails
instead of closing whichever slot the table reused.

## Consequences

- **There is no constructor.** A stream is built by hand from a `File`:
  `Stream { kind: StreamType.InputStream, handle: file.handle }`. That is a gap, not a design
  goal — a `stream_open` would be a small addition, and it is not added here because the
  right shape for it depends on whether pipes and sockets arrive as handles too.
- Closing the stream closes the file: they share one handle. A program that closes both will
  find the second close fails, which is the correct answer rather than a silent success.
- A stream of `kind: InputStream` opened on a write-only handle reads nothing. The `kind` is
  a comment, and the OS decides.
