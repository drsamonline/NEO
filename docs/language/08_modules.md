# Modules

NOVA's module system is a **file loader**, not a namespace system. `use name;` finds a
file, parses it, and merges its declarations into the program being compiled. Nothing is
qualified afterwards: an imported `sqrt` is `sqrt`.

The grammar:

```
use_decl = "use" path ";" ;
path     = identifier { ( "::" | "." ) ( identifier | "*" ) } ;
mod_decl = "mod" identifier ;
```

## Resolution order

For a module named `name`, `compiler/src/modules.rs` looks in this order and stops at
the first hit:

| # | Where |
|---|---|
| 1 | `name.nova`, `name/mod.nova`, `name/name.nova` beside the importing file |
| 2 | the same three shapes under each **module root** |
| 3 | `stdlib/name/name.nova` in the source directory or any ancestor |

Module roots come from `--module-root`, which `nova-pkg` passes once per dependency.
That is how a dependency's source becomes importable, and it is the only reason the
compiler knows anything about packages — it reads no manifest and fetches nothing.

**A module root is searched before `stdlib`**, so a project can shadow a standard
library module with its own.

## What a `use` does

- Every `Item` in the module is merged into the program, **before** the importing
  file's own items.
- Each file is loaded **once**; a canonical path already seen is skipped, so a cycle
  does not loop.
- A declaration name already merged is skipped, so two modules cannot emit two C
  functions with the same name.

That last rule is why `stdlib/io` and `stdlib/core` could not both declare `chdir`: the
second copy was silently dropped, and the emitted C then had one `chdir` where the source
had two. It was a duplicate in the library, and the fix was to delete one.

## Qualified calls

A function from an imported module can be written with the module in front:

```nova
use math;

fn main() -> int {
    print(math.log(2.0));   // 0.693147
    print(log(2.0));        // the same function, unqualified
    return 0;
}
```

Both spellings reach the same function — imported modules share one flat namespace, and the
qualified form is a way of **saying which one you meant**, not a separate scope. It is resolved by
looking the name up in the module that was imported, and it carries the function's return type, so
`math.log(2.0)` prints `0.693147` rather than `0`.

A qualified call is recognised by its head: if the identifier before the dot is a module named by a
`use`, the call is qualified. Otherwise `a.b(...)` is a method call on a value, and `Color.Red` is
an enum variant. Those three readings are checked in that order — module, then method, then enum —
and the order matters, because `math.log` and `file.read` are the same syntax.

**A collision is still fatal.** If two imported modules declare the same name, the import fails and
both modules are named:

```text
error: `log` is declared by both `math` and `io`, and NOVA has one flat namespace: importing both
is ambiguous. Rename one of them.
```

Qualifying the call does not help there, because both definitions cannot be carried into one
program — that needs the backend to emit two functions under one name. See
[ADR 0018](../adr/0018-module-qualified-names.md).

## `use name.*;` and `use a::b;`

The path is parsed and its **first element** is what gets loaded. `use core.*;` and
`use core;` do the same thing. A deeper path loads the same file as its head, so
`use a::b;` loads `a`.

This is deliberate and simple: there is no per-symbol import and no aliasing.

## `mod name;`

Loads exactly what `use name;` loads. There is no inline `mod name { ... }` form.

## A `use` that resolves to nothing

It is left alone. The analyser still sees the item, and name resolution fails with
`E0002` at the first use of a missing name — which points at the use, not at the import.

## The trap worth knowing

**A program named after the module it imports imports itself.** `find_module` looks
beside the importing file first, so a file called `math.nova` containing `use math;`
finds itself, loads it, and reports `E0002` for every name the real module would have
provided. `compiler/tests/end_to_end.rs` names its scratch files `*_program.nova` for
this reason, with a comment saying so.

## What is missing

- No namespacing: two modules declaring `helper` is a collision. It used to be resolved silently by
  first-wins; it is now refused, naming both modules. A qualified call (`math.log(x)`) exists but
  does not resolve a collision, because both definitions cannot live in one program.
- No visibility, no re-export, no `use ... as ...`.
- No cycle detection — a cycle loads each file once and stops, which is not the same as
  reporting it.

## Where this is tested

`compiler/src/modules.rs` unit tests (resolution beside the file, from an ancestor, from
a module root, and root-before-stdlib), plus the stdlib compile gate in CI.
