# Changelog

All notable changes to the NOVA project will be documented in this file.

## [Unreleased] - Phase 2: Runtime System (Active Development)

### Added
#### Generic functions are monomorphised ⭐ NEW
- **A generic function is emitted once per concrete call site.** `fn id<T>(x: T) -> T { x }`
  called with `7` and `"hi"` produces `id_int` and `id_string`; `T` is substituted
  in the parameter types, the return type and every type the body reads. The template
  itself is not emitted, so there is no unresolved `T` in the C.
- **Instantiation is recursive**: an instantiated body that calls another generic
  function schedules that instantiation too, and the lowerer drains the queue after
  the top-level functions, deduplicating by concrete name.
- **Sema no longer reports generic calls as type mismatches.** Arity is still checked;
  argument types are not compared against an uninstantiated `T`, and `-> T` return
  checks are skipped until instantiation.
- **Tests:** a sema test accepts `id(1)` and `id("hi")`; a lowerer test checks the
  four instantiations, that no template is emitted, and that `id_string` takes and
  returns `string`; `generic_functions_compile_for_each_type` builds and runs a
  program printing `7`, `hi`, `42`, `hey`.

#### Enums and `match` compile ⭐ NEW
- **Payload-less enum variants become integer constants**: `enum Color { Red, Green }`
  emits `int Color_Red = 0; int Color_Green = 1;` before the functions, and
  `Color.Red` is lowered to `Color_Red`. An enum-typed parameter is an `int` in C.
- **`match` is lowered to a chain of equality tests** (ternaries in C) against
  literals, `Enum.Variant` paths, and a wildcard arm. A `match` used as a function's
  tail expression becomes an explicit return. This is not a C `switch`: that is
  recorded as the remaining half of milestone 9.
- **Tests:** a lowerer test checks the variant constants and that the lowered match
  compares against `Color_Red`/`Color_Green`; `enums_and_match_compile_and_run` builds
  and runs a program printing `1` and `3`.

#### Arrays work ⭐ NEW
- **Array literals and indexing are lowered**: `let numbers: Array = [5, 2, 8];`
  becomes `int* numbers = (int[]){ 5, 2, 8 };`, `numbers[i]` becomes
  `(numbers)[i]`, and `numbers[i] = 9` becomes an `IndexAssign` node.
- **`examples/array_ops.nova` builds and runs**, printing the original array, its sum
  (`25`), its maximum (`9`) and the reversed array. Arrays are `int*`; there are no
  bounds checks and no element type other than `int` yet.
- **Tests:** a lowerer test checks the array literal, typed index read and index
  assignment; `array_example_runs` runs the shipped example and checks its full
  output.

#### `impl` methods compile ⭐ NEW
- **Methods lower to C functions named `Type_method`**, with `&self`/`&mut self`
  passed as a pointer: `self.value` emits `self->value` and `self.value = ...`
  emits `self->value = ...`. A method without a `self` parameter keeps its
  mangled name and no pointer.
- **Method calls are desugared.** `counter.bump()` becomes
  `Counter_bump(&counter)` when the receiver's struct type and the method are
  known, and the call carries the method's declared return type, so
  `print(c.get())` uses the right runtime function.
- **A returning function that ends in an expression now returns it.**
  `fn six() -> int { 6 }` emitted `6;` as a discarded statement and fell off the
  end of the function; it now emits `return 6;`. This also removes the reliance
  on a stale return register for tail-expression functions.
- **Tests:** a lowerer test checks the mangled name, the `Counter*` receiver, the
  `self.value` assignment and the `&counter` argument; another checks that a
  trailing expression becomes a return; `methods_read_and_mutate_the_receiver`
  builds and runs a `Counter` whose `bump(&mut self)` runs twice and whose
  `get(&self)` is printed, expecting `2`.

#### Structs, including field assignment ⭐ NEW
- **`struct` declarations lower to SIR layouts and C `typedef`s**, and struct
  literals lower to `SirExpr::StructLiteral`, emitted as C compound literals
  (`.x = 1`). A field access carries the field's declared type, so `print(p.x)`
  chooses the same runtime function as printing a variable of that type.
- **Field assignment is implemented.** `p.x = 5;` used to lower its target to
  `unknown` and produce C that would not compile; it now lowers to
  `SirExpr::FieldAssign` and emits `(p).x = 5;`. The compiler's C backend and the
  JavaScript, Rust and WebAssembly generators all handle the new node.
