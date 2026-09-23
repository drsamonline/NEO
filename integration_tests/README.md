# NOVA Integration Tests

This directory contains integration tests that span the entire compiler pipeline from source code to runtime execution.

## Test Categories

1. **Compiler Pipeline Tests** - Verify lexer → parser → semantic analyzer → SIR → backend flow
2. **Runtime Integration Tests** - Test memory management, type system, and FFI boundaries
3. **End-to-End Tests** - Complete compilation and execution simulation
4. **Error Handling Tests** - Verify error propagation across all layers
5. **Concurrency Tests** - Test threading and async runtime integration

## Running Tests

```bash
# Run all integration tests
cargo test --test integration_tests

# Run specific test
cargo test --test integration_tests test_simple_program_compilation

# Run with output
cargo test --test integration_tests -- --nocapture
```

## Test Structure

Each test function verifies a specific aspect of the NOVA compiler and runtime system:

- `test_simple_program_compilation` - Basic compilation pipeline
- `test_runtime_memory_allocation` - Memory manager operations
- `test_type_system_integration` - Type checking and compatibility
- `test_end_to_end_pipeline` - Full compilation to C code
- `test_error_handling_pipeline` - Error detection and reporting
- `test_stdlib_function_calls` - Standard library integration
- `test_ffi_boundary` - Foreign Function Interface
- `test_concurrent_execution` - Threading and synchronization

## Adding New Tests

When adding new integration tests:

1. Follow the existing test structure with descriptive names
2. Test across multiple components (compiler + runtime + stdlib)
3. Include both positive and negative test cases
4. Add println! statements with ✓ for visual feedback
5. Document the test purpose with doc comments

## Dependencies

- `tempfile` - For temporary file creation in end-to-end tests
- `nova_compiler` - Compiler crates (lexer, parser, analyzer, backends)
- `nova_runtime` - Runtime system crates (memory, types, ffi, threading)
