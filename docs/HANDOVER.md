# Handover — where this is, and what to do next

Written 2026-11-22. Every number here came from a command or a CI log; where something
is unverified it says so.

## The repository

| | |
|---|---|
| Release | **v0.8.0** — three platform archives with SHA-256, verified by downloading and running one |

**Numbers are not kept here.** This table used to carry the commit, the test count, the coverage
figure and the ADR count; all four were stale within a day, because they change on every merge
and nothing measured them in this file. What is stable lives in `docs/status.json`, which
`tools/status.py --check` gates:

| Fact | Where |
|---|---|
| version, source lines, ADR count, gate list, stdlib size | `docs/status.json` — gated |
| test and coverage counts | CI, on the commit you are looking at; `coverage-floor.txt` holds the ratchet |

## State as of 2026-11-27

Three language questions were open when this file was last rewritten. All three now have an
accepted ADR, and one of them produced a feature:

| Question | Answer | Where |
|---|---|---|
| Concurrency | **absent, by decision** — the backend contract returns one string, and a thread does not fit that on JS or WASM | [ADR 0020](adr/0020-concurrency-model.md) |
| The unused `runtime/` crate | **deleted** — 2,480 lines nothing depended on | [ADR 0021](adr/0021-delete-runtime-crate.md) |
| `Result<T, E>` | **built as sugar** over the generic enum the language already had: `Ok(7)` needs no declaration and no `Result.` prefix | [ADR 0022](adr/0022-result-type.md) |

ADR 0022 is worth reading for its "Found while building it" section rather than its decision: the
first design was wrong, CI proved it by breaking a passing test, and the failure is what revealed
that the enum path already bound two payload types in one match. **A design that starts from "what
does the machinery already do" would have found that in an hour.**

The v0.8.0 release failed once before it published: the macOS build stopped at
`cannot find value PTRACE_ATTACH in crate libc`, because `nova-dbg` attaches with `ptrace` and that is
a Linux interface. Nothing was published — the workflow builds before it uploads, and the release it
had created was a draft — and the fix was to build `--exclude nova-dbg` off Linux, plus the sentence
in `COMPATIBILITY.md` that would have prevented the failure. **A release failed on something the code
had always known**, which is the same defect shape as everything else in this file.

What is genuinely left:

| Item | Blocked by |
|---|---|
| ~~`nova-dbg` in the macOS and Windows archives~~ | **done** — PR #203 gated `attach` on `target_os = "linux"`; the v0.8.1 dry run on 2026-11-27 built all three platforms green; the `--exclude nova-dbg` branch is gone, so every archive from v0.8.1 on carries four binaries |
| `print <name>` in the debugger | a C compiler that emits local-variable DWARF; `zig cc` does not, gcc/clang do |
| WASM float struct fields, non-`int` closures | rejected by `validate` rather than miscompiled — a decision, not a defect |
| ~~A release after v0.7.0~~ | **done** — v0.8.0 published 2026-11-27 |

## What is done

| Stage | State |
|---|---|
| 0 — one plan, true numbers | ✅ |
| 1 — prove the output correct (G1–G4) | ✅ |
| 2 — diagnostics as a contract | ✅ |
| 3 — harden the front end (G5–G7) | ✅ |
| 4 — syntax tree, formatter, guard, `trait` | ✅ |
| 5 — LSP nav, VS Code extension, path/git deps | ✅ |
| 6 — release and distribution | ✅ v0.8.0 published 2026-11-27 |
| 7 — long tail | 🟡 spec **25/25** ✅, stdlib truth ✅, debugger ✅ except `print <name>` |

### The standard library

| Module | Compiles | Functions | Usable | Blocked |
|---|---|---|---|---|
| `core` | ✅ | 55 | **55** | **0** |
| `io` | ✅ | 59 | **59** | **0** |
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
`tools/type-name-check.py` now fails CI on any of three idioms:

