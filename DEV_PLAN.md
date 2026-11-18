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
| Package manager | ⚠️ `init`/`build`/`run` real; **path and git dependencies resolve and are importable**; registry mocked; no lockfile, no transitive graph | `pkg` tests, `deps.rs` |
| Language server | ⚠️ diagnostics real; definition and references return exact identifier spans, but resolve names textually — shadowing is not distinguished | `lsp` tests, `navigation_stdio.rs` |
| Debugger | ⚠️ REPL/breakpoints real; stack frames and DWARF mocked | `dbg` tests |
| VS Code extension | ✅ manifest, LSP client, TextMate grammar, language configuration; `tsc` and `vsce package` in CI | `extension.yml`, `tools/extension-check.py` |
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
| **G3 Diagnostic snapshots** ✅ in place | `.stderr` goldens per case in `tests/diagnostics/`, with `//~ ERROR` annotations in the source; `tools/diagnostic-check.py`, blessed with `--bless` or a `workflow_dispatch` input | a diagnostic silently changing its code, span or help text |
| **G4 Differential corpus** ✅ in place | `tools/corpus-gen.py` generates programs from a seed and writes the output they must produce, computed independently in Python; `corpus.yml` runs `test --parity`, which checks both agreement between backends and agreement with that oracle | backend divergence on inputs nobody wrote by hand — and four backends that agree and are all wrong |
| **G5 Fuzz** ✅ in place, advisory | `cargo-fuzz` on lexer, parser and the whole front end — 60s per target per PR, 10 minutes weekly — `fuzz.yml`, seeded from `examples/` and `tests/parity/` | panics, hangs, stack overflow on malformed input |
| **G6 Coverage** ✅ in place | `cargo llvm-cov` → LCOV, line coverage compared to `coverage-floor.txt`, which may only go up — `coverage.yml` | tests that assert nothing, untested branches |
| **G7 Mutation** ✅ in place, advisory | `cargo-mutants` on the lowerer and C backend, Saturdays — `mutants.yml` | tests that pass for the wrong reason |
| **Docs** ✅ in place | `tools/docs-check.py` — no document under 120 bytes, every document reachable from a front door, every workflow and tool explained | a placeholder, an orphan page, or a check nobody understands |

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
  the whole corpus cannot pass quietly.

  **Correction.** Earlier revisions of this plan listed "add the WebAssembly
  `testsuite` as a submodule" as the remaining part of this stage. That was wrong:
  the spec `testsuite` is `.wast` scripts that test a WASM *engine*, and NOVA is a
  WASM *producer*. There is nothing for it to run. `wasm-tools validate` is the
  right check for a producer, and it is already in place. The `testsuite` would
  only become relevant if NOVA grew its own WASM runtime.
- **G3 Diagnostic snapshots.** ✅ In place: `tests/diagnostics/`, eight cases,
  `tools/diagnostic-check.py`. rustc's model with the redundancy kept — the
  `.stderr` golden pins the exact text, span and help; the `//~ ERROR` annotation
  says which codes are *expected*, so an extra diagnostic on a line nobody
  touched fails instead of being re-blessed unnoticed. A separate step fails if a
  case that should fail has a golden saying `exit: 0`, because `--bless` would
  otherwise turn a regression into a new expected value.

  Found while building it: **the CLI never printed error codes.** Every
  `Diagnostic` carried a `code`, `docs/errors.md` documented three of them, and
  `compiler/src/main.rs` printed only `diag.message` — so `E0308` existed
  everywhere except in front of the person who needed to search for it. The CLI
  now prints `[Exxxx]` and the diagnostic's `notes`, and does the same for parse
  and lexical diagnostics.
- **G4 Differential corpus.** ✅ In place: `tools/corpus-gen.py` (seeded,
  reproducible, `--dump <seed>` to reproduce a failure) and `corpus.yml`.
  Parity alone would miss four backends that agree and are all wrong, so each
  generated program ships a `.expected` computed in Python and `test --parity`
  checks the C reference against it. **The subset is deliberately narrow** —
  integers, `+`/`-`, comparison, `if`/`else`, `while`, calls, `print`, `return` —
  because that is what every backend claims, so a failure is a compiler bug
  rather than a documented limitation. No `/`, `%` or `*`: division rounds
  differently across C, JavaScript and Rust, and multiplication overflows.

**Done when:** G1–G4 are required checks and green, and at least one real bug has
been found and fixed by each of G1 and G2.

**Why first:** every later stage gets cheaper. A sanitiser failure names the
function that emitted the bad C; without it, the same bug arrives as a mysterious
wrong answer three milestones later.

### Stage 2 — Make the diagnostics a contract (3–5 days)

