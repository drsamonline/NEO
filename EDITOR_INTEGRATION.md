# Editor integration

Two ways in. Use the first if your editor speaks LSP; the second if it does not.

## 1. The language server

Build it:

```bash
cargo build --release -p nova-lsp    # target/release/nova-lsp
```

Run it as a stdio server. There is no socket, no port and no daemon: the editor
spawns it and speaks JSON-RPC over stdin/stdout.

| Capability | State |
|---|---|
| `textDocumentSync` | FULL — the server replaces whole documents, so it must not be sent incremental ranges |
| `publishDiagnostics` | real, from the compiler, with codes and `help:` lines |
| `definition` | exact identifier spans; current document first, then other open documents |
| `references` | exact identifier spans, `includeDeclaration` honoured |
| `documentSymbol` | from the item tree; an `impl`'s methods appear as children |
| `completion`, `hover` | advertised; the quality is not claimed |

**Navigation is token-level, not resolved.** Two identifiers with the same text are
treated as one symbol, so shadowing is not distinguished. `docs/adr/0001` explains
why and what would fix it.

VS Code users: `vscode-extension/` is a working client. Install from source —
`npm install && npm run compile` — then set `nova.server.path` to the binary.

## 2. Without LSP

`nova-compiler` is enough for diagnostics:

```bash
nova-compiler emit file.nova -o /dev/null --error-format=json
```

One JSON object per diagnostic on stdout, one per line, nothing on stderr. The shape
is documented in `docs/errors.md`. An editor plugin can parse that and put markers
on the exact spans — no server, no protocol.

For plain highlighting, `vscode-extension/syntaxes/nova.tmLanguage.json` is a
TextMate grammar whose keyword list is derived from `crates/nova-common/src/token.rs`
and checked against it by `tools/extension-check.py`. It is reusable in any TextMate
host, and its structure is a reasonable starting point for a tree-sitter grammar.

## What is not offered

- **No format-on-save integration.** `nova-compiler fmt` exists and is safe to run
  on any file — it never parses, so it works on half-typed code, and it changes no
  token. It re-indents to bracket depth and normalises the space between items, but
  it will not break a long line or reflow an expression. Wiring it to
  a save hook is a one-line task for whoever wants it; the CLI is
  `nova-compiler fmt <file>` and `--check` reports without writing.
- No rename, for the same reason navigation is not a resolver.
- No debug adapter. `nova-dbg` is a standalone REPL; it does not speak DAP.
