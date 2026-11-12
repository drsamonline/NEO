# Compiler

NOVA source is lowered to C by a four-stage pipeline, then linked with the host C
compiler:

```
source .nova → lexer → parser → semantic analyser → SIR → lowerer → C → cc → executable
```

## Usage

Build the compiler from the repository root:

```bash
cargo build --release          # target/release/nova-compiler
```

Then, from the repository root:

```bash
# Compile and run straight away
./target/release/nova-compiler run examples/hello.nova

# Build an executable next to a .c file of the same name
./target/release/nova-compiler build examples/fibonacci.nova -o /tmp/fibonacci

# Only produce C: print it, or write it with -o
./target/release/nova-compiler examples/hello.nova
./target/release/nova-compiler emit examples/hello.nova -o /tmp/hello.c
```

Options: `-o/--output <path>`, `--target <name>` (only `c` is implemented), `-h/--help`.
`run` passes everything after `--` to the program and exits with the program's status.

`CC` selects the C compiler used by `build` and `run` (default `cc`). `build`
leaves the generated C beside the executable (`hello` → `hello.c`); `run` builds
in a per-process temporary directory.

## Runtime shim

Generated C does not call libc directly. It includes `nova_runtime.h` and calls a
six-function ABI (`nova_print_int`, `nova_print_float`, `nova_print_bool`,
`nova_print_char`, `nova_print_str`, `nova_print_newline`). The compiler writes
`nova_runtime.h` and `nova_runtime.c` itself — beside the C for `emit -o` and
`build`, and into the temporary build directory for `run` — and links the source
into the executable.

To compile emitted C by hand:

```bash
./target/release/nova-compiler emit examples/hello.nova -o /tmp/hello.c
cc /tmp/hello.c /tmp/nova_runtime.c -o /tmp/hello
```

The shim's ABI lives in `compiler/src/runtime.rs`; `nova_runtime_version()`
reports version `1`.

Which function is used comes from the value's type: SIR expressions carry the
type the lowerer knows (`SirExpr::Typed`), taken from a variable's declaration or
inferred from a literal initialiser, and comparisons are typed `bool`. So a
declared `bool` prints `true`/`false` rather than `1`/`0`, and a `string` prints
as text. A call to a NOVA function is typed from that function's declared return
type, so `print(is_ready())` prints `true` when `is_ready` returns `bool`, and the
signature is recorded before any body is lowered so a call may precede the callee.
Values with no known type (external/builtin callees, `if` expressions, arrays,
structs) are printed as `int`.

## Scope

Implemented and exercised by `compiler/tests/end_to_end.rs`: literals, `let`,
arithmetic and comparisons, `if`/`else`, `while`, `for i in a..b`, `return`,
function definitions and calls, recursion, `struct` declarations, struct
literals, field access, field assignment, `impl` methods with `&self`/`&mut self`,
`enum` declarations with `match`, array literals with indexing, generic functions,
and `print`/`println` with `int`, `float`, `bool`, `char` and string arguments.

A payload-less enum variant is an integer constant (`Color.Red` → `Color_Red`). A
`match` in statement position becomes a real C `switch`: each literal or variant arm
is a `case` (the variant's ordinal is the constant) and the wildcard is `default`.
A `match` whose value is used stays a chain of equality tests, which C renders as
ternaries. Arrays are `int*`: a literal becomes a C compound literal and indexing is
unchecked.

A generic function is emitted once per concrete call: `fn id<T>(x: T) -> T` called
with an `int` and a `string` becomes `id_int` and `id_string`, with `T` substituted
in the parameter types, the return type and the body. The template itself is not
emitted, and an unbound type parameter falls back to `int`.

Structs become C `typedef`s; a struct literal becomes a C compound literal
(`(Point){ .x = 1, .y = 2 }`), and `p.x = 5` becomes `(p).x = 5`. A field access
is typed from the struct's declared layout, so printing a field uses the same
runtime function as printing a variable of that type.

An `impl` method becomes a C function named `Type_method`, with `self` passed as a
pointer: `self.value` emits `self->value`, and `counter.bump()` emits
`Counter_bump(&counter)`. A function that declares a return type and ends in an
expression returns that expression.

The semantic analyser checks every call's arity and argument types against the
callee's declared parameters (`E0061` for wrong count, `E0308` for a mismatch), with
the same `int` → `float` promotion and the same rule that an unknown argument type is
never guessed at. Diagnostics point at the offending expression - the returned value,
the argument, or the call - using the spans the parser records.

`compiler/tests/stdlib_parse.rs` requires all four modules under `stdlib/` to lex and
parse. That is a syntax guarantee only: the modules also use payload-carrying
`Option<T>` values, closures and `__builtin_*` functions that are not declared, so
`emit stdlib/...` still stops in semantic analysis even though generic functions are
now monomorphised.

It also checks every `return` (and a trailing expression) against the
function's declared return type: a mismatch is `E0308`, an `int` may promote to a
`float`, `return;` in a non-unit function is rejected, a value from a function with no
return type is rejected, and a non-unit function with no `return` and no tail
expression is rejected. Expression types it cannot infer (identifiers, field access,
arbitrary calls) are never guessed at, so this cannot produce a false error.

Parsed but not lowered yet: closures, slices, references, tuples, `use`/`mod`,
enum variants that carry data, `match` binding or string patterns, and generic
structs or generic methods. The parser accepts them so the standard library can be
checked; SIR and the C backend still cover only the subset above, and tuples and
closures lower to unit.

Arrays are `int*` with no bounds checks and no element type other than `int`. The
JavaScript, Rust and WebAssembly generators exist as separate crates (`backends/`)
but no CLI writes their output yet.