| Idiom | What it costs |
|---|---|
| a type read through `TypeExpr::name` | the generic arguments vanish; `Option<int>` becomes `Option`, which `c_type` maps to `int` |
| a `SirExpr` inspected without unwrapping `Typed` | the annotation hides the shape; `for i in range(..)` iterated zero times |
| an `Option_` struct name built outside `nova_common::types` | two copies of one rule agree until they don't: `[int]` and `int` both became `Option_int` |

The third is the general shape of the other two — **a fact written down twice eventually
disagrees with itself**. It scans `crates/` as well as `compiler/src`, because the shared
function lives in the former.

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

**Nothing remains.** The three groups that were blocked are all done, and each needed an ADR
first:

| Was blocked | How it was decided |
|---|---|
| `serialize`, `deserialize` | the module's own doc comment said "JSON-like", so the format was already chosen; ADR 0014 records it |
| `args` | the entry point became `int main(int argc, char **argv)`; ADR 0012 |
| the four stream functions | a `Stream` is a file handle with a label; ADR 0013 |

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

**2. The standard library is complete.** All 249 functions run, and all 249 have an assertion in
`compiler/tests/stdlib.rs` — none blocked. `deserialize` was the last; it needed two fixes:
inference must take `T` from the binding (ADR 0015), and `lower_instance` must publish the
instantiated return type. Writing the first three tests found two live bugs within minutes; the
second batch found a third. **No example, parity
program or stdlib function contains a `for` loop at all**, which is how `for i in
range(0, 4)` came to iterate zero times without anything noticing — a whole construct with
no coverage. Extending that file is worth more than any of the items below.

**3. 17 specification chapters.** They describe surface that does not exist — ownership,
memory, concurrency, attributes, interfaces. Writing them would be inventing a
specification, which `docs/specs/constitution.md` forbids first.

**4. The DWARF debugger.** Lowest value-per-week in the plan. The REPL works, and since the
refusals landed it says what it cannot do instead of inventing frames.

### `✓ built` is now checked

The C compiler's exit status is a claim about *itself*, not about the file. A wrapper that exits 0
without writing produced `✓ built <path>` for a binary that did not exist — the same shape as
`✓ Published` and "Attached to process N", both of which a user acted on before anyone noticed.

`build` now checks the file exists and dies naming it if not, with a regression test that runs the
build against `/bin/true`. **Seen to bite**: with the check removed the test fails at
`compiler/tests/end_to_end.rs:91`.

The other success messages were audited and are already verified: `✓ wrote` and `✓ formatted`
follow an `fs::write` whose error is fatal, and `write_runtime_for` checks both files.

### A namespace collision the ownership work exposed

`io.nova` defines `log(level: LogLevel, message: String)` and `math.nova` defines
`log(x: float) -> float`. Imported modules are merged into one symbol table, so a program with
both —

```nova
use math;
use io;
```

— fails to build: `math.nova`'s own internal `log(x)` calls are checked against **io's** two-argument
`log`, and the analyser reports `E0061 function `log` takes 2 arguments, but 1 was given`. Measured
2026-11-24; `use math;` alone builds, `use math; use io;` does not.

This is a name-resolution bug, not an ownership one: `use` merges every declaration into one flat
namespace and the last definition of a name wins. It surfaced while counting move warnings, and it
was more serious than the thing being measured — a user could not import two standard-library
modules that happen to share a function name.

**Fixed on 2026-11-24** by renaming `io.log` to `io.log_message` — the smaller of the two options,
and the one that leaves the language's scoping unchanged. `math`'s `log`/`log10`/`log2` family
keeps its names because it is a family; `io`'s logging primitive had a synonym available and they
did not. Verified: `use core; use io; use math; use text;` now builds and runs, where
`use math; use io;` did not.

**And the general case is now handled twice over.** The loader used to drop the second definition
silently; it now refuses and names both modules:

    error: `shared` is declared by both `alpha` and `beta`, and NOVA has one flat namespace:
    importing both is ambiguous. Rename one of them.

