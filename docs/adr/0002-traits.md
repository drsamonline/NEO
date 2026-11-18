# ADR 0002 — traits are not in the language, and the AST stops saying they are

- **Status:** accepted
- **Date:** 2026-11-18
- **Supersedes:** nothing; closes the open question recorded in ADR 0001

## Context

`crates/nova-common/src/ast.rs` had an `Item::Trait(TraitDef)` variant, a `TraitDef`
struct with `name`, `generics`, `methods`, `is_pub` and `span`, and three consumers
that matched on it: an empty arm in `sema`, a `None` arm in module loading, and a
symbol-emitting arm in the language server.

None of them could ever run.

- `token.rs` has no `Trait` variant, so `trait` lexes as an ordinary identifier.
- `grammar/grammar.ebnf` has no production for it.
- `compiler/src/parser/mod.rs` contains no reference to traits at all.

So the AST described a language feature that the compiler cannot parse, cannot check
and cannot emit. `website/docs/language-tour.md` said *"No traits, though the parser
accepts `trait` blocks"* — which was wrong twice: there are no traits, and the parser
does not accept the blocks.

## Decision

Remove `Item::Trait`, `TraitDef`, and every arm that matched on them. `trait` remains
an ordinary identifier, and a file containing one produces the ordinary diagnostics
any other unknown identifier would.

## The rule

**An AST node the parser cannot construct is a lie.** It is the same failure as a
stub document or a mock registry: something in the repository claims a capability
that does not exist, and the claim is discovered only by someone who tries to use it.

`constitution.md` rule 1 says never invent completed functionality. A dead AST
variant is that rule's quietest violation, because no test fails and no user sees an
error — they see a type in the docs and assume.

## Precedent

Compilers that keep "reserved for later" syntax nodes generally regret it. `rustc`
does the opposite: unstable syntax sits behind a feature gate so that the parser
*does* understand it and rejects it **loudly**, with a diagnostic naming the gate.
The choice is between parsing something and refusing it clearly, and not parsing it
at all. What is never right is a type that pretends the middle option exists.

## Consequences

- **Nothing was lost.** The `sema` arm was empty, the module-loading arm returned
  `None`, and the language-server arm was unreachable. Deleting them removed no
  behaviour.
- **A false claim is gone** from the language tour and from the AST's own surface.
- **`trait` is a plain identifier**, so `trait Show { }` produces "undefined name
  `trait`" rather than a confusing partial parse.

## When traits are added

All of it lands together, in one change:

1. `TokenKind::Trait` and its lexeme, which makes `tools/extension-check.py` fail
   until the TextMate grammar knows the word — the check exists for exactly this.
2. A production in `grammar/grammar.ebnf`, so `tools/grammar-check.py` keeps the
   grammar describing the parser.
3. Parser support, and `Item::Trait` returning with something that constructs it.
4. `sema` checking it, and at least one backend emitting it — or a `validate` that
   rejects it, which is the honest alternative.
5. A program in `tests/parity/`, so all four backends are held to it.
6. `compiler/tests/item_tree.rs` will then require the item tree to report traits
   too, because the guard compares the two descriptions.

## Known gap

`docs/language/16_interfaces.md` is still a stub describing interfaces. It carries
the "not written yet" banner, so it claims nothing — but the chapter is a plan, and
this ADR does not create the feature it plans.
