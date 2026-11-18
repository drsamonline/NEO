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
| `tools/docs-check.py` | a document is a stub, a `docs/` page is unreachable from `docs/README.md`, a workflow or tool has no explanation | This page's own guarantee |

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
| `ci.yml` | Build, 214 tests, every example runs, stdlib emits C, generated JS and Rust run, `test --parity`, docs check | **yes** |
| `accuracy.yml` | **G1** generated C under ASan+UBSan; **G2** emitted WASM validated by `wasm-tools` | **yes** |
| `diagnostics.yml` | **G3** eight diagnostic goldens plus the JSON contract; error-code agreement | **yes** |
| `corpus.yml` | **G4** 20 generated programs, each compared to an independent Python oracle *and* across four backends | **yes** |
| `coverage.yml` | **G6** line coverage from LCOV `LF`/`LH`, against `coverage-floor.txt`, which may only go up | **yes** |
| `security.yml` | Licences against the policy in `deny.toml`, crates.io-only sources, bans, RustSec advisories | **yes**, advisories report separately |
| `extension.yml` | `npm ci` from the lockfile, `tsc`, `vsce package`, archive contents | **yes** |
| `fuzz.yml` | **G5** `cargo-fuzz` on lexer, parser and front end, seeded | **advisory** — until a weekly 10-minute run is clean |
| `mutants.yml` | **G7** `cargo-mutants` on the lowerer and C backend, weekly | **advisory** — until the survivors are triaged |
| `bench.yml` | Wall-clock bench and compiler-speed bench, weekly and on demand | no — a trend, not a gate |
| `release.yml` | Tag → verify → draft release → three-platform build with a packaged-compiler smoke test → publish | on tag only |
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

## What is not covered

- **`cargo llvm-cov` cannot see code exercised through a subprocess.** The CLI, the
  language server and the debugger are all tested that way, so their coverage is
  under-reported. `coverage-floor.txt` records this rather than hiding it.
- **No SBOM generation** yet, though `NOTICE` and `deny.toml` cover licences.
- **No Windows or macOS CI** for the compiler; `release.yml` builds all three only
  when a tag is pushed.