A compiler's diagnostics are its interface. Right now they are a `String` and
three codes, and editors re-parse stderr.

- `--error-format=json` — ✅ one JSON object per diagnostic on stdout, one per
  line, nothing on stderr. `code` and `span` are `null` rather than absent.
- `nova-compiler explain [Exxxx]` — ✅ the long form, printed by the binary.
- `tools/error-code-check.py` — ✅ fails if the emitted codes, the described
  codes and `docs/errors.md` are not the same set.
- `tools/diagnostic-check.py` — ✅ now also runs every case with
  `--error-format=json` and asserts the contract: valid JSON per line, no stderr,
  and the codes matching the `//~ ERROR` annotations.

**Remaining:** the LSP still re-derives diagnostics instead of reading
`--error-format=json`. That is the one line of this stage not yet crossed, and it
belongs with the LSP navigation work in Stage 5.

### Stage 3 — Harden the front end (1 week)

- **G5 Fuzz** the lexer and parser. ✅ `fuzz/` with three targets — `lex`,
  `parse`, and `frontend` (source to SIR). Seeded by `fuzz/seed.sh` from the
  repository's own programs, because a random-byte fuzzer spends its whole budget
  failing to produce a valid program and never reaches the parser. **Advisory
  until a weekly run is clean** — a compiler that has never been fuzzed will find
  things on the first runs, and a new gate that starts red is not a gate.
- **G6 Coverage** with a ratchet. ✅ `coverage.yml` computes line coverage from
  LCOV `LF`/`LH` records rather than the summary table, whose column order is a
  presentation detail a gate should not depend on.
- **G7 Mutation** weekly. ✅ `mutants.yml` mutates `compiler/src/lowerer/mod.rs`
  and `compiler/src/backend/mod.rs` on Saturdays, shardable by dispatch input.
  Advisory until the survivors are triaged: the first run is a map of where the
  suite is weak, not a list of things to fix before merging.
- Replace the wall-clock benchmark with instruction counts (`iai-callgrind` /
  Gungraun), which are stable enough to gate on.
- **Compiler-speed benchmark** ✅ in place: `benches/compiler.sh` times `emit`
  over the corpus and `bench.yml` records it. It measures the compiler, not the
  code the compiler produces.

**Done when:** no fuzz target has found a panic in 24 hours of accumulated runtime,
and coverage may not fall.

### Stage 4 — A real syntax tree: hybrid, not either/or (in progress)

This was written as a choice between a full `rowan` migration and declining. Both
are worse than the third option, which is now the decision: **a lossless tree at
item boundaries, with bodies deferred and expanded on demand.** Recorded in
[`docs/adr/0001-lossless-syntax-tree.md`](docs/adr/0001-lossless-syntax-tree.md),
with the precedent — IntelliJ chameleons, rust-analyzer's `ItemTree`, rustc's lazy
token streams, tree-sitter's `ERROR` nodes.

The invariant that makes it sound: **a deferred node holds a span into the source
and nothing else.** A deferral that stores anything derived is lossy, and a lossy
deferral cannot be expanded correctly later.

- ✅ `compiler/src/syntax/mod.rs` — top-level items with exact spans, names and
  deferred body ranges. No parsing inside a body.
- ✅ The outline in the language server is its first consumer, and an `impl`'s
  methods appear only by expanding its body.
- ✅ `nova fmt`: `compiler/src/fmt.rs` normalises whitespace — one blank line between
  items, no trailing whitespace, one final newline — and **changes no token**. It
  never parses, so it is total: it works on half-typed code.
  `compiler/tests/format.rs` checks against every shipped program that the token
  stream is identical, no comment is lost, the item structure is unchanged and
  formatting twice changes nothing. It **re-indents** every line of code to its
  bracket depth from the token stream — no parse, so it cannot disagree with the
  parser — and leaves string literals and comment-only lines alone.
  **What it does not do:** break a long line or reflow an expression. That needs a
  lossless tree over every expression, which is the next line.
- ✅ A guard that the item tree and the parser agree about what an item is —
  `compiler/tests/item_tree.rs` compares the two on every shipped program, the way
  `grammar-check.py` guards the grammar against the parser. One asymmetry is
  allowed and named: the parser turns a bare top-level expression into a synthetic
  `__main` function, and the tree reports no item there.

  **It found two real bugs on its first run.** `fn(A) -> B` — a function *type* inside
  a parameter list — was read as a declaration, because parentheses do not change the
  brace depth, so `stdlib/core/core.nova` reported eight functions that do not exist.
  And `static` was invisible to the tree while the parser recorded it as a `ConstDef`.
  Both fixed; three unit tests pin them.