- **Tests:** a lowerer test checks the collected layout, the typed literal and the
  typed field access; another checks that a field write keeps its object, field and
  value; `structs_compile_and_run` builds and runs `print(p.x + p.y)`; and
  `struct_fields_can_be_assigned` builds, assigns `p.x = 5` and expects `5\n2\n`.

#### Diagnostics point at the offending expression ⭐ NEW
- **Expressions carry their source spans.** The parser wraps every expression it
  returns in `Expr::Spanned`, and `Expr::span()` reports that span instead of
  `Span::zero()`. Return-type, argument-type and call-arity errors now point at the
  returned value, the argument, or the call: `return 1;` on line 2 reports line 2,
  column 12, not the enclosing function.
- **Token spans start at the token.** The lexer recorded a token's start position
  before skipping whitespace, so every column pointed at the preceding whitespace;
  it now records the position after whitespace and comments.
- **Four tests** cover the reported line and column of a return value, an argument and
  a call, plus a parser test that statements carry spans.

#### The standard library parses ⭐ NEW
- **All four modules under `stdlib/` lex and parse.** `compiler/tests/stdlib_parse.rs`
  is the acceptance check: `core`, `io`, `math` and `text` must lex and parse without
  diagnostics. This does not claim they compile - struct, enum and `match` lowering is
  still missing, so `emit stdlib/...` stops in semantic analysis.
- **Lexer:** `#` line comments (the modules' file headers), `\xNN` escapes (ANSI
  colours), `\b`, `\f`, `\v`, `\a`, and `=>` is now produced as `FatArrow` (the token
  existed but nothing emitted it).
- **Parser:** generic parameters and type arguments (`fn id<T>`, `Option<int>`), slice
  types `[T]`, references `&T`/`&mut T`, tuple and `fn(A) -> B` types, the never type
  `!`, tuple expressions, `as` casts, `&mut value` arguments, struct literals,
  `match` with literal/identifier/variant/path/wildcard patterns, closures,
  `if` expressions with `else if`, `static` declarations, local `const`, `<<`/`>>`,
  and glob imports (`use core.*;`). A block's trailing expression is now recorded as
  its value.
- **Nine parser tests** cover the new syntax, and `grammar/grammar.ebnf`,
  `tools/grammar-check.py` and `tools/example-check.py` were updated to match.

#### Call arguments are checked ⭐ NEW
- **Arity and argument types are validated against the callee's signature.** Signatures
  now carry parameters as well as return types. A wrong count is `E0061` (`function
  `add` takes 2 arguments, but 1 was given`) and a wrong type is `E0308` (`argument 1 of
  `takes_flag` is `int`, but the parameter is `bool``). `int` still promotes to `float`.
- **Sema tracks local variable types**, from parameters and annotated `let` bindings, so
  arguments that are variables are checked rather than skipped. Arguments whose type is
  still unknown (indexing, field access, unknown callees) are not guessed at.
- **The plan is now maintained in `DEV_PLAN.md`**: component status with the check that
  verifies it, 18 ordered milestones with acceptance criteria, and the deliberate
  limitations. `MILESTONES.md` points at it as the source of truth.
- **Five unit tests** cover accepted arguments, wrong arity, wrong argument type, the
  `int`→`float` promotion, and the unknown-type no-false-positive case.

#### Return types are checked ⭐ NEW
- **`return` is validated against the enclosing function's declared return type.** The
  analyser tracks the function being checked (including `impl` methods) and infers the
  returned expression's type from literals, arithmetic, comparisons, `!`, `cast`, `if`
  and `block` tails, and - via a signature table collected in the first pass - calls to
  other NOVA functions. Mismatches are `E0308` with the function, expected type and found
  type in the message: `function `flag` returns `bool`, but this return value is `int``.
- **Also rejected:** `return;` in a function that declares a value, a value from a
  function with no return type, and a non-unit function whose body has no `return` and
  no trailing expression. An `int` still promotes to `float` (`fn ratio() -> float {
  return 1; }` is accepted), and bodies ending in `loop`/`while true` are not flagged.
- **No false positives by construction:** an expression whose type cannot be inferred
  (a variable, field access, index, unknown callee) is never guessed at, so existing
  programs continue to compile. Diagnostics attach to the function's span, because the
  AST does not yet carry spans for expressions.
