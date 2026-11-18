# ADR 0001 — the syntax tree: hybrid, not either/or

- **Status:** accepted
- **Date:** 2026-11-18
- **Supersedes:** the "lossless CST or explicitly defer" fork in `DEV_PLAN.md` Stage 4

## Context

Two problems were being treated as one.

**Navigation is textual.** Go-to-definition and find-all-references work on
identifier tokens, so two names that happen to match are one symbol. Shadowing is
not resolved, a field is not told apart from a local, and rename would be unsafe.

**There is no formatter, and cannot be one.** The AST is lossy: comments,
whitespace and the exact spelling of a literal are not in it, so nothing can
reproduce the source text it came from. `nova fmt` needs to rewrite a file while
preserving everything it did not mean to change, and there is nothing to preserve
from.

Stage 4 was written as a choice between a full `rowan` migration and declining. Both
are worse than the third option.

## Decision

Build a **lossless tree at item boundaries, with bodies deferred**, and expand a
body only when something asks.

- The top level of a file is mapped exactly: every item's kind, name, the name's
  own span, and the span of the whole item.
- An item's body is recorded as a `Span` into the source and **nothing else**.
- Expanding a body re-walks the tokens already lexed for that span. No second lex,
  no partial tree to keep consistent.

The first half is implemented: `compiler/src/syntax/mod.rs`. The outline in the
language server is its first consumer.

## The invariant

**A deferred node holds a span into the source and nothing else.** Not a partial
tree, not a normalised string, not an interpretation of what is inside.

This is the whole reason the hybrid is sound. A deferral that stores anything
derived is lossy, and a lossy deferral can never be expanded correctly later —
you would have thrown away the text you needed. Storing only a span is also what
makes it cheap: a body costs eight bytes and a walk, not a tree.

## What this is not

**Not error recovery.** A deferred body is text nobody has looked inside; a
recovery node is text that failed to parse and was skipped. Conflating them is how
a formatter reformats garbage and an editor offers navigation into it. `ItemTree`
never produces a recovery node, because it never parses and so cannot fail.

Recovery is a separate problem with its own cost — the parser currently produces
three cascading errors for one missing initialiser, and that is pinned by a
diagnostic golden rather than fixed.

## Precedent

This is not novel, and it should not be built from scratch where an upstream
answer exists:

| Project | Mechanism | What it shows |
|---|---|---|
| IntelliJ platform | *chameleon* nodes (`LazyParseableElement`) | Unparsed subtrees wrapped in a node that parses itself on first access. The closest match to this design, in production for two decades. |
| rust-analyzer | `rowan` CST + a separate `ItemTree` | The full lossless tree for editing, plus a coarse item index for cheap crate-wide queries. Bodies are lowered lazily per function. |
| rustc | lazy token streams (`LazyAttrTokenStream`) | Tokenization itself deferred until something reads the tokens. |
| Roslyn (C#) | full-fidelity syntax trees + `SkippedTokensTrivia` | Lossless tokens with skipped text kept as trivia — recovery *and* losslessness at once. |
| tree-sitter | `ERROR` and `MISSING` nodes, incremental reparse | Lossless CST with explicit recovery nodes, and re-parsing limited to the changed range. |
| libSyntax (Swift) | lossless syntax tree over a lossy AST | Both layers, kept in step. |

The `rowan` crate remains the likely destination for the full tree. The item tree is
deliberately hand-written and dependency-free so it can land now, be used now, and
be replaced later without touching its consumers.

## Consequences

**What it buys, immediately**

- File-level indexing — outline, folding, workspace symbols — from one pass over
  the tokens, with no expression parsed.
- Exact ranges instead of whole lines. The outline test asserts `struct Pair`'s
  selection range is `7..11`, not the line.
- An `impl`'s methods available as children on demand, which is the pattern
  working end to end.
- A place for the formatter to stand: item boundaries are known and lossless
  today; the gap to a full formatter is the *inside* of bodies.

**What it does not buy**

- **It does not make type-checking faster.** `sema` needs bodies, so full
  diagnostics still expand everything. The laziness is real only for features that
  can decline to look inside: outline, folding, symbols, and eventually a
  formatter that rewrites item by item.
- **It does not fix navigation's precision.** Resolving scopes needs a scope map
  over a parsed body, which is the full-tree work, not this.
- **It is not the formatter.** Round-tripping is possible at item granularity; a
  formatter needs it inside every body too.

**Costs**

- One more walk over the tokens per file, on top of the parse. Linear, small, and
  only paid by features that want it.
- Two representations of "what is in this file" — the AST and the item tree — which
  must not disagree about what an item is. `tools/grammar-check.py` already guards
  the grammar against the parser; the item tree has no equivalent guard yet, and
  that is the open risk.

## Known gaps found while writing it

- **`trait` is not a keyword.** `token.rs` has no `Trait` variant, so `trait`
  lexes as an ordinary identifier and no token-level tool — including this one and
  the outline — can find a trait declaration. The AST has `Item::Trait`, so the
  parser handles it somewhere; the lexer does not. That is a language inconsistency
  to settle before the item tree can claim to cover the item kinds.
- **`Span.offset` is a character offset, not a byte offset**, because the lexer
  indexes a `Vec<char>`. Every consumer here wants line and column, which the same
  span carries, but anything that slices source bytes must not use `offset`.

## Next steps, in order

1. ✅ **The formatter's first milestone** — `compiler/src/fmt.rs`. Item separators,
   trailing whitespace and the final newline are normalised; bodies are byte-identical
   and the formatter never parses, so it is total. `compiler/tests/format.rs` checks
   the claim against every shipped program. The remaining half is the *inside* of a
   body, which is where `rowan` would come in.
2. **Guard the item tree against the parser**, the way `grammar-check.py` guards the
   grammar: for every `examples/**/*.nova`, the item kinds the tree reports must
   match the top-level `Item`s the parser produces. Without that, two
   representations will drift.
3. **Decide `trait`**: add the keyword, or drop `Item::Trait` and the parser's
   handling of it.
4. **Then, and only then**, consider `rowan` for the inside of bodies — with the
   item tree's consumers unchanged, because they never depended on how a body is
   represented.
