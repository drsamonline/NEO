# `text`

59 functions over strings: `split`, `split_whitespace`, `join`, `trim`,
`trim_left`, `trim_right`, `starts_with`, `ends_with`, `replace`, `repeat`, and
case conversion.

Strings are byte sequences, and `split` returns a real `[String]` — arrays of `char *`
exist now, so the collection-returning functions hold their strings rather than truncating
pointers into `int` slots.

**This module does not compile yet.** String indexing and `as` casts are both done
([ADR 0007](../../docs/adr/0007-string-indexing.md), [ADR 0008](../../docs/adr/0008-casts.md)),
which took it from 50 errors to 12. The twelve are two other gaps: an `if` used as a value
does not report its type and loses the `else` of a chain, and arrays of arrays (`[[int]]` in
`levenshtein`) have no representation. See [`../not-compiling.txt`](../not-compiling.txt).

**54 of its 59 functions can run** — the best of the four modules, because most of it
is pure NOVA with no builtin underneath. Numbers in
[`../README.md`](../README.md).
