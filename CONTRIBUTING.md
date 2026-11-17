# Contributing to NOVA

Read `AGENTS.md` and `DEV_PLAN.md` first. `DEV_PLAN.md` is the status source of
truth: if it and the code disagree, the code wins and the plan gets fixed.

## The one rule

**Do not claim a milestone is done until its acceptance check runs and passes.**
That check is named in the milestone table in `DEV_PLAN.md`. A green local build
is not the check; the check is.

## Before you start

- Open an issue describing what you intend to change, unless it is a typo or a
  one-line fix.
- Check `DEV_PLAN.md` "Milestones, in order". Work happens in that order unless
  an issue says otherwise.
- Check `DEV_PLAN.md` "Known limitations". Several things that look like bugs are
  documented, deliberate gaps. Fixing one of those is a real contribution — say
  so in the issue.

## Setting up

```bash
git clone https://github.com/drsamonline/NEO.git
cd NEO
cargo build --workspace
cargo test --workspace
```

You also need, for the parts that use them:

| Tool | Needed for | Install |
|---|---|---|
| a C compiler (`cc`, `gcc`, `clang`) | `build`, `run`, and the C backend tests | your package manager |
| `node` | executing generated JavaScript, and the WASM host | <https://nodejs.org> |
| `wat2wasm` | the WASM target and `test --parity` | `npm install --global wat2wasm` |

`nova-compiler doctor` tells you which of these it can find.

## The checks that must pass

Run these before you open a pull request. CI runs the same set.

```bash
python3 tools/grammar-check.py      # the grammar still describes the parser
python3 tools/example-check.py      # every examples/**/*.nova is a NOVA program
cargo build --workspace --all-targets
cargo test --workspace
for f in examples/**/*.nova; do ./target/debug/nova-compiler run "$f" > /dev/null; done
BENCH_MAX_RATIO=1.5 ./benches/run.sh 5
```

If your change touches a backend, also run the parity harness over the corpus:

```bash
for f in examples/*.nova tests/parity/*.nova; do
  ./target/debug/nova-compiler test --parity "$f"
done
```

## Where a change goes

| You changed | Add a test in |
|---|---|
| the lexer, parser, semantic analyser or lowerer | the `#[cfg(test)]` module at the bottom of that file |
| the C backend, or anything end to end | `compiler/tests/end_to_end.rs` |
| the standard library | `compiler/tests/stdlib_parse.rs` |
| a whole pipeline path, source to emitted code | `integration_tests/lib.rs` |
| the JavaScript, Rust or WASM backend | that crate's tests, plus a program in `tests/parity/` if it should agree across targets |
| the language server | `lsp/src/` unit tests, and `lsp/tests/diagnostics_stdio.rs` for the wire protocol |
| the debugger or the package manager | that crate's tests |

A feature without a test at the layer it lives in is not finished.

## Commits

Conventional Commits: `type(scope): imperative subject`, 72 characters or less,
no trailing full stop.

```
feat(lowerer): lower value-position match with a binding
fix(backend-c): bounds-check array writes on struct fields
docs(security): add a disclosure policy
test(parity): add a growable-array program
chore(deps): bump lsp-types to 0.94.1
```

Types in use: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`, `ci`.
Use `BREAKING CHANGE:` in the footer when a language or CLI change breaks
existing programs, and say which programs.

## Documentation

`README.md`, `CHANGELOG.md`, `DEV_PLAN.md` and `compiler/README.md` must stay
consistent with the code. If your change makes a number in one of them wrong,
fix the number in the same commit. Counts in the README are `wc -l` output —
regenerate them, do not estimate them.

Add user-facing changes to the `[Unreleased]` section of `CHANGELOG.md`.
`[Unreleased]` always sits above the newest release.

## Adding a language feature

1. Write the grammar production in `grammar/grammar.ebnf`. `grammar-check.py`
   fails until the parser agrees with it.
2. Lexer, if new tokens are needed.
3. Parser.
4. Semantic analysis — including the error case.
5. Lowering to SIR.
6. Every backend that can support it. A backend that cannot must **reject** the
   program in `validate` with a message naming the unsupported feature; it must
   never emit a placeholder that compiles and misbehaves.
7. A parity program in `tests/parity/` if the feature should agree across
   targets.
8. `DEV_PLAN.md` — move the limitation out of "Known limitations" if it was one.

## Style

- `rustfmt` defaults. `cargo fmt --all` before you commit.
- `clippy` clean where you touched the code.
- Comments explain *why*, not *what*. The existing code is dense with
  why-comments; match that, and skip the ones that restate the line.
- `unsafe` in `runtime/` needs a `// SAFETY:` comment on the block saying what
  invariant makes it sound.

## Reporting a bug

Use the issue template. Include the NOVA source, the command you ran, what you
expected, and what happened. A four-line program that reproduces it is worth more
than a description of a large one.

For a **security** issue, do not open an issue — see `SECURITY.md`.

## Licence

By contributing you agree your work is licensed under the MIT licence in
`LICENSE`. No contributor licence agreement is required.
