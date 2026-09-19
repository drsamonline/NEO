use nova_compiler::{Lexer, Parser, SemanticAnalyzer};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <source_file>", args[0]);
        std::process::exit(1);
    }
    
    let filename = &args[1];
    
    // Read source file
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filename, e);
            std::process::exit(1);
        }
    };
    
    println!("Compiling {}...", filename);
    
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
    
    println!("✓ Lexical analysis complete ({} tokens)", tokens.len());
    
    // Phase 2: Parsing
    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => {
            println!("✓ Parsing complete ({} items)", program.items.len());
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
            println!("✓ Semantic analysis complete");
        }
        Err(_) => {
            eprintln!("Semantic errors:");
            for diag in analyzer.diagnostics.iter() {
                eprintln!("  {}", diag.message);
            }
            std::process::exit(1);
        }
    };
    
    println!("✓ Compilation successful!");
}