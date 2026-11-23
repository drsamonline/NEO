# The Semantic IR

Between the AST and C there is one more representation. It is called SIR — the semantic
intermediate representation — and it exists for one reason: **the AST is shaped for reading
source and the backend needs something shaped for emitting C.** Lowering is the pass that
converts one into the other.

```
source → lexer → parser → AST → sema → lowerer → SIR → backend → C
```

The SIR lives in `crates/nova-common/src/sir`, which the compiler and every future backend can
depend on without depending on each other.

## What lowering decides

The AST still contains things a backend should not have to think about. Lowering removes them:

| In the AST | In the SIR |
|---|---|
| `for x in items` | `For { pattern, iterable, body }` — still a loop, but the pattern is a name, not a binding form |
| `x.method(a)` | `Call { callee: "Type_method", args: [AddressOf(x), a] }` — the receiver is a pointer, always |
| `\|x\| x + 1` | a lifted `SirFunction` with an `env_type`, plus a `Closure` value holding its heap environment |
| `Some(v)` | `StructLiteral { name: "Option_int", fields: [tag, payload] }` |
| a generic call | a call to a monomorphised name, `array_pop_int` |
| `match` with constant arms | `Switch { scrutinee, cases, default }` |
| `match` with string arms | an if/else chain, because C `switch` cannot match a string |

None of these are syntax. They are the decisions a backend would otherwise make repeatedly and
inconsistently.

## The shape

```rust
pub struct SirProgram {
    pub functions: Vec<SirFunction>,
    pub globals: Vec<SirGlobal>,
    pub structs: Vec<SirStruct>,   // emitted before the functions that use them
}

pub struct SirFunction {
    pub name: String,
    pub params: Vec<SirParam>,
    pub return_type: Option<String>,
    pub body: Vec<SirStatement>,
    pub span: Span,
    pub env_type: Option<String>,  // set for a lifted closure: an extra `void *__env`
}
```

**Types are strings.** `SirParam.typ`, `SirStructField.typ` and `SirExpr::Typed.typ` all hold
the NOVA type name — `int`, `[String]`, `Option<File>`, `fn(int) -> int`. This is deliberate and
it is the source of the project's most persistent class of defect: a type string that loses its
arguments between two places that both need it. `Option<int>` becoming `Option` makes
`c_type` answer `int`, and the program compiles to something that is not what was written.
`tools/type-name-check.py` gates the known idioms.

## Statements

```rust
pub enum SirStatement {
    Let { name, typ: Option<String>, value: SirExpr },
    Expr(SirExpr),
    Return(Option<SirExpr>),
    If { condition, then_block, else_block: Option<Vec<SirStatement>> },
    While { condition, body },
    For { pattern: String, iterable, body },
    Block(Vec<SirStatement>),
    Switch { scrutinee, cases: Vec<(SirExpr, Vec<SirStatement>)>, default },
    Break,
    Continue,
}
```

`If`, `While` and `For` appear here **and** as expressions below. That is not redundancy: a
statement-position `if` becomes a C `if`, and an expression-position `if` becomes a C block that
assigns its result. Emitting the wrong one is how `if a { x = 1; }` once produced `if (a) { ; }`
and dropped the assignment.

## Expressions

The variants fall into five groups.

**Values.** `Literal` (int, float, bool, string, char, unit), `Var`, `BinOp`, `UnaryOp`,
`Array`, `Tuple`, `StructLiteral`, `Closure`.

**Places.** `AddressOf`, `Deref`, `Index`, `StringSlice`, `FieldAccess`, `Assign`,
`FieldAssign`, `IndexAssign`. A `&mut` parameter is a pointer in C, so reading through it is
`Deref` and taking one is `AddressOf` — the lowerer decides, once, and the backend does not
have to know which parameters are by-reference.

**Control.** `If`, `Block`, `Call`, `CallValue`, `Cast`, `SizeOf`.

**Metadata.** `Typed { expr, typ }` wraps an expression whose static type the lowerer knows.
Codegen needs it because some types cannot be recovered from their C representation — a `bool`
and an `int` are the same C type, so `print(x)` chooses between `nova_print_bool` and
`nova_print_int` by reading the annotation, not the value.

**Closures.** `Closure { name, env }` is a lifted function plus its heap environment;
`EnvAccess { env_type, field }` reads a captured variable out of it; `CallValue` calls through
it. A closure that captures nothing has `env: None`.

## What the SIR does not carry

- **No types beyond strings.** There is no type table, no resolved type identity, no
  interning. Two spellings of one type are two strings.
- **No source spans on expressions**, except the function's own `span`. A backend cannot
  report "line 12" for a bad expression, which is part of why the DWARF debugger is
  unfinished.
- **No borrow or lifetime information.** `&mut` survives as pointer-taking and pointer-reading,
  and nothing checks it.

## Backends

The SIR exists so that more than one backend can consume it, and four do:

| Backend | Where | Emits |
|---|---|---|
| C | `compiler/src/backend` | C11, compiled to a binary |
| JavaScript | `backends/javascript` | JavaScript |
| Rust | `backends/rust` | Rust |
| WASM | `backends/wasm` | WebAssembly |

**Only the C backend produces something this project runs.** Every test, every example and
every program in the standard library goes through it. The other three are asserted by
`every_backend_emits_for_the_same_program`, which checks that each accepts the same SIR and
returns output — **not** that the output runs, or is correct. Treat them as works in progress.

`SirProgram` carries `reachable_functions`, which walks from `main` so a backend can skip
imported-but-unused helpers — the JavaScript, Rust and WASM backends use it for exactly that.
Several fields (`FieldAccess::struct_type`, `FieldAssign::struct_type`) carry the declared
struct type so a backend that does not re-derive types can still find a field offset; the C
backend re-derives and ignores them.
