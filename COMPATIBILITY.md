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
  only what it declares. A change to it is a change to the compiler's output
  contract, so it gets a version bump.
- **Error codes.** `E0002`, `E0061`, `E0308` are searchable identifiers and do not
  change meaning. New codes are added; old ones are not repurposed. See
  `docs/errors.md`.
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

**`v0.7.0` is the first release that ships binaries:**

| Archive | Size |
|---|---|
| `nova-v0.7.0-x86_64-linux.tar.gz` | 3.8 MB |
| `nova-v0.7.0-arm64-macos.tar.gz` | 3.5 MB |
| `nova-v0.7.0-x86_64-windows.zip` | 3.1 MB |

Each has a `.sha256` beside it and contains the four binaries — `nova-compiler`,
`nova-pkg`, `nova-lsp`, `nova-dbg` — plus `stdlib/`, `examples/`, the licence and the
changelog.

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
