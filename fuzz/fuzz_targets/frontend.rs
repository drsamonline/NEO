#![no_main]

use libfuzzer_sys::fuzz_target;
use nova_compiler::{Lexer, Lowerer, Parser, SemanticAnalyzer};

// The deepest target: source all the way to SIR. It only gets past the parser on
// input that is a syntactically valid program, which is rare from random bytes —
// so `fuzz/corpus/frontend/` carries the seed programs from `examples/` and
// `tests/parity/` (see `fuzz/seed.sh`). Without seeds this target spends its
// whole budget in the lexer.
fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let mut lexer = Lexer::new(source);
    let (tokens, _diagnostics) = lexer.tokenize();
    let mut parser = Parser::new(tokens);
    let Ok(program) = parser.parse() else {
        return;
    };

    let mut analyzer = SemanticAnalyzer::new();
    if analyzer.analyze(&program).is_err() {
        return;
    }

    let mut lowerer = Lowerer::new();
    let _sir = lowerer.lower_program(&program);

    // The item tree is a second, independent walk over the same tokens. It has to
    // survive input the parser rejects — it is what an editor uses while a file is
    // half-typed — so it runs on everything that reaches here, and the expansion of
    // every deferred body is exercised too.
    let tree = nova_compiler::syntax::ItemTree::parse(source);
    for item in tree.items() {
        let _ = tree.children_of(item);
    }
});
