# Diagnostic goldens

One `.nova` case per diagnostic, one `.stderr` golden beside it, checked by
`tools/diagnostic-check.py` in CI.

A diagnostic is part of the compiler's interface. Its code, its message, the span
it points at and the `help:` line under it are what a user acts on. Before this
directory existed, none of them was pinned: a message could be reworded, a span
could drift onto the wrong line, or a diagnostic could vanish, and every test in
the repository still passed.

## Two checks, deliberately redundant

| Check | Pins | Fails when |
|---|---|---|
| `.stderr` golden | exact text, span and help | anything in the output changes |
| `//~ ERROR [Exxxx]` annotation in the source | which diagnostics are *expected* | an extra diagnostic appears, or an expected one stops firing |

Either alone would catch a change. They fail *differently*, and that is the point:
the golden catches "the message changed", the annotation catches "a diagnostic
nobody asked for now fires on this program". rustc's UI tests work the same way
for the same reason.

The annotation is file-level, not line-anchored. rustc anchors `//~` to a line,
which is more precise; here the golden already records the span exactly, so the
annotation's job is the coarser question of which codes should appear.

## Format

```
exit: 1
---
tests/diagnostics/<name>.nova: semantic errors:
  <case>:4:11: [E0002] undefined name `missing`
    help: declare `missing` first (`let missing = ...` or `fn missing(...)`), import the module
        that defines it (`use core;`, `use io;`, `use math;`, `use text;`), or check the spelling
error: semantic analysis failed
```

- Line 1 is the exit code, so a case that starts succeeding fails the check.
- Everything after `---` is the compiler's stderr, with the case path replaced by
  `<case>` and the `✓ wrote …` line removed (it names a temp path).
- A case with no annotation must exit 0 and print nothing. That is how a
  diagnostic that starts firing on valid code gets caught.

## Running it

```bash
cargo build -p nova-compiler
python3 tools/diagnostic-check.py
```

A failure prints the golden and the compiler's actual output. To accept the
compiler's output:

```bash
python3 tools/diagnostic-check.py --bless
git diff tests/diagnostics/
```

**Read the diff before committing it.** A blessed golden nobody looked at is a
test that agrees with the bug.

## Adding a case

1. Write `tests/diagnostics/<CODE>_<short_name>.nova`, minimal — the smallest
   program that produces the one diagnostic.
2. Add `//~ ERROR Exxxx` above the line that triggers it.
3. `--bless`, then read the diff.

Keep the program as small as possible. Extra syntax produces extra diagnostics,
and a golden full of unrelated output stops being readable at exactly the moment
someone needs to read it.
