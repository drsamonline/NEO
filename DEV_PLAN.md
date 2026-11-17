# NOVA Development Plan

**This is the only plan.** Four other documents in this repository describe
planning that no longer applies; they are listed at the bottom with what they are
for. If anything outside this file disagrees with this file, this file is right —
and if this file disagrees with `cargo test`, this file is wrong.

Last restructured: 2026-11-17. Every claim in the status table is checked against
the code and a CI run, and the run is named where it matters.

## The rules

1. **A milestone is done when its acceptance check runs and passes in CI.** Not
   when the code exists. Not when it builds locally.
2. **Never invent completed functionality.** A stub is honest; a plausible
   implementation that does not work is not.
3. **A backend that cannot support a feature rejects the program** with a message
   naming the feature. It never emits a placeholder that compiles and misbehaves.
4. **A flaky test is worse than no test.** It teaches people to re-run CI instead
   of reading it, and from then on the suite tells you nothing.
5. **Documentation is checked, not asserted.** Numbers come from `wc -l` or a CI
   run. Counts are never estimated.

---

## Where the project actually is

| Component | State | Verified by |
|---|---|---|
| Lexer | ✅ complete | `compiler/src/lexer` tests |
| Parser | ✅ complete for the supported grammar: generics, slices, references, struct literals, `match`, closures, casts, `static` | `tools/grammar-check.py`, parser tests |
| Semantic analyser | ✅ names, scopes, return-type checking, call arity/type checking | `compiler/src/sema` tests |
| SIR | ✅ typed expressions (`SirExpr::Typed`) | lowerer tests |
| Lowerer | ✅ AST → SIR for the supported subset | lowerer tests |
| C backend | ✅ runnable output, ABI v3 runtime shim | 52 end-to-end tests |
| C runtime shim | ✅ `compiler/src/runtime.rs`, embedded in the binary | runtime + end-to-end tests |
| CLI | ✅ `emit` / `build` / `run` / `doctor` / `test --parity` | end-to-end tests |
| JS / Rust / WASM backends | ⚠️ exercised subset; C is the reference | `tests/parity/` in CI |
| Runtime crate | ✅ compiles and tests; **not linked to generated C** | 33 runtime tests |
| Standard library | ⚠️ 4 modules, thin | `stdlib_parse.rs` |
| Package manager | ⚠️ `init`/`build`/`run` real; **registry mocked, deps not built** | `pkg` tests |
| Language server | ⚠️ diagnostics real; go-to-definition is a string search; `references` unimplemented | `lsp` tests |
| Debugger | ⚠️ REPL/breakpoints real; stack frames and DWARF mocked | `dbg` tests |
| VS Code extension | ❌ stub — 4-field `manifest`, 2-line `extension.ts` | — |
| Language spec | ❌ 1 of 25 chapters written | `docs/language/README.md` |
| **Test suite** | **199 passing, 0 failing, 0 ignored** | CI run 44117607709, the branch merged as #21 |

---

## The change this plan makes: correctness before features

The plan used to be a feature list — LSP navigation, DWARF, a registry, a
formatter, an extension. Every one of those is worth having. None of them makes
the compiler more **trustworthy**, and a compiler is trusted or it is nothing.

A transpiler has a specific failure mode: it can produce output that compiles,
runs, prints the right answer on the programs you tried, and is still wrong on the
program you did not try. The seven parity programs in this repository cannot
detect that. Nineteen more features cannot either.

So the plan is reordered around one question: **what evidence would let a stranger
trust this compiler with their code?** Everything below is ordered by how much
trust it buys per week of work.

The three gaps that matter most, none of which the old plan mentioned at all:

1. **Generated C is never run under a sanitiser.** The backend writes C that
   indexes arrays, copies structs and manages a growable buffer. Nothing checks
   that output for out-of-bounds access or undefined behaviour. This is the single
   largest hole in the project and it costs an afternoon to close.
2. **Emitted WebAssembly is never validated.** CI runs `wat2wasm` and a
   hand-written Node host. It never asks a WebAssembly validator whether the
   module is well-formed, so a malformed module can only fail in ways the host
   happens to exercise.
