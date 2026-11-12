# NOVA Programming Language Examples

This directory contains example programs demonstrating various features of the NOVA programming language.

## Basic Examples

### hello.nova
The classic "Hello, World!" program - demonstrates basic I/O with the `print` function.

```nova
print("Hello, NOVA");
```

### fibonacci.nova
Recursive Fibonacci sequence calculation demonstrating:
- Function definitions with parameters and return types
- Recursive function calls
- Conditional statements (`if`)
- While loops
- Multiple function organization

**Run:** `nova compile examples/fibonacci.nova && ./fibonacci`

### array_ops.nova
Array manipulation examples showing:
- Array literal initialisation
- Array indexing and mutation
- Passing arrays to functions
- Common array algorithms (sum, max, reverse)
- Loop patterns for array traversal

> **On the type annotation.** The parser has no array *type* syntax — `parse_type_expr`
> reads a single name, so `[int]` and `[int; 5]` do not parse. Array parameters and
> variables are therefore annotated with the plain name `Array`, exactly as `int` and
> `string` are plain names. The annotation is a name, not a checked type; adding real
> array types is a parser change, not an example change.

**Run:** `nova compile examples/array_ops.nova && ./array_ops`

## Advanced Examples

### basic/ Directory
`basic/hello.nova` is the same hello-world as `hello.nova`, kept as the entry point
of the "start here" path. The other fundamentals — typed declarations, `if`/`else`,
`while`, `for`, function definitions and calls — are demonstrated in `fibonacci.nova`
and `array_ops.nova` rather than in separate files.

### web/ Directory
`web/app.nova` builds and prints the page a small site would serve. It is a program,
not a server: NOVA has no HTTP server, no JSON serialisation and no `async`/`await`
yet — those are Phase 4 work, and this file does not pretend otherwise.

## Every example is checked

`tools/example-check.py` reads `grammar/grammar.ebnf`, turns it into a recogniser and
requires every `examples/**/*.nova` to parse from `program` to EOF. It runs in CI
beside `tools/grammar-check.py`, so an example that is not a NOVA program fails the
build instead of being discovered by a reader. It also refuses a file of comments with
no program in it, and it self-tests the recogniser against known-good and known-bad
sources so the check cannot pass by being vacuous.

```bash
python3 tools/example-check.py
```

## Running Examples

### Prerequisites
1. Install Rust toolchain: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. A C compiler on `PATH` (`cc`, `gcc` or `clang`; override with `CC`)
3. Build the compiler: `cargo build --release`

### Compilation and execution
```bash
# Compile and run in one step
./target/release/nova-compiler run examples/hello.nova

# Build an executable (leaves fibonacci.c beside it for inspection)
./target/release/nova-compiler build examples/fibonacci.nova -o fibonacci
./fibonacci

# Only emit C: to stdout, or to a file
./target/release/nova-compiler examples/hello.nova
./target/release/nova-compiler emit examples/hello.nova -o hello.c
```

Only the C target is wired into the CLI; `--target javascript|rust|wasm` is
rejected rather than silently ignored. The four generators live in `backends/`.


## Example Output

### fibonacci.nova
```
First
10
Fibonacci numbers:
0
1
1
2
3
5
8
13
21
34
```

### array_ops.nova
```
Original array:
5
2
8
1
9
Sum:
25
Max:
9
Reversed array:
9
1
8
2
5
```

## Contributing Examples

When adding new examples:

1. **Keep it focused** - Each example should demonstrate 1-2 specific features
2. **Add comments** - Explain non-obvious code sections
3. **Include expected output** - Document what the program should print
4. **Test thoroughly** - `python3 tools/example-check.py` must pass; that proves the
   example is a NOVA program, and a CI run proves the compiler accepts it
5. **Update this README** - Add your example to the appropriate section

Example template:
```nova
// NOVA Example: [Feature Name]
// Brief description of what this demonstrates

fn main() -> int {
    // Your code here
    return 0;
}
```

## Learning Path

For beginners learning NOVA:

1. Start with `hello.nova` - understand basic syntax
2. Move to `basic/` directory - learn fundamentals
3. Try `fibonacci.nova` - understand recursion and control flow
4. Explore `array_ops.nova` - work with data structures
5. Check `web/` directory - build real applications

## Troubleshooting

**Compilation errors:**
- Ensure all statements end with semicolons (at top level the `;` is required)
- Verify type annotations are names the parser reads — a single identifier, never `[T]`
- Check function signatures match calls

**Runtime errors:**
- Check array bounds access
- Verify division operations don't divide by zero
- Ensure recursive functions have base cases

For more help, see the main [README.md](../README.md) or [documentation](../docs/).
