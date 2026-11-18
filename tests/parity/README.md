# Parity corpus

Programs that must produce identical stdout and exit status on all four backends.
Run by `nova-compiler test --parity`, which CI applies to every file here and every
`examples/*.nova`.

| Program | Exercises |
|---|---|
| `array_push.nova` | growable arrays, and a `&mut [int]` parameter whose push is visible to the caller |
| `blocks.nova` | block expressions in value position, nested, with locals |
| `closures.nova` | block-bodied closures, string parameters, printing a closure's result |
| `structs.nova` | struct literals, field access, tuples and destructuring |
| `unicode_comment.nova` | non-ASCII in a comment, ASCII output — the lexer must not corrupt what follows |
| `unicode_string.nova` | `नमस्ते` and `કેમ છો` printed and compared byte-for-byte across all four backends |

## Writing one

The smallest program that exercises the feature, printing everything it computes —
the harness compares stdout, so a value that is not printed is not compared. Keep it
inside the subset all four backends claim, or it belongs in `examples/` instead where
only the C path is required to run it.

## Why six files and not six hundred

Because `tools/corpus-gen.py` generates hundreds. Parity alone cannot catch four
backends that agree and are all wrong, so the generated corpus ships an `.expected`
file computed in Python and `test --parity` checks the C reference against it. These
six are the hand-written cases that pin specific language features by name.
