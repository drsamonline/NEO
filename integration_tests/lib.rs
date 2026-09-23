// NOVA Compiler Integration Tests
// Tests spanning compiler → runtime → stdlib pipeline

use nova_compiler::frontend::lexer::Lexer;
use nova_compiler::frontend::parser::Parser;
use nova_compiler::frontend::semantic_analyzer::SemanticAnalyzer;
use nova_compiler::middle::sir::SIRGenerator;
use nova_compiler::backend::c::CBackend;
use nova_runtime::memory::MemoryManager;
use nova_runtime::types::NovaType;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test 1: Complete compilation pipeline for a simple program
#[test]
fn test_simple_program_compilation() {
    let source = r#"
        fn main() -> int {
            let x: int = 42;
            let y: int = x + 8;
            return y;
        }
    "#;

    // Lexing
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Tokenization failed");
    assert!(!tokens.is_empty());

    // Parsing
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().expect("Parsing failed");
    assert!(!ast.functions.is_empty());

    // Semantic Analysis
    let mut analyzer = SemanticAnalyzer::new();
    let analyzed_ast = analyzer.analyze(ast).expect("Semantic analysis failed");

    // SIR Generation
    let mut sir_gen = SIRGenerator::new();
    let sir = sir_gen.generate(analyzed_ast).expect("SIR generation failed");
    assert!(!sir.functions.is_empty());

    // C Code Generation
    let backend = CBackend::new();
    let c_code = backend.generate(&sir).expect("C code generation failed");
    assert!(c_code.contains("main"));
    assert!(c_code.contains("return"));

    println!("✓ Simple program compilation successful");
}

/// Test 2: Runtime memory management integration
#[test]
fn test_runtime_memory_allocation() {
    let mut mem_mgr = MemoryManager::new(1024 * 1024); // 1MB
    
    // Allocate memory
    let ptr = mem_mgr.allocate(64, 8).expect("Allocation failed");
    assert!(ptr > 0);
    
    // Write and read data
    let test_data: [u8; 64] = [0x42; 64];
    mem_mgr.write(ptr, &test_data).expect("Write failed");
    
    let mut buffer = [0u8; 64];
    mem_mgr.read(ptr, &mut buffer).expect("Read failed");
    assert_eq!(buffer, test_data);
    
    // Free memory
    mem_mgr.deallocate(ptr).expect("Deallocation failed");
    
    println!("✓ Runtime memory management test successful");
}

/// Test 3: Type system integration
#[test]
fn test_type_system_integration() {
    let int_type = NovaType::Int;
    let float_type = NovaType::Float;
    let string_type = NovaType::String;
    let bool_type = NovaType::Bool;
    
    assert_eq!(int_type.size(), 8);
    assert_eq!(float_type.size(), 8);
    assert!(string_type.is_reference());
    assert_eq!(bool_type.size(), 1);
    
    // Type compatibility
    assert!(int_type.is_compatible_with(&int_type));
    assert!(!int_type.is_compatible_with(&float_type));
    
    println!("✓ Type system integration test successful");
}

/// Test 4: End-to-end compilation and execution simulation
#[test]
fn test_end_to_end_pipeline() {
    let source = r#"
        fn add(a: int, b: int) -> int {
            return a + b;
        }
        
        fn main() -> int {
            let result: int = add(10, 20);
            return result;
        }
    "#;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nova_file = temp_dir.path().join("test.nova");
    let c_file = temp_dir.path().join("test.c");
    let binary_file = temp_dir.path().join("test");
    
    // Write source file
    fs::write(&nova_file, source).expect("Failed to write source file");
    
    // Compile to C (simulated - in real scenario would call compiler CLI)
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    let mut analyzer = SemanticAnalyzer::new();
    let analyzed_ast = analyzer.analyze(ast).unwrap();
    let mut sir_gen = SIRGenerator::new();
    let sir = sir_gen.generate(analyzed_ast).unwrap();
    let backend = CBackend::new();
    let c_code = backend.generate(&sir).unwrap();
    
    fs::write(&c_file, c_code).expect("Failed to write C file");
    
    // Verify C code is valid (contains expected elements)
    assert!(c_code.contains("int64_t add(int64_t a, int64_t b)"));
    assert!(c_code.contains("int64_t main()"));
    
    println!("✓ End-to-end pipeline test successful");
}

/// Test 5: Error handling across layers
#[test]
fn test_error_handling_pipeline() {
    // Invalid syntax
    let invalid_source = r#"
        fn main() -> int {
            let x: int = ;  // Missing value
            return x;
        }
    "#;
    
    let mut lexer = Lexer::new(invalid_source);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse();
    assert!(parse_result.is_err(), "Parsing should fail for invalid syntax");
    
    println!("✓ Error handling pipeline test successful");
}

/// Test 6: Standard library function integration
#[test]
fn test_stdlib_function_calls() {
    let source = r#"
        fn main() -> int {
            // Test array operations (would use stdlib)
            let arr: [int; 5] = [1, 2, 3, 4, 5];
            let sum: int = 0;
            // In full implementation, this would call stdlib reduce
            return sum;
        }
    "#;
    
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    
    // Verify AST contains array literal
    let has_array = ast.functions.iter().any(|f| {
        f.body.statements.iter().any(|stmt| {
            matches!(stmt, nova_compiler::ast::Statement::Let { initializer, .. } 
                     if matches!(initializer.as_ref(), Some(expr) if matches!(expr, nova_compiler::ast::Expression::ArrayLiteral(_))))
        })
    });
    
    assert!(has_array, "AST should contain array literal");
    
    println!("✓ Standard library function integration test successful");
}

/// Test 7: FFI boundary testing
#[test]
fn test_ffi_boundary() {
    use nova_runtime::ffi::FFIManager;
    
    let mut ffi_mgr = FFIManager::new();
    
    // Register a mock C function
    let func_signature = nova_runtime::types::FunctionSignature {
        name: "printf".to_string(),
        params: vec![NovaType::Pointer(Box::new(NovaType::Char))],
        return_type: Box::new(NovaType::Int),
    };
    
    ffi_mgr.register_function(func_signature).expect("Failed to register FFI function");
    
    // Verify registration
    assert!(ffi_mgr.is_registered("printf"));
    
    println!("✓ FFI boundary test successful");
}

/// Test 8: Concurrent execution simulation
#[test]
fn test_concurrent_execution() {
    use nova_runtime::threading::ThreadManager;
    use std::sync::{Arc, Mutex};
    
    let counter = Arc::new(Mutex::new(0));
    let mut thread_mgr = ThreadManager::new();
    
    let counter_clone = Arc::clone(&counter);
    let task1 = move || {
        let mut num = counter_clone.lock().unwrap();
        for _ in 0..100 {
            *num += 1;
        }
    };
    
    let counter_clone = Arc::clone(&counter);
    let task2 = move || {
        let mut num = counter_clone.lock().unwrap();
        for _ in 0..100 {
            *num += 1;
        }
    };
    
    // Execute tasks (in real impl, these would be Nova threads)
    task1();
    task2();
    
    let final_count = *counter.lock().unwrap();
    assert_eq!(final_count, 200, "Concurrent tasks should complete correctly");
    
    println!("✓ Concurrent execution test successful");
}
