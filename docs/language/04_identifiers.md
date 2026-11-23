# Identifiers

An identifier names a binding, a function, a type, a field, a module or a variant.

## The rule

An identifier **starts** with an ASCII letter or `_`, and **continues** with any
alphanumeric character or `_`:

```
identifier = ( letter | "_" ) { letter | digit | "_" } ;
letter     = "a".."z" | "A".."Z" ;
digit      = "0".."9" ;
```

The two halves of that rule are not the same rule, and the difference is deliberate to
state rather than to discover:

| Position | Accepted | Where |
|---|---|---|
| first character | ASCII letters and `_` only | the lexer's dispatch |
| later characters | any alphanumeric, plus `_` | `read_identifier` |

The continuation test is Unicode-aware, so `café` is a valid identifier while `écaf` is not:
the first character is outside the dispatch table and becomes `Unknown(char)`. Widening the
first-character rule is a change to the language, not a bug fix, so it is recorded here
rather than quietly supported.

## What an identifier is not

- **Not a keyword.** The twenty-five reserved words in [Keywords](03_keywords.md) lex as
  their own token kinds, so `let` cannot name anything. Type names such as `int` and `string`
  are *not* keywords — they are ordinary identifiers recognised in type position, which is
  why `string` can also be a variable.
- **Not a number.** A digit cannot start an identifier, so `1st` is an integer followed by
  an identifier.
- **Not a path.** `a.b` and `a::b` are one identifier, a dot or a double colon, and another
  identifier; the parser decides what the sequence means.

## `_` on its own

A single `_` lexes as an ordinary identifier, and the parser treats it as a pattern that
binds nothing — a wildcard. It is not a keyword and not a distinct token kind, so a program
may not use `_` as the name of a binding it intends to read.

## Case and style

The lexer is case-sensitive and attaches no meaning to case: `Name`, `name` and `NAME` are
three identifiers.

The conventions this reference uses — types in `UpperCamelCase`, functions and bindings in
`snake_case`, constants in `UPPER_SNAKE_CASE` — are conventions only. **Nothing enforces
them**: not the lexer, not the parser, and not the formatter, which reformats whitespace and
leaves identifiers exactly as written. A program in which every name is `x1`, `x2`, `x3`
compiles and runs.
