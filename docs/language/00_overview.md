# Language Overview

The only written chapter of this specification. It describes what NOVA is, how a
program becomes a binary, and which parts of the language are real. For the
chapter-by-chapter state, see the [status page](README.md).

## What NOVA is

A statically typed, compiled language whose compiler does not emit machine code.
It lowers source to a typed intermediate representation and then writes out one
of four target languages, each of which has its own compiler and its own runtime
already.

| Target | Emitted | The CLI can |
|---|---|---|
| C | C source plus `nova_runtime.h/.c` | `build` and `run` it, by calling the system C compiler |
| JavaScript | one `.js` file | `emit` it; `test --parity` runs it under `node` |
| Rust | one `.rs` file | `emit` it; `test --parity` builds it with `rustc` |
| WebAssembly | `.wat` text | `emit` it; `test --parity` assembles with `wat2wasm` and runs the Node host |

C is the reference target. A feature is real when it works there. The other three
run an exercised subset, and the subset is defined by what `tests/parity/` and
`examples/` actually run, not by intention.

## How a program is compiled

```
source.nova
   │
   ├─ lexer        tokens, with spans
   ├─ parser       AST (crates/nova-common/src/ast.rs)
   ├─ modules      `use` and `mod` pull in sibling and stdlib files
   ├─ sema         scopes, name resolution, arity and type checks
   ├─ lowerer      SIR — typed expressions, monomorphised generics
   └─ backend      C | JavaScript | Rust | WebAssembly
```

Each arrow is a crate boundary or a module boundary, and each has its own tests.
`integration_tests/` walks the whole chain from a source string to emitted text.

The SIR lives in `crates/nova-common/src/sir/mod.rs`. It is typed: an expression
carries its type once sema has decided it, which is why the backends never have
to re-infer anything.

## What the language is not

NOVA is pre-release and the gaps are deliberate and documented. The short list:

- **No optimisation.** `--release` chooses an output directory. The compiler has
  no optimisation levels; the C compiler you call afterwards does the optimising.
- **Generics are monomorphised from concrete call sites.** A generic function
  that is never called is never emitted. An unbound type parameter falls back to
  `int`.
- **Arrays hold `int`.** They are growable and bounds-checked, and the element
  type is not yet general.
- **Payload enums carry one scalar payload.** `Option<T>` is the built-in of that
  shape. A variant with two payload fields does not lower.
- **`int` → `float` is the only implicit conversion.** Everything else must be
  written out.
- **No `async`/`await` in the language.** The runtime crate has futures and
  channels; the language does not expose them.
- **The package registry is not implemented.** Dependencies are recorded in a
  manifest and not built; every command that would reach the registry refuses.
- **The debugger runs, breaks, steps, inspects and walks the stack.** `run` starts the program
  under `ptrace`; `break <function>` resolves the symbol and `break <file:line>` scans the line table, and
  either writes an `int3` that reports the `.nova` line when it fires; `step` and `continue` move it; `backtrace` follows the frame-pointer chain;
  `attach <pid>` stops a running process (ADR 0016); `where` symbolizes the real instruction
  pointer; `line <address>` maps any address to a `.nova` file and line; `list` shows the real
  source. `print 0xADDRESS` reads memory; `print <name>` refuses, because a variable needs its stack
  offset and type and only line information is emitted. A frame or stop outside the program's
  own code is reported as an address with no source line.

`DEV_PLAN.md` is the authority for this list. Where it and this page disagree,
`DEV_PLAN.md` is right and this page is stale.

## Naming

The project is called **NOVA**. The repository is called **NEO**. Both names
appear in the tree; they refer to the same thing, and the compiler binary is
`nova-compiler`.
