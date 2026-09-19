# NOVA Compiler Implementation Plan

## Current Status (Verified)
- ✅ Rust toolchain installed (v1.98.1)
- ✅ Workspace builds successfully
- ✅ 13 tests passing (name resolver tests)
- ✅ Lexer implemented with basic tests
- ⚠️ Parser expects only declarations (no statement-level code)
- ⚠️ Multiple warnings for unused variables/imports

## Architecture Overview
```
Lexer → Parser → AST → Semantic Analyzer → SIR → Backend
```

## Immediate Implementation Priorities

### Phase 1: Fix Parser to Support Expression Statements (CURRENT)
The parser currently only accepts top-level declarations. Need to support:
- Expression statements (e.g., `print "Hello"`)
- Statement-level code in programs

### Phase 2: Complete Core Compiler Pipeline
1. **Parser Improvements**
   - Add expression statement support
   - Improve error recovery
   - Add operator precedence handling

2. **Semantic Analyzer**
   - Complete type checking
   - Finish name resolution
   - Add scope management

3. **SIR (Semantic Intermediate Representation)**
   - Define SIR format
   - Implement AST to SIR lowering

4. **C Backend**
   - Implement code generation
   - Support basic types and control flow

### Phase 3: Runtime & Standard Library
1. Basic runtime support
2. Core standard library functions
3. Print/IO support

### Phase 4: Tooling
1. Package manager
2. Language server
3. Debugger

## Test Strategy
- Unit tests for each compiler phase
- Integration tests with .nova files
- End-to-end compilation tests

## Success Criteria
- [ ] Compile and run hello.nova example
- [ ] All compiler phases working end-to-end
- [ ] Comprehensive test suite (>80% coverage)
- [ ] No compiler warnings
