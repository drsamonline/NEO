# NOVA Compiler Architecture

## Overview

NOVA follows a traditional multi-pass compiler architecture with semantic analysis at its core:

```
Source Code → Lexer → Parser → AST → Semantic Analyzer → SIR → Lowerer → Backend → Target Code
```

## Compilation Pipeline

### 1. Lexer (309 lines) ✅
**Location**: `compiler/src/lexer/mod.rs`

Transforms source code into tokens:
- Tokenizes keywords, identifiers, literals, operators
- Handles Unicode characters
- Produces span information for error reporting
- Strips comments

**Input**: Raw source text  
**Output**: Token stream with spans

---

### 2. Parser (907 lines) ✅
**Location**: `compiler/src/parser/mod.rs`

Recursive descent parser building AST:
- Expression parsing with operator precedence
- Statement and declaration parsing
- Error recovery for robust compilation
- Syntax validation

**Input**: Token stream  
**Output**: Abstract Syntax Tree (AST)

---

### 3. Semantic Analyzer (773 lines) ✅
**Location**: `compiler/src/sema/`

Type checking and name resolution:
- **checker.rs** (176 lines): Type system enforcement
- **resolver.rs** (130 lines): Name/scope resolution
- **types.rs** (96 lines): Type definitions
- **mod.rs** (369 lines): Main analyzer logic

Performs:
- Symbol table construction
- Scope management
- Type checking and inference
- Semantic error detection

**Input**: AST  
**Output**: Annotated AST with type information

---

### 4. SIR - Semantic Intermediate Representation (146 lines) ✅
**Location**: `compiler/src/sir/mod.rs`

Language-agnostic intermediate representation:
- Control flow instructions
- Data flow operations
- Type-preserving IR
- Optimization-ready structure

**Input**: Typed AST  
**Output**: SIR program

---

### 5. Lowerer (550 lines) ✅
**Location**: `compiler/src/lowerer/mod.rs`

Converts AST to SIR:
- Expression lowering
- Statement translation
- Control flow flattening
- Type annotation propagation

**Input**: Annotated AST  
**Output**: SIR with complete semantics

---

### 6. Backends (1,529 lines total) ✅

#### C Backend (449 lines)
**Location**: `backends/c/src/lib.rs`
- Generates portable C code
- Compatible with C99+
- Tested and production-ready

#### JavaScript Backend (343 lines)
**Location**: `backends/javascript/src/lib.rs`
- ES6+ JavaScript output
- Browser and Node.js compatible

#### Rust Backend (352 lines)
**Location**: `backends/rust/src/lib.rs`
- Idiomatic Rust code generation
- Cargo-compatible output

#### WebAssembly Backend (385 lines)
**Location**: `backends/wasm/src/lib.rs`
- WASM module generation
- Web platform ready

**Input**: SIR  
**Output**: Target language source code

---

## Workspace Structure

```
/workspace
├── compiler/              # Main compiler crate (~3,500 LOC)
│   └── src/
│       ├── lexer/        # Tokenization
│       ├── parser/       # AST construction
│       ├── sema/         # Semantic analysis
│       ├── sir/          # IR definition
│       ├── lowerer/      # AST→SIR conversion
│       ├── backend/      # Backend interface
│       ├── ast/          # AST node definitions
│       ├── diagnostics/  # Error reporting
│       └── cli/          # Command-line interface
│
├── backends/             # Target code generators (1,529 LOC)
│   ├── c/               # C backend
│   ├── javascript/      # JS backend
│   ├── rust/            # Rust backend
│   └── wasm/            # WebAssembly backend
│
├── runtime/              # Runtime system (scaffold)
├── package-manager/      # Package management (scaffold)
├── language-server/      # LSP implementation (scaffold)
├── debugger/             # Debugger (scaffold)
│
└── crates/
    └── nova-common/      # Shared utilities
```

## Design Principles

1. **Modularity**: Each phase is independent and testable
2. **Error Recovery**: Parser continues after errors for better diagnostics
3. **Type Safety**: Semantic analyzer ensures type correctness before codegen
4. **Multi-Target**: Backend abstraction enables multiple output formats
5. **Incremental**: SIR enables future optimization passes

## Current Implementation Status

| Phase | Component | Lines | Status |
|-------|-----------|-------|--------|
| Frontend | Lexer | 309 | ✅ Complete |
| Frontend | Parser | 907 | ✅ Complete |
| Frontend | Semantic Analyzer | 773 | ✅ Complete |
| Middle-end | SIR | 146 | ✅ Complete |
| Middle-end | Lowerer | 550 | ✅ Complete |
| Backend | C | 449 | ✅ Complete |
| Backend | JavaScript | 343 | ✅ Complete |
| Backend | Rust | 352 | ✅ Complete |
| Backend | WebAssembly | 385 | ✅ Complete |
| **Total** | **Compiler Core** | **~5,000+** | **✅ Phase 1 Done** |

## Future Architecture (Phase 2+)

### Runtime System (Planned)
- Memory allocator
- Garbage collector (optional)
- Standard library runtime support
- FFI bridge

### Tooling (Planned)
- Package manager with dependency resolution
- Language server protocol implementation
- Source-level debugger

---

**Last Updated**: Real-time tracking enabled  
**Architecture Version**: 1.0 (Phase 1 Complete)