- ✅ Settle `trait`: it is not a keyword, the grammar has no production for it and
  the parser cannot produce one, so `Item::Trait` and `TraitDef` were dead code
  claiming a feature. Removed — [`docs/adr/0002-traits.md`](docs/adr/0002-traits.md).
  When traits are added, the keyword, the grammar production, the parser and a
  parity program land together.

**What it does not buy:** type-checking is not faster — `sema` needs bodies. The
laziness is real only for features that can decline to look inside.

**Done when:** the item tree and the parser agree on every example in CI, and a
formatter can rewrite the *inside* of a body without changing meaning. ✅ **Both are
true:** `compiler/tests/item_tree.rs` compares the two on every shipped program, and
`nova fmt` re-indents from the token stream, so it changes no token and cannot
disagree with the parser about anything.

What is left is not a stage: **reflowing** an expression — breaking a long line,
aligning arguments — needs a lossless tree over every expression, which is where
`rowan` would come in. Nothing above it depends on that answer, because nothing
outside `compiler/src/syntax` and `compiler/src/fmt` depends on how a body is
represented.

### Stage 5 — Close the tooling milestones that are now cheap (1 week)

- **LSP go-to-definition and references** (old milestone 14). ✅ Token-level: the
  compiler's own tokens give exact identifier spans, `includeDeclaration` is
  honoured, and the capability is advertised because it now answers. It used to
  return the whole line at column zero for any line containing `fn <name>`.
  **Not a resolver** — two identifiers with the same text are one symbol, so
  shadowing is not distinguished. Closing that gap is Stage 4: it needs a lossless
  tree with a scope map, which is the same thing `nova fmt` needs.
- **VS Code extension** (old milestone 18). ✅ A real manifest (`engines`,
  `main`, `activationEvents`, `contributes`), a client that spawns `nova-lsp` over
  stdio and reports clearly when it cannot, a TextMate grammar whose keyword list
  is derived from `crates/nova-common/src/token.rs`, and a language configuration.
  `tsc` and `vsce package` run in CI, and `tools/extension-check.py` fails if the
  grammar stops describing the compiler's keywords.
- **Package manager**: replace the mock registry with git and path dependencies.
  ✅ `pkg/src/deps.rs` resolves each dependency to a directory and passes
  `--module-root` to the compiler, so `use utils;` finds a dependency's source. A
  git dependency is cloned into `.nova/deps/<name>` and checked out at
  `rev`/`tag`/`branch`; a version-only one fails with a message naming the registry
  as unimplemented. The compiler still reads no manifest and fetches nothing.

  **Not done:** a lockfile, and a transitive graph. Both are named in
  `PACKAGE_ARCHITECTURE.md` rather than implied — two machines can currently resolve
  the same `nova.toml` to different revisions.

### Stage 6 — Release and distribution (in progress)

The hand-written workflow is real but has **never executed** — it is tag-triggered, and
no tag has been pushed. "It works" is currently an assumption.

- ✅ `workflow_dispatch` runs it as a **dry run**: verify, build all three platforms,
  package, smoke-test the packaged compiler, print the checksums, stop. No draft, no
  upload, no publication. `docs/tooling.md` → "Cutting a release" has the procedure.
- ✅ The verify step's logic is checked locally: `v0.6.1` passes against
  `Cargo.toml`/`VERSION`, `v0.7.0` fails. That is the gate that stops the four-version
  defect from shipping.
- ⬜ **The first real run.** It needs a tag, and a tag publishes a public release, so
  it is the owner's call. Cutting one means promoting `[Unreleased]` to a version
  first — the accumulated work is not in 0.6.1.
- ⬜ Adopt `dist` (formerly cargo-dist) once the pipeline is proven, if it earns its
  keep: it generates the archives, the checksums and a PR-time check of the release
  process, at the cost of a dependency on its own conventions.

**Done when:** a tag produces a published release with three platform archives, and
the packaged compiler compiles hello world on each.

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
| No editor syntax highlighting | ✅ TextMate grammar in `vscode-extension/syntaxes/`, generated from `token.rs` | Highlighting in VS Code now; `tree-sitter-nova` would add other editors and a fallback parser for the LSP |
| Supply chain unguarded | ✅ `cargo-deny` — licences, sources, bans, advisories (`security.yml`) | Licence and advisory gate; `deny.toml` is now the policy `NOTICE` claims. **SBOM generation is still open.** |
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
- **Stop leaving placeholders.** `tools/docs-check.py` now fails a build for a
  document under 120 bytes, an unlinked page, or a workflow with no explanation.

---

## Known limitations (deliberate, documented)

- `int` → `float` is the only implicit conversion.
- Source may be UTF-8, including in comments, identifiers and string literals.
  Non-ASCII *output* is compared byte-for-byte across backends by
  `tests/parity/unicode_string.nova`.
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
