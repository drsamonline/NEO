# NOVA Language for VS Code

Syntax highlighting, diagnostics and navigation for NOVA.

Everything except highlighting is answered by `nova-lsp` over stdio. The extension
finds that binary, starts it, and reports clearly when it cannot.

## Install from source

```bash
git clone https://github.com/drsamonline/NEO.git
cd NEO
cargo build --release -p nova-lsp          # produces target/release/nova-lsp
cd vscode-extension
npm install
npm run compile
```

Then either put `target/release/nova-lsp` on your `PATH`, or set
`nova.server.path` to its absolute path in VS Code settings.

## What it does

| Feature | From |
|---|---|
| Syntax highlighting | `syntaxes/nova.tmLanguage.json`, generated from `crates/nova-common/src/token.rs` |
| Bracket matching, comments, auto-closing | `language-configuration.json` |
| Diagnostics with codes and `help:` lines | `nova-lsp` |
| Go to definition | `nova-lsp` |
| Find all references | `nova-lsp` |

## What it does not do

Navigation is **token-level**: two identifiers with the same text are treated as one
symbol, so shadowing in nested scopes is not distinguished. That needs a lossless
syntax tree with a scope map, which is the Stage 4 work in `DEV_PLAN.md` and the
same thing a formatter needs.

The formatter exists (`nova-compiler fmt`) but is not wired to save: it normalises
the space between top-level items and changes nothing inside a body. Wiring it to
`editor.formatOnSave` is a small addition, not a missing capability.

## Settings

| Setting | Default | Meaning |
|---|---|---|
| `nova.server.path` | `nova-lsp` | The language server binary. A bare name is looked up on `PATH`. |
| `nova.trace.server` | `off` | JSON-RPC traffic, in the "NOVA Language Server" output channel. |

## Known limitations

The grammar covers the syntax the parser accepts; it is not a validator, and a
wrong-coloured token is cosmetic. Diagnostics come from the compiler and are the
authority.
