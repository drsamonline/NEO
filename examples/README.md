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
- Array declaration and initialization
- Array indexing and mutation
- Passing arrays to functions
- Common array algorithms (sum, max, reverse)
- Loop patterns for array traversal

**Run:** `nova compile examples/array_ops.nova && ./array_ops`

## Advanced Examples

### basic/ Directory
Contains fundamental examples covering:
- Variable declarations and type annotations
- Control flow (if/else, while, for)
- Function definitions and calls
- Basic data types (int, float, bool, string)

### web/ Directory
Demonstrates NOVA's web development capabilities:
- HTTP server creation
- Request/response handling
- JSON serialization
- Async/await patterns

## Running Examples

### Prerequisites
1. Install Rust toolchain: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Build the compiler: `cargo build --release`
3. Add to PATH: `export PATH="$PWD/target/release:$PATH"`

### Compilation
```bash
# Compile a NOVA file to native code
nova compile examples/hello.nova

# Compile with optimization
nova compile --optimize examples/fibonacci.nova

# Compile to different backends
nova compile --target c examples/hello.nova
nova compile --target javascript examples/hello.nova
nova compile --target rust examples/hello.nova
nova compile --target wasm examples/hello.nova
```

### Execution
```bash
# Run compiled binary
./hello

# Or use the run command
nova run examples/hello.nova
```

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
4. **Test thoroughly** - Ensure the example compiles and runs correctly
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
- Ensure all statements end with semicolons
- Verify type annotations are correct
- Check function signatures match calls

**Runtime errors:**
- Check array bounds access
- Verify division operations don't divide by zero
- Ensure recursive functions have base cases

For more help, see the main [README.md](../README.md) or [documentation](../docs/).
