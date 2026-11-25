# Style Guide

NOVA has one formatter and a handful of conventions. The conventions below are not invented: each
is what the standard library already does, and each is stated with the count that made it a rule.

## Naming

| Kind | Convention | Evidence |
|---|---|---|
| functions | `snake_case` | **230 of 230** standard-library functions; none is `CamelCase` |
| structs | `PascalCase` | `Range`, `File`, `FileMetadata` |
| enums | `PascalCase` | `StreamType`, `LogLevel` |
| constants | `SCREAMING_SNAKE_CASE` | `PI`, `E`, `PHI` — 31 constants, all of them |
| variables | `snake_case` | `let mut line`, `let numbers` |
| modules | `snake_case`, matching the file | `use math;` loads `math.nova` |

A name that reads as a question — `is_empty`, `has_next` — is the house style for a predicate;
`get_` prefixes are not used, because a NOVA function that returns a value does not need to say so.

## Formatting is mechanical

```text
$ nova-compiler format prog.nova        # rewrite in place
$ nova-compiler format prog.nova -o out.nova
```

The formatter **normalises indentation only**. It does not respace a line: given

```nova
fn  badly_formatted( x:int,y :int)->int{
  return x+y;
}
```

it indents the body and leaves the rest exactly as written. That is worth knowing before running it
on a file you expect to be tidied — it is a re-indenter, not a formatter in the `gofmt` sense.

It is **idempotent**: a second run says `already formatted`, verified.

## Indentation

Four spaces per level, and **no tabs** — zero files in the standard library contain one. A block is
indented one level from the line that opens it:

```nova
fn main() -> int {
    if true {
        print(1);
    }
    return 0;
}
```

## Declarations

- **A function ends with an explicit `return`.** The standard library does this in all four modules
  (27 `return` statements), rather than relying on a trailing expression. A trailing expression is
  legal — see [Functions](07_functions.md) — but the library does not use it.
- **`let` takes a type only when it is not obvious** from the initialiser: 2 of 31 bindings in
  `core.nova` carry one.
- **`let mut` is required to reassign.** There is no other way to write to a binding.

## Comments

- **`///` documents an item**, above it, and says *why* the item exists rather than restating its
  name: 53 such comments in `core.nova` alone.
- **`//` explains a decision** inside a function. The rule this repository follows is that a
  comment which restates the code is noise, and a comment which records *why* is the only defence
  the next reader has.
- A comment is **not** a substitute for a rejection. If the compiler cannot express something, it
  says so with a diagnostic — a comment saying "unsupported" in the path where the work should
  happen is how several defects in this repository survived.

## What is not enforced

Nothing in this chapter is checked by the compiler. `nova-compiler format` enforces indentation and
nothing else; the rest is convention, held by review. The numbers in the tables are what the corpus
does today, not what a tool will reject tomorrow.