3. **Diagnostics have no golden corpus.** Three error codes and a set of help
   strings exist and nothing pins their text, span or code. Any diagnostic can
   silently degrade to a worse message and no test notices.

---

## Gates: what "accurate" means from now on

These become CI jobs. They are the acceptance check for the stages below, and once
in place they are the acceptance check for everything else.

| Gate | Command | Catches |
|---|---|---|
| **G1 Sanitisers** ✅ in place | generated C built with `-fsanitize=address,undefined -fno-sanitize-recover=all`, then run — `.github/workflows/accuracy.yml` | out-of-bounds access, use-after-free, signed overflow, misaligned access in backend output |
| **G2 WASM validation** ✅ in place | `wasm-tools validate --features=wasm1 out.wat` for every WASM-emitting program — `accuracy.yml`, wasm-tools pinned by version and SHA-256 | malformed modules, wrong arity, invalid indices — independent of the host |
| **G3 Diagnostic snapshots** | `.stderr` golden files per case, blessed by a flag, with `//~ ERROR` annotations in the source | a diagnostic silently changing its code, span or help text |
| **G4 Differential corpus** | generated NOVA programs run on all four backends, stdout compared | backend divergence on inputs nobody wrote by hand |
| **G5 Fuzz** | `cargo-fuzz` on lexer and parser, 60s per target per PR, longer nightly | panics, hangs, stack overflow on malformed input |
| **G6 Coverage** | `cargo llvm-cov` with a ratchet that may only go up | tests that assert nothing, untested branches |
| **G7 Mutation** | `cargo-mutants` on the lowerer and C backend, weekly | tests that pass for the wrong reason |

---

## The plan, in order

Effort is one person, part-time, and is an estimate — not a commitment.

### Stage 0 — One plan, true numbers (½ day)

- This file is the plan. The other four get a banner saying what they are.
- Every count in `README.md`, `DEV_PLAN.md` and the landing page comes from a
  command or a CI run.

**Done when:** `rg 'passing tests'` across the repo returns one number, and that
number appears in a CI run id.

### Stage 1 — Prove the output is correct (2–3 weeks)

The highest-value work in the repository, and nothing else starts until it is in.

- **G1 Sanitisers.** ✅ In place as `accuracy.yml`. Every program under
  `examples/` and `tests/parity/` is emitted, compiled with
  `-fsanitize=address,undefined -fno-sanitize-recover=all` and run. Leak
  detection is off for now: the shim's growable array never frees its buffer, so
  LeakSanitizer would fail every program that pushes. That is a real limitation
  and its own milestone, not something this gate should hide.
- **G2 WASM validation.** ✅ In place as `accuracy.yml`. Every program the WASM
  backend accepts is validated with `wasm-tools validate --features=wasm1`.
  wasm-tools is pinned by version and SHA-256. A program the backend rejects is
  counted as *held*, and the count is printed — a backend that starts rejecting
  the whole corpus cannot pass quietly. **Still to do:** the upstream WebAssembly
  `testsuite` as a submodule, for the subset the backend claims to support.
- **G3 Diagnostic snapshots.** One directory, one file per case, `.stderr` files
  blessed by a flag. Adopt rustc's model: the source carries `//~ ERROR` line
  annotations *and* there is a `.stderr` file, and the redundancy is deliberate —
  the annotation catches a wrong span, the file catches an unmentioned diagnostic.
- **G4 Differential corpus.** A generator that emits valid NOVA programs from the
  grammar, runs each on all four backends and compares stdout. Seven hand-written
  programs become a few hundred generated ones.

**Done when:** G1–G4 are required checks and green, and at least one real bug has
been found and fixed by each of G1 and G2.

**Why first:** every later stage gets cheaper. A sanitiser failure names the
function that emitted the bad C; without it, the same bug arrives as a mysterious
wrong answer three milestones later.

### Stage 2 — Make the diagnostics a contract (3–5 days)

A compiler's diagnostics are its interface. Right now they are a `String` and
three codes, and editors re-parse stderr.

