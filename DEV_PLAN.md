# NOVA Compiler v1.0 Development Plan

## Vision
A fully functional semantic programming language with compiler, runtime, package manager, language server, and debugger.

## Phase 1: Core Compiler Pipeline (Weeks 1-4)
### 1.1 Lexer (COMPLETE - needs testing)
- [x] Basic tokenization
- [ ] Comprehensive test suite
- [ ] Unicode support
- [ ] Better error messages with spans

### 1.2 Parser (Priority: HIGH)
- [ ] Recursive descent parser
- [ ] AST construction
- [ ] Error recovery
- [ ] Operator precedence
- [ ] Expression parsing
- [ ] Statement parsing
- [ ] Declaration parsing

### 1.3 Semantic Analyzer (Priority: HIGH)
- [ ] Symbol table population
- [ ] Type checking
- [ ] Scope resolution
- [ ] Name resolution
- [ ] Type inference (basic)

### 1.4 SIR (Semantic Intermediate Representation)
- [ ] SIR definition
- [ ] Lowering from AST
- [ ] Optimization passes

### 1.5 Backends
- [ ] C backend
- [ ] JavaScript backend
- [ ] Rust backend
- [ ] WebAssembly backend

## Phase 2: Runtime System (Weeks 5-6)
- [ ] Memory management
- [ ] Garbage collection (optional)
- [ ] Standard library
- [ ] FFI support

## Phase 3: Tooling (Weeks 7-8)
- [ ] Package manager implementation
- [ ] Language server protocol
- [ ] Debugger integration

## Phase 4: Polish & Testing (Weeks 9-10)
- [ ] Documentation
- [ ] Performance optimization
- [ ] Example programs
- [ ] Release preparation

## Current Status
- Lexer: Implemented, needs testing
- Parser: Scaffold only
- Semantic Analyzer: Scaffold only
- Backends: Scaffold only
- Runtime: Scaffold only
- Tools: Scaffold only

## Immediate Next Steps
1. Complete lexer tests
2. Implement parser
3. Implement semantic analyzer
4. Create SIR
5. Implement at least one backend (C)
