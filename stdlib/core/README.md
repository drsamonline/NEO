# `core`

The module every program can reach with `use core;`. 60 functions and the maths
helpers the tests exercise by name: `factorial`, `fibonacci`, `sign`, `clamp`, `gcd`.

It also wraps the compiler's builtins — `print`, `panic`, `typeof`, `strlen`,
`array_push` and the rest — so source can call a function instead of a
`__builtin_*`. Both work.

**49 of its 55 functions can run**; 6 call a builtin the compiler does not implement.
The authoritative numbers are in [`../README.md`](../README.md), derived from the
source by `tools/stdlib-check.py` and checked in CI.
