# Glossary

Terms this book uses with a specific meaning. Where a term has a definition in the code, the
definition here matches it.

## ADR

Architecture Decision Record. A short file in [`docs/adr/`](../adr/) that records one decision
— the context, the options considered, the choice, and its consequences. There are fourteen.

The point of an ADR is not to describe the current design; it is to record **why the
alternatives lost**, so that the next person can tell a deliberate choice from an oversight
before changing it.

## AST

Abstract syntax tree. The parser's output: a tree of the source's structure, with spans.

The AST is shaped for reading source. It still contains things a code generator should not
have to think about — a method call, a closure, a generic call, a `match` — which is what
[lowering](#lowering) exists to remove.

## backend

The last stage: it takes [SIR](#sir) and emits a language. There are four — C, JavaScript, Rust
and WebAssembly. **Only the C backend produces something this project runs**; the other three
are asserted to emit, not to be correct.

## builtin

A function whose body is a `;` and whose name starts `__builtin_`, mapped by the lowerer onto a
function in the C runtime. `__builtin_sha256` becomes `nova_sha256`. They are how the standard
library reaches the machine.

A builtin with no runtime implementation does not fail silently: it traps with
`builtin not implemented: <name>`. That is deliberate — see [trap](#trap).

## closure lifting

How a closure becomes something C can run. The closure's body is lifted into a top-level
function, and the variables it captures are copied into a heap-allocated environment struct
passed as an extra leading `void *__env` parameter. The lifted function's `SirFunction` carries
an `env_type`; reading a captured variable out of it is `EnvAccess`.

## constitution

[`docs/specs/constitution.md`](../specs/constitution.md). Eight rules that govern what may be
claimed. The ones a reader will meet most:

1. **Never invent completed functionality** — do not document what does not exist.
3. **A backend that cannot express something rejects it** — a diagnostic, not C that compiles
   and misbehaves.
4. **Numbers come from a command** — every figure in a document is produced by something that
   ran.
5. **Say what you did not verify.**

## diagnostic

A compiler message with a severity, a message, an optional help line and an optional span.
Three error codes exist:

| Code | Meaning |
|---|---|
| `E0002` | undefined name |
| `E0061` | wrong number of arguments |
| `E0308` | mismatched types — one code for four situations, distinguished by the message |

`docs/errors.md` is checked against the compiler's own table by `tools/error-code-check.py`, so
the page cannot list a code that is never emitted.

## gate

A script under `tools/` that fails the build when a property stops holding. There are nine and
none of them needs a C toolchain: docs, stdlib, grammar, example, type-name, extension,
diagnostic, error-code, status.

A gate is worth having only if it has been seen to fail — a check that has never caught
anything is not evidence.

## handle

An index into the runtime's 64-slot file table. `File.handle` and `Stream.handle` are both
handles; `-1` means "not open". Slots are reused after a close, and opening a 65th file traps
rather than leaking.

A handle is why a `Stream` needs no runtime of its own — see [ADR 0013](../adr/0013-streams-are-file-handles.md).

## lowering

The pass that converts the [AST](#ast) into [SIR](#sir). It is where the decisions a backend
would otherwise make repeatedly — and inconsistently — are made once: how a method call becomes
a function call with a pointer receiver, how `Some(v)` becomes a struct literal, how a `for`
loop becomes an iterable and a name.

## mangling

Encoding a type name into an identifier. `array_pop` at `int` becomes `array_pop_int`;
`Option<int>` becomes `Option_int`. Two instantiations of one generic are two C functions with
two names.

Mangling is also where a type can lose information: `[int]` and `int` both mangle to `int`
unless the brackets are handled, which is a defect the project has hit.

## monomorphisation

Generating one concrete function per type a generic is used at, rather than dispatching at run
time. NOVA has no boxing and no vtables: a generic function at `int` is a function that takes an
`int`. Code size grows with the number of instantiations, not with the number of definitions.

## NovaArray

The runtime's array value: a heap buffer, its length and its capacity, so indexing can be
checked and `push` can grow it. Structs are copied by value, so passing an array copies a
header, not the buffer.

Because a `double` and a `char *` do not fit in an `int` slot, float arrays and string arrays
have their own headers (`NovaArray_float`, `NovaArray_string`). Since [ADR 0011](../adr/0011-int-is-64-bit.md)
an `int` element is 8 bytes.

## Option, Some, None

The built-in generic for a value that may be absent. `Option<T>` is monomorphised like any
instantiation, so `Option<int>` and `Option<String>` are distinct C structs with a `tag` and a
`payload`.

`None` has no type of its own and takes its payload from context — a binding's declared type, or
the enclosing function's return type. Without an annotation in reach it is `Option<int>`.

## ratchet

The coverage floor in `coverage-floor.txt`. Line coverage may not fall below it. A rise raises
the floor in the same commit; **a fall lowers it in the same commit with the reason written
down**, even when the fall is inside the tolerance.

The ratchet exists because a tolerance that silently absorbs a fall is how a ratchet becomes
decoration.

## reachable

Reachable from `main`. `reachable_functions` walks the call graph from the entry point so a
backend can skip imported-but-unused helpers it cannot translate.

## SIR

Semantic intermediate representation. The tree between the [AST](#ast) and every
[backend](#backend), defined in `crates/nova-common/src/sir`. Shaped for emitting code rather
than for reading source — see [the SIR chapter](20_sir_intro.md).

Its types are **strings**, not a type table, which is the source of the project's most
persistent defect class.

## trap

A run-time failure that stops the program with a message on stderr:
`runtime error: <message>`. Traps are how the compiler refuses something it cannot express —
`nova_unsupported` for an unimplemented builtin, `panic` for a deliberate stop, `fatal` for a
logged one.

The distinction that matters: a construct the compiler cannot express must **trap**, not compile
into something that misbehaves.

## Typed

A [SIR](#sir) wrapper carrying the static type of the expression inside it. Codegen needs it
because some types cannot be recovered from their C representation — a `bool` and an `int` are
the same C type, so `print(x)` chooses between `nova_print_bool` and `nova_print_int` by reading
the annotation, not the value.

## unit

The type of a value that carries nothing: what a function returns when it declares no return
type, and what a block ends with when it has no final expression.

`SirLiteral::Unit` renders as an empty string in C, which is why assigning one produced
`__block_0 = ;` and why a `break` inside a branch once failed to compile.