- `--error-format=json` — one JSON object per diagnostic, with level, code, span,
  message, notes and helps. This is what the LSP should consume instead of
  re-deriving diagnostics from a second compiler pass.
- `nova-compiler --explain Exxxx` — the long form of `docs/errors.md`, printed by
  the binary, so the documentation and the compiler cannot drift.
- Generate the error-code list from the source into `docs/errors.md` in CI, and
  fail if the page and the code disagree.

**Done when:** the LSP gets its diagnostics from `--error-format=json`, and CI
fails if `docs/errors.md` lists a code the compiler does not emit.

### Stage 3 — Harden the front end (1 week)

- **G5 Fuzz** the lexer and parser.
- **G6 Coverage** with a ratchet.
- **G7 Mutation** weekly, starting with the lowerer and the C backend.
- Replace the wall-clock benchmark with instruction counts (`iai-callgrind` /
  Gungraun), which are stable enough to gate on.
- **Compiler-speed benchmark** ✅ in place: `benches/compiler.sh` times `emit`
  over the corpus and `bench.yml` records it. It measures the compiler, not the
  code the compiler produces.

**Done when:** no fuzz target has found a panic in 24 hours of accumulated runtime,
and coverage may not fall.

### Stage 4 — A real syntax tree (2 weeks, or explicitly deferred)

`nova fmt` is impossible today and LSP navigation is a string search, both for the
same reason: the AST is lossy. Comments and whitespace are not in it, so nothing
can reproduce source text, and a name's position is not a range in the tree.

- Move to a lossless concrete syntax tree (`rowan`), keeping the typed AST as a
  layer above it. This is the change rust-analyzer is built on.
- With it: `nova fmt`, precise go-to-definition and references, and rename.

**Done when:** `nova fmt` round-trips every file in `examples/` and `stdlib/`
byte-identically, and `references` returns real ranges.

**If deferred:** say so in this file, and say what it costs — the LSP stays a
string search and no formatter is possible.

### Stage 5 — Close the tooling milestones that are now cheap (1 week)

- **LSP go-to-definition and references** (old milestone 14) from the symbol table.
  Cheap once Stage 2 exists.
- **VS Code extension** (old milestone 18): `engines`, `main`, `contributes`, a
  client that spawns `nova-lsp`. Two days once the LSP answers navigation.
- **Package manager**: replace the mock registry with git and path dependencies
  plus a lockfile. A real registry is a product decision, not a compiler feature;
  git dependencies are a week and remove the "mocked" asterisk entirely.

### Stage 6 — Release and distribution (2 days)

Delete the hand-written release workflow and adopt `dist` (formerly cargo-dist). It
generates the release pipeline, the archives, the checksums, the installers and a
PR-time check of the release process itself — which is the part this repository
most needs, because the current workflow has never run.

**Done when:** a `v0.6.1` tag produces a published release with three platform
archives, and the packaged compiler compiles hello world on each.

### Stage 7 — The long tail (ongoing, lowest priority)

- **DWARF debugger** (old milestone 15). Real breakpoints need real line tables.
  Lowest trust-per-week of anything here: the REPL already works, and a debugger
  nobody uses does not make the compiler more trustworthy.
- **Standard library**: one test per exported function, run by the compiler itself.
- **Language spec**: write a chapter only when every claim in it can be checked
  against `grammar/grammar.ebnf` or a passing test.
- **Docs generated from code** (old milestone 17).

---

## Upstream: what fills each gap

The gaps below are not novel. Every one has a maintained upstream answer, and the
plan should borrow rather than invent.

