# `text`

59 functions over strings: `split`, `split_whitespace`, `join`, `trim`,
`trim_left`, `trim_right`, `starts_with`, `ends_with`, `replace`, `repeat`, and
case conversion.

Strings are byte sequences, and `split` returns a real `[String]` — arrays of `char *`
exist now, so the collection-returning functions hold their strings rather than truncating
pointers into `int` slots.

**This module does not compile yet**, and string indexing is no longer why. `s[i]` on a
`String` is the byte at `i`, typed `char` ([ADR 0007](../../docs/adr/0007-string-indexing.md)),
which is 38 of the module's original 50 errors. The twelve that remain are three other
gaps: `as` casts are a no-op, arrays of arrays (`[[int]]` in `levenshtein`) have no
representation, and an `if` used as a value does not report its type. See
[`../not-compiling.txt`](../not-compiling.txt).

**54 of its 59 functions can run** — the best of the four modules, because most of it
is pure NOVA with no builtin underneath. Numbers in
[`../README.md`](../README.md).
