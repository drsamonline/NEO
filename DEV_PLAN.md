# NOVA Development Plan

**Status source of truth.** Every claim here is checked against the code and the test
suite; if a row disagrees with `cargo test`, the row is wrong. Last verified: 2026-11-12.

## Current status

| Component | State | Verified by |
|---|---|---|
| Lexer | ✅ complete | `compiler/src/lexer` tests |
| Parser | ✅ complete for the supported grammar: generics, slices, references, struct literals, `match`, closures, casts, `static` | `grammar-check.py`, parser tests |
| Semantic analyser | ✅ names, scopes, return-type checking, call arity/type checking | `compiler/src/sema` tests |
| SIR | ✅ typed expressions (`SirExpr::Typed`) | lowerer tests |
| Lowerer | ✅ AST → SIR for the supported subset, including structs, field assignment, `impl` methods, enums, `match`, arrays and generic instantiation | lowerer tests |
| C backend | ✅ runnable output (`int main`), struct `typedef`s and compound literals, enum constants, tagged payload-enum structs, bounds-checked array values, runtime-shim calls | end-to-end tests |
| C runtime shim | ✅ `nova_runtime.h/.c`, ABI v3 (print, strings, bounds-checked growable arrays, closures, panic/exit, unsupported-builtin trap) | runtime + end-to-end tests |
| CLI | ✅ `emit` / `build` / `run`, `-o`, `--target c|js|rust|wasm`, `--` args; build/run are C-only | end-to-end tests |
| Parity harness | ✅ `nova-compiler test --parity` compares stdout/exit across C, JS, Rust and WASM; WASM needs `node` + `wat2wasm`, otherwise it is explicitly `held` and the run fails. CI runs every `examples/*.nova` and `tests/parity/*.nova` program | end-to-end tests, CI `--parity` corpus |
| Package manager | ⚠️ real `init`/`build`/`run`; registry mocked; dependencies recorded but not built | `pkg` tests |
| LSP | ⚠️ real compiler diagnostics with spans, cause and `help:` fixes; FULL document sync; cache keyed by document text. Go-to-definition is still a string search; `references` TODO | `lsp` unit tests + stdio integration test |
| Benchmarks | ✅ `benches/run.sh`: recursive fib(35) and 2000 short-lived array literals, NOVA→C vs hand-written C, same `-O2`, stdout compared before timing, 1.5× regressions fail | `benches/README.md`, CI |
| Debugger | ⚠️ REPL/breakpoints/session real; stack frames, source lines and DWARF mocked | `dbg` tests |
| Runtime crate (Rust) | ✅ memory, GC, I/O, threading, async compile and test; **not linked to generated C** | `runtime` tests |
| JS / Rust / WASM backends | ⚠️ all three emit through `--target`; JS and Rust output (arrays and closures included) is executed in tests. Rust closures are `Rc<dyn Fn(..) -> ..>` values over generated environment structs; WASM executes ints/strings/recursion/loops, growable int arrays, structs, tuples and `int` closures via `backends/wasm/host.mjs`. Unsupported signatures are rejected by `validate` instead of emitted as placeholders | `backends/wasm` and `rust_closures_run_and_match_c` tests, Node + `wat2wasm` end-to-end tests, `tests/parity/` |
| Standard library | ✅ all four modules parse and emit C (`use` merges `core` and `io`); `core` math helpers return their values (`factorial`, `fibonacci`, `sign`, `clamp`, `gcd`) | `compiler/tests/stdlib_parse.rs`, `compiler/tests/end_to_end.rs::core_stdlib_math_functions_return_their_values` |
| Examples | ✅ `array_ops.nova` now builds and runs; the remaining examples need the unimplemented features listed above | `nova-compiler run` |
| Docs | ❌ 59 files under 200 bytes; language spec describes syntax the parser lacks | — |

**Test suite:** 187 passing, 0 failing (`cargo test --workspace`; WASM execution tests need `node` + `wat2wasm`).

## Milestones, in order

Each milestone is done only when its acceptance check passes in CI.

