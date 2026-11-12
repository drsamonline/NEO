<div align="center">

<img src="docs/assets/logo.svg" alt="NEO — NOVA" width="200">

# NOVA — Semantic Programming Platform

[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

**A statically-typed, compiled systems programming language.** Its compiler lowers NOVA source through a semantic pipeline — lexer, parser, semantic analyser, SIR, lowerer — to four backends: **C, JavaScript, Rust and WebAssembly**, with a runtime, package manager, language server and debugger alongside.

<a href="https://drsamonline.github.io/NEO/"><img src="https://img.shields.io/badge/Landing%20Page-Visit-3b82f6?style=flat-square" alt="Landing page"></a>
<a href="https://img.shields.io/badge/Phase%201-Complete-brightgreen"><img src="https://img.shields.io/badge/Phase%201-Complete-brightgreen?style=flat-square" alt="Phase 1 Complete"></a>
<a href="https://img.shields.io/badge/Phase%202-Complete-brightgreen"><img src="https://img.shields.io/badge/Phase%202-Complete-brightgreen?style=flat-square" alt="Phase 2 Complete"></a>
<a href="https://img.shields.io/badge/Phase%203-Complete-brightgreen"><img src="https://img.shields.io/badge/Phase%203-Complete-brightgreen?style=flat-square" alt="Phase 3 Complete"></a>
<a href="https://img.shields.io/badge/Phase%204-Next-yellow"><img src="https://img.shields.io/badge/Phase%204-Next-yellow?style=flat-square" alt="Phase 4 Next"></a>
<a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.6.0-blue?style=flat-square" alt="Version 0.6.0"></a>
<a href="backends/"><img src="https://img.shields.io/badge/Backends-4%20targets-orange?style=flat-square" alt="4 backend targets"></a>
<a href="https://img.shields.io/badge/language-Rust-dea584"><img src="https://img.shields.io/badge/language-Rust-dea584?style=flat-square" alt="Rust"></a>

**[Landing page](https://drsamonline.github.io/NEO/)** · **[Examples](examples/)** · **[Language spec](docs/language/)** · **[Development plan](DEV_PLAN.md)** · **[Architecture](ARCHITECTURE.md)** · **[All projects](https://drsamonline.github.io/)**

<sub>Live demo: not yet available — the project is pre-release (Phase 4 in progress).</sub>

</div>

---


Semantic Programming Platform

[![Phase 1 Complete](https://img.shields.io/badge/Phase-1%20Complete-brightgreen)](DEV_PLAN.md)
[![Phase 2 Complete](https://img.shields.io/badge/Phase-2%20Complete-brightgreen)](DEV_PLAN.md)
[![Phase 3 Complete](https://img.shields.io/badge/Phase-3%20Complete-brightgreen)](DEV_PLAN.md)
[![Phase 4 Next](https://img.shields.io/badge/Phase-4%20Next-yellow)](DEV_PLAN.md)
[![Compiler Core](https://img.shields.io/badge/Compiler-6000%2B%20LOC-blue)](compiler/)
[![Runtime System](https://img.shields.io/badge/Runtime-2359%20LOC-orange)](runtime/)
[![Tooling](https://img.shields.io/badge/Tooling-3600%2B%20LOC-orange)](pkg/)
[![Backends](https://img.shields.io/badge/Backends-4%20Targets-orange)](backends/)

## Overview

NOVA is a statically-typed, compiled systems programming language with a complete tooling ecosystem. The compiler transforms NOVA source code into multiple target languages (C, JavaScript, Rust, WebAssembly) through a robust semantic analysis pipeline.

## Current Status

**✅ Phase 1: Compiler Core - COMPLETE**
- Full lexer, parser, semantic analyzer implemented
- SIR (Semantic Intermediate Representation) defined
- Lowerer converts AST to SIR
- 4 backends: C, JavaScript, Rust, WebAssembly
- Total: ~3,500+ lines of compiler code

**✅ Phase 2: Runtime System - COMPLETE**
- ✅ Memory management (192 lines, arena allocation, GC foundation)
- ✅ Type system (260 lines, type registry, dynamic values)
- ✅ Error handling (297 lines, panic handling, backtraces)
- ✅ FFI support (332 lines, C interop, FFI arrays)
- ✅ Core runtime library (139 lines, initialization, stats)
- ✅ Garbage collection (315 lines, mark-and-sweep implementation)
- ✅ I/O runtime (535 lines, file I/O, streams, directories)
- ✅ Threading primitives (314 lines, mutexes, RW locks, condition variables)
- ✅ Async runtime (114 lines, futures, channels, delay)
- Total: 2,881 lines, 30+ tests

**✅ Phase 3: Tooling - COMPLETE**
- ✅ Package manager (1,674 lines) - CLI with 12 commands, manifest parsing, dependency resolution, registry
- ✅ Language server (522 lines) - LSP handlers, completions, diagnostics, hover, go-to-definition
- ✅ Debugger (1,405 lines) - Interactive REPL, breakpoints, session management, target control
- Total: 3,601 lines across all tools

**🔄 Phase 4: Polish & Integration - NEXT**
- Standard library implementation
- LSP-compiler integration for real-time diagnostics
- Debugger-binary integration
- End-to-end testing
- Performance optimization
- Documentation

See [Development Plan](DEV_PLAN.md) for detailed status.

## Quick Start

```bash
# Build the compiler (Rust toolchain, plus a C compiler on PATH)
cargo build --release

# Compile and run a program
./target/release/nova-compiler run examples/hello.nova

# Build an executable
./target/release/nova-compiler build examples/fibonacci.nova -o fibonacci
```

## Project Structure

```
/workspace
├── compiler/           # Main compiler (~3,500 LOC)
│   ├── src/lexer/     # Tokenization (340 lines)
│   ├── src/parser/    # Recursive descent parser (1,435 lines)
│   ├── src/sema/      # Semantic analysis (1,501 lines)
│   ├── src/sir/       # IR definition (212 lines)
│   ├── src/lowerer/   # AST→SIR lowering (1,619 lines)
│   └── src/backend/   # C code generation (497 lines)
├── backends/          # Standalone target generators (1,936 lines)
│   ├── c/             # C backend (672 lines)
│   ├── javascript/    # JavaScript backend (408 lines)
│   ├── rust/          # Rust backend (411 lines)
│   └── wasm/          # WebAssembly backend (445 lines)
├── runtime/           # Runtime system (2,359 lines, 33 tests)
│   ├── src/memory.rs  # Memory utilities (195 lines)
│   ├── src/types.rs   # Runtime string/array/option types (276 lines)
│   ├── src/error.rs   # Error handling (232 lines)
│   ├── src/ffi.rs     # C interop (193 lines)
│   ├── src/gc.rs      # Reference-counted GC (420 lines)
│   ├── src/lib.rs     # Core library (81 lines)
│   ├── src/io/        # File and stream I/O (534 lines)
│   ├── src/threading/ # Threading primitives (314 lines)
│   └── src/async_rt/  # Futures and delay (114 lines)
├── pkg/               # Package manager (1,780 lines)
│   └── src/           # CLI, manifest, resolver, registry
├── lsp/               # Language server (931 lines)
│   └── src/main.rs    # LSP handlers, completions, diagnostics
├── dbg/               # Debugger (1,733 lines)
│   └── src/           # REPL, breakpoints, sessions, target control
├── crates/nova-common/ # Shared utilities
├── stdlib/            # Standard library (scaffolds)
├── examples/          # Example programs
└── tests/             # Test suite
```

## Features

### Implemented ✅
- **Lexical Analysis**: complete tokenization, `#`/`//` comments, escapes, `=>`
- **Parsing**: recursive descent for the full grammar, including generics, slices,
  struct literals, `match`, closures, casts and `static`
- **Semantic Analysis**: scopes, name resolution, call arity/type checking with
  expression spans, return checking, generic-call acceptance
- **Lowering to SIR**: structs, field assignment, `impl` methods, enums, `match`,
  arrays, generic instantiation
- **C code generation**: runnable programs linked against the runtime shim (ABI v3);
  statement `match` becomes a C `switch`, arrays are bounds-checked and growable,
  payload-carrying enums/`Option<T>` are tagged structs, generic structs are
  monomorphised
- **Modules**: `use` and `mod` load sibling and standard-library files
- **Strings**: slicing (`text[a..b]`) and string-literal `match` arms
- **Generics**: functions, enums, structs and impl methods are monomorphised
- **Closures and tuples**: lifted closures with heap environments and `fn(A) -> B`
  values; `NovaTuple_*` structs with `let (a, b) = pair;` destructuring
- **Backends**: `emit --target c|js|rust` selects a generator; build/run link C
- **Builtins**: compiler-declared `__builtin_*` functions for printing, strings,
  array length, panic and exit
- **Runtime crate**: memory utilities, runtime types, errors, FFI, reference-counted
  GC, file I/O, threading, async helpers (not yet linked to generated C)
- **Tooling**: package manager, language server and debugger crates with tests

### Coming Soon 🚧
- General (non-array) reference lowering
- Growable arrays are implemented; element types other than `int` are not
- A build/run path for the JavaScript and Rust targets, and `--target wasm`
- LSP go-to-definition/references, DWARF debugger, real package registry

## Documentation

- [Development Plan](DEV_PLAN.md) - Current progress and roadmap
- [Architecture](ARCHITECTURE.md) - System design
- [Language Tour](website/docs/language-tour.md) - Language features
- [Getting Started](website/docs/getting-started.md) - Installation guide
- [FAQ](FAQ.md) - Common questions

<!-- support:sweep -->
## Support

One person's work: no company, no ads, no tracking, no paid tier. If it saved you an
afternoon, a contribution keeps the next one coming — any amount, or none.

| | |
|---|---|
| **UPI** | `drsohilboi@ybl` |
| **Ko-fi** | [ko-fi.com/matneo](https://ko-fi.com/matneo) |
| **PayPal** | [paypal.me/drsohilmomin](https://paypal.me/drsohilmomin?locale.x=en_GB&country.x=IN) |
| **Books** | [the author's other work](https://www.amazon.com/stores/author/B08ZT331LY/allbooks) |
| **Elsewhere** | [all projects](https://drsamonline.github.io/) |
| **Email** | [sohilmomin2000@gmail.com](mailto:sohilmomin2000@gmail.com) — bugs, corrections, requests |

## License

See LICENSE file in repository root.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

**Last Updated**: Real-time status tracking enabled
**Total Implementation**: ~5,000+ lines of production code