- **Nine unit tests** in `compiler/src/sema/mod.rs` cover acceptance, the promotion,
  literal mismatch, call-return mismatch, value-in-void, missing value, missing
  `return`, and the unknown-type case.

#### Call results are typed from their signatures ⭐ NEW
- **The lowerer collects function signatures before lowering any body**, so a call is
  typed even when the callee is defined later in the file (including recursion).
- **`SirExpr::Call` is wrapped in `Typed` with the declared return type**, which means
  `print(is_ready())` uses `nova_print_bool` and prints `true`/`false`, a `-> string`
  function prints through `nova_print_str`, and `-> float` prints as a float. Calls to
  unknown/builtin callees and functions with no declared return type keep the previous
  behaviour (printed as `int`).
- **Tests.** Two lowerer unit tests cover a typed call and a forward reference; an
  end-to-end test compiles and runs a program printing `bool`, `float` and `string` call
  results, expecting `true / 1.5 / hi`.

#### SIR expressions carry their type ⭐ NEW
- **`SirExpr::Typed { expr, typ }`** — the lowerer now records what it knows about an
  expression's static type instead of leaving codegen to guess from C syntax. Variables
  take the type from their `let` annotation or parameter list; an unannotated `let` with
  a literal initialiser is inferred (`1` → `int`, `1.5` → `float`, `true` → `bool`,
  `"…"` → `string`, `'c'` → `char`); comparisons and `&&`/`||` are typed `bool`.
- **`print` reads the type.** `print(flag)` on a `let flag: bool` now calls
  `nova_print_bool` and prints `true`/`false`; a `string` variable prints through
  `nova_print_str` instead of being treated as an `int`; arithmetic on floats still
  prints as a float. Where no type is known (function-call results, arrays, structs) the
  backend falls back to inferring from the expression, as before.
- **The lowerer tracks scopes.** `let` bindings are recorded per scope, so a function's
  parameters and locals are typed correctly without leaking between functions.
- **All four backends unwrap the annotation**; it is codegen metadata, not a value, so
  JavaScript, Rust and WebAssembly emit the inner expression unchanged.
- **Tests.** Two unit tests in `compiler/src/lowerer/mod.rs` assert that declared
  variables and comparisons arrive as `Typed`, and that literal initialisers are
  inferred; an end-to-end test compiles and runs a program whose `bool`, `int`, `string`
  and inferred `float` values print as `true / false / 7 / nova / 1.5`.

#### Generated C talks to a NOVA runtime, not to libc ⭐ NEW
- **`compiler/src/runtime.rs`** — the runtime ABI as generated source: `nova_runtime.h`
  declares `nova_print_int`, `nova_print_float`, `nova_print_bool`, `nova_print_char`,
  `nova_print_str`, `nova_print_newline` and `nova_runtime_version()` (ABI v1);
  `nova_runtime.c` implements them. It is the one place the generated-code contract
  lives, so runtime features can grow without changing what backends emit.
- **The C backend now emits runtime calls.** `print`/`println` used to be rewritten into
  a `printf` with a guessed format specifier embedded in the backend; the backend now
  emits `nova_print_<kind>(value)` followed by `nova_print_newline()`, and the generated
  C includes `nova_runtime.h` instead of `<stdio.h>`/`<stdlib.h>`/`<string.h>`.
  Booleans print `true`/`false` rather than `1`/`0`, floats print as `%g` rather than six
  fixed decimals, and comparisons and `!` are booleans rather than integers.
- **The compiler writes and links the shim.** `emit -o FILE` writes `nova_runtime.h` and
  `nova_runtime.c` beside `FILE`; `build`/`run` write them into the output directory and
  pass both C files to `cc`. Printing C to stdout now says (on stderr) that the runtime
  files are needed, since that output is no longer standalone.
- **Tests.** `compiler/src/runtime.rs` checks every function the backend can emit is both
  declared and defined; `compiler/tests/end_to_end.rs` gained a `true`/`false`/`3.5`/`'x'`
  printing test and a test that emits C, asserts it includes the runtime and contains no
  `printf`, links it with the generated `nova_runtime.c` by hand, and runs the result.

