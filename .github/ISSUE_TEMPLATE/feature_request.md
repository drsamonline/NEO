---
name: Feature request
about: A language feature, a backend capability, or a tooling gap
title: ""
labels: enhancement
assignees: ""
---

<!--
Before filing: read DEV_PLAN.md. Milestones are worked in the order listed there,
and "Known limitations" already names several gaps that are deliberate rather
than forgotten. If your request is on that list, say so — it helps to know which
gap someone actually needs.
-->

## What you want to be able to write

```nova
// the program you wish compiled
```

## Which layer

- [ ] language (grammar, lexer, parser, sema, lowerer)
- [ ] a specific backend — which: c / js / rust / wasm
- [ ] standard library
- [ ] package manager
- [ ] language server
- [ ] debugger
- [ ] docs

## Why it matters

What you cannot do today because of the gap. A real program you are trying to
write is the strongest case.

## Does it exist in DEV_PLAN.md?

- [ ] Yes, as a milestone — number:
- [ ] Yes, under "Known limitations"
- [ ] No, this is new

## If it is a language feature

A backend that cannot support a feature must **reject** the program with a message
naming it, never emit a placeholder that compiles and misbehaves. Is that
acceptable for the backends you care about, or does the feature need all four?
