# Compatibility

What a NOVA program can rely on, and what changes without warning.

## The four targets

| Target | Status | Built and run by the CLI? |
|---|---|---|
| `c` | **the reference** | yes — `build` and `run` link it |
| `js` | exercised subset | no — take the output to Node |
| `rust` | exercised subset | no — take the output to `rustc` |
| `wasm` | narrower subset | no — take the `.wat` to `wat2wasm` |

"Exercised subset" is a precise claim, not a vague one: it means the features that
`examples/**/*.nova` and `tests/parity/**/*.nova` actually run, checked on every
push by `nova-compiler test --parity`. Anything outside that is untested on that
target, whatever the code looks like.

## What is stable

- **The C ABI.** `nova_runtime.h` is versioned (currently v3) and generated C calls
  only what it declares. **An addition does not bump the version; a change to an
  existing declaration or a removal does.** That distinction is the whole rule: C
  emitted by an older compiler calls only what it declared, so a new function is
  invisible to it, while a changed signature or a deleted one breaks it. `nova_array_clone`
  was added under v3 on that reading — recorded here because the previous wording
  ("a change gets a bump") would have required v4 for a function nothing older calls.
- **Error codes.** `E0002`, `E0010`, `E0061`, `E0308` are searchable identifiers and do
  not change meaning. New codes are added; old ones are not repurposed. See
  `docs/errors.md`. This list is checked against the compiler by
  `tools/error-code-check.py`, so it cannot quietly fall behind again — `E0010` was
  missing from it until 2026-11-26.
- **The diagnostics JSON shape.** `--error-format=json` is documented in
  `docs/errors.md` and the language server is meant to consume it.

## What changes

- **The language itself, freely.** This is pre-release. Syntax can change, and
  `DEV_PLAN.md` → "Known limitations" is the list of what is deliberately absent.
- **Generated code.** Nothing promises the emitted C looks the same between
  versions — only that it behaves the same. Do not parse it.
- **The CLI surface.** New verbs and flags arrive without deprecation cycles.

## Versioning

One version for the whole workspace (`Cargo.toml` → `[workspace.package]`), and
`VERSION` must match it. `CHANGELOG.md` records what changed; `[Unreleased]` always
sits above the newest release.

## What each backend refuses

NOVA has four backends and they are not equally capable. **A backend that cannot express something
refuses it**, with a message naming the construct — never by lowering it to a placeholder that
compiles and misbehaves. This table is the list of those refusals, and
`tools/backend-limits-check.py` fails if a backend gains one that is not here.

| Backend | Refuses |
|---|---|
| **C** | nothing: it is the reference backend and every construct in the spec lowers to it |
| **JavaScript** | nothing at `validate` time; `sizeof` is reported as `0`, which is correct only because the JavaScript `nova_env_new` ignores its size argument |
| **Rust** | nothing at `validate` time; the same `sizeof` arm exists and is unreachable, since this backend has no `nova_env_new` |
| **WebAssembly** | the seven constructs below |

**WebAssembly refuses.** Two constructs have left this list: `print` of a `float`, through a `print_f64` host import that formats with `printf("%g")` to match the C runtime, and a struct with a `float` field, now that a slot carries its width and an `f64` takes eight bytes on an eight-byte boundary.

| Construct | Message |
|---|---|
| a `for` over something that is not a range | *a for-loop over a non-range iterable* |
| a range used outside a `for` | *a range outside a for-loop* |
| a standard-library call this backend does not implement | *runtime calls this backend does not implement* |
| slicing a string | *string slicing* |
| field access on an unknown struct type | *a field access with no known struct type* |
| field assignment on an unknown struct type | *a field assignment with no known struct type* |
| a closure taking or returning a `float` | *closure signatures with floating-point parameters/results* — **and this one stays refused**, see below |

**Why a float closure is still refused — measured, not assumed.** A parity program was written to
support it, and it showed the construct is broken in three of four backends:

| Backend | A closure `|x| x * 2.5` called with `2.0`, `0.5`, `1.5` |
|---|---|
| **C** (the reference) | prints `5`, `0`, `25` — it truncates the `f64` to an `i32` |
| **JavaScript** | prints `5`, `1.25`, `3.75` — correct |
| **Rust** | does not compile: `expected i32, found f64`, `Rc<dyn Fn(f64) -> f64>` against `Rc<dyn Fn(i32) -> f64>` |
| **WebAssembly** | refuses |