#### `nova-pkg build` and `nova-pkg run` work ⭐ NEW
- **The package manager drives the real compiler CLI.** `build` used to invoke
  `nova build -i <entry> [-r] [--no-incremental] [-o <dir>]`: a binary that does not
  exist, with flags that do not exist. It now runs `nova-compiler build <entry> -o
  <build/<profile>/<name>>` (binary found via `NOVA_COMPILER` or on `PATH`), streams the
  compiler's diagnostics, and fails with the compiler's own message. `run` builds and
  then executes that exact path with the user's arguments, propagating the exit status.
- **Output paths agree.** One helper resolves `<out_dir or build/<profile>>/<name>`, and
  both `build` and `run` use it, so `run` no longer looks for a binary `build` never
  produced. `build.out_dir` in `Nova.toml` is honoured, and package names keep their
  executables typo-free (`my-app` → `my_app`).
- **`nova-pkg init --lib` now scaffolds a library that parses.** The template emitted
  `pub fn hello()`, but `pub` is lexed as a reserved keyword with no parser production
  yet, so every generated library failed at `Expected expression at 'Pub'`.
- **Libraries say what they are.** An entry of `src/lib.nova` is emitted as C
  (`emit -o <out>.c`) instead of being linked, which previously ended in
  `undefined reference to 'main'`; `run` on a library reports that there is no binary.
- **Honest flags.** `--release` and `--no-incremental` still exist on the package CLI,
  but print that the compiler has no optimisation levels and no incremental mode rather
  than passing flags the compiler would reject.
- **`reqwest` uses rustls instead of native-tls**, so `cargo build --workspace` no longer
  needs system OpenSSL headers — the whole workspace now builds on a bare toolchain.
- **Four unit tests** in `pkg/src/commands.rs` cover the output path (default, `--release`,
  manifest `out_dir`) and library detection.

#### A NOVA program now compiles to a binary that runs ⭐ NEW
- **`nova-compiler build` and `nova-compiler run`** — the CLI took a source file and
  printed C, and nothing else; the docs told readers to use `nova build`, `--target`
  and `-o`, none of which existed. The binary now has a real front end:
  `build <file> [-o PATH] [--target c]` writes the C, calls the host C compiler
  (`CC`, default `cc`) and leaves a runnable executable; `run <file> [-- ARGS...]`
  builds in a per-process temp directory, forwards arguments after `--` and exits with
  the program's status; `emit` and the bare-path form write C, with `-o` optional.
  An unimplemented target is refused with a message instead of being ignored.
- **The C codegen emits a real `main`.** A program was emitted as `void __main()` (top
  level statements) or `void main()` (a `fn main` with no return type), so `cc` failed
  with `undefined reference to 'main'` — nothing the compiler produced could link. The
  entry function is now `int main(void)`, gets no conflicting prototype, and returns 0
  when the NOVA body falls off the end.
- **`for` loops emit one loop with the real start bound.** The old arm emitted a dead
  `0..0` loop and then a second loop whose start was hardcoded to `0`, so
  `for i in 2..5` printed `0 1 2 3 4`. Ranges (`a..b` and `range(a, b)`) now lower to a
  single C `for` with both bounds taken from the source.
- **`compiler/tests/end_to_end.rs`** — five tests that drive the real binary: build and
  run a hello program, build and run `examples/fibonacci.nova` and compare its whole
  stdout, check `run` forwards the exit status, check a missing source file and an
  unimplemented `--target` both fail loudly. They need a host C compiler and say so
  when none is installed. `compiler/README.md` gained the actual usage and an explicit
  list of what the backend does and does not support.

#### The grammar is now a checked artefact ⭐ NEW
- **`grammar/grammar.ebnf`** — replaced the three-line scaffold with the grammar of the
  subset `compiler/src/parser/mod.rs` actually implements: items, statements, the
  expression chain in the parser's own precedence order, and a written-down list of the
  tokens that are lexed but not parsed yet (`match`, `type`, `as`, `loop`, `pub`, and
  nine operators).
- **`tools/grammar-check.py`** — a test for the grammar itself. It reads `token.rs` and
  `parser/mod.rs` and fails when the grammar and the code disagree: a keyword or operator
  that is neither used nor declared reserved, a word literal that is not a real keyword
  (invented syntax), a rule that is undefined or unreachable, or an expression level
  edited on one side only. Wired into CI, so a new keyword cannot be added without
  deciding where it belongs.

