---
name: Bug report
about: Something the compiler did that it should not have
title: ""
labels: bug
assignees: ""
---

<!--
Security issue? Do not file it here. See SECURITY.md.
A feature that is simply missing is not a bug — check DEV_PLAN.md
"Known limitations" first, then use the feature request form.
-->

## What happened

## What you expected

## Reproduce it

A four-line program that shows the problem is worth more than a description of a
large one.

```nova
fn main() -> int {
    print("Hello, NOVA");
    return 0;
}
```

## Command

```
./target/release/nova-compiler <verb> <file> [--target c|js|rust|wasm]
```

## Output

```
paste the diagnostics, including any `help:` lines and the `file:line:col`
```

## Environment

| | |
|---|---|
| NOVA version or commit | `nova-compiler --version` |
| OS and architecture | |
| Rust toolchain | `rustc --version` |
| C compiler | `cc --version` |
| `nova-compiler doctor` output | |
| Target (`c`/`js`/`rust`/`wasm`) | |

## Does it disagree between backends?

If it works on one target and not another, say which — that narrows it to a single
generator.

```
./target/release/nova-compiler test --parity yourfile.nova
```
