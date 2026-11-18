# Control flow

Everything here is an expression, including `if` and `match`. A block's last expression,
when it has no semicolon, is the block's value.

## `if` / `else`

```nova
let label = if n > 0 { "positive" } else { "not positive" };
```

`else if` chains are parsed. The condition must be `bool`; there is no truthiness.

## `while`

```nova
while i < 10 {
    i = i + 1;
}
```

The body is a block and must be one — `while cond stmt;` is not accepted.

## `loop`

```nova
loop {
    if done { break; }
}
```

`break` and `continue` are statements. `break` does not take a value, so a `loop` is
`unit`.

## `for`

```nova
for i in 0..10 {
    print(i);
}
```

The iterable is an expression — in practice a range, `start..end`, which is exclusive of
`end`. The loop variable is a fresh binding each iteration. Arrays are **not** iterable
yet, which is why `stdlib` has `array_len` and indexing rather than `for x in arr`.

## `match`

```nova
match value {
    0 => print("zero"),
    n if n > 0 => print("positive"),
    _ => print("negative"),
}
```

- Arms are `pattern [if guard] => expr`, with an optional trailing comma.
- Patterns: integers, string literals, `Enum.Variant`, `Some(x)`, bindings, and `_`.
- A variant carrying more than one payload is not lowered.
- There is no exhaustiveness check. A `match` that covers nothing falls through to a
  zero value.

## `return`

```nova
return value;   // in a function with a return type
return;         // in a unit function
```

A bare `return` in a function that declares a type is `E0308`; so is `return value;` in
one that does not.

## Blocks as expressions

```nova
let x = {
    let y = 3;
    y + 4
};
```

The C backend has no block expressions, so it hoists the block into a typed temporary and
writes the statements before the enclosing statement. That is why the generated C is
standard C rather than a GCC statement expression.

## What is missing

- No `for x in array`, no iterators, no `yield`.
- No labelled `break`, no `break` with a value.
- No exhaustiveness checking, so a missing `match` arm is a zero rather than an error.
- No `do ... while`, no `goto`.

## Where this is tested

`tests/parity/blocks.nova` (block expressions across all four backends),
`examples/fibonacci.nova` and `examples/array_ops.nova` (loops), and
`tests/diagnostics/E0308_bare_return.nova` for the `return` rules.
