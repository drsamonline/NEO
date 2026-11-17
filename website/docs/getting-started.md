# Getting started

Build the compiler and run a program. Ten minutes, one toolchain, one C compiler.

## What you need

| Tool | Why | Check |
|---|---|---|
| Rust (stable) | to build the compiler | `cargo --version` |
| a C compiler — `cc`, `gcc` or `clang` | `build` and `run` link C | `cc --version` |
| `node` | optional: run generated JavaScript, and the WASM host | `node --version` |
| `wat2wasm` | optional: the WASM target | `npm install --global wat2wasm` |

The compiler itself has no external Rust dependencies; everything else is a
workspace crate.

## Build

```bash
git clone https://github.com/drsamonline/NEO.git
cd NEO
cargo build --release
```

## Check what your machine can do

```bash
./target/release/nova-compiler doctor
```

It reports which toolchains it found and which backends are therefore ready. Add
`--json` if you want to script it.

## Run a program

```bash
./target/release/nova-compiler run examples/hello.nova
```

```
Hello, NOVA
```

## Compile one

```bash
./target/release/nova-compiler build examples/fibonacci.nova -o fibonacci
./fibonacci
```

`build` leaves `fibonacci.c` beside the binary, which is the whole point of the
project: the generated C is readable, and you can compile it yourself.

## Emit another target

```bash
./target/release/nova-compiler emit examples/hello.nova --target js   > hello.js
./target/release/nova-compiler emit examples/hello.nova --target rust > hello.rs
./target/release/nova-compiler emit examples/hello.nova --target wasm > hello.wat
```

Only C can be built and run by the CLI. For the other three, take the output to
that language's own toolchain. To check that all four agree on a program:

```bash
./target/release/nova-compiler test --parity examples/hello.nova
```

## Write your first program

```nova
fn main() -> int {
    let name: string = "world";
    print("Hello, " + name);
    return 0;
}
```

Save it as `first.nova` and `run` it. Note that `main` returns `int`, `print` is
a builtin rather than a library function, and statements end in `;`.

## Where to go next

- [Language tour](language-tour.md) — the syntax the compiler accepts today.
- [Examples](../../examples/) — five programs, all built and run in CI.
- [Development plan](../../DEV_PLAN.md) — what is done, what is partial, and the
  "Known limitations" list. Read this before you file a bug: several things that
  look broken are documented gaps.
- [Contributing](../../CONTRIBUTING.md) — the checks a pull request must pass.

## When it goes wrong

| Symptom | Cause |
|---|---|
| `cannot read …` | wrong path, or the file is not UTF-8 |
| `parsing failed` | the source uses syntax the parser does not accept; check the line:column it prints |
| `semantic analysis failed` | a name, an arity or a type is wrong; the diagnostic carries a `help:` line |
| `C compiler not found` | install `cc`, or set `CC=/path/to/compiler` |
| `the wasm target does not support …` | the WASM backend rejected a signature it cannot express; use `--target c`, `js` or `rust` |

`nova-compiler doctor` answers most environment questions in one command.
