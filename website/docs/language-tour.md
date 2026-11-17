# Language tour

The syntax the compiler accepts today. Every snippet here is taken from
`examples/`, `stdlib/` or the tests, so it is code that compiles — not a plan.
Where a feature stops short, it says so and points at `DEV_PLAN.md`.

## A program

```nova
fn main() -> int {
    print("Hello, NOVA");
    return 0;
}
```

`main` returns `int`. `print` is a compiler builtin, not a library import, and it
ends the line itself. Statements end in `;`. A function with no declared return
type returns nothing.

## Comments

```nova
// a line comment
# also a line comment
/// a doc comment, attached to the item below it
```

Both `//` and `#` are accepted. `#` exists because the standard library is written
in it.

## Bindings

```nova
let x: int = 40;          // annotated
let mut i = 0;            // inferred, reassignable
const PI: float = 3.141592653589793;
static SEED: int = 12345;
```

`int` → `float` is the only implicit conversion. Everything else is written out.

## Functions

```nova
fn add(a: int, b: int) -> int {
    return a + b;
}

fn sign(x: int) -> int {
    if x < 0 { -1 }
    else if x > 0 { 1 }
    else { 0 }
}
```

A block's last expression is its value, so `sign` needs no `return`. A function
that never returns declares `-> !`:

```nova
fn panic(message: String) -> ! {
    __builtin_panic(message)
}
```

## Control flow

```nova
if n <= 1 {
    return n;
} else {
    return 0;
}

while i < count {
    i = i + 1;
}

for i in 0..10 {
    print(i);
}
```

`if`, `while` and a range `for` are the three loops and branches. `for i in a..b`
is the range form. `match` is the fourth:

```nova
match value {
    0 => print("zero"),
    n => print(n),
}
```

`match` supports integer and string literals, `Enum.Variant` paths, binding
patterns and guards. On the C target a statement `match` becomes a `switch`;
guarded or string arms become an if/else chain.

## Structs and methods

```nova
struct Range {
    start: int,
    end: int,
    current: int,
}

impl Range {
    fn next(&mut self) -> Option<int> {
        if self.current < self.end {
            let value = self.current;
            self.current = self.current + 1;
            Some(value)
        } else {
            None
        }
    }
}

fn main() -> int {
    let mut r = Range { start: 0, end: 3, current: 0 };
    print(r.next());
    return 0;
}
```

`Some(x)` and `None` are built in: `Option<T>` is a payload enum of the shape the
compiler already understands.

## Enums

```nova
enum Shape {
    Circle(float),
    Square(int),
}
```

A variant carries **one** scalar payload — `int`, `float` or `string`. A variant
with two payload fields does not lower yet.

## Arrays

```nova
fn total(arr: [int]) -> int {
    let sum: int = 0;
    let mut i = 0;
    while i < array_len(arr) {
        sum = sum + arr[i];
        i = i + 1;
    }
    sum
}

fn push_all(arr: &mut [int]) {
    array_push(arr, 1);
}

fn main() -> int {
    let numbers: Array = [5, 2, 8, 1, 9];
    print(numbers[0]);
    numbers[0] = 42;
    return 0;
}
```

Arrays are growable and bounds-checked. An out-of-range index aborts with
`array index out of bounds` rather than reading memory it does not own. `&mut
[int]` passes the header by address, so a push in the callee is visible to the
caller. Elements are `int` for now.

## Strings

```nova
let text: string = "Hello, NOVA";
let part: string = text[0..5];
```

Slicing is `text[a..b]`. There is no string interpolation — write the markup out
in full.

## Generics

```nova
fn print_value<T>(value: T) {
    print(value);
}

fn map<A, B>(arr: [A], f: fn(A) -> B) -> [B] {
    // ...
}
```

Generics are monomorphised from concrete call sites: only the instantiations you
call are emitted, and a type parameter nothing binds falls back to `int`.
Functions, structs, enums and `impl` methods all take them.

## Closures

```nova
fn compose<A, B, C>(f: fn(B) -> C, g: fn(A) -> B) -> fn(A) -> C {
    |x| f(g(x))
}
```

A closure is a lifted function plus a heap environment; captures are copied by
value when the closure is created. On the Rust target a closure is an
`Rc<dyn Fn(..) -> ..>` over a generated environment struct.

## Modules

```nova
use core;
use io;
```

`use` and `mod` load sibling and standard-library files. There are four standard
library modules — `core`, `io`, `math` and `text` — and they are thin: `core` has
the maths helpers (`factorial`, `fibonacci`, `sign`, `clamp`, `gcd`) and the rest
is largely scaffolding.

## Builtins

Names beginning `__builtin_` are declared by the compiler and lowered to the
runtime shim:

| Builtin | Does |
|---|---|
| `__builtin_print` | print a value and end the line |
| `__builtin_array_len`, `__builtin_array_push`, `__builtin_array_pop` | array operations |
| `__builtin_strlen`, `__builtin_strcat`, `__builtin_strcmp`, `__builtin_substr` | strings |
| `__builtin_panic`, `__builtin_exit` | stop the program |
| `__builtin_typeof`, `__builtin_tostring` | type inspection |

Some resolve but trap through `nova_unsupported` on the C target: `args`,
`getenv`, `now`, `sleep`, `cwd`, `chdir`. They are declared, not implemented.

## What is not here

- No `async`/`await` in the language, though the runtime crate has futures.
- No traits, though the parser accepts `trait` blocks.
- No lifetimes, no borrow checker: references are lowered only for array
  parameters.
- No `impl Trait`, no operator overloading, no macros.
- No optimisation levels: `--release` picks an output directory.

`DEV_PLAN.md` "Known limitations" is the full list, and it is maintained. If a
snippet in this page stops compiling, the page is wrong — please open an issue.
