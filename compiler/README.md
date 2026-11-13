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

Options: `-o/--output <path>`, `--target c|js|rust|wasm` (`c` is the default and the
only one `build`/`run` link), `-h/--help`.
`run` passes everything after `--` to the program and exits with the program's status.

`CC` selects the C compiler used by `build` and `run` (default `cc`). `build`
leaves the generated C beside the executable (`hello` → `hello.c`); `run` builds
in a per-process temporary directory.

## Runtime shim

Generated C does not call libc directly. It includes `nova_runtime.h` (ABI v3) and
calls the print family (`nova_print_int`, `nova_print_float`, `nova_print_bool`,
`nova_print_char`, `nova_print_str`, `nova_print_newline`), the string helpers
(`nova_strlen`, `nova_strdup`, `nova_strcat`, `nova_strcmp`, `nova_substr`,
`nova_tostring_int`, `nova_tostring_bool`) and the bounds-checked array accessors
(`nova_array_get`, `nova_array_set`, `nova_array_len`), plus `nova_fail`,
`nova_panic`, `nova_exit` and `nova_unsupported`. Arrays are the `NovaArray`
struct (pointer + length) declared in the header. The compiler writes
`nova_runtime.h` and `nova_runtime.c` itself — beside the C for `emit -o` and
`build`, and into the temporary build directory for `run` — and links the source
into the executable.

To compile emitted C by hand:

```bash
./target/release/nova-compiler emit examples/hello.nova -o /tmp/hello.c
cc /tmp/hello.c /tmp/nova_runtime.c -o /tmp/hello
```

The shim's ABI lives in `compiler/src/runtime.rs`; `nova_runtime_version()`
reports version `2`.

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

Module loading lives in `compiler/src/modules.rs`: `use` and `mod name;` are resolved
at compile time and the imported items are merged ahead of the program's own.

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
ternaries. A guarded arm (`x if x > 10`) cannot be a `case`, so it uses the if/else
chain with the binding declared before the guard. Arrays are `NovaArray` values: a literal becomes `(NovaArray){ (int[]){…}, n }`
and indexing goes through `nova_array_get`/`nova_array_set`, so both reads and
writes are bounds-checked and abort on a bad index. Literals are heap-allocated with
`nova_array_from`, and `__builtin_array_push`/`__builtin_array_pop` grow and shrink
the local array through `nova_array_push`/`nova_array_pop`.

`__builtin_*` names need no declaration: `__builtin_print`, `__builtin_println`,
`__builtin_typeof` (answered at compile time), `__builtin_tostring`,
`__builtin_strlen`, `__builtin_strcat`, `__builtin_strcmp`, `__builtin_substr`,
`__builtin_array_len`, `__builtin_panic` and `__builtin_exit` lower to the shim.
Other builtins resolve but trap with `nova_unsupported` until they are implemented.

An array parameter declared `&mut [T]` is passed by address: C sees `NovaArray*`,
the call site passes `&array`, and indexing dereferences it, so a push inside the
callee updates the caller's header. Other reference positions still erase to their
base value.

Generic structs and their methods are monomorphised from the annotation or the
receiver: `Box<int>` emits `Box_int` with `T` replaced in the field types
(`Pair<A, B>` → `Pair_int_float`), and `impl Box<T> { fn get(&self) -> T }` emits
`Box_int_get` and `Box_float_get`. An unannotated struct literal keeps the erased
generic type.

A string range index is a slice: `text[0..5]` emits `nova_substr(text, 0, 5 - 0)`.
String literals in `match` compare with `nova_strcmp`; a statement-position match
with string arms becomes an if/else chain because C `case` labels are integers.

`Option<T>` is built in: `Some(value)` and `None` resolve without declarations, are
stored as a tagged struct, and `Some(v)`/`None` patterns in a statement `match`
test the tag and bind the payload. Declared enums with payloads work the same way
(`Shape.Circle(7)`, or `Result<T, E>` with the payload type substituted from the
matched type); a payload is one scalar (`int`, `float` or `string`).

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
parse. All four modules now emit C: `use core.*;` (and text's `use io.*;`) pull the
sibling module's declarations in, and the module loader searches beside the
importing file and then `stdlib/<name>/<name>.nova` in the source directory or an
ancestor. A two-file program using `use helper.*;` builds and runs.

It also checks every `return` (and a trailing expression) against the
function's declared return type: a mismatch is `E0308`, an `int` may promote to a
`float`, `return;` in a non-unit function is rejected, a value from a function with no
return type is rejected, and a non-unit function with no `return` and no tail
expression is rejected. Expression types it cannot infer (identifiers, field access,
arbitrary calls) are never guessed at, so this cannot produce a false error.

Closures lower to a lifted top-level function plus a heap environment: a closure
value is a `NovaClosure*`, its captures are copied into a generated `__Env_N`
struct, the lifted body reads them through `__env`, and every lifted function takes
`void *__env` first. Closure parameters take an annotation (`|x: float|`) or the
binding's function type, and otherwise default to `int`. Tuples lower to generated
`NovaTuple_*` structs with `_0`,
`_1`, … fields and destructure with `let (a, b) = pair;`.

Parsed but not lowered yet: general slices and references (only `&mut` array
parameters and string range indexes are lowered).

Arrays are `NovaArray` (heap buffer, length and capacity) with `int` elements.
The JavaScript, Rust and WebAssembly generators live in `backends/` and consume the
SIR from `nova-common`; `emit --target js|rust|wasm` selects them, while build/run
remain C-only.
