#![no_main]

use libfuzzer_sys::fuzz_target;
use nova_compiler::Lexer;

fuzz_target!(|data: &[u8]| {
    // NOVA source is UTF-8. Bytes that are not UTF-8 are a different question,
    // answered by the reader rather than the lexer, so they are skipped here.
    // Everything else — however malformed — must not panic, hang or overflow.
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let mut lexer = Lexer::new(source);
    let (tokens, diagnostics) = lexer.tokenize();

    // A span that points past the end of the source would make an editor's
    // `file:line:col` useless, and a diagnostic with no message is not one.
    for diagnostic in diagnostics.iter() {
        assert!(!diagnostic.message.is_empty(), "empty diagnostic message");
        if let Some(span) = &diagnostic.span {
            assert!(span.start.line >= 1, "span line is 0-based");
            assert!(span.start.column >= 1, "span column is 0-based");
        }
    }

    let _ = tokens;
});
