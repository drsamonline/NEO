# Tooling: what every check guards

Eleven workflows and six scripts. This page exists because a check nobody
understands gets deleted the first time it is inconvenient.

The rule behind all of it: **a claim in this repository must be produced by a
command or a CI run.** Numbers come from `wc -l` or a log, never from memory.

## The Python checks

Each is runnable locally with no toolchain, which is why they are Python.

| Script | Fails when | Why it matters |
|---|---|---|
| `tools/grammar-check.py` | `grammar/grammar.ebnf` stops describing `compiler/src/parser/mod.rs` and `token.rs` | The grammar is the contract every other layer is checked against |
| `tools/example-check.py` | an `examples/**/*.nova` is not a NOVA program, or the recogniser accepts something it should reject | An example that does not parse is worse than no example |
| `tools/diagnostic-check.py` | a diagnostic's text, span, code or help changes; or the `--error-format=json` contract breaks | Diagnostics are the compiler's interface |
| `tools/error-code-check.py` | the codes emitted, described by `explain`, and documented in `docs/errors.md` are not the same set | Three lists that can drift, in both directions |
| `tools/extension-check.py` | the TextMate grammar's keywords stop matching `token.rs`, or a contributed path does not exist | Highlighting a keyword the parser dropped is a small lie nothing else catches |
| `tools/status.py` | `docs/status.json` no longer matches the version, line counts, ADR count, gate list or stdlib ratchet | Other repositories read that file instead of copying numbers into prose; drift is how the NEO skill claimed v0.6.0 for a week |
| `tools/type-name-check.py` | a type is read through `TypeExpr::name` instead of `type_display`, or a helper inspects a `SirExpr` without unwrapping `Typed` | Ten defects in one session came from those two idioms; both drop information silently |
| `tools/docs-check.py` | a document is a stub, a `docs/` page is unreachable from `docs/README.md`, a workflow or tool has no explanation | This page's own guarantee |
| `tools/architecture-check.py` | `ARCHITECTURE.md`'s line counts drift from the source, or a `Location` it names does not resolve | A count typed into prose is wrong within a week; `--fix` rewrites it |
| `tools/module-check.py` | a module is declared and never used, or a file exists that nothing references | Two empty modules survived for months behind `pub mod` lines |
| `tools/stdlib-check.py` | the README's function table disagrees with the source, a function calls a builtin the compiler lacks, or two modules declare one name | The library's own claims, checked against the library |
| `tools/manifest-check.py` | `MANIFEST.json` no longer describes the files it records | Provenance nobody checks drifts into a claim nobody can trust |
| `tools/workspace-dep-check.py` | `[workspace.dependencies]` names a crate no member opts into | `nova-runtime` sat there reading as "NOVA has a Rust runtime" while nothing used it |
| `tools/no-network-check.py` | the compiler, language server or debugger gains an HTTP client crate or a socket call | `PRIVACY.md` and `FAQ.md` promise they never reach the network; that is a claim about the dependency graph, so it is checked |
| `tools/stale-claim-check.py` | a document calls a capability "mocked" or "missing" after the source made it real | `ARCHITECTURE.md` said the registry, `references` and the debugger's frames were all mocked, months after each was fixed |
| `tools/count-check.py` | a document states a number the workspace does not have — tests, chapters, parity programs | Six documents quoted the suite as 199, 316 and 193 while it was 342 |

## The shell scripts

| Script | Does |
|---|---|
| `benches/run.sh` | Times NOVA→C against hand-written C, same `-O2`, stdout compared before timing |
| `benches/compiler.sh` | Times the compiler's own `emit` over the corpus — the half of "performance" nobody measured |
| `fuzz/seed.sh` | Copies `examples/` and `tests/parity/` into the fuzz corpus, because a random-byte fuzzer never reaches the parser |
| `scripts/bootstrap.sh` | Sets up a working tree from scratch |

## The workflows

