# ADR 0020 — a concurrency model, and why it cannot be one model

- **Status:** **proposed** (2026-11-26)
- **Date:** 2026-11-26
- **Related:** [0017](0017-ownership-proposal.md) (reclamation), [0019](0019-mangling-for-module-collisions.md), [Concurrency chapter](../language/13_concurrency.md), `BACKEND_API.md`

## The question

NOVA has no concurrency of any kind. `docs/language/13_concurrency.md` documents the
absence and proves it with a command. This ADR asks what to add, and — the harder half —
**which of the four targets can express it.**

## What is true today, measured

| Fact | Command | Result |
|---|---|---|
| no concurrency keyword in the grammar | `grep -ciE "async\|await\|thread\|spawn\|mutex\|atomic" grammar/grammar.ebnf` | **0** |
| none in the lexer | same pattern, `crates/nova-common/src/token.rs` | **0** |
| none in the C shim | `grep -ciE "pthread\|atomic\|mutex" compiler/src/runtime.rs` | **0** |
| none in the standard library | `grep -rciE "async\|thread\|mutex\|spawn" stdlib/*/*.nova` | **0** |
| the JS backend has no worker support | `grep -ci "Worker\|SharedArrayBuffer" backends/javascript/src/lib.rs` | **0** |
| the WASM backend refuses 7 things it cannot express | `grep -c 'return Err(' backends/wasm/src/lib.rs` | **7** |

**And one structural fact that decides more than any of them.** `BACKEND_API.md` states the
contract as one function:

```rust
fn generate(&mut self, program: &SirProgram) -> String
```

**One program in, one string of source out.** A thread on the JavaScript target is a
`Worker` — a *second file*, loaded by URL. A thread on the WebAssembly target needs shared
memory and the threads proposal, which `wat2wasm` only produces with `--enable-threads` and
which the host must instantiate with a shared `Memory`. Neither fits "one string".

The C target is the exception, and it has a precedent: `emit -o file.c` already writes
`nova_runtime.h` and `nova_runtime.c` beside its output, because generated C links a shim
rather than libc. C is the only target whose output is already more than one file.

## What each target can actually do

| Target | Threads available | Fits the one-string contract? |
|---|---|---|
| C | `pthread_create`, `_Atomic`, C11 threads | **yes** — the shim is a separate file already |
| Rust | `std::thread`, `Arc<Mutex<T>>` | **yes** — `std::thread::spawn` is in `std` |
| JavaScript | `Worker` + `SharedArrayBuffer` | **no** — a Worker is a separate script, and the backend emits one |
| WebAssembly | threads proposal + shared memory | **no** — needs a flag at assembly time and a host that shares memory |

## The cost of the smallest possible addition

`clone` is the cheapest construct this compiler has gained: a **builtin**, with no new
syntax at all. It touched 6 files and about 90 lines (#149, #152, #153).

A concurrency construct needs syntax, so it touches every layer:

| Layer | File |
|---|---|
| token | `crates/nova-common/src/token.rs` |
| grammar | `grammar/grammar.ebnf` |
| AST | `crates/nova-common/src/ast.rs` |
| parser | `compiler/src/parser/mod.rs` |
| sema | `compiler/src/sema/mod.rs` |
| SIR | `crates/nova-common/src/sir/mod.rs` |
| lowerer | `compiler/src/lowerer/mod.rs` |
| C backend | `compiler/src/backend/mod.rs` |
| three other backends | `backends/{javascript,rust,wasm}/src/lib.rs` |
| runtime shim | `compiler/src/runtime.rs` |

**Eleven files, four of them backends, and two of those cannot express the feature.**

## Options

### A. `spawn` + a handle, C and Rust only; JS and WASM refuse

The compiler accepts `spawn f(x)`, producing a handle with `join`. The C backend emits
`pthread_create`; Rust emits `std::thread::spawn`. The JavaScript and WebAssembly backends
return an error naming the feature, which is what `BACKEND_API.md` rule 1 already requires:

> Reject what you cannot express; never emit a placeholder.

**Cost:** the full eleven layers, plus a handle type in sema. **Benefit:** real threads on the
two targets that have them, and no lie on the other two. **Risk:** the type system has no
notion of shared mutable state, and NOVA has no collector — a thread that allocates a
`String` leaks it just as the main thread does, but now the leak is concurrent and
unbounded in time.

### B. Cooperative concurrency only — `spawn` compiles to a queue on all four targets

One thread, a run-to-completion queue, `yield` points. Expressible everywhere: C gets a
loop, JS gets `Promise`, Rust gets a `VecDeque`, WASM gets a function table.

**Cost:** the same eleven layers, plus a scheduler in the shim. **Benefit:** the same
semantics on all four targets, which is the property the parity harness can actually check.
**Risk:** it is not parallelism, and calling it `spawn` would be a false claim of exactly the
kind this repository has spent the week removing. It would have to be named `defer` or
`queue` and documented as cooperative.

### C. No new syntax — the standard library only, C and Rust

`core.spawn` / `core.join` as library functions over the shim, no keyword. Sema needs to know
the return type is a handle, which means a builtin type, which is smaller than syntax but not
free.

**Cost:** 3–4 files. **Benefit:** the smallest step that produces something usable.
**Risk:** a function that starts a thread and returns a handle cannot express a closure over
the caller's environment without closures being `Send`-safe, which nothing checks.

### D. Channels first, threads never

A message-passing primitive (`chan<T>`, `send`, `recv`) with no shared state at all. Shared
nothing means no data races by construction, which is the only concurrency story this
language can tell *honestly* while it has no collector and no borrow checker.

**Cost:** eleven layers again, but no handle type — a channel is a value. **Benefit:** the
ownership story stays one rule. **Risk:** a channel needs a runtime queue, and a blocking
`recv` on the JavaScript target blocks the only thread.

### E. Decide not to decide

Document the absence, keep `13_concurrency.md` as the answer, and spend the effort on the
reclamation model instead. Concurrency without a memory model is how languages acquire
undefined behaviour they cannot remove later.

## Recommendation

**E now, D when a user asks.**

The reason is the measurement above, not a preference: **the one-string backend contract
cannot carry a thread on two of four targets**, and both alternatives that fit it (B, C) buy
the feature by making `spawn` mean something weaker than the word implies. This repository's
rule is that a construct the compiler cannot express gets a diagnostic rather than C that
misbehaves — and on JS and WASM, a thread *is* such a construct.

Option D is the one that would survive: a channel is a value, a value fits the contract, and
"no shared state" is the only guarantee the current type system can actually keep. It should
be designed when someone has a program that needs it, not before — because ADRs 0017, 0018
and 0019 each under-estimated their emission cost by exactly the information the emission
needed, and this one is four backends wide.

## What this ADR does not decide

- A memory model for shared state. There is none, and D avoids needing one.
- Whether `spawn` should exist at all on C and Rust while JS refuses. The parity harness
  would report those programs as `held` on two backends forever, which is a permanent
  asymmetry in the one check that compares targets.
- How a handle is released. No collector means a joined handle is freed by hand or not at
  all.

## Consequences of choosing E

- `docs/language/13_concurrency.md` stays accurate with no edit.
- `DEV_PLAN.md` keeps concurrency as an open decision rather than an item.
- If a user asks for it, D is the design to write, and this ADR is where the measurement
  lives.
