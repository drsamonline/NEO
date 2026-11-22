# `text`

59 functions over strings: `split`, `split_whitespace`, `join`, `trim`,
`trim_left`, `trim_right`, `starts_with`, `ends_with`, `replace`, `repeat`, and
case conversion.

Strings are byte sequences, and `split` returns a real `[String]` — arrays of `char *`
exist now, so the collection-returning functions hold their strings rather than truncating
pointers into `int` slots.

**This module compiles and its tested functions are correct.** It was the last module
outside the compile gate, and it took four separate fixes to get there: `[String]` arrays,
`s[i]` on a `String` ([ADR 0007](../../docs/adr/0007-string-indexing.md)), `as` casts
([ADR 0008](../../docs/adr/0008-casts.md)) and arrays of arrays
([ADR 0009](../../docs/adr/0009-nested-arrays.md)).

Two of its defects were only visible by running it, not by compiling it: `split` was an
infinite loop because a `while` body whose only item is an expression was dropped, and
`==` on two `String`s compared pointers rather than contents, so `split` never matched a
delimiter. `to_upper`, `trim`, `split`, `join` and `levenshtein` are asserted end to end in
`compiler/tests/end_to_end.rs`; the rest are not.

**54 of its 59 functions can run** — the best of the four modules, because most of it
is pure NOVA with no builtin underneath. Numbers in
[`../README.md`](../README.md).
