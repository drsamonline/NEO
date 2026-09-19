# NOVA Language Specification v1.0

## Overview
NOVA is a statically-typed, compiled systems programming language focused on simplicity, performance, and safety.

## Lexical Structure

### Keywords
`fn`, `let`, `mut`, `if`, `else`, `while`, `for`, `return`, `true`, `false`, `struct`, `enum`, `match`, `use`, `mod`, `pub`, `const`, `type`, `as`, `in`

### Operators
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!`
- Assignment: `=`, `+=`, `-=`, `*=`, `/=`
- Bitwise: `&`, `|`, `^`, `<<`, `>>`, `~`

### Literals
- Integer: `42`, `0xFF`, `0b1010`, `1_000_000`
- Float: `3.14`, `2.5e10`
- String: `"hello"`, `"world\n"`
- Boolean: `true`, `false`
- Character: `'a'`, `'\n'`

### Identifiers
Start with letter or underscore, followed by letters, digits, or underscores. Case-sensitive.

### Comments
- Line: `// comment`
- Block: `/* comment */`

## Type System

### Primitive Types
- `i8`, `i16`, `i32`, `i64`, `i128` (signed integers)
- `u8`, `u16`, `u32`, `u64`, `u128` (unsigned integers)
- `f32`, `f64` (floating point)
- `bool` (boolean)
- `char` (unicode scalar)
- `str` (string slice)
- `String` (owned string)
- `()` (unit/void)

### Compound Types
- Arrays: `[T; N]`
- Tuples: `(T1, T2, ...)`
- Structs: `struct Name { field: Type }`
- Enums: `enum Name { Variant1, Variant2(Type) }`
- Functions: `fn(T1, T2) -> R`
- References: `&T`, `&mut T`
- Pointers: `*const T`, `*mut T`
- Option: `Option<T>`
- Result: `Result<T, E>`

## Syntax Examples

```nova
// Function definition
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

// Variable declaration
let x: i32 = 42;
mut y: i32 = 10;

// Control flow
if x > 10 {
    print("large");
} else {
    print("small");
}

// Loops
while x < 100 {
    x = x + 1;
}

for i in 0..10 {
    print(i);
}

// Struct definition
struct Point {
    x: f64,
    y: f64,
}

// Enum definition
enum Option<T> {
    Some(T),
    None,
}

// Pattern matching
match value {
    Some(x) => print(x),
    None => print("nothing"),
}
```
