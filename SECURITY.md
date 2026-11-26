# Security Policy

NOVA is a compiler. It reads source files you give it and writes C, JavaScript,
Rust or WebAssembly. A bug in it can therefore be a security bug in anything it
compiles, which is why this file exists.

## Supported versions

| Version | Supported |
|---|---|
| 0.6.x | ✅ yes |
| < 0.6 | ❌ no |

No release is published yet, so in practice the supported version is the tip of
`master`. Fixes land there and nowhere else.

## What counts as a vulnerability

In scope:

- **Memory safety in generated C** — a NOVA program whose C output reads or
  writes outside a buffer, or uses a value after it is freed.
- **A missing bounds check** — `NovaArray` indexing is bounds-checked by design;
  a path that skips the check is a bug.
- **Code injection through generated output** — a NOVA string or identifier that
  escapes its quoting and changes the meaning of the emitted C, JavaScript or
  Rust. Escaping lives in `escape_string_literal` in `crates/nova-common`.
- **Path traversal in `use`/`mod` resolution or in the package manager** — a
  manifest or module path that reads or writes outside the project directory.
- **The language server or debugger acting on a hostile workspace** — a
  `nova-lsp` or `nova-dbg` process that executes code, escapes the workspace, or
  crashes on malformed input.
- **The published landing page / PWA** — script injection, or a service worker
  that serves stale or wrong content.

Out of scope:

- A NOVA program that is simply wrong. NOVA does not sandbox what it compiles:
  if you write a program that deletes files, it deletes files.
- Panics or crashes on malformed source. Those are bugs, and they are welcome as
  ordinary issues — a compiler should not crash on bad input, but a crash is not
  a vulnerability.
- The known limitations listed in `DEV_PLAN.md`. They are documented, not secret —
  and two of them are no longer what they were when this list was written. The package
  registry **refuses** rather than answering from a mock, and the debugger is **real**:
  `ptrace`, breakpoints, a frame-chain backtrace and `.nova` line numbers (ADR 0016).
  A defect in either is in scope if it meets the criteria above; "mocked" was true in
  October and is not true now.
- Anything requiring the attacker to already have write access to the victim's
  machine.

## Reporting

**Do not open a public issue for a vulnerability.**

- Email **sohilmomin2000@gmail.com** with `NOVA security` in the subject.
- Or use GitHub's private advisory form:
  <https://github.com/drsamonline/NEO/security/advisories/new>

Include: the NOVA source (or manifest, or LSP request) that triggers it, the
target you used (`c`/`js`/`rust`/`wasm`), the version or commit, and what you
observed versus what should have happened.

## What to expect

| Stage | Target |
|---|---|
| Acknowledgement | within 7 days |
| Initial assessment | within 14 days |
| Fix or a decision to accept the risk | within 90 days |
| Credit in the changelog | on request, unless you prefer to stay anonymous |

This is a single-maintainer project, so these are intentions rather than a
service-level agreement. If a report goes unanswered for 30 days, send it again
— the mail may simply have been missed.

## Disclosure

Please give a fix 90 days before publishing. If the issue is being exploited, or
if 90 days pass with no fix and no explanation, publish — and say so in the
report so the timeline is agreed rather than assumed.

## Safe harbour

Good-faith research on your own machine, or on a copy of this repository, is
welcome. Do not test against infrastructure you do not own. Do not send a report
containing someone else's data.

## A note on generated code

If you compile untrusted NOVA source with `nova-compiler`, treat the output the
way you would treat untrusted C: it is a program, and running it is your
decision. The compiler is not a sandbox and does not claim to be one.
