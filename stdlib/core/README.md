# `core`

The module every program can reach with `use core;`. 60 functions and the maths
helpers the tests exercise by name: `factorial`, `fibonacci`, `sign`, `clamp`, `gcd`.

It also wraps the compiler's builtins — `print`, `panic`, `typeof`, `strlen`,
`array_push` and the rest — so source can call a function instead of a
`__builtin_*`. Both work.

A large part of this module is scaffolding: it declares more than the backends
currently lower. `DEV_PLAN.md` → "Known limitations" is the authority on which
builtins trap.
