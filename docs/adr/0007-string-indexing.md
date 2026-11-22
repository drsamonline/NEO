# ADR 0007 — `s[i]` is a byte, and `char` is one byte whether the spec says so or not

- **Status:** accepted
- **Date:** 2026-11-22
- **Related:** [0006](0006-array-element-types.md)

## Context

`stdlib/text` does not compile. It was in `stdlib/not-compiling.txt` for a year of
sessions with the reason "`[String]` arrays do not exist". That was half of it, and
[ADR 0006](0006-array-element-types.md) fixed that half. The other half is this: `text`
indexes a `String`.

```nova
fn to_upper(s: String) -> String {
    let mut i = 0;
    while i < strlen(s) {
        let c = s[i];
        if c >= 'a' && c <= 'z' { ... }
```

`s[i]` currently lowers to `nova_array_get(s, i)`, and `s` is a `char *`, not an array.
That is 35 of the module's 50 remaining C errors, and most of the rest follow from it: a
`char` read this way types as `int`, so three functions return `Option_int` where they
declare `Option_char`.

**The question is not how to lower it. It is what `s[i]` means.** The language documents
say two things that do not agree with each other or with the code:

- `docs/language/05_literals.md` says *"A string is a sequence of characters"* and
  *"A character literal holds one character, which may be multi-byte in UTF-8."*
- The implementation says a `char` is a C `char`: `c_type` maps `"char"` to `"char"`, and
  `SirLiteral::Char` renders through `escape_c_char`.

**Measured, not inferred.** `print('क')` prints `\x15` — the low byte of a three-byte
UTF-8 sequence. So the sentence in the spec is not true of the compiler, and has not been
for as long as `char` has been a `char`. That defect is not this ADR's to fix, but it
decides this ADR: **a `char` cannot hold a codepoint today, so `s[i]` cannot return one
without first changing what `char` is.**

## Options

**(a) A byte.** `s[i]` is the byte at offset `i`, typed `char`, bounds-checked.

Consistent with everything the string runtime already does: `strlen` counts bytes,
`substr` slices bytes, `nova_fn_strcmp` compares bytes, and the parity test compares
"byte-for-byte". `text` is written against exactly this — every loop is
`while i < strlen(s)` with `s[i]` compared to an ASCII literal. Cheap: one runtime
function, one lowerer arm, no type changes.

Cost: on non-ASCII text `to_upper("नमस्ते")` returns its input unchanged, because no
single byte is in `a`–`z`. That is a real limitation and it would be silent.

**(b) A codepoint.** `s[i]` is the `i`-th codepoint, typed `char`.

What a reader of the spec would expect, and what makes `to_upper` work on Devanagari.
Cost: `char` must become a 32-bit type that holds a codepoint. That changes the C type,
the literal rendering, comparison, `tostring`, the `Option<char>` payload struct, and all
four backends — and it is a language-visible change to what a character *is*, not an
implementation detail of indexing. It is a bigger decision than the one being asked, and
taking it here would mean deciding `char` by accident.

**(c) Reject.** `s[i]` on a `String` is a diagnostic; callers use a function.

Honest under the constitution's rule 3, and it is what happens today — badly, by way of a
C error. But it leaves `text` uncompilable for a reason the language has not actually
decided against, and byte access is genuinely needed to build the codepoint function that
(b) would want anyway. A `char_at` implemented in NOVA needs `s[i]` to be a byte.

## Decision

**(a), now.** `s[i]` on a `String` is the byte at `i`, typed `char`, bounds-checked
against `strlen` with the same `nova_fail` the array accessors use.

Three reasons, in order of weight:

1. **It is what the code already assumes.** `strlen` is the loop bound in every function
   in `text`; a codepoint index would need a codepoint count to bound the same loop, and
   `strlen` would silently disagree with it.
2. **It does not decide what `char` is.** (b) does, and that decision should be made
   deliberately, in an ADR about `char`, with the truncation above as its motivation.
3. **It unblocks the thing that (b) will need.** The codepoint-aware `char_at` is a NOVA
   function over bytes. Without byte indexing there is nothing to write it on.

**What this costs, stated plainly:** `s[i]` is a byte, so it is not a character on any
non-ASCII input. Anything built on it — `text`'s `to_upper`, any `char_at` — is correct for
ASCII and silent about the rest, and a reader who assumes otherwise gets no warning. That
is recorded here and in `stdlib/text/README.md` rather than discovered by a user.

**Assignment is rejected.** `s[i] = c` on a `String` lowers to `nova_unsupported`, which
names the construct and exits. A `char *` from a literal may be read-only memory, so
writing through it is not something this compiler should emit C for.

## What this ADR does not decide

- **Whether `char` should hold a codepoint.** It should probably be asked: the spec
  already says it does, and it does not. That ADR would supersede option (b) here.
- **Whether indexing should be bounds-checked or total.** It is checked, matching arrays,
  because a silent read past the terminator is the class of bug the accuracy gates exist
  to catch.
- **Whether `[String]` elements are owned or borrowed.** [ADR 0006](0006-array-element-types.md)
  left that open and this does not close it.

## What this did not fix, measured

`nova-compiler emit stdlib/text/text.nova` and a C compiler: **50 errors before, 12
after.** `text` still does not compile, and none of the twelve are string indexing. They
are three separate pre-existing defects that this work uncovered by getting the module far
enough to reach them:

1. **`as` is a no-op.** `Expr::Cast(inner, _) => self.lower_expr(inner)` in the lowerer —
   the cast is dropped, and no `Cast` arm exists in `expr_type`, so a cast expression
   reports no type at all. That is why `tostring((c as int - 32) as char)` in `to_upper`
   still produces the code `"69"` rather than `"E"`: the value never becomes a `char`.
   It is also behind the two `expected expression` errors in `from_hex`. Making casts real
   changes codegen for every `as` in the language, so it wants its own ADR rather than a
   line in this one.
2. **Arrays of arrays do not exist.** `levenshtein` declares `let mut matrix: [[int]] = []`
   and writes `matrix[i][j] = cost`. `NovaArray` holds `int`, so `[[int]]` has no
   representation — the same gap [ADR 0006](0006-array-element-types.md) closed for
   `float` and `String`, one level deeper.
3. **An `if` used as a value does not report its type.** `format_number`'s
   `let digits = if is_negative { substr(...) } else { s };` types as `int` and emits
   `int digits = ...` for a `String`.

Two of these are the same lost-type defect as the one that blocked `[float]`. They are
recorded here because discovering them was this ADR's work, and because `not-compiling.txt`
now names them instead of blaming arrays.

`print('क')` printing `\x15` is a run, not a reading. The `strlen`-as-loop-bound claim is
`grep -c 'strlen(s)' stdlib/text/text.nova` = 52.
