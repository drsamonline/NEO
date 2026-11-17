## What this changes

<!-- One sentence. If it fixes an issue, say which. -->

## Why

<!-- The reason, not a restatement of the diff. -->

## Which layer

- [ ] lexer / parser / sema / lowerer
- [ ] C backend
- [ ] JavaScript / Rust / WASM backend
- [ ] runtime crate
- [ ] stdlib
- [ ] pkg / lsp / dbg
- [ ] docs / CI only (no code)

## Checklist

- [ ] `python3 tools/grammar-check.py` passes
- [ ] `python3 tools/example-check.py` passes
- [ ] `cargo build --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] Every `examples/**/*.nova` still builds and runs
- [ ] If a backend changed: `test --parity` passes over `examples/**` and `tests/parity/**`
- [ ] A test was added at the layer this change lives in
- [ ] `README.md` / `DEV_PLAN.md` / `CHANGELOG.md` updated if a number, a limitation
      or a user-facing behaviour changed
- [ ] If this completes a milestone: its acceptance check in `DEV_PLAN.md` ran and
      passed, and the milestone is ticked

## If this touches a language feature

- [ ] `grammar/grammar.ebnf` updated and `grammar-check.py` agrees
- [ ] A backend that cannot support it **rejects** it in `validate` with a message
      naming the feature — it does not emit a placeholder
- [ ] A parity program added if the feature should agree across targets

## Not verified

<!-- Say what you could not run, and why. A pull request that hides this is worse
     than one that admits it. -->
