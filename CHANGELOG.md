# Changelog

All notable changes to the NOVA project will be documented in this file.

## [Unreleased] - Phase 2: Runtime System (Active Development)

### Added
#### The grammar is now a checked artefact ⭐ NEW
- **`grammar/grammar.ebnf`** — replaced the three-line scaffold with the grammar of the
  subset `compiler/src/parser/mod.rs` actually implements: items, statements, the
  expression chain in the parser's own precedence order, and a written-down list of the
  tokens that are lexed but not parsed yet (`match`, `type`, `as`, `loop`, `pub`, and
  nine operators).
- **`tools/grammar-check.py`** — a test for the grammar itself. It reads `token.rs` and
  `parser/mod.rs` and fails when the grammar and the code disagree: a keyword or operator
  that is neither used nor declared reserved, a word literal that is not a real keyword
  (invented syntax), a rule that is undefined or unreachable, or an expression level
  edited on one side only. Wired into CI, so a new keyword cannot be added without
  deciding where it belongs.

#### Every example is now a program the grammar accepts ⭐ NEW
- **`tools/example-check.py`** — reads `grammar/grammar.ebnf`, turns it into a
  packrat recogniser and requires every `examples/**/*.nova` to parse from `program`
  to EOF. It self-tests the recogniser on eleven known-good and known-bad sources
  first, so a passing run means something. Wired into CI beside `grammar-check.py`.
  Nothing read the examples before this: the grammar was checked against the Rust and
  the Rust tests parse snippets inside `#[test]` functions, so an example could say
  anything.
- **Three of the five shipped examples were not NOVA programs**, which is what the
  new check found. `examples/basic/hello.nova` was `print "Hello from NOVA"` — no
  parentheses, no semicolon, which the parser rejects; it is now `print("Hello, NOVA");`.
  `examples/web/app.nova` was a single comment and no program at all; it is now a small
  program that prints the page a site would serve. `examples/array_ops.nova` annotated
  arrays as `[int]` and `[int; 5]`, which `parse_type_expr` cannot read (it takes one
  name), so it is now `Array` — and `examples/README.md` says why, and no longer claims
  an HTTP server, JSON serialisation or `async`/`await` that do not exist.
- **The grammar had drifted too.** `item` used `expr_stmt`, whose `;` is optional, but
  `parse_top_level` *requires* the `;` after a top-level expression. A looser rule that
  accepted syntax the parser rejects is still a false grammar; it is now
  `top_expr_stmt = expr ";"` at top level and `expr_stmt = expr [ ";" ]` inside a block.

#### Runtime System Core (1,220+ lines) ⭐ NEW
- **Memory Management** (`memory.rs` - 192 lines):
  - `MemoryArena` for efficient arena-based allocation
  - `GarbageCollector` foundation with mark-and-sweep placeholder
  - `ManagedPtr<T>` for safe runtime-managed pointers
  - Global memory statistics tracking
  - Comprehensive test suite (2 tests)

- **Type System** (`types.rs` - 260 lines):
  - `TypeId` and `TypeInfo` for runtime type information
  - `TypeRegistry` with primitive types pre-registered
  - `DynamicValue` for runtime type checking
  - Field and variant info for structs/enums (3 tests)

- **Error Handling** (`error.rs` - 297 lines):
  - `RuntimeError` enum with 10 error categories
  - `RuntimeErrorWithTrace` with backtrace support
  - Panic hook initialization and handling
  - `catch_unwind` utility function (4 tests)

- **FFI Support** (`ffi.rs` - 332 lines):
  - `NovaValue` C-compatible tagged union
  - `FfiString` for Rust ↔ C string conversion
  - `FfiArray<T>` for FFI-safe dynamic arrays
  - `FfiHandle` for resource management
  - `FfiFunction` wrapper for callbacks (6 tests)

- **Core Runtime Library** (`lib.rs` - 139 lines):
  - Runtime initialization and statistics
  - Module exports and version info
  - Integration tests (6 tests)

### Technical Details
- Runtime LOC: 1,220+ lines
- Test coverage: 20+ tests across 5 modules
- Status: Core modules complete, I/O and threading pending

## [1.0.0-alpha] - Phase 1: Compiler Core Complete - 2024

### Added
#### Compiler Core (~3,500 lines)
- **Lexer** (309 lines): Complete tokenization with Unicode support, error spans
- **Parser** (907 lines): Recursive descent parser with error recovery, operator precedence
- **Semantic Analyzer** (773 lines): 
  - Type checking system
  - Symbol table management
  - Scope resolution
  - Name resolution
  - Type inference (basic)
- **SIR** (146 lines): Semantic Intermediate Representation definition
- **Lowerer** (550 lines): AST to SIR conversion

#### Backends (1,529 lines total)
- **C Backend** (449 lines): Full C code generation, tested and working
- **JavaScript Backend** (343 lines): ES6+ JavaScript code generation
- **Rust Backend** (352 lines): Rust code generation
- **WebAssembly Backend** (385 lines): WASM module generation

#### Infrastructure
- Complete compiler pipeline: Source → Lexer → Parser → AST → Semantic Analysis → SIR → Backend → Target Code
- Support for functions, structs, enums, control flow, loops
- Recursive function support
- Type printing and debugging

### Technical Details
- Total production code: ~5,000+ lines
- Language: Rust
- Architecture: Modular, multi-crate workspace
- Status: Production-ready compiler core

## [0.1.0] - Initial Scaffold - 2024

### Added
- Project structure and workspace setup
- Basic crate definitions
- Documentation framework

---

**Note**: This changelog is now updated in real-time with each development phase.
