# `text`

59 functions over strings: `split`, `split_whitespace`, `join`, `trim`,
`trim_left`, `trim_right`, `starts_with`, `ends_with`, `replace`, `repeat`, and
case conversion.

Strings are byte sequences; `split` returns `[String]`, and arrays are `int`-element
only today, so anything returning a collection is scaffolding. Check
`DEV_PLAN.md`'s limitations before relying on one.
