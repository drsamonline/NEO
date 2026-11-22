# Handover — where this is, and what to do next

Written 2026-11-22. Every number here came from a command or a CI log; where something
is unverified it says so.

## The repository

| | |
|---|---|
| `master` | `c4edb0b` |
| Tests | **259** passing, 0 failing, 0 ignored |
| Coverage | 66.32%, ratchet in `coverage-floor.txt` |
| Release | **v0.7.0** — three platform archives with SHA-256, verified by downloading and running one |
| ADRs | 6 |

## What is done

| Stage | State |
|---|---|
| 0 — one plan, true numbers | ✅ |
| 1 — prove the output correct (G1–G4) | ✅ |
| 2 — diagnostics as a contract | ✅ |
| 3 — harden the front end (G5–G7) | ✅ |
| 4 — syntax tree, formatter, guard, `trait` | ✅ |
| 5 — LSP nav, VS Code extension, path/git deps | ✅ |
| 6 — release and distribution | ✅ v0.7.0 published |
| 7 — long tail | 🟡 spec 8/25, stdlib truth ✅, debugger ⬜ |

### The standard library

| Module | Compiles | Functions | Usable |
|---|---|---|---|
| `core` | ✅ | 55 | 54 |
| `io` | ✅ | 59 | 28 |
| `math` | ✅ | 76 | 75 |
| `text` | ❌ | 59 | 54 |

## The wrong answer is gone

`sum_float([1.5, 2.5])` returned **3**, not 4, because `NovaArray` held `int` and a
`[float]` array was truncated before anything read it. `mean`, `variance`, `stddev` and
`median` all built on it. The module compiled and the compile gate passed, which is why
it survived: the emitted C was valid C.

`[float]` arrays now exist as arrays of `double` — `NovaArray_float` with its own six
runtime functions — and the four functions return 4, 2, 2, 0.25 and 0.5 for
`[1.5, 2.5]`. `[String]` arrays followed the same way as `NovaArray_string`, so `char *`
no longer goes into an `int` slot. [ADR 0006](adr/0006-array-element-types.md) is
`accepted` and records what each cost.

**Three element types is what turned two copies of the runtime into a pattern.** The
lowerer asks one question — `array_kind` — and `array_len`, `array_push`, `array_pop`,
`arr[i]` and `arr[i] = value` all dispatch on the answer. Adding a fourth element type is
now a header, six functions, and one match arm.

**The runtime was the easy half.** The blocker was one layer earlier: `stdlib/core`
declares `fn array_len<T>(arr: [T]) -> int`, and `[T]` is stored in the type's *name*
with an empty `generics` list, so the step that binds a call's type arguments to a
generic declaration never saw `T`. Every instantiation fell through to the `int` default
and `math` emitted `array_len_int` called with a `NovaArray_float`.

Binding now matches the parameter's type as a *pattern*: `[T]` against `[float]` binds
`T` to `float`, `&mut [T]` against `&mut [float]` does the same, and a bare `T` keeps the
argument's type exactly as given so `id<T>(x: T) -> T` is unchanged. A `[T]` pattern
binds only when the argument names an element type — a bare `Array`, which is what an
unannotated `let values = [1, 2]` types as, names none. One function fixed eight stdlib
functions: `array_len`, `array_push`, `array_pop`, `array_get`, `array_set`, `find`,
`filter` and `reverse`, all of which had been instantiating as `_int`.

**Two gates would have caught this and one did not.** The compile gate asked whether the
C compiled, and it did. The Accuracy gates — which compile and run every parity program
under ASan and UBSan — caught a regression introduced *while* fixing it, within minutes.
When something types as `int` unexpectedly, look for a type string that lost its
arguments between two places that both need it; that is now six defects of this shape.

## What is next, in order

**1. `as` is a no-op.** `Expr::Cast(inner, _) => self.lower_expr(inner)` in the lowerer:
the cast is dropped and no `Cast` arm exists in `expr_type`, so a cast reports no type at
all. `tostring((c as int - 32) as char)` in `stdlib/text`'s `to_upper` therefore produces
the code `"69"` instead of `"E"`, and `from_hex`'s nested casts produce two
`expected expression` errors. Making casts real changes codegen for every `as` in the
language, which is why it is an ADR before it is a patch.

**2. Arrays of arrays.** `levenshtein` declares `[[int]]` and writes `matrix[i][j]`.
`NovaArray` holds `int`, so a nested array has no representation — the gap ADR 0006 closed
for `float` and `String`, one level deeper, and the last thing between `text` and the
stdlib gate.

