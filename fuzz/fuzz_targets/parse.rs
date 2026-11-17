#![no_main]

use libfuzzer_sys::fuzz_target;
use nova_compiler::{Lexer, Parser};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let mut lexer = Lexer::new(source);
    let (tokens, _diagnostics) = lexer.tokenize();
    let mut parser = Parser::new(tokens);
    let _ = parser.parse();
});
