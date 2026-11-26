# Packages

A **package** is a directory with a `Nova.toml` in it. `nova-pkg` creates one, builds it, and
resolves the dependencies that are written in that file. There is no registry, so a package can
depend on **a path** or **a git URL** — and a bare version number is refused rather than guessed at.

This chapter describes what the tool does today. Every claim here was run.

## Creating one

```text
$ nova-pkg init app
$ nova-pkg init lib --lib
```

| | An application | A library |
|---|---|---|
| entry | `src/main.nova` | `src/<package>.nova` |
| `main` | required | absent |
| built to | `build/debug/<package>` | emitted as C, not linked |

Both write `Nova.toml`, a `.gitignore` and a `README.md`.

**A library's entry is named after the package, not `lib.nova`**, because that is the file
`use <package>;` resolves to: the loader looks for `<root>/src/<name>.nova` in each module root. A
library called `lib.nova` could not be imported by the name of its own package, which is exactly
what it is for.

## The manifest

```toml
[package]
name = "app"
version = "0.1.0"
description = "A NOVA application"
authors = []
nova-version = ">=0.3.0"

[build]
entry = "src/main.nova"
debug = true
features = []

[dependencies]
lib = { path = "../lib" }

[dev-dependencies]

[target]
```

`[build] entry` is the file compiled. `[dependencies]` and `[dev-dependencies]` are maps of name to
specification.

## Importing a dependency

A dependency is importable by its **package name**:

```nova
use core;
use lib;          // the package in [dependencies]

fn main() -> int {
    print(double(21));   // 42
    return 0;
}
```

This one is a fragment of a package: `double` is declared by the `lib` dependency, so it compiles
only inside a project whose `Nova.toml` names that dependency. Every other program in this chapter
compiles on its own.

`nova-pkg` passes each resolved dependency's directory to the compiler as `--module-root`, and the
loader searches it for `src/<name>.nova` before it searches the standard library — so a project can
shadow a stdlib module with its own, as every other package manager allows.

## The three kinds of dependency

| Written as | Resolved? |
|---|---|
| `lib = { path = "../lib" }` | **yes** — the directory is used directly |
| `lib = { git = "https://…" }` | **yes** — cloned with `git`, into `.nova/deps/` |
| `lib = "1.0"` | **no** — refused: there is no registry to fetch it from |

The third one is refused with the fix in the message:

```text
cannot resolve `somepkg = "*"`: the NOVA registry is not implemented, so a version-only
dependency cannot be fetched.
Use a path or a git dependency instead:
    somepkg = { path = "../somepkg" }
```

`nova-pkg add somepkg` writes nothing when this happens: it resolves **before** it saves, so a
failed add leaves the manifest exactly as it was.

## What the registry commands do

They refuse, and say why:

| Command | Answer |
|---|---|
| `nova-pkg search <q>` | `the NOVA registry is not implemented: search cannot answer` |
| `nova-pkg info <name>` | `get_latest_version cannot answer` |
| `nova-pkg publish` | `publish cannot upload` |

A refusal is the point. Before 2026-11-25 the resolver fell through to a *placeholder* manifest —
version `1.0.0`, authors `["Unknown"]` — and `add` printed `✓ Resolved 1 dependencies` for a package
that does not exist, while `build` refused the same dependency. The two commands disagreed, and the
one that ran first lied.

## Building and running

```text
$ nova-pkg build          # build/debug/<package>
$ nova-pkg run            # build, then run it
$ nova-pkg build --release
```

`nova-pkg` shells out to `nova-compiler`, found on `PATH` or named by `NOVA_COMPILER`. It is not
linked in, so a compiler built from a different checkout can be used by setting the variable.

## What is missing

Stated here rather than implied by silence:

- **No registry.** No version can be fetched; `search`, `info` and `publish` cannot answer.
- **No lockfile.** A `path` dependency is used as it is; a `git` one is cloned at the branch or tag
  named, and nothing records the commit that was used.
- **No version solving across the graph.** A dependency's own dependencies are not fetched, so a
  package whose dependency has a dependency will fail at the import.
- **No visibility or re-export.** Everything a dependency declares is merged into one flat
  namespace; a name two packages both declare must be written `package.name` (see
  [Modules](08_modules.md)).

Each is a decision or a piece of work, not a bug: the tool refuses where it cannot act, which is
what makes the rest of it trustworthy.
