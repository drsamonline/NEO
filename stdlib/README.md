# The NOVA standard library

Four modules, 250 functions. **196 of them can run; 54 cannot**, and this page is the
only place that says which.

| Module | What it is |
|---|---|
| [`core`](core/README.md) | printing, arrays, strings, maths helpers, `Range`, the builtin wrappers |
| [`io`](io/README.md) | files, streams, paths, terminal |
| [`math`](math/README.md) | trigonometry, rounding, powers, roots |
| [`text`](text/README.md) | splitting, joining, trimming, case, search |

## What actually works

<!-- stdlib-check:begin -->
| Module | Functions | Usable | Blocked |
|---|---|---|---|
| `core` | 55 | 49 | 6 |
| `io` | 60 | 18 | 42 |
| `math` | 76 | 75 | 1 |
| `text` | 59 | 54 | 5 |
| **total** | **250** | **196** | **54** |
<!-- stdlib-check:end -->

The compiler knows **forty-three** `__builtin_*` functions. A stdlib function whose body
calls any other name is a function the compiler cannot lower — it is scaffolding,
written against builtins that were planned and not implemented.

**"Usable" is the strongest claim available here, and it is a weak one.** It means the
function calls no builtin the compiler does not implement. It does **not** mean the
function is correct: the stdlib has no tests, which is a separate and larger gap. A
function that is usable has been read, not run.

`math` went from 48 usable to **75** when the floating-point builtins landed, and
`io` gained three parsing functions. `io` is still the worst case: 42 of its 60
functions call builtins that do not exist (`__builtin_fopen`, `__builtin_fread_line`,
`__builtin_stat`, `__builtin_mkdir` and the rest), so most of that module cannot be
called at all.

## Keeping this true

`tools/stdlib-check.py` derives the table from the source — it reads the builtin names
out of `compiler/src/sema` and `compiler/src/lowerer`, then walks every function body —
and fails the build if this page disagrees with it.

```bash
python3 tools/stdlib-check.py            # compare
python3 tools/stdlib-check.py --print    # regenerate the table above
```

So a new stdlib function that needs a missing builtin cannot quietly join the
"usable" column, and implementing a builtin shows up as a number moving.

## Why the scaffolding is here at all

Because the modules document intent: `io` says what file handling should look like,
`math` says what the numeric surface should be. That is worth having while the
language is being built. It is not worth *claiming* — which is why this page exists,
why each module's own README says what it is, and why `DEV_PLAN.md` → "Known
limitations" carries the same fact.

## What is missing entirely

No tests. There is no per-function test anywhere in `stdlib/`, and no CI job that
calls a stdlib function other than the ones `examples/` and `tests/parity/` happen to
use. That is the next real piece of work here, and it needs the builtins first.
