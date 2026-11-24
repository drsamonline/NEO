# NOVA Documentation

NOVA is a statically-typed, compiled systems language. Its compiler lowers source
through a semantic pipeline — lexer, parser, semantic analyser, SIR and lowerer —
to four backends: C, JavaScript, Rust and WebAssembly.

Current stage: **Phase 1 complete, Phase 3 partial, Phase 4 in progress**. The
[development plan](../DEV_PLAN.md) is authoritative; where this site and the plan
disagree, the plan wins.

## Start here

- [Getting started](docs/getting-started.md) — build the compiler and run a program.
- [Language tour](docs/language-tour.md) — the syntax the compiler accepts.
- [Main README](../README.md) — features, limitations and project layout.
- [Development plan](../DEV_PLAN.md) — what is complete, partial or still a scaffold.
- [Changelog](../CHANGELOG.md) — what changed in each milestone.

## Backends

| Backend | Status |
|---|---|
| C | Reference target; links the runtime shim. Arrays are bounds-checked and growable. |
| JavaScript | Executed in tests, including arrays and closures. |
| Rust | Executed in tests, including arrays and `Rc<dyn Fn(..)>` closures over generated environment structs. |
| WebAssembly | Executed through `backends/wasm/host.mjs`, including growable int arrays, structs, tuples and `int` closures. |

`nova-compiler test --parity` compares stdout and exit status across all four
backends when `node` and `wat2wasm` are installed. CI runs it over every
`examples/*.nova` and `tests/parity/*.nova` program.

## Known gaps

- WASM float struct fields and non-`int` closure signatures are rejected by
  `validate` rather than miscompiled.
- WASM out-of-bounds indexing traps while C reports a runtime error, so those
  programs are not stdout-identical.
- The language server has diagnostics, but go-to-definition is a string search
  and `references` is not implemented.
- The debugger runs a program under `ptrace`, so `break <function>`, `break <file:line>`, `step`, `continue` and
  `backtrace` really work — every frame and hit reports its `.nova` line; `attach <pid>` stops a
  running process and `where` reports its real source line; `line <address>` maps an address
  through the line table, and `list` shows the real source. `print 0xADDRESS` reads memory directly; `print <name>`
  refuses, because reading a variable needs its type, which the compiler does not emit.
- The package registry is not implemented, and every command that would reach it
  **refuses** rather than answering from a mock. `publish` in particular does not
  pretend to succeed.
