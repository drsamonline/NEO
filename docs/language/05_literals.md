# Literals

The forms a value can be written in. The lexical detail is in
[the lexical grammar](01_lexical_grammar.md); this chapter is about what each form
*means* once it is a token.

## Integers

`42`, `1_000`, `0xFF`, `0b1010`, `0o777`. Stored as `i64` by the lexer, and typed `int`
in the language — every integer width collapses to one type, so a literal has no
suffix and no width of its own.

A literal that does not fit in `i64` is a lexical error naming the text.

## Floats

`1.5`, `1e9`, `1.5e-3`, `1_0.5`. Any `.` or `e`/`E` after a digit makes the literal a
float. Typed `float`.

## Strings

`"hello"`, `"नमस्ते"`, `"line\ntwo"`. A string is a sequence of characters; it is
single-line, and a raw newline inside one is an error. The escape table is in
[the lexical grammar](01_lexical_grammar.md#strings).

Non-ASCII content is preserved and is compared byte-for-byte across all four backends
by [`tests/parity/unicode_string.nova`](../../tests/parity/unicode_string.nova).

## Characters

`'a'`, `'\n'`, `'ક'`. Typed `char`. A character literal holds one character, which
may be multi-byte in UTF-8.

## Booleans

`true`, `false`. Typed `bool`. These are keywords, not identifiers.

## Collections

There is no literal syntax for an array or a tuple type, but both can be written:

```nova
let numbers: Array = [5, 2, 8];
let pair = (1, 2);
```

An array literal produces an array of `int` elements — arrays are `int`-only today,
which is a limitation rather than a design (`DEV_PLAN.md`).

## What a literal's type comes from

A literal has no type of its own until its context gives it one. `1` is an `int`
because nothing said otherwise; the same source in a position expecting `float` relies
on the one implicit conversion the language has, `int` → `float`.

## Where this is tested

`compiler/src/lexer/mod.rs` for the forms, `tests/diagnostics/` for the errors, and
the parity corpus for the values surviving all four backends.
