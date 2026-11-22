# ADR 0006 — arrays hold `int`, and three of the four stdlib modules want otherwise

- **Status:** accepted for `[float]`; `[String]` is the follow-up
- **Date:** 2026-11-18
- **Decided:** 2026-11-22
- **Related:** [0003](0003-option-payload.md), [0004](0004-deferred-global-initialisers.md)

## Context

`NovaArray` is:

```c
typedef struct {
    int *data;
    long long len;
    long long cap;
} NovaArray;
```

Every array is an array of `int`, and the whole array API says so:

```c
NovaArray nova_array_from(const int *items, long long len);
int       nova_array_get(NovaArray array, long long index);
void      nova_array_set(NovaArray array, long long index, int value);
void      nova_array_push(NovaArray *array, int value);
int       nova_array_pop(NovaArray *array);
```

The standard library asks for more. Counted by `tools/stdlib-check.py`, which reports
declared array types with a non-`int` element — declarations only, so `arr[i]` does not
count:

| Module | Declared `[T]` where `T` is not `int` |
|---|---|
| `core` | 0 |
| `io` | 4 |
| `math` | 7 |
| `text` | 9 |

`text`'s nine are the reason it does not compile: `join(strings: [String])` passes an
`int` where a `char *` is wanted. `math`'s seven are the reason it computes the wrong
answer. `core`'s one — `args() -> [String]` — is in a function no example calls.

**Two consequences, one of them a correctness problem rather than a missing feature.**

`stdlib/text` does not compile: `join(strings: [String])` passes an `int` where a
`char *` is wanted, because indexing a `[String]` yields an `int`.

`stdlib/math` **does** compile, and is quietly wrong:

```nova
fn sum_float(arr: [float]) -> float {
    let mut total = 0.0;
    ...
        total = total + arr[i];
```

`arr[i]` is `nova_array_get`, which returns `int`. A `[float]` array cannot exist, so
whatever the caller built was truncated to integers on the way in. `mean`, `variance`,
`stddev` and `median` all compute on those. **A module that compiles and returns wrong
numbers is worse than one that does not compile**, and nothing in the gates catches it
because the gate only asks whether the C compiles.

**Measured, not inferred.** `sum_float([1.5, 2.5])` returns **3**, not 4 — the literals
were truncated to `1` and `2` before anything summed them. The test
`a_float_array_sum_is_not_truncated` in `compiler/tests/end_to_end.rs` asserts the correct
answer and is `#[ignore]`d with this defect as its reason, so it stays runnable:

```bash
cargo test --test end_to_end -- --ignored a_float_array_sum
```

## Options

**(a) `void *data` plus an element size.**

```c
typedef struct {
    void *data;
    long long len;
    long long cap;
    size_t elem;
} NovaArray;
```

One set of runtime functions, `memcpy` in and out. Costs: every `nova_array_get` in
generated code becomes an out-parameter or a cast at the call site, the ABI test that
pins the three-field layout changes, and all four backends construct arrays differently.

**(b) Monomorphise — `NovaArray_int`, `NovaArray_string`, `NovaArray_float`.**

Consistent with what `Option` already does ([ADR 0003](0003-option-payload.md)), and the
lowerer knows the element type at every construction site. Costs: three copies of six
runtime functions, and `array_len`-style builtins need the element type to pick one.

**(c) Keep `int`-only and fix the standard library to match.**

`join` could take a delimiter and build the string directly; `sum_float` could take a
`String` and parse. This is the smallest change and the worst outcome: it shapes the
language around a runtime limitation and leaves `[float]` accepted by the type checker
while being silently truncated.

**(d) Reject non-`int` arrays in the analyser.**

Honest, cheap, and it turns the silent `math` bug into a diagnostic. It does not make
`text` compile, but it stops the compiler lying.

## Recommendation

**(d) now, (b) next.**

(d) is a day and it removes a wrong-answer bug, which matters more than a missing module.
(b) is the shape the codebase already uses for `Option`, so the pattern and its tests
exist. (a) is the C-idiomatic answer and the wrong fit here: this compiler generates
typed code and monomorphising is what it already does.

Doing (b) before (d) leaves the `math` bug live for as long as the work takes.

## What this ADR does not decide

Whether `[T]` should be growable for non-`int` `T`, whether strings in arrays are owned
or borrowed (the runtime leaks every `nova_strdup`, so "owned" is the status quo), and
whether the four backends monomorphise or share one representation. Those are (b)'s
problem and want their own record.

## Outcome

**Option (b) was taken, for `[float]` only, and (d) was not needed.** Making the
representation correct removed the wrong answer that (d) existed to prevent, so a
diagnostic would have been a stop-gap for a bug that no longer exists.

What the implementation cost, measured rather than estimated:

| Part | Where | Size |
|---|---|---|
| `NovaArray_float` and its six runtime functions | `compiler/src/runtime.rs` | 82 lines |
| `c_type` maps `[float]`/`[f32]`/`[f64]`/`Array<float>` to it | `compiler/src/backend/mod.rs` | 27 lines |
| `array_len`, `array_push`, `array_pop`, `arr[i]` pick the float runtime | `compiler/src/lowerer/mod.rs` | 58 lines |
| Generic binding of a parameter's type *pattern* | `compiler/src/lowerer/mod.rs` | 1 function |

**The runtime was the easy half and it was not the blocker.** The branch that added
`NovaArray_float` compiled and emitted `NovaArray_float` for every `[float]` parameter
correctly, then failed with:

```
error: incompatible type for argument 1 of 'array_len_int'
```

`stdlib/core` declares `fn array_len<T>(arr: [T]) -> int`. The parameter's type is `[T]`,
and `[T]` is stored in the type's *name* with an empty `generics` list, so the binding
step — which compared `param.typ.name` against the generic list — never saw `T`. Every
instantiation fell through to the `int` default and `math` emitted `array_len_int` called
with a `NovaArray_float`.

This is the recurring defect this repository keeps meeting, in its sixth form: **a type
string that loses its arguments between two places that both need it.** The fix is that
binding now matches the parameter's type as a *pattern* — `[T]` against `[float]` binds
`T` to `float`, `&mut [T]` against `&mut [float]` does the same — instead of comparing
names. It is one function, `bind_type_pattern`, and it fixes the same loss for
`array_push`, `array_pop`, `array_get`, `array_set`, `find`, `filter` and `reverse`,
which had been silently instantiating as `_int` too.

**What it cost in gates.** Three tests, because the existing gate could not see this:
`array_len_int` was valid C that compiled. Two unit tests assert the instantiation *name*
(`array_len_float`, `reverse_float`) and one end-to-end test asserts all five numbers —
`sum_float` 4, `mean` 2, `median` 2, `variance` 0.25, `stddev` 0.5. The lesson is the
repository's own: a compile gate answers whether the C compiles, and this defect was C
that compiled.

**What is still not done.** `[String]` arrays, which need `NovaArray_string` rather than
a cast because a `char *` is eight bytes. `stdlib/text` does not compile for that reason
and stays in `stdlib/not-compiling.txt`. The `int` default in `resolve_generic_call`
remains for a generic parameter no argument determines — `deserialize<T>(s: String)`,
which has no way to name `T` at a call site.

## Where this is measured

`tools/stdlib-check.py` counts functions per module; the `[String]`/`[float]` counts above
came from `grep` over `stdlib/*/*.nova`. The `math` truncation follows from
`nova_array_get`'s signature and `sum_float`'s body, both quoted here.