So the refusal was the only honest behaviour, and the WebAssembly backend was right to have it. Making
this construct work means fixing the **reference** first — its output defines what the other three are
compared against — and that is a separate piece of work with a program that already fails. Until then
the refusal stays, and it now says why.

**Why the WebAssembly gaps exist rather than being fixed:** struct fields are four-byte `i32` slots
in this backend's linear memory, so an `f64` field would need a different layout — alignment,
offsets, and the host bridge all change. Nothing in `examples/`, `tests/parity/` or the standard
library uses one, so it is refused rather than built for a construct nothing asks for. The closures
and string-slicing refusals are the same kind of statement: a limit that is named, not a bug.

## Installable from a release, not from a package manager

Not on crates.io, not on npm — `Cargo.toml` says `publish = false` on every crate,
which is what makes that statement checkable rather than aspirational.

**`v0.8.1` is the current release** — the first whose macOS and Windows archives carry all four
binaries. Its sizes are read off the release page once it is published; the table below is **v0.8.0's**,
kept because those numbers were measured, and its macOS and Windows archives hold three binaries
rather than four for the reason described under "What an artifact needs at run time".

**`v0.8.0`**, published 2026-11-27, at these sizes — read off the release page, not estimated:

| Archive | Size | Binaries |
|---|---|---|
| `nova-v0.8.0-x86_64-linux.tar.gz` | 3,983,473 B (3.8 MB) | four |
| `nova-v0.8.0-arm64-macos.tar.gz` | 3,030,060 B (2.9 MB) | three — no `nova-dbg` |
| `nova-v0.8.0-x86_64-windows.zip` | 2,790,355 B (2.7 MB) | three — no `nova-dbg` |

**Verified by downloading the published `x86_64-linux` archive**, checking its SHA-256 against the
published `.sha256` (`7561fb55…66031c` — they match), running `nova-compiler --version`
(`nova-compiler 0.8.0`) and emitting C for a hello-world, which produced `nova_print_str("Hello,
NOVA");` and wrote `nova_runtime.h` and `nova_runtime.c` beside it.

**`v0.7.0` was the first release that shipped binaries**, at these sizes — kept because they were
measured, and they differ from v0.8.0's because the debugger and the compiler both grew:

| Archive | Size |
|---|---|
| `nova-v0.7.0-x86_64-linux.tar.gz` | 3.8 MB |
| `nova-v0.7.0-arm64-macos.tar.gz` | 3.5 MB |
| `nova-v0.7.0-x86_64-windows.zip` | 3.1 MB |

Each has a `.sha256` beside it and contains `stdlib/`, `examples/`, the licence, the changelog and
the binaries.

**The v0.8.0 archives carry three binaries on macOS and Windows** — no `nova-dbg` — because `attach`
was gated `#[cfg(unix)]`, and macOS is unix, so the build there compiled a `PTRACE_ATTACH` that
`libc` does not define. That release found it the hard way: the tag went up, the macOS build stopped,
and nothing published because the workflow builds before it uploads.

**Fixed after it:** `attach` carries the same `target_os = "linux"` gate as its six siblings, with a
refusal in words elsewhere, and the v0.8.1 dry run on 2026-11-27 built all three platforms green. From
that release on, every archive carries four binaries and `nova-dbg` on macOS and Windows says why it
cannot attach rather than being absent.

**The debugger itself is still Linux-only.** The refusal is the honest part: `ptrace` exists on macOS
with a different API, so supporting it is a separate question nobody has needed to answer.

### What an artifact needs at run time

- **A C compiler on `PATH`.** `nova-compiler build` and `run` lower to C and link it
  with the host compiler, so a machine with no `cc` gets a clear error rather than a
  working build. `emit` needs nothing.
- **glibc 2.39 or newer on Linux.** The binary is built on `ubuntu-latest` (24.04).
  On Debian 12 (glibc 2.36) it prints `weak version GLIBC_2.39 not found` and still
  runs; older still is untested. Building from source avoids the question entirely.
- `node` and `wat2wasm` for the WASM execution tests, not for the compiler itself.

Verified by downloading the published `x86_64-linux` archive, checking its SHA-256
against the published one, running `nova-compiler --version` (`nova-compiler 0.7.0`)
and emitting C for `examples/hello.nova` — which produced
`nova_print_str("Hello, NOVA");` and wrote `nova_runtime.h` beside it.

The two earlier releases, `v0.6.0` and `v0.6.1`, remain empty: both tagged on one
commit, neither carrying an artifact.
