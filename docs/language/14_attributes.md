# Attributes

NOVA has **no attributes**. There is no `@name`, no `#[name]`, and no mechanism for attaching
metadata to a declaration. A `@` at the start of a line is a parse error, not a marker the compiler
reads:

```nova
@derive(Clone)
struct P { x: int }
```

```text
error: parse errors:
  prog.nova:1:1: Expected expression at '@'
```

This chapter exists to say that once, so a reader does not look for a derive list, a `#[test]`, or
a `#[inline]` that could change code generation. There are none.

## What is reserved instead

The grammar keeps a section of **reserved words and operators** — spellings that cannot be used as
identifiers, so that a program written today keeps its meaning if they are implemented later. Three
of them are reserved and *rejected* by the parser: `type`, `pub` and `loop`; so are the compound
assignment operators `+=`, `-=`, `*=`, `/=`, `^` and `~`.

[Keywords](03_keywords.md) lists them with what each one does, checked by compiling a program that
uses it. None of them is an attribute; they are the closest thing the language has to "declared but
not yet meaningful", and they are the reason a future attribute syntax has somewhere to go.

## What a program does instead

Nothing — and that is the honest answer. Every behaviour an attribute would normally choose is
fixed:

| Usually an attribute | In NOVA |
|---|---|
| `@derive(Debug)` | no printing of a struct beyond its fields, by hand |
| `@inline` | the C compiler decides |
| `@test` | tests are `#[test]` functions in Rust, calling the compiler as a library |
| `@deprecated` | nothing; a name is a name |
| `@repr(C)` | the layout is whatever C gives the struct |

## If this changes

An attribute is a surface with a grammar, a parser rule, a place to store what it means, and a
decision about which ones the compiler honours versus which it records and ignores. Those have to
be chosen together, and the last one is the trap: an attribute the compiler records but does not
act on is a promise it does not keep. Until that is decided and written in `docs/adr/`, there are
no attributes, and this chapter is the specification.
