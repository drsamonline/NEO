# Tokens

The lexer turns source text into a flat list of tokens. Every token carries a `TokenKind`, a
`Span` and the `lexeme` it was read from. The kinds are defined in
[`crates/nova-common/src/token.rs`](../../crates/nova-common/src/token.rs), and
`tools/grammar-check.py` fails the build if `grammar/grammar.ebnf` names a keyword or an
operator the lexer cannot produce.

## The kinds

| Group | Kinds |
|---|---|
| Literals | `Integer(i64)`, `Float(f64)`, `String`, `Char` |
| Names | `Identifier(String)` |
| Keywords | twenty-five, listed in [Keywords](03_keywords.md) |
| Operators | `+ - * / %`, `== != < > <= >=`, `&& \|\| !`, `= += -= *= /=`, `& \| ^ ~`, `<< >>` |
| Delimiters | `( ) { } [ ]`, `, ; : :: . ..`, `-> =>` |
| Special | `Eof`, `Unknown(char)` |

`Unknown(char)` is what an unrecognised character becomes — `@` in `1 @ 2` lexes as
`Unknown('@')`. The lexer does not report it; it emits the token and lets the parser fail on
it, which is why the diagnostic reads `Expected ';' after variable declaration at '@'` rather
than naming the character as illegal. It is not dropped silently, but the error arrives one
stage later than the mistake was made.

`Bool` exists in the enum and the lexer never produces it — `true` and `false` lex as the
keywords `TokenKind::True` and `TokenKind::False`. The variant is listed here because it is
in the enum, not because a program can meet it.

## Whitespace and comments

Four characters separate tokens: space, tab, carriage return and newline. Everything else is
significant.

Three comment forms are recognised:

| Form | Ends at |
|---|---|
| `// ...` | the end of the line |
| `# ...` | the end of the line |
| `/* ... */` | the closing `*/` |

`#` is supported because the standard library's file headers use it; `//` and `/* */` are the
forms this reference documents. All three are skipped between any two tokens, so a comment
may appear wherever whitespace may.

**An unterminated `/*` is an error**, reported as `Unterminated block comment`. It used to be
accepted silently, which meant a missing `*/` swallowed the rest of the file and the program
compiled to nothing without complaint. The string and character readers already reported the
same mistake.

## Numbers

A number starts with an ASCII digit.

| Form | Example | Kind |
|---|---|---|
| decimal | `42` | `Integer` |
| hexadecimal | `0xFF` | `Integer` |
| binary | `0b1010` | `Integer` |
| octal | `0o755` | `Integer` |
| float | `1.5`, `2e9`, `3E-4` | `Float` |

`_` may appear between digits and is ignored, so `1_000_000` is the same integer as
`1000000`. A `.` is part of a float only when a digit follows it, so `1.foo` is an integer
followed by a dot. An `e` or `E` makes the literal a float, with an optional sign after it.

## Strings and characters

A string is delimited by `"` and a character by `'`. Both report `Unterminated string` or
`Unterminated char` when the closing delimiter is missing before the end of the line or the
file.

Escapes are read in both, and include `\xNN` for a byte written as two hex digits — the
standard library uses it for ANSI colour codes. An unrecognised escape is a warning and the
backslash is kept, rather than the sequence being dropped.

## What is not a token

A comment, a space and a newline produce nothing. There is no preprocessor, no line
continuation and no token pasting: what the lexer reads is what the parser sees.
