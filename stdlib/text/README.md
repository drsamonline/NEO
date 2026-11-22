# `text`

59 functions over strings: `split`, `split_whitespace`, `join`, `trim`,
`trim_left`, `trim_right`, `starts_with`, `ends_with`, `replace`, `repeat`, and
case conversion.

Strings are byte sequences, and `split` returns a real `[String]` — arrays of `char *`
exist now, so the collection-returning functions hold their strings rather than truncating
pointers into `int` slots.

**This module does not compile yet**, and the reason is no longer the array: indexing a
`String` (`s[i]`) lowers to `nova_array_get`, which wants an array. That is a
representation decision — byte, codepoint or grapheme — and wants its own ADR. See
[`../not-compiling.txt`](../not-compiling.txt).

**54 of its 59 functions can run** — the best of the four modules, because most of it
is pure NOVA with no builtin underneath. Numbers in
[`../README.md`](../README.md).