**3. An `if` used as a value does not report its type.** `format_number`'s
`let digits = if is_negative { substr(...) } else { s };` types as `int`.

Two of the three are the lost-type defect the rest of this page keeps meeting. All three
were found by ADR 0007 getting `text` far enough to reach them.

**4. 38 stdlib functions still call a builtin the runtime does not implement.** Mostly
`fopen`/`fclose`/`fread`/`fwrite` — a `FILE *` does not fit in the `int` that
`File.handle` declares, so these want a handle table or a changed `File`. The eleven that
were done on 2026-11-18 show the pattern: one shim function, one lowerer arm, one sema
type, and `tools/stdlib-check.py` proves the count moved.

**5. 17 specification chapters.** They describe surface that does not exist — ownership,
memory, concurrency, attributes, interfaces. Writing them would be inventing a
specification, which `docs/specs/constitution.md` forbids first.

**6. The DWARF debugger.** Lowest value-per-week in the plan. The REPL works.

## Working without a Rust toolchain

`static.rust-lang.org` is not reachable from the sandbox, so `rustup` cannot install
anything. **There is a better loop than the two below, and it was found on 2026-11-22:**
`conda.anaconda.org` *is* reachable, and conda-forge ships a complete stable toolchain as
plain tarballs. Extract them, put `bin` on `PATH`, and `cargo test` runs locally in
seconds instead of four minutes:

```bash
curl -sL https://conda.anaconda.org/conda-forge/linux-64/rust-<ver>-<build>.conda -o rust.conda
curl -sL https://conda.anaconda.org/conda-forge/noarch/rust-std-x86_64-unknown-linux-gnu-<ver>-<build>.conda -o std.conda
# each is a zip holding pkg-*.tar.zst; unzip, zstd-decompress, untar both into one prefix
```

There is no `cc` in the sandbox, so point the linker at Zig — `pip install ziglang`, then
`-C linker=<wrapper that execs 'zig cc'>`, or set `CC` for the tests. That is what made a
259-test suite run locally. What it cannot do: `cargo llvm-cov`, because the instrumented
link fails on `__llvm_profile_runtime` through Zig's linker. **Coverage stays a CI-only
measurement**, so run it there and read the number rather than predicting it.

The two older loops still work and are still useful:

**Build on GitHub, run here.** `.github/workflows/dev-build.yml` builds the workspace and
uploads the binaries as a seven-day artifact.

```bash
./scripts/dev-loop.sh <branch>     # dispatch → wait → download → set CC
```

**Compile C locally.** `pip install ziglang` gives Zig 0.17.1; `zig cc` compiles, links and
runs C with libm. Enough to prototype a codegen design, reproduce a C-level failure, and
check the runtime shim.

**Note:** `workflow_dispatch` through the API needs a token with `actions:write`; `gh`
gets a 403. Either use the Actions tab, or rely on the push-triggered workflows.

## Things that will cost you a cycle if you do not know them

- **`/tmp` is 64 MB in the sandbox** and fills with clones. Work in `/scratch/work`.
- **`-std=c11` is strict ISO**, so POSIX hides `struct timespec` and `nanosleep`. The
  runtime defines `_POSIX_C_SOURCE`; only the sanitiser job uses strict C, which is why
  the stdlib gate passes and the accuracy gate catches it.
- **A type table that drops information is the recurring defect.** Seven separate bugs
  now, including the one that made `[float]` arrays fail: a type string losing its
  generic arguments or a name losing its payload. When something types as `int`
  unexpectedly, look for `.name` where `type_display` was meant.
- **The stdlib gate compiles every module with `-Werror=builtin-declaration-mismatch`.**
  A NOVA function named after a C library function gets a `nova_fn_` prefix; do not
  reintroduce a collision.
- **`stdlib/not-compiling.txt` is a ratchet.** A module outside it failing is an error; a
  module inside it compiling is also an error, so the list can only shrink.
- **Coverage is a ratchet with a 0.05% tolerance**, which hides about six lines. If a
  change lowers it, lower the floor in the same commit and say why. If it raises it,
  raise the floor in the same commit — the job prints the line to commit.
- **The Accuracy gates compile every program under `examples/` and `tests/parity/` with
  ASan and UBSan.** They catch codegen that is valid C but wrong, and they run in about
  30 seconds. When a change touches the type system, they are the cheapest signal there
  is.
- **`Option<T>` is monomorphised** per payload type, and `None` takes its type from
  context. Do not reintroduce the three-slot struct.
