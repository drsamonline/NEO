# NOVA Architecture

## Overview

NOVA follows a multi-stage compilation pipeline with a semantic-first approach.

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│   Source    │────▶│   Frontend   │────▶│  SIR (IR)   │────▶│   Backend    │
│   (.nova)   │     │ (Lexer+Parser│     │(Semantic IR)│     │ (C/JS/Rust/  │
│             │     │  +Semantic)   │     │             │     │   Wasm)      │
└─────────────┘     └──────────────┘     └─────────────┘     └──────────────┘
                           │                    │                    │
                           ▼                    ▼                    ▼
                    ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
                    │     AST      │     │   Lowerer   │     │ Target Code  │
                    │              │     │             │     │              │
                    └──────────────┘     └─────────────┘     └──────────────┘
```

## Components

### 1. Frontend (~2,000 lines)
- **Lexer** (309 lines): Tokenization with Unicode support
- **Parser** (907 lines): Recursive descent with error recovery
- **Semantic Analyzer** (773 lines): Type checking, symbol tables, scope resolution

### 2. Middle End (~700 lines)
- **SIR** (146 lines): Semantic Intermediate Representation
- **Lowerer** (550 lines): AST to SIR conversion

### 3. Backends (~1,500 lines)
- **C Backend** (449 lines): Generates portable C code
- **JavaScript Backend** (343 lines): ES6+ JavaScript
- **Rust Backend** (352 lines): Safe Rust code
- **WebAssembly Backend** (385 lines): WASM modules

### 4. Runtime System (~1,400 lines) 🆕
- **Memory Management** (195 lines): Allocation, stack buffers
- **Type System** (276 lines): String, Array, Option types
- **Error Handling** (224 lines): Error types and handlers
- **FFI** (193 lines): C interoperability
- **Garbage Collection** (420 lines): RC-based GC with weak refs

## Data Flow

1. **Lexical Analysis**: Source → Tokens
2. **Parsing**: Tokens → AST
3. **Semantic Analysis**: AST + Symbol Table → Typed AST
4. **Lowering**: Typed AST → SIR
5. **Code Generation**: SIR → Target Language

## Key Design Principles

- **Semantic-First**: Type checking before code generation
- **Multi-Target**: Single IR, multiple backends
- **Modular**: Each phase is independent and testable
- **Runtime Support**: Optional runtime for advanced features

## Runtime Integration

Compiled NOVA programs can optionally link against the runtime system for:
- Automatic memory management (GC)
- Dynamic type operations
- Error handling and recovery
- Foreign function interface (FFI)
- I/O operations (future)

---

**Last Updated**: Phase 2 Active Development
**Total LOC**: ~5,000+ (Compiler: 3,500+, Runtime: 1,380, Backends: 1,529)