| Workflow | Guards | Blocking? |
|---|---|---|
| `ci.yml` | Build, 317 tests, every example runs, stdlib emits C, generated JS and Rust run, `test --parity`, docs check | **yes** |
| `accuracy.yml` | **G1** generated C under ASan+UBSan; **G2** emitted WASM validated by `wasm-tools` | **yes** |
| `diagnostics.yml` | **G3** eight diagnostic goldens plus the JSON contract; error-code agreement | **yes** |
| `corpus.yml` | **G4** 20 generated programs, each compared to an independent Python oracle *and* across four backends | **yes** |
| `coverage.yml` | **G6** line coverage from LCOV `LF`/`LH`, against `coverage-floor.txt`, which may only go up | **yes** |
| `security.yml` | Licences against the policy in `deny.toml`, crates.io-only sources, bans, RustSec advisories | **yes**, advisories report separately |
| `extension.yml` | `npm ci` from the lockfile, `tsc`, `vsce package`, archive contents | **yes** |
| `fuzz.yml` | **G5** `cargo-fuzz` on lexer, parser and front end, seeded | **advisory** — until a weekly 10-minute run is clean |
| `mutants.yml` | **G7** `cargo-mutants` on the lowerer and C backend, weekly | **advisory** — until the survivors are triaged |
| `bench.yml` | Wall-clock bench and compiler-speed bench, weekly and on demand | no — a trend, not a gate |
| `dev-build.yml` | `workflow_dispatch` → build the workspace, smoke-test, upload the four binaries as a seven-day artifact so a machine without `rustc` can run them | on demand |
| `release.yml` | Tag → verify → draft release → three-platform build with a packaged-compiler smoke test → publish. `workflow_dispatch` runs the same pipeline as a **dry run** and stops before creating anything | on tag, or on demand |
| `lint` job in `ci.yml` | `cargo fmt --check`, `clippy -D warnings` | **advisory** — the tree has never been formatted |

## Why some are advisory

An advisory check is a decision with a reason written next to it, not an oversight:

- **Fuzzing** — a compiler that has never been fuzzed finds things on its first
  runs. A new gate that starts red tells nobody anything actionable. It becomes
  blocking when a weekly 10-minute run is clean.
- **Mutation testing** — the first run's survivors are a map of where the suite is
  weak, not a list of things to fix before merging.
- **fmt/clippy** — `cargo fmt --check` on a tree that has never been formatted fails
  on every file at once. It goes blocking in the same change that lands
  `cargo fmt --all`.

## Cutting a release

```bash
# 1. Promote [Unreleased] in CHANGELOG.md to a version heading, and bump the version
#    in Cargo.toml ([workspace.package]) and VERSION. All three must agree or the
#    workflow stops at its first step.
# 2. Rehearse: Actions → Release → Run workflow, with the tag you intend to push.
#    That builds all three platforms, packages, smoke-tests the packaged compiler and
#    prints the checksums. It creates no release.
# 3. Publish:
git tag v0.7.0 && git push --tags
```

The dry run exists because this workflow was written in a pull request and, until it
runs, "it works" is an assumption. It is the one piece of the repository that cannot
be checked by pushing a branch.

**What the first real release proved the dry run could not.** The dry run skips the
three steps guarded on `push` — create, upload and publish — so it cannot cover them.
The first `v0.7.0` tag built all three platforms and uploaded six assets, and then
failed at the last step:

```
failed to run git: fatal: not a git repository (or any of the parent directories): .git
```

`publish` has no `actions/checkout`, so `gh release edit` could not work out which
repository it was talking to. Every `gh` call in the workflow now passes
`-R "$GITHUB_REPOSITORY"` rather than relying on the working directory being a
checkout. The lesson is not "test harder": it is that a guard which makes a step safe
also makes it untested, and the untested step is where the bug was.

**The verify step is what stops the version bug from shipping.** It refuses a tag
whose version disagrees with `Cargo.toml` or `VERSION` — the defect that once left
four different version numbers in one repository.

## What is not covered

- **`cargo llvm-cov` cannot see code exercised through a subprocess.** The CLI, the
  language server and the debugger are all tested that way, so their coverage is
  under-reported. `coverage-floor.txt` records this rather than hiding it.
- **No SBOM generation** yet, though `NOTICE` and `deny.toml` cover licences.
- **No Windows or macOS CI** for the compiler; `release.yml` builds all three only
  when a tag is pushed.
