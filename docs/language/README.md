# The NOVA language specification — status

**This specification is mostly unwritten.** Fifteen of the twenty-five chapters
below are a title and one sentence. The landing page calls this "25 chapters
covering grammar, types, memory, concurrency, packages and the semantic IR"; that
is the plan, not the state.

Ten are written, and they were written from the code rather than from intent:
[the overview](00_overview.md), [lexical grammar](01_lexical_grammar.md),
[tokens](02_tokens.md), [keywords](03_keywords.md),
[identifiers](04_identifiers.md), [literals](05_literals.md), [types](06_types.md),
[functions](07_functions.md), [modules](08_modules.md) and
[control flow](09_control_flow.md). Each one names the file it was derived from, so a
reader can check it.

What exists today, and what you should read instead:

| You want | Read |
|---|---|
| the syntax the parser actually accepts | [`grammar/grammar.ebnf`](../../grammar/grammar.ebnf) — 58 productions, checked against `compiler/src/parser/mod.rs` by `tools/grammar-check.py` in CI |
| what is implemented and what is not | [`DEV_PLAN.md`](../../DEV_PLAN.md), especially "Known limitations" |
| working programs | [`examples/`](../../examples/) — every file there builds and runs in CI |
| the design of the compiler | [`ARCHITECTURE.md`](../../ARCHITECTURE.md) |
| a walkthrough of the features that work | [`website/docs/language-tour.md`](../../website/docs/language-tour.md) |

## Chapter status

`—` means the file is a stub. A stub carries a banner pointing back to this page.

| # | Chapter | State |
|---|---|---|
| 00 | [Overview](00_overview.md) | ✅ written |
| 01 | [Lexical grammar](01_lexical_grammar.md) | ✅ written |
| 02 | [Tokens](02_tokens.md) | ✅ written |
| 03 | [Keywords](03_keywords.md) | ✅ written |
| 04 | [Identifiers](04_identifiers.md) | ✅ written |
| 05 | [Literals](05_literals.md) | ✅ written |
| 06 | [Types](06_types.md) | ✅ written |
| 07 | [Functions](07_functions.md) | ✅ written |
| 08 | [Modules](08_modules.md) | ✅ written |
| 09 | [Control flow](09_control_flow.md) | ✅ written |
| 10 | [Ownership](10_ownership.md) | — |
| 11 | [Memory](11_memory.md) | ✅ written |
| 12 | [Errors](12_errors.md) | — |
| 13 | [Concurrency](13_concurrency.md) | — |
| 14 | [Attributes](14_attributes.md) | — |
| 15 | [Generics](15_generics.md) | ✅ written |
| 16 | [Interfaces](16_interfaces.md) | — |
| 17 | [Packages](17_packages.md) | — |
| 18 | [Domain packs](18_domain_packs.md) | — |
| 19 | [Semantic model](19_semantic_model.md) | — |
| 20 | [SIR](20_sir_intro.md) | ✅ written |
| 21 | [Style guide](21_style_guide.md) | — |
| 22 | [Best practices](22_best_practices.md) | — |
| 23 | [Glossary](23_glossary.md) | ✅ written |
| 24 | [RFC process](24_rfc_process.md) | — |

## Why the stubs are still here

They are listed in the service worker's cache manifest (`docs/sw.js`), so
deleting one makes `cache.addAll` reject and the offline site stops installing.
Remove the file and its manifest entry in the same change, or fill the file.

## Writing a chapter

A chapter is finished when every claim in it can be checked against
`grammar/grammar.ebnf` or a passing test. A chapter that describes syntax the
parser lacks is worse than a stub, because a reader will trust it. The rule in
`AGENTS.md` applies: never invent completed functionality.
