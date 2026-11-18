# Documentation index

Everything written down, and where to start.

## Start here

| If you want to | Read |
|---|---|
| see what this project is | [`README.md`](../README.md) |
| build it and run a program | [`website/docs/getting-started.md`](../website/docs/getting-started.md) |
| know what the language accepts | [`website/docs/language-tour.md`](../website/docs/language-tour.md), then [`grammar/grammar.ebnf`](../grammar/grammar.ebnf) |
| know what is done and what is next | [`DEV_PLAN.md`](../DEV_PLAN.md) — the only plan |
| contribute | [`CONTRIBUTING.md`](../CONTRIBUTING.md) |
| understand the design | [`ARCHITECTURE.md`](../ARCHITECTURE.md) |
| understand a diagnostic | [`errors.md`](errors.md) |
| know what each CI check guards | [`tooling.md`](tooling.md) |
| report a vulnerability | [`SECURITY.md`](../SECURITY.md) |

## Reference

| Document | What it is |
|---|---|
| [`errors.md`](errors.md) | Every error code the compiler emits, the `--error-format=json` shape, and `explain` |
| [`tooling.md`](tooling.md) | The eleven workflows, the six Python checks and the scripts, and what each would catch |
| [`compiler.md`](compiler.md) | The compiler's stages, one paragraph each |
| [`architecture.md`](architecture.md) | The design at a systems level |
| [`language.md`](language.md) | The language's shape, and a pointer to the chapters |
| [`language/README.md`](language/README.md) | The specification's status: 1 of 25 chapters written, and why the rest are stubs |
| [`language/00_overview.md`](language/00_overview.md) | The one written chapter |
| [`RUNTIME_PROGRESS.md`](RUNTIME_PROGRESS.md) | The Rust runtime crate: what it provides and what it does not |

## Decisions

| Document | Decision |
|---|---|
| [`adr/README.md`](adr/README.md) | How decisions are recorded here |
| [`adr/0001-lossless-syntax-tree.md`](adr/0001-lossless-syntax-tree.md) | A lossless tree at item boundaries with deferred bodies, instead of a full CST migration or nothing |

## Contracts

| Document | The contract |
|---|---|
| [`BACKEND_API.md`](../BACKEND_API.md) | What a code generator must provide, and the rule that it rejects rather than emitting a placeholder |
| [`LSP_ARCHITECTURE.md`](../LSP_ARCHITECTURE.md) | How the language server works, and what its navigation does not do |
| [`PACKAGE_ARCHITECTURE.md`](../PACKAGE_ARCHITECTURE.md) | What the package manager is, and that its registry is not implemented |
| [`COMPATIBILITY.md`](../COMPATIBILITY.md) | What a program may rely on across versions |
| [`EDITOR_INTEGRATION.md`](../EDITOR_INTEGRATION.md) | Two ways to connect an editor, with and without LSP |

## Project

| Document | What it is |
|---|---|
| [`FAQ.md`](../FAQ.md) | The questions that come up, answered plainly |
| [`VISION.md`](../VISION.md) | Three lines of intent |
| [`AGENTS.md`](../AGENTS.md) | Rules for anyone — human or agent — making changes |
| [`specs/constitution.md`](specs/constitution.md) | The non-negotiables this project holds itself to |
| [`PRIVACY.md`](../PRIVACY.md) | What the landing page and PWA do with data (nothing) |
| [`CODE_OF_CONDUCT.md`](../CODE_OF_CONDUCT.md) | How people are expected to behave here |

## History

Superseded, kept because the history is cheap to keep and expensive to reconstruct.

| Document | What it is |
|---|---|
| [`../MILESTONES.md`](../MILESTONES.md) | The original 12-milestone narrative; its counts are from an early snapshot and are wrong |
| [`../IMPLEMENTATION_PLAN.md`](../IMPLEMENTATION_PLAN.md) | An 8-phase plan from before the backends worked |
| [`../PHASE2_SUMMARY.md`](../PHASE2_SUMMARY.md) | An accurate description of the runtime crate at one point in time |
| [`../FILES_CREATED.md`](../FILES_CREATED.md) | A session log from an early compiler session |
| [`../ARCHIVED.md`](../ARCHIVED.md) | Files moved out of this repository, and where they went |
| [`../CHANGELOG.md`](../CHANGELOG.md) | What changed in each release |

## Component READMEs

Each directory that holds something explains itself in its own README.

| Directory | What it holds |
|---|---|
| [`compiler/`](../compiler/README.md) | The compiler crate: stages, tests, how to build it |
| [`backends/`](../backends/README.md) | The three non-C generators, and what a backend must provide |
| [`backends/javascript/`](../backends/javascript/README.md) · [`backends/rust/`](../backends/rust/README.md) · [`backends/wasm/`](../backends/wasm/README.md) | One per target, with its supported subset |
| [`runtime/`](../runtime/README.md) | The Rust runtime crate — and that it is not linked into generated C |
| [`stdlib/`](../stdlib/README.md) | The standard library, and the four modules: [`core`](../stdlib/core/README.md) · [`io`](../stdlib/io/README.md) · [`math`](../stdlib/math/README.md) · [`text`](../stdlib/text/README.md) |
| [`crates/nova-common/`](../crates/nova-common/README.md) | Shared AST, tokens, spans, diagnostics and SIR |
| [`examples/`](../examples/README.md) | Five programs, all built and run in CI |
| [`benches/`](../benches/README.md) | Generated-code and compiler-speed benchmarks |
| [`integration_tests/`](../integration_tests/README.md) | Cross-crate pipeline tests, a workspace member |
| [`tests/parity/`](../tests/parity/README.md) | Programs that must agree across all four backends |
| [`tests/diagnostics/`](../tests/diagnostics/README.md) | The eight diagnostic goldens and the JSON contract |
| [`tests/integration/`](../tests/integration/README.md) · [`tests/unit/`](../tests/unit/README.md) | Signposts: the tests live elsewhere, and these say where |
| [`compiler/tests/`](../compiler/tests/README.md) · [`compiler/benches/`](../compiler/benches/README.md) | Signposts for the same reason |
| [`fuzz/`](../fuzz/README.md) | The three cargo-fuzz targets and how to seed them |
| [`vscode-extension/`](../vscode-extension/README.md) | The editor extension: install, settings, limitations |
| [`website/`](../website/index.md) | The docs site's own landing page |

## Keeping this true

`tools/docs-check.py` fails the build if a document is a stub, if a document under
`docs/` is unreachable from this index, or if a workflow or tool has no explanation.
The index is the root of the reachability check, so a new page that nobody links is
a red build rather than a page nobody finds.