`tools/stdlib-check.py` additionally fails if any two standard-library modules declare the same
name, so the library cannot reintroduce the problem. Seen to bite: renaming `math.log10` to
`math.log_message` produces `FAIL two standard-library modules declare the same name`.

Module-qualified resolution is still the real fix — a program cannot import two modules that share
a name at all — and remains a language change nobody has decided on.

### Why `print <name>` is blocked here, and what would unblock it

`print 0xADDRESS` reads memory. `print counter` refuses, and the reason is not that nobody has
written the parser:

- a variable's value lives at a stack offset, and the type that says how many bytes to read is in
  `.debug_info` — a different DWARF section from the `.debug_line` this debugger reads;
- **the C compiler decides whether that information exists at all.** Measured on 2026-11-24: a
  two-line C program compiled with this sandbox's `CC` (a `zig cc` wrapper) produces **3**
  `DW_AT_location` entries, none of them the program's own local, and a NOVA build's `counter`
  appears as a `DW_TAG_variable` with a name, a type and a line — and **no location**.

So `print <name>` cannot be implemented against this toolchain, and a version written here could
not be verified. It is not unimplemented; it is unverifiable in this environment.

**What would unblock it**, in order of cost:

1. **gcc or clang as `CC`.** Both emit local variable locations with `-g` at `-O0`. CI has them;
   the sandbox does not. A build under gcc would make the DWARF present, and then the work is a
   parser, not a compiler change.
2. **A DWARF location parser** for the common `DW_OP_fbreg <offset>` form: read `rbp`, add the
   offset, read the number of bytes the type says.
3. **Type resolution** for anything that is not a scalar: the `DW_AT_type` chain says `long long`
   or a struct, and a struct needs its members walked.

The first is a decision about the build environment; the second and third are a project the size
of the line-table work, which took two attempts.

### What the line-table work actually costs — measured, not estimated

An attempt on 2026-11-24 found the shape, which is worth knowing before starting again:

1. **`SirStatement` carries no span.** `SirFunction` does (`span`), statements do not. A line
   table maps instructions to lines, and a statement is the unit that has one.
2. **The cheap way to add it is a wrapper**, not a field on all eleven variants:
   `SirStatement::Spanned(Box<SirStatement>, Span)`. Every existing match arm keeps working
   through the inner statement.
3. **The wrapper ripples through every backend.** Adding the variant produced five
   non-exhaustive-match errors in `backends/wasm` alone; `javascript`, `rust`, the C backend and
   `collect_callees` in `nova-common` each need an arm too. That is the whole cost, and it is
   mechanical rather than hard.
4. **The payoff is bigger than a DWARF emitter.** With spans on statements the C backend can
   emit `#line N "program.nova"` before each one, and the host C compiler then generates the
   `.debug_line` section itself — the debugger reads NOVA line numbers without a DWARF writer.
   `Span` has no filename, so the source name has to reach the backend: pass it to `generate`
   rather than adding a field to `SirProgram`, which many tests construct by literal.

The attempt was reverted rather than half-landed: a `Spanned` variant that only some backends
handle compiles into four different answers to the same question.

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
whole test suite run locally. What it cannot do: `cargo llvm-cov`, because the instrumented
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

## A check that used to be untrustworthy

- **"Emitted WASM validates" failed once for reasons that were not the compiler.** On commit
  `cb51f71` it passed in the `pull_request` run and failed in the `push` run — identical code,
  two results, 32 seconds apart. The only part of the job that can differ between two runs is
  the network: a `rustup` install and a `wasm-tools` download. Both are pinned, and the
  download now retries three times and says **"this is tooling, not the compiler"** when it
  gives up, so a red result means one thing again.
- **The job's own guard is sound** and should not be removed with it: `validated -eq 0` fails
  the job, so a backend that rejects the whole corpus cannot pass quietly.
- **The retry path is untested in CI** — it only runs when a download fails, which is exactly
  the case that is hard to provoke. That is recorded rather than claimed as verified.

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
