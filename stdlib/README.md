# The NOVA standard library

Four modules, 249 functions. **211 of them can run; 38 cannot**, and this page is the
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
| `core` | 55 | 54 | 1 |
| `io` | 59 | 49 | 10 |
| `math` | 76 | 75 | 1 |
| `text` | 59 | 59 | 0 |
| **total** | **249** | **237** | **12** |
<!-- stdlib-check:end -->

The compiler knows **forty-three** `__builtin_*` functions. A stdlib function whose body
calls any other name is a function the compiler cannot lower — it is scaffolding,
written against builtins that were planned and not implemented.

**`math`'s float-array functions are correct.** `NovaArray` holds `int`, so a `[float]`
array used to be truncated before it was read: `sum_float([1.5, 2.5])` returned **3**.
Arrays of `double` now exist — `NovaArray_float` and its own runtime functions — and the
generic parameter inside `[T]` is bound to the element type at the call site. The
scalar and float-array functions are proved end to end by
`a_float_array_sum_is_not_truncated` and `float_array_statistics_are_not_truncated` in
`compiler/tests/end_to_end.rs`; [ADR 0006](../docs/adr/0006-array-element-types.md)
records what it cost.

**"Usable" is the strongest claim available here, and it is a weak one.** It means the
function calls no builtin the compiler does not implement. It does **not** mean the
function is correct. A function that is usable has been read, not run.

**About a hundred of the 249 distinct functions are now run.** `compiler/tests/stdlib.rs`
builds a program per module against the real library and asserts the values — `core`,
`text` and `math`, in nine tests, including the higher-order `map`/`filter`/`reduce` called
with closures. Every expected value is checkable by hand rather than recorded from a run;
the one exception is `hash_string`, which is asserted for *stability* rather than for a
digest, because a recorded digest is a change detector.

The rest are still read, not run, and that is the largest gap in the library. Writing the
first three tests found two live bugs within minutes, and the second batch found a third —
`for i in range(0, 4)` iterated zero times, and no example, parity program or stdlib
function contains a `for` at all, so nothing had ever exercised it.

**And it is per function, not per module.** The per-function number answers "does this
call a missing builtin", and nothing else.

**All four modules now compile** — `core`, `io`, `math` and `text` — so
`stdlib/not-compiling.txt` is empty. Getting there took six defects, and the last three
were found only by running the functions: `text` compiled while `split` was an infinite
loop and while `==` on two `String`s compared pointers. **A module that compiles is not a
module that works**, and this table measures neither of those things.

`text` is now **fully usable** — 59 of 59 — after the checksum, digest, encoding and
path builtins landed. `math` went from 48 usable to 75 when the floating-point builtins
did. `io` is still the worst case at 49 of 59, and what it is missing is not a backlog of
similar work: `serialize`/`deserialize` need a decision about the format, `args` needs the
emitted `main` to take `argc`/`argv`, and the rest are stream types.

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
