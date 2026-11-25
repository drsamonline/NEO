# Keywords

Twenty-five words are reserved. The list is not maintained by hand: it is read out of
[`crates/nova-common/src/token.rs`](../../crates/nova-common/src/token.rs) by
`tools/extension-check.py`, which fails the build if the editor's grammar highlights a
word the compiler does not reserve, or misses one it does.

## Declarations

| Keyword | Introduces | Grammar production |
|---|---|---|
| `fn` | a function | `function` |
| `let` | a binding | `let_stmt` |
| `mut` | mutability on a binding, a reference or `self` | part of `let_stmt`, `&mut` |
| `struct` | a struct type | `struct_def` |
| `enum` | an enum type | `enum_def` |
| `impl` | an inherent method block | `impl_block` |
| `const` | a constant | `const_def` |
| `static` | a mutable global | parsed as a `const_def` |
| `type` | **reserved, not parsed** — `type Meters = int;` is `Expected expression at 'type'` | `type_alias` |
| `use` | a module import | `use_decl` |
| `mod` | a module declaration | `module_decl` |
| `pub` | **reserved, not parsed** — `pub fn f()` is `Expected expression at 'pub'` | prefix on any item |

## Control flow

| Keyword | Meaning |
|---|---|
| `if` / `else` | conditional; both are expressions |
| `while` | loop |
| `for` … `in` | range loop: `for i in 0..10` |
| `loop` | **reserved, not parsed** — `loop { }` is `Expected expression at 'loop'`; use `while true` |
| `match` | pattern matching, arms are `pattern => expr` |
| `break` / `continue` | loop control |
| `return` | early return, with or without a value |
| `as` | cast, in an expression and in a `use` alias |
| `in` | the `for` range separator |

## Literals

| Keyword | Value |
|---|---|
| `true` / `false` | the two `bool` values |

## Reserved, and not accepted

Three of the words above are reserved by the lexer and listed in the grammar, and the parser
rejects them: **`type`**, **`pub`** and **`loop`**. So do the compound assignment operators the
grammar reserves (`+=`, `-=`, `*=`, `/=`, `^`, `~`) — `i += 1` is `Expected expression at '+='`.

Reserved-but-rejected is a deliberate state, not an oversight to be discovered: the words cannot be
used as identifiers, which means a program written today will not change meaning when they are
implemented. Every one of them was checked by compiling a program that uses it — 2026-11-25.

`static` and `const` are the opposite case: reserved *and* accepted. `static COUNTER: int = 0;`
compiles and prints `0`.

## Not a keyword

`trait` is an ordinary identifier. There is no trait support, and
[`docs/adr/0002-traits.md`](../adr/0002-traits.md) records why the AST no longer
pretends otherwise — `trait Show { }` produces "undefined name `trait`".

`int`, `float`, `string`, `bool` and `char` are **not** keywords either: they are
ordinary identifiers that the semantic analyser recognises in type position. So a
variable may be called `string`, and a type alias may shadow a primitive name.

## Where this is tested

`tools/extension-check.py` compares this list against `token.rs` in both directions.
`compiler/tests/item_tree.rs` compares the item tree's idea of a declaration keyword
against the parser's on every shipped program.
