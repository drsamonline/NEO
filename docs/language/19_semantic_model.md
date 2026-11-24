# The semantic model

Between parsing and lowering there is a pass that answers one question: **does this program mean
what it says?** It is `compiler/src/sema`, and what it checks is narrower than the name suggests.

```
parse → AST → sema → lowerer → SIR → backend
```

If `sema` reports an error, the program stops there. Nothing is lowered and no C is emitted.

## Two passes over the whole file

```rust
pub fn analyze(&mut self, program: &Program) -> Result<(), ()> {
    self.symbol_table.enter_scope();          // global scope
    for item in &program.items { self.collect_declaration(item); }   // 1: names
    for item in &program.items { self.check_item(item); }            // 2: bodies
    ...
}
```

**The first pass collects every declaration in the file; the second checks every body.** That
ordering is why a function declared later is visible to one declared earlier:

```nova
fn main() -> int {
    print(later(2));    // works — `later` is collected before any body is checked
    return 0;
}

fn later(x: int) -> int { x * 2 }
```

There is no forward-declaration syntax and none is needed.

## What it checks

| Check | Code |
|---|---|
| A name is used that nothing has declared in scope | `E0002` |
| A call passes a different number of arguments than the callee declares | `E0061` |
| An argument's type disagrees with its parameter | `E0308` |
| A `return` value disagrees with the declared return type | `E0308` |
| A bare `return` in a function that declares a return type | `E0308` |
| A block's final expression disagrees with the declared return type | `E0308` |

Three codes cover seven situations; `E0308` carries four of them and the message says which.
That is deliberate — the code is for searching, the message is for reading.

**`int` → `float` is the only implicit conversion.** Everything else is written with `as`.

## What it does not check

This is the part a reader needs, because each gap shows up later in a less helpful place.

- **Duplicate function definitions are now detected**, and were not before. `SymbolTable::insert`
  returns the symbol it replaced and every caller discarded it, so a second `fn find` silently
  won. It surfaced when a user function named `find` was replaced by the standard library's
  `find<T>`: the emitted C called the library's signature with the user's arguments. It is now
  `E0308` with both line numbers and a help line. Duplicate *variables* in one scope are still
  not checked — only functions.
- **There is no type table.** Types are strings here as they are in the [SIR](20_sir_intro.md),
  so two spellings of one type are two strings and structural comparison is textual.
- **Nothing checks mutation or aliasing.** The parser has already discarded the difference
  between `&` and `&mut`, so there is nothing left to check — see [Memory](11_memory.md).
- **Nothing checks that `main` exists**, or that it has a shape a program can start from. A
  library file is a valid program.
- **A function that declares a return type can fall off the end without returning.** There is a
  `check_missing_return` in the analyser, but this compiles without a diagnostic:

  ```nova
  fn bad(x: int) -> int {
      if x > 0 { return 1; }
  }
  ```

  The C compiler accepts it too, and the function returns whatever was in the return register.
  Treat a missing `return` as your responsibility, not the compiler's.
- **Nothing checks generics for consistency beyond arity.** A generic is instantiated at the
  call site; whether the body type-checks at that instantiation is discovered when the
  instantiated body is lowered.

## Diagnostics are accumulated, not thrown

A failed check does not abort the pass. The analyser records a diagnostic and continues, so one
run reports as many problems as it can find rather than only the first. `analyze` returns
`Err(())` at the end if any diagnostic was an error.

Warnings exist and do not stop the build.

## Why the model is small

The checks are the ones that can be made without a type system richer than strings. A borrow
checker, an exhaustiveness check over enums, a trait solver — each needs a resolved type
identity the front end does not build. They are not missing because nobody thought of them;
they are missing because the representation cannot express the question.

`docs/errors.md` lists the three codes and is checked against the compiler's own table by
`tools/error-code-check.py`, so the page cannot list a code that is never emitted.
