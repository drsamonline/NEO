# Standard Library

The standard library is written in NOVA under `stdlib/`:

- `core/core.nova` — math and collection helpers (`factorial`, `fibonacci`,
  `sign`, `clamp`, `gcd`, `compose`, array helpers).
- `io/io.nova` — input/output helpers.
- `text/text.nova` — string helpers such as `join`, `split`, `substr` and word
  capitalisation.
- `math/math.nova` — numeric helpers.

All four modules lex and parse in `compiler/tests/stdlib_parse.rs`, and all four
emit C in CI. `use core;` resolves beside the importing source or through
`stdlib/<name>/<name>.nova` in an ancestor directory. The exercised subset is
covered by end-to-end tests (for example `compose` with closures); not every
helper is executed on every backend.