| # | Milestone | Acceptance | State |
|---|---|---|---|
| 1 | Run a program end to end | `run examples/hello.nova` prints its output | ✅ done |
| 2 | Runtime shim linked by the compiler | emitted C compiles with `nova_runtime.c` alone | ✅ done |
| 3 | Types on SIR, including call results | `print` uses declared types; bools print `true`/`false` | ✅ done |
| 4 | Return-type checking | `fn f() -> bool { return 1; }` is `E0308` | ✅ done |
| 5 | Call argument checking | wrong arity and wrong argument types are errors | ✅ done |
| 6 | Spans on expressions | every type diagnostic points at the offending expression | ✅ done |
| 7 | Make the standard library parse | all `stdlib/**/*.nova` emit C | ✅ done |
| 8 | Resolve calls, `use`, `mod` across files | a two-file program builds and runs | ✅ done |
| 9 | Enums + `match` end to end | a `match` program compiles to `switch` and runs | ✅ done |
| 10 | Arrays with bounds checks in the shim | `examples/array_ops.nova` builds and runs | ✅ done |
| 11 | CI: build → link → run every example | new CI job fails on a wrong-output regression | ✅ done |
| 12 | Real end-to-end test crate | `cargo test --workspace` includes compiler→binary tests (`integration_tests/` today imports APIs that do not exist) | ⬜ |
| 13 | Wire JS/Rust/WASM targets | `--target javascript|rust|wasm` emits; JS, Rust and WASM output is executed in tests, and `test --parity` compares all four | ✅ done |
| 14 | LSP go-to-definition/references from the symbol table | editor navigation works | ⬜ |
| 15 | Debugger reads DWARF (`gimli`) | breakpoints hit real lines with real variables | ⬜ |
| 16 | Package manager builds dependencies; real or explicit local-only registry | `publish`/`search` stop being mocks | ⬜ |
| 17 | Documentation generated from code and grammar | no status table drifts; `docs/language/*` filled | ⬜ |
| 18 | VS Code extension ships the LSP client | installable extension | ⬜ |

## Working rules

- Follow this order unless a user request says otherwise.
- Do not claim a milestone done until its acceptance check runs and passes.
- Keep `README.md`, `CHANGELOG.md`, `compiler/README.md` and this file consistent with code.
- Every implemented feature gets a test at the layer it lives in.

## Known limitations (deliberate, documented)

- `int` → `float` is the only implicit conversion.
- Expression spans are recorded for expressions the parser returns from `parse_expr`;
  an operand inside a larger expression has no span of its own and falls back to the
  enclosing expression's span.
- Tuples lower to generated `NovaTuple_*` structs (`_0`, `_1`, …) and destructure
  with `let (a, b) = pair;`. Closures are lifted functions plus a heap
  environment: captures are copied by value at creation, parameters take their
  annotation or the binding's function type and otherwise default to `int`, and a
  function type is a `NovaClosure*`.
- Generics are monomorphised from concrete call sites: only instantiated functions
  are emitted, and an unbound type parameter falls back to `int`. Generic functions,
  structs, enums and impl methods are supported.
- `match` supports literals, string literals, `Enum.Variant` paths, binding
  patterns and guards. A statement `match` becomes a C `switch`; guarded or
  string arms fall back to an if/else chain, and a value `match` stays a ternary
  chain. A value-position match with a binding pattern cannot declare its binding.
  Variants with more than one payload are not lowered.
- Arrays are `NovaArray` (a heap buffer, length and capacity) with `int` elements;
  reads and writes are bounds-checked at runtime and abort with
  `array index out of bounds`. Push/pop grow and shrink a local array; a `&mut [T]`
  parameter passes the header by address, so pushes in the callee are visible.
- Generic structs and their methods are monomorphised from the receiver/binding
  type (`Pair<int, float>` becomes `Pair_int_float`); an unannotated literal keeps
  the erased type. References are only lowered for array parameters.
- String slices (`text[a..b]`) lower to `nova_substr`; string literals match through
  `nova_strcmp`.
- Payload enums store one scalar payload (`int`, `float` or `string`) in a fixed
  tagged struct; `Option<T>` is a built-in of the same shape and other generic enums
  substitute their payload types from the matched type. Variants with more than one
  payload field, and payload matching in value position, are not supported.
- `__builtin_*` printing, type, string, array-length, panic and exit builtins lower
  to the shim. OS and collection builtins (`args`, `array_push`, `array_pop`,
  `getenv`, `now`, `sleep`, `cwd`, `chdir`) resolve but trap through
  `nova_unsupported`.
- Expression types that cannot be inferred are never guessed, so type checking is sound
  but incomplete.
- `--release` selects an output directory only; the compiler has no optimisation levels.
