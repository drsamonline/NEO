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

## Nothing is installable yet

Not on crates.io, not on npm. `Cargo.toml` says `publish = false` on every crate,
which is what makes that statement checkable rather than aspirational.

Two GitHub Releases exist — `v0.6.0` and `v0.6.1` — and **neither carries an
artifact**: the release pages are empty. Both tags also point at the same commit, so
they are one snapshot published twice. Neither is a thing you can download and run.

The pipeline that would fix that is verified: a dry run builds `x86_64-linux`,
`arm64-macos` and `x86_64-windows` archives, writes a SHA-256 beside each, and runs
the packaged compiler on all three. No tag has been pushed since it was written, and
`v0.6.1` already exists — so the first real release needs a version above it.
