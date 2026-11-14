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

### 3. Backends
- **C Backend**: portable C linked with the ABI v3 runtime shim (reference target)
- **JavaScript Backend**: executable JavaScript; arrays and bound-environment closures
- **Rust Backend**: executable Rust; `Vec<i32>` arrays and `Rc<dyn Fn(..)>` closures over generated environment structs
- **WebAssembly Backend**: WAT assembled with `wat2wasm`; growable int arrays, structs, tuples and `int` closures through a `funcref` table

Line counts are omitted because they drift; `DEV_PLAN.md` and the parity harness are authoritative for coverage.

### 4. Runtime System (Rust crate; not linked into generated programs yet)
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

**Last Updated**: 2026-11-14
**Current stage**: Phase 1 complete, Phase 3 partial, Phase 4 in progress (see `DEV_PLAN.md`)
**Test suite**: 184 passing
