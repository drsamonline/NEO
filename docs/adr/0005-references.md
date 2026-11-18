# ADR 0005 — a borrow is a pointer, and borrowing a reference is identity

- **Status:** accepted
- **Date:** 2026-11-18
- **Related:** [0003](0003-option-payload.md), [0004](0004-deferred-global-initialisers.md)

## Context

Two placeholders sat in `compiler/src/lowerer/mod.rs` long enough to be mistaken for the
design:

```rust
fn lower_un_op(&self, op: &AstUnOp) -> SirUnOp {
    match op {
        AstUnOp::Neg => SirUnOp::Neg,
        AstUnOp::Not => SirUnOp::Not,
        AstUnOp::Deref => SirUnOp::Neg,   // Placeholder for Deref
        AstUnOp::AddrOf => SirUnOp::Not,  // Placeholder for AddrOf
    }
}
```

`&x` lowered to `!x` and `*x` to `-x`. `stdlib/io` emitted `fread_all(&((!file)))`.

The shape of the mistake is worth naming: `SirUnOp` has exactly two variants, `Neg` and
`Not`, because unary operators in C are arithmetic and logical. Address-of and
dereference are not operators on a value — they are constructors. The mapping had
nowhere correct to go, so a placeholder was written, and the placeholder outlived the
reason for it.

There was a second half. `expr_type_raw` had no arm for `Expr::Unary` at all, so a
borrow expression reported no type. `lower_call_args` decides whether to take an
argument's address by asking whether its type already starts with `&`:

```rust
let wants_reference = params.get(index).is_some_and(|typ| typ.trim_start().starts_with('&'));
let is_reference = self.expr_type(arg).is_some_and(|typ| typ.trim_start().starts_with('&'));
if wants_reference && !is_reference {
    SirExpr::AddressOf(Box::new(lowered))
}
```

With no type reported, `is_reference` was false for an explicit `&mut file`, and the
argument was wrapped a second time. Fixing only the placeholder would have produced
`&&file` — the trap this ADR exists to record.

## Decision

**1. `&` and `*` lower to `SirExpr::AddressOf` and `SirExpr::Deref`, and `lower_un_op`
is deleted.** The four `AstUnOp` variants are matched where the expression is lowered.

**2. A borrow reports `&mut T`.** Not because the language distinguishes `&` from
`&mut` — it does not: `parse_unary` consumes the `mut` token and drops it, so the AST
cannot tell them apart. `&mut T` is chosen because it is what a by-address parameter is
declared as, and the two must compare equal in `lower_call_args`.

**3. Borrowing something that is already a reference is that same pointer.** `&mut file`
where `file: &mut File` yields one `File*`, not a `File**`. NOVA has no `&&T`, and
inventing one to satisfy a formality would mean every by-address parameter needed two
spellings.

## What this changes for a program

| Source | Emitted C |
|---|---|
| `fread_all(&mut file)`, `file: File` | `fread_all(&(file))` — one address-of |
| `fread_all(file)` where the parameter is `&mut File` | `fread_all(&(file))` — the implicit wrap |
| `touch(&mut file)` where `file: &mut File` | `touch(file)` — the pointer already |
| `!x`, `-x` | unchanged |

## Consequences

- `AstUnOp::Deref` is **unreachable from parsed source** today — `parse_unary` handles
  `!`, `-` and `&` and there is no `*` prefix. It is lowered correctly anyway, so adding
  `*` later does not reintroduce a silent placeholder.
- `expr_type` now reports `&mut T` for a borrow, which also changes
  `lower_array_value` and `lower_array_handle`: they wrap in `Deref` when the type
  starts with `&`, so `(*&(arr))` where before they saw `!arr`. That is the correct
  branch; it was broken behaviour being repaired, not working behaviour changing.
- Mutability is **not** enforced. `&mut T` in the type string is spelling, not a
  capability: the language has no borrow checker, and this ADR does not add one.

## Where this is tested

`explicit_and_implicit_borrow_take_one_address` in `compiler/src/lowerer/mod.rs` — both
call shapes must end with exactly one `AddressOf` whose target is the variable. It fails
before this decision and passes after.
