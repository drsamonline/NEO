# Handover — where this is, and what to do next

Written 2026-11-22. Every number here came from a command or a CI log; where something
is unverified it says so.

## The repository

| | |
|---|---|
| `master` | `f496112` |
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
| 7 — long tail | 🟡 spec 10/25, stdlib truth ✅, debugger ⬜ |

### The standard library

| Module | Compiles | Functions | Usable | Blocked |
|---|---|---|---|---|
| `core` | ✅ | 55 | 54 | 1 |
| `io` | ✅ | 59 | 49 | 10 |
| `math` | ✅ | 76 | **76** | **0** |
| `text` | ✅ | 59 | **59** | **0** |

The counts come from `python3 tools/stdlib-check.py`, which is the gate. This table used to
disagree with it — it said `io` 28 and `text` 54 for three releases' worth of work after both
had moved. The gate checks `stdlib/README.md` and not this file, which is why; copy the
numbers from the gate, never from memory.

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
arguments between two places that both need it; that is now eight defects of this shape.

### And then the same defect, five more times

Each fix below was the next thing standing between `stdlib/text` and the compile gate, and
each was the same shape: a type, or a construct, losing its meaning between two places that
both needed it.

| Defect | What it did | Record |
|---|---|---|
| `[String]` arrays | `char *` in a four-byte `int` slot | ADR 0006 |
| `s[i]` on a `String` | reached the array getter with a `char *` | ADR 0007 |
| `as` casts | dropped entirely; `x as char` was a comment | ADR 0008 |
| `if` as a value | lost its `else if` chain, and reported no type | — |
| arrays of arrays | `[[int]]` had no representation | ADR 0009 |
| `mangle_type` | `[int]` and `int` produced the same instantiation name | — |
| a loop body of one expression | dropped entirely — `while (c) { }` | — |
| `==` on two `String`s | compared pointers, so it was always false | — |
| a nested `if` statement | its branch's trailing expression lowered as an expression, so `if a { if b { x = 1 } }` emitted `if (a) { ; }` | — |
| `for` over a range | `range_bounds` matched the call but not the `Typed` wrapper around it, so every `for i in range(..)` iterated zero times | — |
| `for` over anything else | emitted a comment and an empty body — a silent no-op. Now traps with the construct named | — |
| `Option<T>` return type of an instantiation | computed from the type's `.name`, which is `Option` — `c_type` maps that to `int`, so `find_int` returned an `Option_int` from an `int` function | — |
| `Option<T>` return type of a generic call | same `.name` mistake in `expr_type`, so `let a = find(..)` inferred `int` | — |
| `match` over an `Option` | read its scrutinee twice, once for the tag and once for the payload — `match array_pop(a)` popped two elements | — |
| struct field types | a field of `Option<int>` declared an `int` — and fixing it showed structs were emitted in source order, so a struct came before the type it embeds | — |
| `int` width | variables and array elements were C `int` while literals were 64-bit, so `let big = 3000000000` printed `-1294967296` on the line after `print(3000000000)` printed it correctly | [0011](adr/0011-int-is-64-bit.md) |

**Three of those compiled cleanly and were wrong.** The loop body and the string
comparison were found by running `split`, not by any gate. That is the standing lesson of
this file, and it is why item 2 below is what it is.

### The audit those rows earned

Nine of the defects above are one mistake: reading a type through `TypeExpr::name` when
`type_display` was meant. `Option<int>`'s name is `Option`, and `c_type` maps that to
`int`, so the argument vanishes silently. Rather than wait for a tenth, every such site was
swept:

| Site | What it broke |
|---|---|
| `resolve_generic_call` | a generic call's reported return type |
| `lower_instance` | a generic instantiation's declared return type |
| `expr_type`'s call arm | `let a = find(..)` inferred `int` |
| `lower_method` | a method returning `Option<T>` declared to return `int` |
| `lower_statement`'s local `const` | `const LIMIT: Option<int>` declared an `int` |

The same sweep covered the other half of the pattern — helpers that match a `SirExpr`
without unwrapping its `Typed` annotation. `range_bounds`, `global_init_to_c` and the
`match` scrutinee all had it; the first made every `for i in range(..)` iterate zero times.

**If you add a helper that inspects a `SirExpr`, unwrap `Typed` first.** Every existing one
does now, and `methods_and_consts_keep_their_generic_return_type` plus
`core_predicates_and_range_iteration` are the tests that notice when one stops.

### And a gate, because the audit was by eye

The sweep above still missed two places — struct field types, and seven sites in the
analyser — which is the argument for a check rather than a careful reader.
`tools/type-name-check.py` now fails CI on either idiom:

| Idiom | What it costs |
|---|---|
| a type read through `TypeExpr::name` | the generic arguments vanish; `Option<int>` becomes `Option`, which `c_type` maps to `int` |
| a `SirExpr` inspected without unwrapping `Typed` | the annotation hides the shape; `for i in range(..)` iterated zero times |

It is a heuristic — a handful of exact shapes the bug has actually taken, not a type
checker. A compiler warning would be better and this repository has no lint infrastructure;
the alternative to a heuristic gate is the no gate that let ten of these through.

## What is next, in order

**1. The file builtins are done; the rest of the missing builtins are not.** The ten
`__builtin_f*` calls now have shims, lowerer arms and sema types, behind a handle table —
[ADR 0010](adr/0010-file-handles.md) explains why a handle is an index rather than a
pointer, and why `File` did not change. That is the pattern for the ones still missing: one
shim, one arm, one type, and `tools/stdlib-check.py` proves the count moved.

`crc32`, `base64_encode`/`decode`, `md5`, `sha256` and the four terminal calls are done too,
which makes **`text` fully usable for the first time** — 59 of 59.

The path helpers are done too — `list_dir`, `basename`, `extension`, `parent_dir`,
`join_path`, `resolve_path` — which takes `io` to 48 of 59 and the library to 236 of 249.

What remains is 12 functions, and the two groups left each need a decision first:

| Group | What it needs |
|---|---|
| `serialize`, `deserialize` | a decision: what the wire format is. An ADR before code. |
| `args` | the emitted `main` is `int main(void)`, so there is no `argc`/`argv` to hand over. Changing it is a decision about every program's entry point. |
| ~~`stat`~~ | **done.** `Option<FileMetadata>` works, and the runtime still does not build the struct: it answers scalars and NOVA composes. See below. |

`resolve_path` joins against `cwd` rather than calling `realpath`, which needs
`_XOPEN_SOURCE` and a `PATH_MAX` buffer and fails on a path that does not exist — exactly
when a caller wants the answer. It does not resolve symlinks, and says so.

**`stat` composes its struct in NOVA, not in the runtime.** A builtin returning
`FileMetadata` would need to know a layout the language defines, while the runtime is C that
only ever sees the generated struct. It asks for `file_exists`, `file_size`,
`is_directory` and three timestamp builtins, and builds the `Option` itself. That is the
pattern for anything else that returns a struct: the runtime answers scalars.

Wiring up the file group found **two defects that had nothing to do with files**, both of
the shape this file keeps recording: `match` in *expression* position evaluated its
scrutinee once per arm, so `write_file` opened its own output a second time and truncated it
(the statement form was fixed in #77; the expression form had the same bug and a different
code path), and `fread_all` rebuilt a string line by line, dropping every newline and
stopping at the first blank line. The second is why `nova_file_read_all` exists.

**2. Most of the standard library is still untested, and so is `for`.** 203 of 249 functions
are asserted in `compiler/tests/stdlib.rs`. **`math` is the first module with an assertion
for every function it exports — 76 of 76.** `io` is the remaining gap at 14 of 59, because
its functions want a filesystem rather than pure values. Writing the first three tests
found two live bugs within minutes; the second batch found a third. **No example, parity
program or stdlib function contains a `for` loop at all**, which is how `for i in
range(0, 4)` came to iterate zero times without anything noticing — a whole construct with
no coverage. Extending that file is worth more than any of the items below.

**3. 17 specification chapters.** They describe surface that does not exist — ownership,
memory, concurrency, attributes, interfaces. Writing them would be inventing a
specification, which `docs/specs/constitution.md` forbids first.

**4. The DWARF debugger.** Lowest value-per-week in the plan. The REPL works.

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
- **`break` and `continue` in statement position lower as statements, not as expressions.**
  They parse as `Statement::Expr(Expr::Break)`, and the *expression* form wraps them in a
  block whose value is `Unit` — which renders as an empty string, so a branch holding only
  `break` emitted `__block_0 = ;` and did not compile:

  ```nova
  while i < n {
      if arr[i] == target { break; }
      else if arr[i] > target { return 0 - 1; }
      else { i = i + 1; }
  }
  ```

  A search loop did not compile. Fixed in the lowerer's statement arm, with the backend
  refusing to emit an empty assignment if a `Unit` block reaches it another way.
  `break_and_continue_inside_a_branch_compile` covers both keywords.
- **`docs/status.json` is published, and gated.** `python3 tools/status.py --check` fails
  when it stops matching the version, the line counts, the ADR count, the gate list or the
  stdlib ratchet. Other repositories read it: `drsamonline/skills` rewrites the NEO skill's
  status block from it every week, so a stale file misinforms them the same way a stale
  handover misinforms us. It deliberately carries **no** test count and no coverage — those
  move on every commit and a file carrying them could never be checked. They are in this
  document instead, which CI cannot verify either, so keep them current by hand.