| Gap | Upstream answer | Why it fits |
|---|---|---|
| Generated C never checked for memory errors | `-fsanitize=address,undefined` on the emitted C | Finds the class of bug a transpiler is most likely to create, at the point it is created |
| Emitted WASM never validated | `wasm-tools validate` (bytecodealliance) | Validates `.wat` directly, with explicit feature sets (`wasm1`/`wasm2`/`wasm3`) |
| No conformance corpus for WASM | WebAssembly `testsuite` as a submodule | The canonical `.wast` corpus every WASM producer is measured against |
| No way to shrink a failing WASM case | `wasm-tools shrink` | Turns a 400-line failure into a 4-line one |
| Diagnostics unpinned | rustc's `tests/ui` model: `.stderr` snapshots, `--bless`, `//~ ERROR` annotations | The most battle-tested diagnostic-testing design that exists |
| Snapshot harness | `ui_test` crate (used by rustc, rust-analyzer, miri) or `trybuild` | `ui_test` can ignore the snapshot and rely on annotations alone, which is far less brittle |
| No fuzzing | `cargo-fuzz` (libFuzzer) | Standard, and the lexer/parser are the right targets |
| Tests may assert nothing | `cargo-mutants` | A surviving mutant is a test that does not test |
| No coverage signal | `cargo-llvm-cov` | Ratchet it so it can only go up |
| Wall-clock benchmarks flake | `iai-callgrind` / Gungraun — Valgrind instruction counts | Deterministic in CI, single-shot, no repeat-and-average |
| Hand-rolled release pipeline | `dist` (formerly cargo-dist) | Generates the workflow, archives, checksums and a PR-time release check |
| Lossy AST blocks formatter and LSP | `rowan` lossless CST, `salsa` incremental queries | The architecture rust-analyzer is built on |
| Hand-rolled lexer | `logos` (if lexer speed becomes the bottleneck) | Not needed yet — the current lexer is fine and has no dependency |
| Hand-maintained docs site | `mdBook` + `mdbook-linkcheck` | `docs/language/` is already markdown; the HTML is hand-written |
| No editor syntax highlighting | `tree-sitter-nova` grammar | Gives editors highlighting and the LSP a fallback parser for free |
| Supply chain unguarded | `cargo-deny` + `cargo-audit` + SBOM | Licence and advisory gate; feeds `NOTICE` |
| MSRV undeclared | `rust-version` in `[workspace.package]` + `cargo-msrv` | A pin that means something |

---

## What to stop doing

- **Stop adding backends.** Four is already more than can be kept equal. C is the
  reference; the other three should be *provably* equal on the supported subset
  before any of them grows.
- **Stop estimating line counts.** They were wrong by 2.5× in the README and by
  more in `MILESTONES.md`. Generate them.
- **Stop maintaining a bespoke release workflow** once `dist` is in.
- **Stop the wall-clock benchmark gate.** Done — it moved to a weekly job.
- **Stop writing plan documents.** This is the fifth. There should be one.

---

## Known limitations (deliberate, documented)

- `int` → `float` is the only implicit conversion.
- Expression spans are recorded for expressions the parser returns from
  `parse_expr`; an operand inside a larger expression falls back to the enclosing
  span.
- Tuples lower to generated `NovaTuple_*` structs; closures are lifted functions
  over a heap environment, captures copied by value.
- Generics are monomorphised from concrete call sites; an unbound type parameter
  falls back to `int`.
- `match` supports literals, string literals, `Enum.Variant`, bindings and guards.
  A variant with more than one payload is not lowered.
- Arrays are `NovaArray` with `int` elements, bounds-checked at runtime.
- References are lowered only for array parameters.
- `__builtin_*` OS and collection builtins resolve but trap through
  `nova_unsupported`.
- `--release` selects an output directory; the compiler has no optimisation levels.
- The runtime crate is not linked to generated C; the C shim is.
- The package registry is mocked.

---

## The other planning documents

| File | What it is |
|---|---|
| `MILESTONES.md` | The original 12-milestone narrative, superseded. Its line and test counts are from an early snapshot and are wrong; kept for the history. |
| `IMPLEMENTATION_PLAN.md` | An 8-phase compiler implementation plan from before the backends worked. Superseded. |
| `PHASE2_SUMMARY.md` | An accurate description of the runtime crate and what it does and does not do. Still worth reading; not a plan. |
| `FILES_CREATED.md` | A session log from an early compiler session. Not a status document. |
| `VISION.md` | Three lines of intent. |

None of them is authoritative. This file is.
