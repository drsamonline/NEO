//! C Backend CLI Tool
//! 
//! Compiles NOVA source files to C code

use nova_compiler::{Lexer, Parser, SemanticAnalyzer, Lowerer};
use nova_backend_c::CGenerator;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <source_file> [output_file]", args[0]);
        eprintln!("  If output_file is not specified, outputs to stdout");
        std::process::exit(1);
    }
    
    let input_file = &args[1];
    let output_file = args.get(2).cloned();
    
    // Read source file
    let source = match fs::read_to_string(input_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", input_file, e);
            std::process::exit(1);
        }
    };
    
    eprintln!("Compiling {} to C...", input_file);
    
    // Phase 1: Lexing
    let mut lexer = Lexer::new(&source);
    let (tokens, diagnostics) = lexer.tokenize();
    
    if diagnostics.has_errors() {
        eprintln!("Lexical errors:");
        for diag in diagnostics.iter() {
            eprintln!("  {}", diag.message);
        }
        std::process::exit(1);
    }
    
    eprintln!("✓ Lexical analysis complete ({} tokens)", tokens.len());
    
    // Phase 2: Parsing
    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => {
            eprintln!("✓ Parsing complete ({} items)", program.items.len());
            program
        }
        Err(diagnostics) => {
            eprintln!("Parsing errors:");
            for diag in diagnostics.iter() {
                eprintln!("  {}", diag.message);
            }
            std::process::exit(1);
        }
    };
    
    // Phase 3: Semantic Analysis
    let mut analyzer = SemanticAnalyzer::new();
    match analyzer.analyze(&program) {
        Ok(_) => {
            eprintln!("✓ Semantic analysis complete");
        }
        Err(_) => {
            eprintln!("Semantic errors:");
            for diag in analyzer.diagnostics.iter() {
                eprintln!("  {}", diag.message);
            }
            std::process::exit(1);
        }
    };
    
    // Phase 4: Lowering to SIR
    let mut lowerer = Lowerer::new();
    let sir_program = lowerer.lower_program(&program);
    eprintln!("✓ Lowering complete ({} functions, {} globals)", 
             sir_program.functions.len(), sir_program.globals.len());
    
    // Phase 5: Generate C code
    let mut generator = CGenerator::new();
    let c_code = generator.generate(&sir_program);
    eprintln!("✓ C code generation complete");
    
    // Output the generated C code
    match output_file {
        Some(output_path) => {
            match fs::write(&output_path, &c_code) {
                Ok(_) => {
                    eprintln!("✓ C code written to {}", output_path);
                    
                    // Try to compile with gcc if available
                    let output_exe = Path::new(&output_path)
                        .with_extension("");
                    let gcc_result = std::process::Command::new("gcc")
                        .arg(&output_path)
                        .arg("-o")
                        .arg(&output_exe)
                        .output();
                    
                    match gcc_result {
                        Ok(result) => {
                            if result.status.success() {
                                eprintln!("✓ Compiled to executable: {:?}", output_exe);
                            } else {
                                eprintln!("⚠ GCC compilation failed:");
                                eprintln!("{}", String::from_utf8_lossy(&result.stderr));
                            }
                        }
                        Err(_) => {
                            eprintln!("ℹ GCC not found, skipping compilation");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error writing to '{}': {}", output_path, e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            println!("{}", c_code);
        }
    }
    
    eprintln!("✓ Compilation successful!");
}
