# NOVA Compiler Implementation Plan

## Overview
Complete implementation of the NOVA compiler from semantic analysis to executable generation, with strict error fixing at each stage and QA as the final phase.

---

## Phase 1: Robust Statement & Control Flow Handling
**Goal**: Properly distinguish between expressions and statements, implement full control flow.

### Steps:
1. **Refactor Lowerer Architecture**
   - Create separate methods for `lower_expr()` and `lower_stmt()`
   - Update AST node handling to route correctly based on context
   - Fix unused variables (`then_stmts`, `else_stmts`, etc.) by proper usage

2. **Implement Return Statements**
   - Add `lower_return_stmt()` method
   - Handle return with value vs return without value (Unit)
   - Generate proper C `return` statements

3. **Implement For Loops**
   - Add `lower_for_stmt()` method
   - Support range-based iteration: `for i in start..end`
   - Generate C `for` loops with proper initialization, condition, increment

4. **Fix If/While Statement Context**
   - When used as statements, don't generate temporary variables
   - Direct code emission for statement context
   - Preserve expression context for if-expressions that return values

5. **Error Fixing Stage 1**
   - Compile and fix all errors before proceeding
   - Run existing tests to ensure no regressions
   - Verify generated C code compiles with gcc

**Deliverable**: Compiler handles all basic control flow correctly

---

## Phase 2: Advanced Expression Support
**Goal**: Complete expression lowering for all AST node types.

### Steps:
1. **Field Access Expressions**
   - Implement `lower_field_access()`
   - Support struct field access: `person.name`
   - Generate C struct member access: `person->name`

2. **Index Expressions**
   - Implement `lower_index_expr()`
   - Support array indexing: `arr[0]`
   - Generate C array access: `arr[0]`

3. **Range Expressions**
   - Implement `lower_range_expr()`
   - Support `start..end` syntax
   - Generate appropriate C representation or helper function calls

4. **Match Expressions**
   - Implement `lower_match_expr()`
   - Support pattern matching with multiple arms
   - Generate C switch/if-else chains

5. **Cast Expressions**
   - Implement `lower_cast_expr()`
   - Support type casting: `x as i32`
   - Generate C casts: `(int)x`

6. **Block Expressions**
   - Implement `lower_block_expr()`
   - Support blocks that return values: `{ let x = 5; x }`
   - Manage scope and variable lifetime

7. **Error Fixing Stage 2**
   - Compile and fix all errors
   - Test each expression type individually
   - Verify complex nested expressions work

**Deliverable**: All expression types lower correctly to C

---

## Phase 3: Type System & Format Specifiers
**Goal**: Intelligent code generation based on static type information.

### Steps:
1. **Type Propagation in Lowerer**
   - Track types of expressions during lowering
   - Store type information in SIR nodes
   - Use types to guide code generation

2. **Format Specifier Detection**
   - Analyze argument types in `print`/`println` calls
   - Map NOVA types to C format specifiers:
     - `Int` → `%d`
     - `Float` → `%f`
     - `String` → `%s`
     - `Bool` → `%s` (with "true"/"false" conversion)
   - Generate correct `printf` format strings

3. **Boolean Conversion**
   - Convert NOVA booleans to C strings for printing
   - Generate ternary operators: `b ? "true" : "false"`

4. **Type Mismatch Detection**
   - Add compile-time errors for invalid operations
   - Verify function argument types match signatures

5. **Error Fixing Stage 3**
   - Test type-dependent code generation
   - Verify format specifiers match argument types
   - Fix any type-related compilation errors

**Deliverable**: Smart code generation with correct format specifiers

---

## Phase 4: Code Generation Optimization
**Goal**: Clean, efficient, and idiomatic C code output.

### Steps:
1. **Remove Extra Semicolons**
   - Fix statement termination logic
   - Ensure clean C syntax without `;;`

2. **Variable Declaration Optimization**
   - Declare variables at appropriate scope
   - Avoid unnecessary `void` initializations

3. **Function Signature Cleanup**
   - Generate proper parameter lists
   - Handle void functions correctly
   - Add necessary forward declarations

4. **Include Management**
   - Only include headers that are actually used
   - Add custom includes for user-defined types

5. **Code Formatting**
   - Consistent indentation in generated C
   - Readable output for debugging

