# Handover — where this is, and what to do next

Written 2026-11-19. Every number here came from a command or a CI log; where something
is unverified it says so.

## The repository

| | |
|---|---|
| `master` | `bb63d30` |
| Tests | **254** passing, 0 failing, 1 ignored (a known defect, see below) |
| Coverage | 65.91%, ratchet in `coverage-floor.txt` |
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

`stdlib/io` compiled for the first time on 2026-11-18. `stdlib/math` works end to end
except for its float-array functions, which are **wrong** — see below.

## The one wrong answer in the library

`sum_float([1.5, 2.5])` returns **3**, not 4. `NovaArray` holds `int`, so a `[float]`
array is truncated before anything reads it, and `mean`, `variance`, `stddev` and
`median` build on it. The module compiles and the compile gate passes, which is why
[ADR 0006](adr/0006-array-element-types.md) exists and why `stdlib/README.md` and
`stdlib/math/README.md` warn about it at the top.

`compiler/tests/end_to_end.rs::a_float_array_sum_is_not_truncated` asserts the correct
answer and is `#[ignore]`d with this defect as its reason. It stays runnable:

```bash
cargo test --test end_to_end -- --ignored a_float_array_sum
```

## What is next, in order

**1. `[float]` arrays — the work is half done on an unmerged branch.**

`feat/float-array-runtime` has two commits that are individually sound and do not pass
CI:

- `NovaArray_float` and its six runtime functions (`_from`, `_len`, `_get`, `_set`,
  `_push`, `_pop`), in `compiler/src/runtime.rs`;
- `c_type` maps `[float]`/`[f32]`/`[f64]`/`Array<float>` to `NovaArray_float`, and a
  float literal builds `nova_array_float_from((double[]){...}, n)`;
- `array_len`, `array_push`, `array_pop` and `arr[i]` pick the `_float` runtime when the
  array's type says so.

**It fails on generics, and that is the next task:**

```
error: incompatible type for argument 1 of 'array_len_int'
```

`stdlib/core` declares `fn array_len<T>(arr: [T]) -> int`. `math`'s `mean` calls it with
`arr: [float]`, and the instantiation is named **`array_len_int`** — the element type is
lost when the generic is monomorphised, before any of the float-array work runs. Find
`generic_bindings` in `compiler/src/lowerer/mod.rs`; the instantiation must be named after
the *argument's* element type.

**2. `[String]` arrays** — the same treatment, and it is what stops `stdlib/text`. A
`char *` is eight bytes, so it needs `NovaArray_string` rather than a cast. Nine declared
`[String]` in `text`, four in `io`.

**3. 38 stdlib functions still call a builtin the runtime does not implement.** Mostly
`fopen`/`fclose`/`fread`/`fwrite` — a `FILE *` does not fit in the `int` that
`File.handle` declares, so these want a handle table or a changed `File`. The eleven that
were done on 2026-11-18 show the pattern: one shim function, one lowerer arm, one sema
type, and `tools/stdlib-check.py` proves the count moved.

**4. 17 specification chapters.** They describe surface that does not exist — ownership,
memory, concurrency, attributes, interfaces. Writing them would be inventing a
specification, which `docs/specs/constitution.md` forbids first.

**5. The DWARF debugger.** Lowest value-per-week in the plan. The REPL works.

## Working without a Rust toolchain

`static.rust-lang.org` is not reachable from the sandbox, so `rustup` cannot install
anything. Two loops replace it, both documented in `DEV_PLAN.md`:

**Build on GitHub, run here.** `.github/workflows/dev-build.yml` builds the workspace and
uploads the binaries as a seven-day artifact.

```bash
./scripts/dev-loop.sh <branch>     # dispatch → wait → download → set CC
```

**Compile C locally.** `pip install ziglang` gives Zig 0.17.1; `zig cc` compiles, links and
runs C with libm. Enough to prototype a codegen design, reproduce a C-level failure, and
check the runtime shim.

**Verified end to end**: a compiler built from `master` on a runner, downloaded, compiling
a `use math;` program that printed `4 3 3 1024` with `zig cc` linking it.

**Note:** `workflow_dispatch` through the API needs a token with `actions:write`; `gh`
gets a 403. Either use the Actions tab, or rely on the push-triggered workflows.

## Things that will cost you a cycle if you do not know them

- **`/tmp` is 64 MB in the sandbox** and fills with clones. Work in `/scratch/work`.
- **`-std=c11` is strict ISO**, so POSIX hides `struct timespec` and `nanosleep`. The
  runtime defines `_POSIX_C_SOURCE`; only the sanitiser job uses strict C, which is why
  the stdlib gate passes and the accuracy gate catches it.
- **A type table that drops information is the recurring defect.** Six separate bugs this
  session were a type string losing its generic arguments or a name losing its payload.
  When something types as `int` unexpectedly, look for `.name` where `type_display` was
  meant.
- **The stdlib gate compiles every module with `-Werror=builtin-declaration-mismatch`.**
  A NOVA function named after a C library function gets a `nova_fn_` prefix; do not
  reintroduce a collision.
- **`stdlib/not-compiling.txt` is a ratchet.** A module outside it failing is an error; a
  module inside it compiling is also an error, so the list can only shrink.
- **Coverage is a ratchet with a 0.05% tolerance**, which hides about six lines. If a
  change lowers it, lower the floor in the same commit and say why.
- **`Option<T>` is monomorphised** per payload type, and `None` takes its type from
  context. Do not reintroduce the three-slot struct.