#### Every example is now a program the grammar accepts ⭐ NEW
- **`tools/example-check.py`** — reads `grammar/grammar.ebnf`, turns it into a
  packrat recogniser and requires every `examples/**/*.nova` to parse from `program`
  to EOF. It self-tests the recogniser on eleven known-good and known-bad sources
  first, so a passing run means something. Wired into CI beside `grammar-check.py`.
  Nothing read the examples before this: the grammar was checked against the Rust and
  the Rust tests parse snippets inside `#[test]` functions, so an example could say
  anything.
- **Three of the five shipped examples were not NOVA programs**, which is what the
  new check found. `examples/basic/hello.nova` was `print "Hello from NOVA"` — no
  parentheses, no semicolon, which the parser rejects; it is now `print("Hello, NOVA");`.
  `examples/web/app.nova` was a single comment and no program at all; it is now a small
  program that prints the page a site would serve. `examples/array_ops.nova` annotated
  arrays as `[int]` and `[int; 5]`, which `parse_type_expr` cannot read (it takes one
  name), so it is now `Array` — and `examples/README.md` says why, and no longer claims
  an HTTP server, JSON serialisation or `async`/`await` that do not exist.
- **The grammar had drifted too.** `item` used `expr_stmt`, whose `;` is optional, but
  `parse_top_level` *requires* the `;` after a top-level expression. A looser rule that
  accepted syntax the parser rejects is still a false grammar; it is now
  `top_expr_stmt = expr ";"` at top level and `expr_stmt = expr [ ";" ]` inside a block.

#### Runtime System Core (1,220+ lines) ⭐ NEW
- **Memory Management** (`memory.rs` - 192 lines):
  - `MemoryArena` for efficient arena-based allocation
  - `GarbageCollector` foundation with mark-and-sweep placeholder
  - `ManagedPtr<T>` for safe runtime-managed pointers
  - Global memory statistics tracking
  - Comprehensive test suite (2 tests)

- **Type System** (`types.rs` - 260 lines):
  - `TypeId` and `TypeInfo` for runtime type information
  - `TypeRegistry` with primitive types pre-registered
  - `DynamicValue` for runtime type checking
  - Field and variant info for structs/enums (3 tests)

- **Error Handling** (`error.rs` - 297 lines):
  - `RuntimeError` enum with 10 error categories
  - `RuntimeErrorWithTrace` with backtrace support
  - Panic hook initialization and handling
  - `catch_unwind` utility function (4 tests)

- **FFI Support** (`ffi.rs` - 332 lines):
  - `NovaValue` C-compatible tagged union
  - `FfiString` for Rust ↔ C string conversion
  - `FfiArray<T>` for FFI-safe dynamic arrays
  - `FfiHandle` for resource management
  - `FfiFunction` wrapper for callbacks (6 tests)

- **Core Runtime Library** (`lib.rs` - 139 lines):
  - Runtime initialization and statistics
  - Module exports and version info
  - Integration tests (6 tests)

### Technical Details
- Runtime LOC: 1,220+ lines
- Test coverage: 20+ tests across 5 modules
- Status: Core modules complete, I/O and threading pending

## [1.0.0-alpha] - Phase 1: Compiler Core Complete - 2024

### Added
#### Compiler Core (~3,500 lines)
- **Lexer** (309 lines): Complete tokenization with Unicode support, error spans
- **Parser** (907 lines): Recursive descent parser with error recovery, operator precedence
- **Semantic Analyzer** (773 lines): 
  - Type checking system
  - Symbol table management
  - Scope resolution
  - Name resolution
  - Type inference (basic)
- **SIR** (146 lines): Semantic Intermediate Representation definition
- **Lowerer** (550 lines): AST to SIR conversion

#### Backends (1,529 lines total)
- **C Backend** (449 lines): Full C code generation, tested and working
- **JavaScript Backend** (343 lines): ES6+ JavaScript code generation
- **Rust Backend** (352 lines): Rust code generation
- **WebAssembly Backend** (385 lines): WASM module generation

#### Infrastructure
- Complete compiler pipeline: Source → Lexer → Parser → AST → Semantic Analysis → SIR → Backend → Target Code
- Support for functions, structs, enums, control flow, loops
- Recursive function support
- Type printing and debugging

### Technical Details
- Total production code: ~5,000+ lines
- Language: Rust
- Architecture: Modular, multi-crate workspace
- Status: Production-ready compiler core

## [0.1.0] - Initial Scaffold - 2024

### Added
- Project structure and workspace setup
- Basic crate definitions
- Documentation framework

---

**Note**: This changelog is now updated in real-time with each development phase.
