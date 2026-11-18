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

- No namespacing: two modules declaring `helper` is a collision, silently resolved by
  first-wins.
- No visibility, no re-export, no `use ... as ...`.
- No cycle detection — a cycle loads each file once and stops, which is not the same as
  reporting it.

## Where this is tested

`compiler/src/modules.rs` unit tests (resolution beside the file, from an ancestor, from
a module root, and root-before-stdlib), plus the stdlib compile gate in CI.
