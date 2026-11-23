# ADR 0011 — `int` is 64-bit, in variables as well as literals

- **Status:** accepted
- **Date:** 2026-11-23
- **Related:** [0006](0006-array-element-types.md)

## Context

`docs/language/06_types.md` says, of `int` and every sized spelling of it:

> **The widths are accepted and ignored.** `i8` and `u64` are both `int`, so writing one
> does not get you its range — it gets you a **64-bit signed value**.

The compiler did not. `type_to_c_str` mapped `int` to C `int`, which is 32-bit on every
target this compiler supports. A literal reached `print` as a 64-bit value because the
literal was emitted directly into a `long long` parameter, so the two disagreed:

```nova
print(3000000000);            // 3000000000
let big = 3000000000;
print(big);                   // -1294967296
```

A variable silently truncated a value the same program printed correctly one line earlier.
The same held for arrays: `array_push(a, 3000000000)` stored 32 bits and `a[0]` read them
back negative.

This surfaced while adding `crc32`, whose IEEE polynomial result for `"123456789"` is
`0xCBF43926` — a value that does not fit in a signed 32-bit int, so a correct checksum
returned a negative number.

## Options

**1. Change the documentation to say 32-bit.** Rejected: `nova_print_int`, `nova_array_len`,
`nova_file_size` and most of the runtime already take and return `long long`. The runtime
was written for a 64-bit `int`; the backend was the outlier.

**2. Keep 32-bit scalars and add `i64` as a distinct type.** Rejected: the reference
explicitly promises the widths collapse, and doing this makes `int` mean something
different from what every existing NOVA program assumes.

**3. Emit `long long` for `int`, and widen `NovaArray` to match.** Accepted.

## Decision

`type_to_c_str` maps every integer spelling to `long long`, and `NovaArray`'s element
storage becomes `long long *`.

The array half is not optional. Widening scalars alone would leave `let x = a[0]` truncating
while `let x = 3000000000` did not — a *new* inconsistency in place of the old one. The
element type and the scalar type have to agree, or the truncation just moves.

`NovaArray_float` and `NovaArray_string` are untouched: they hold `double` and `char *`,
neither of which is affected.

## Consequences

- A `[int]` is now 8 bytes per element instead of 4, so a large array doubles in size. That
  is the cost of the documented type, and it is visible in `sizeof(NovaArray)`.
- Every integer in every program is 64-bit, including loop counters and array indices.
  Nothing in the test suite changed behaviour; the 285 tests passed with no edits.
- The C ABI of the runtime changed: `nova_array_from`, `nova_array_get`, `nova_array_set`,
  `nova_array_push` and `nova_array_pop` take and return `long long`, and emitted array
  literals are `(long long[]){...}`. A stale `.c` file from before this change will not link,
  which is the correct failure.
- `int` still cannot represent values above 2^63−1. The sized spellings remain synonyms, as
  the reference says; giving them ranges is a separate change that this does not block.