6. **Error Fixing Stage 4**
   - Verify generated C passes `gcc -Wall -Wextra`
   - Fix any warnings in generated code
   - Optimize for readability and correctness

**Deliverable**: Clean, warning-free C code generation

---

## Phase 5: Runtime & Standard Library Integration
**Goal**: Support for runtime features and standard library functions.

### Steps:
1. **Runtime Helper Functions**
   - Implement `nova_runtime.c` with helper functions
   - Add string operations, memory management
   - Provide range iteration helpers

2. **Standard Library Bindings**
   - Map NOVA stdlib functions to C implementations
   - Support common operations (string manipulation, math)

3. **Panic Handling**
   - Implement `panic()` function
   - Generate proper error messages and exit codes

4. **Memory Management**
   - Handle allocation/deallocation patterns
   - Support NOVA's ownership model in C

5. **Error Fixing Stage 5**
   - Link compiled C with runtime library
   - Test runtime behavior of generated programs
   - Fix linking and runtime errors

**Deliverable**: Fully linked executables with runtime support

---

## Phase 6: End-to-End Integration
**Goal**: Complete pipeline from .nova source to executable.

### Steps:
1. **Pipeline Integration**
   - Connect lexer → parser → semantic analyzer → lowerer → codegen
   - Handle errors at each stage gracefully
   - Provide meaningful error messages

2. **CLI Tool Enhancement**
   - Add `nova build` command
   - Support `nova run` for compile-and-execute
   - Add `--output` flag for custom output paths

3. **Multi-file Compilation**
   - Support modules and imports
   - Handle multiple source files
   - Manage dependencies correctly

4. **Build Artifacts**
   - Generate object files and link
   - Support both static and dynamic linking
   - Create distributable binaries

5. **Error Fixing Stage 6**
   - Test complete compilation pipeline
   - Verify multi-file projects compile
   - Fix integration issues

**Deliverable**: Production-ready `nova` CLI tool

---

## Phase 7: Comprehensive Testing
**Goal**: Validate all functionality with extensive test coverage.

### Steps:
1. **Unit Tests**
   - Test each compiler component independently
   - Cover edge cases in lexer, parser, semantic analyzer
   - Test lowering logic for each AST node type

2. **Integration Tests**
   - Test complete compilation of sample programs
   - Verify generated C compiles and runs correctly
   - Test error handling and recovery

3. **Regression Tests**
   - Maintain test suite for previously fixed bugs
   - Automate testing in CI pipeline
   - Track test coverage metrics

4. **Performance Tests**
   - Measure compilation speed
   - Benchmark generated code performance
   - Identify optimization opportunities

5. **Error Fixing Stage 7**
   - Fix all failing tests
   - Achieve >90% code coverage
   - Document known limitations

**Deliverable**: Comprehensive test suite with high coverage

---

## Phase 8: Quality Assurance (QA)
**Goal**: Final validation, documentation, and release preparation.

### Steps:
1. **Manual Testing**
   - Test real-world use cases
   - Verify user experience of CLI tools
   - Test error messages for clarity

2. **Documentation**
   - Write user guide for NOVA language
   - Document compiler architecture
   - Create API reference for developers

3. **Code Review**
   - Review all code for best practices
   - Check for security vulnerabilities
   - Ensure consistent coding style

4. **Performance Optimization**
   - Profile compiler performance
   - Optimize hot paths
   - Reduce memory usage

5. **Release Preparation**
   - Create release notes
   - Tag version in git
   - Prepare distribution packages

6. **Final Error Fixing**
   - Address all critical and high-priority issues
   - Resolve remaining warnings
   - Verify stability under stress

**Deliverable**: Production-ready NOVA compiler with documentation

---

## Success Criteria
- [ ] Zero compiler errors and warnings
- [ ] All AST node types properly lowered
- [ ] Generated C code compiles without warnings
- [ ] Executables run correctly with expected output
- [ ] Comprehensive test suite passing
- [ ] Full documentation available
- [ ] CLI tool supports build, run, and package commands

## Risk Mitigation
- Fix errors immediately at each stage before proceeding
- Maintain backward compatibility when possible
- Keep detailed logs of changes for debugging
- Use version control for easy rollback if needed
