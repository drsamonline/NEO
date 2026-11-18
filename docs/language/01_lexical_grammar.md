# Lexical grammar

How source text becomes tokens. Every rule here is taken from
[`compiler/src/lexer/mod.rs`](../../compiler/src/lexer/mod.rs); the grammar in
[`grammar/grammar.ebnf`](../../grammar/grammar.ebnf) describes the tokens that come
out, and `tools/grammar-check.py` keeps the two describing the same language.

## Source encoding

Source is UTF-8 and may contain any character — in comments, in identifiers, in
string literals. This is tested rather than assumed:
[`tests/parity/unicode_string.nova`](../../tests/parity/unicode_string.nova) prints
`नमस्ते` and `કેમ છો` and compares the output byte-for-byte across all four backends.

Positions are counted in **characters**, not bytes: the lexer walks a `Vec<char>` and
records line and column from 1. Anything that slices source by byte offset is a bug —
one that silently corrupted every token after the first non-ASCII character until
2026-11-18.

## Whitespace

Space, tab, carriage return and newline separate tokens and are otherwise ignored.
There is no significant indentation.

## Comments

Three forms, all line-ending except the last:

```nova
// a line comment
# a line comment
/* a block comment */
```

`#` exists because the standard library's file headers use it. `/* */` does not nest.
A comment is not a token: it is skipped, so nothing downstream can see one.

## Identifiers

An identifier starts with any alphanumeric character or `_` and continues with the
same. Because the test is `char::is_alphanumeric` rather than `is_ascii_alphanumeric`,
identifiers may contain non-ASCII letters — `क` and `ક` are both identifier
characters. A leading digit makes a number instead.

There is no restriction on an identifier starting with `_`.

## Numbers

| Form | Example | Notes |
|---|---|---|
| Decimal integer | `123`, `1_000` | `_` may appear anywhere after the first digit |
| Hexadecimal | `0xFF`, `0xdead_beef` | prefix `0x` or `0X` |
| Binary | `0b1010` | prefix `0b` or `0B` |
| Octal | `0o777` | prefix `0o` or `0O` |
| Float | `1.5`, `1_0.5`, `1e9`, `1.5e-3` | a `.` or an `e`/`E` makes it a float |

An exponent may carry a sign. `1e9` is a float even though it has no `.`.

Two consequences worth knowing:

- `_` is stripped before the value is parsed, so `1_0` is `10`.
- The lexer's exponent branch is entered on any `e` or `E` after a digit, so an
  identifier beginning with `e` immediately after a number is absorbed into it.
  `1efoo` lexes as a malformed float, not as `1` followed by `efoo`.

An integer literal is an `i64`. A literal that does not fit is a lexical error
naming the text; a radix literal with a bad digit is an error naming the radix.

## Strings

Double-quoted, single-line. A newline inside a string is an unterminated-string error
rather than a continuation.

| Escape | Result |
|---|---|
| `\n` `\r` `\t` | newline, carriage return, tab |
| `\\` `\"` `\'` | backslash, double quote, single quote |
| `\0` | NUL |
| `\b` `\f` `\v` `\a` | backspace, form feed, vertical tab, bell |
| `\xNN` | the byte `NN` in hex — the standard library uses it for ANSI colours |

An unknown escape is a warning, not an error: the backslash and the character are
kept as written.

## Characters

A single-quoted character literal, with the escapes `\n \r \t \0 \\ \' \b \f`. A
missing closing quote is an error.

## Operators and punctuation

`+ - * / % = == != < > <= >= && || ! & | ^ ~ << >> += -= *= /=`
`( ) { } [ ] , ; : :: . .. -> =>`

Reserved but unused: the grammar records three reserved keywords and six reserved
operators. `tools/grammar-check.py` counts them; none is accepted in source today.

## What is not here

- No raw strings, no interpolated strings, no multi-line strings.
- No numeric suffixes (`1u8`). A literal's type is its context's.
- No doc-comment syntax that the compiler reads. `///` is an ordinary `//` comment.

## Where this is tested

`compiler/src/lexer/mod.rs` unit tests, `tests/diagnostics/` for the errors,
`tests/parity/unicode_*.nova` for UTF-8, and `fuzz/fuzz_targets/lex.rs`, which asserts
on every input that each diagnostic has a message and a 1-based span.
