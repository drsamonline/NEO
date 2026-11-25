# Language server architecture

`nova-lsp` is a `tower-lsp` server over stdio. It has no network surface, no daemon
mode and no state beyond the documents the editor has opened.

## Shape

```
editor ──JSON-RPC/stdio──▶ tower_lsp::LspService
                              │
                              ├─ documents: RwLock<HashMap<Url, Document>>
                              ├─ compiler_integration: front end + diagnostic cache
                              └─ navigation: tokens → spans
```

`Document` holds the text, the version and the last diagnostics. Nothing is cached
across a restart, and nothing is persisted.

## Sync model

**FULL**, not incremental. `did_change` applies the editor's change as the
document's new whole text. That is a deliberate simplification: incremental sync
means applying ranges to text, and getting that wrong corrupts the document
silently. The cost is re-lexing on every keystroke, which is cheap at the file
sizes this language is aimed at.

## Diagnostics

The server runs the same front end the CLI does and publishes what it produces.
Codes and `help:` lines come through because the compiler attaches them.

`--error-format=json` exists so this server *can* consume a machine-readable
diagnostic instead of re-deriving one. It does not yet — that is the one line of
`DEV_PLAN.md` Stage 2 not crossed, and it belongs with the next round of LSP work.

## Navigation

Both handlers work on the compiler's own tokens:

- `definition` — the identifier under the cursor → its declaration's span. Current
  document first, then every other open document.
- `references` — every identifier token with that text, sorted, de-duplicated,
  `includeDeclaration` honoured.

**Not a resolver.** Two identifiers with the same text are one symbol, so shadowing
is not distinguished. `docs/adr/0001` says what would fix it.

## Completion and hover

Both exist, and both are narrower than the names suggest.

**`completion`** offers the language's keywords and built-in types, **plus the names the current
document declares** — read from the same deferred item tree the outline uses, so a function body is
never parsed to complete a name. It does not offer the standard library and does not filter by the
prefix already typed; the client does the filtering.

Until 2026-11-26 it returned the keyword list and nothing else: it bound `uri` and `position` and
used neither, so "autocomplete" offered `fn` and `let` in a file whose own functions were invisible.
A capability advertised and answered with a constant is the failure mode this repository keeps
finding, and it is why the handler now reads the document.

**`hover`** extracts the identifier under the cursor and answers `Symbol: \`name\`` — the name, not
a type or a doc comment. It is a name-at-cursor, and calling it hover *info* would be generous.

## Outline

`documentSymbol` uses `compiler/src/syntax` — the item tree — rather than scanning
lines. Top-level items come from one pass over the tokens; an `impl`'s methods appear
as children only because the outline asks, which expands that body on demand. This
is the deferred-body pattern working end to end.

## Failure behaviour

If the compiler cannot start or a document cannot be read, the server returns `null`
for that request rather than failing the session. A language server that dies takes
the editor's diagnostics with it.

## Testing

`lsp/tests/diagnostics_stdio.rs` and `lsp/tests/navigation_stdio.rs` spawn the real
binary and speak JSON-RPC to it, because the contract under test *is* the wire
protocol. That also means `cargo llvm-cov` cannot see the server code — it
instruments the test binary, not the child — which is why the coverage floor is
lower than the tests deserve. `coverage-floor.txt` records that.
