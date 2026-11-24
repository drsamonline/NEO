# Changing the language

`CONTRIBUTING.md` is the operational guide: how to build, which checks to run, what a pull
request looks like. This chapter covers the other half — **how a change to the language itself
gets decided**, and what has to be written down before it is made.

## There is no separate RFC process

NOVA does not have an RFC series, a proposal numbering scheme, or a committee. What it has is
**architecture decision records**: one file per decision that was hard to make and would be
expensive to unmake, in [`docs/adr/`](../adr/).

That is the RFC equivalent, and it is deliberately smaller:

| An RFC series | An ADR |
|---|---|
| written before the work | written when the decision is made — before or during |
| reviewed by a body | reviewed in the pull request that carries it |
| numbered and archived forever | numbered, and superseded rather than amended |
| describes a proposal | records what was chosen **and what lost** |

The last row is the point. An ADR that lists only the choice is a changelog entry; the value is
in the options that were rejected and why, because that is what stops the same debate happening
again in six months.

## The format

```
# ADR NNNN — title
- Status: proposed | accepted | superseded by NNNN
- Date: YYYY-MM-DD
- Supersedes: what this replaces, if anything

## Context      — the problem, and what is being treated as one problem but is two
## Options      — each with the reason it lost
## Decision     — what was chosen
## Consequences — including what this makes worse
```

`docs/adr/README.md` holds the format and the index. There are seventeen.

## When a decision needs an ADR

Write one when the change is **visible to a program that already exists**, or when undoing it
later would be expensive:

- a new type, or a change to one that is already emitted — [`0011`](../adr/0011-int-is-64-bit.md)
  made `int` 64-bit, which changed the size of every `[int]`;
- a new runtime mechanism — [`0010`](../adr/0010-file-handles.md) introduced the handle table;
- a public format — [`0014`](../adr/0014-serialize-is-json.md) chose JSON for `serialize`;
- a change to the entry point — [`0012`](../adr/0012-entry-point-arguments.md).

Do not write one for a bug fix, a test, a documentation correction, or anything reversible in a
single commit. An ADR per commit is a diary, not a decision record.

## The constitution outranks the process

[`docs/specs/constitution.md`](../specs/constitution.md) is eight rules that no proposal may
override. The ones that decide arguments most often:

1. **Never invent completed functionality.** A chapter, a table or a status page may not
   describe surface that does not exist.
2. **A milestone is done when its acceptance check runs and passes.** Not when the code looks
   right.
3. **A backend that cannot express something rejects it.** A diagnostic, not code that compiles
   and misbehaves.
4. **Numbers come from a command.** Every figure in a document was produced by something that
   ran.
5. **Say what you did not verify.**
6. **A flaky check is worse than no check.**

A proposal that requires breaking one of these is not a proposal to discuss; it is a proposal to
change the constitution first, which is a separate and much larger decision.

## The path a change takes

1. **An issue**, unless it is a typo or a one-line fix. Describe what you intend and why.
2. **A decision**, if the change needs one — an ADR in the same pull request, not a later one.
3. **A branch from `origin/master`**, not from a stale local branch.
4. **The gates**, all nine of them, locally. None needs a C toolchain.
5. **A pull request** using the template, which asks which layer the change touches and whether
   the checklist was run.
6. **A rebase merge.** `CONTRIBUTING.md` does not state a merge policy; every merged pull
   request in this repository has a rebase, so the individual commits survive and the history
   stays linear. Recorded here as the observed shape rather than as a rule.

## What the checklist exists for

The template's "Which layer" question is not bookkeeping. A change to the lexer, the lowerer and
the C backend has three chances to disagree about one type string, and that disagreement is the
single most common defect in this project's history. Saying which layers were touched is how a
reviewer knows where to look.

## Changing the standard library

The standard library is the most-tested part of the project: 248 of its 249 functions are usable
and 248 have an assertion. A change there is expected to come with a test that fails without it,
in `compiler/tests/stdlib.rs`.

`tools/stdlib-check.py` holds the README's table to the code, so adding a function means updating
both in the same commit or the gate fails.

## Proposing something the language cannot express yet

Ten chapters of this book are unwritten because the features they describe do not exist. That is
the intended state, not an oversight: a chapter describing an ownership model NOVA does not have
would be rule 1 violated in the most expensive possible way — a reader would trust it.

If you want one of those features, the sequence is: an issue describing the problem, an ADR
choosing a design, the implementation, and only then the chapter. **Documentation follows the
code, never the other way round.**
