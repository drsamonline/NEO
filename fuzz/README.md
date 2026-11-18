# Fuzzing

Three `cargo-fuzz` targets over the compiler's front end. `fuzz.yml` runs them on
every push, weekly for ten minutes.

| Target | Reaches | Also asserts |
|---|---|---|
| `lex` | the lexer | every diagnostic has a non-empty message and a 1-based span |
| `parse` | lexer + parser | — |
| `frontend` | source → SIR, and the item tree with every deferred body expanded | — |

## Seeding

```bash
bash fuzz/seed.sh
```

A random-byte fuzzer spends its entire budget failing to produce a valid program, so
the front-end target would never get past the parser. Seeding from `examples/` and
`tests/parity/` is what makes it find anything.

## Running

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run lex -- -max_total_time=60
```

`+nightly` is required: cargo-fuzz needs `-Zsanitizer=address`, which is what turns a
memory error in the compiler into a crash rather than a wrong answer.

## The crate is detached from the workspace

`fuzz/Cargo.toml` declares its own `[workspace]`, and the root `Cargo.toml` excludes
it. The fuzzer needs its own lockfile and its own sanitizer profile, and neither
should leak into the build everyone else uses.

## A reproducer worth keeping is a test, not an artifact

`fuzz/artifacts/` is gitignored. If a crash is worth preventing, turn it into a named
case in `tests/diagnostics/` or `tests/parity/` where it is reviewed, diffed and
named — an opaque blob under `artifacts/` is a bug that has been filed rather than
fixed.

## Status

Advisory in CI, on purpose. This compiler had never been fuzzed, so the first runs
were expected to find things. The first run did not: ~1.04 million executions across
180 seconds, no crashes. It becomes blocking once a weekly 10-minute run is clean —
`DEV_PLAN.md` states that condition.
