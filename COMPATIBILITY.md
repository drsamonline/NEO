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

## Installable from a release, not from a package manager

Not on crates.io, not on npm — `Cargo.toml` says `publish = false` on every crate,
which is what makes that statement checkable rather than aspirational.

**`v0.8.0` is the current release**, published 2026-11-27. Sizes read off the release page, not
estimated:

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
