# NOVA

Semantic Programming Platform

[![Phase 1 Complete](https://img.shields.io/badge/Phase-1%20Complete-brightgreen)](DEV_PLAN.md)
[![Compiler Core](https://img.shields.io/badge/Compiler-3500%2B%20LOC-blue)](compiler/)
[![Backends](https://img.shields.io/badge/Backends-4%20Targets-orange)](backends/)

## Overview

NOVA is a statically-typed, compiled systems programming language with a complete tooling ecosystem. The compiler transforms NOVA source code into multiple target languages (C, JavaScript, Rust, WebAssembly) through a robust semantic analysis pipeline.

## Current Status

**✅ Phase 1: Compiler Core - COMPLETE**
- Full lexer, parser, semantic analyzer implemented
- SIR (Semantic Intermediate Representation) defined
- Lowerer converts AST to SIR
- 4 backends: C, JavaScript, Rust, WebAssembly

**🚧 Phase 2: Runtime System - NOT STARTED**
**🚧 Phase 3: Tooling - NOT STARTED**

See [Development Plan](DEV_PLAN.md) for detailed status.

## Quick Start

```bash
# Compile a NOVA program to C
cd compiler
# (Rust toolchain required)
cargo run -- ../examples/test_final.nova --target c
```

## Project Structure

```
/workspace
├── compiler/           # Main compiler (~3,500 LOC)
│   ├── src/lexer/     # Tokenization (309 lines)
│   ├── src/parser/    # Recursive descent parser (907 lines)
│   ├── src/sema/      # Semantic analysis (773 lines)
│   ├── src/sir/       # IR definition (146 lines)
│   ├── src/lowerer/   # AST→SIR lowering (550 lines)
│   └── src/backend/   # Backend interface
├── backends/          # Target code generators (1,529 lines)
│   ├── c/             # C backend (449 lines)
│   ├── javascript/    # JavaScript backend (343 lines)
│   ├── rust/          # Rust backend (352 lines)
│   └── wasm/          # WebAssembly backend (385 lines)
├── runtime/           # Runtime system (scaffold)
├── package-manager/   # Package management (scaffold)
├── language-server/   # LSP implementation (scaffold)
├── debugger/          # Debugger (scaffold)
└── crates/nova-common/ # Shared utilities
```

## Features

### Implemented ✅
- **Lexical Analysis**: Complete tokenization with Unicode support
- **Parsing**: Recursive descent with error recovery
- **Semantic Analysis**: Type checking, scope resolution, name resolution
- **Code Generation**: C, JavaScript, Rust, WebAssembly backends
- **Type System**: Functions, structs, enums, control flow

### Coming Soon 🚧
- Runtime system with memory management
- Standard library
- Package manager
- Language server (LSP)
- Integrated debugger
- FFI support

## Documentation

- [Development Plan](DEV_PLAN.md) - Current progress and roadmap
- [Architecture](ARCHITECTURE.md) - System design
- [Language Tour](website/docs/language-tour.md) - Language features
- [Getting Started](website/docs/getting-started.md) - Installation guide
- [FAQ](FAQ.md) - Common questions

## License

See LICENSE file in repository root.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

**Last Updated**: Real-time status tracking enabled
**Total Implementation**: ~5,000+ lines of production code
