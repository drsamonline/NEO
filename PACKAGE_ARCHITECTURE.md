# Package manager architecture

`nova-pkg` is `pkg/`. It has a manifest, a resolver and a CLI. It does not have a
working registry, and it no longer pretends to.

## What is real

- **`nova.toml`** — package name, version, dependencies. Parsed with `toml` and
  validated.
- **Path and git dependencies, end to end.** `nova build` resolves each dependency
  to a directory and passes it to the compiler as `--module-root`, so `use utils;`
  finds the dependency's source. A path dependency is used where it is; a git
  dependency is cloned into `.nova/deps/<name>` with `git clone --depth 1`, then
  checked out at `rev`/`tag`/`branch` if one is named.
- **`init` / `build` / `run`** — they work.

```toml
[dependencies]
utils = { path = "../utils" }
math  = { git = "https://github.com/you/nova-math", tag = "v0.2.0" }
```

The compiler knows nothing about manifests or networks: `nova-pkg` reads `nova.toml`,
materialises each dependency, and hands over one `--module-root` per dependency. A
module root is searched **before** `stdlib`, so a project can shadow a stdlib module
with its own.

## What is not

- **A version-only dependency.** `utils = "1.0"` cannot be fetched, because there is
  no registry. `nova build` says so, names both alternatives, and stops — it does not
  record the dependency and quietly ignore it.
- **A dependency's own dependencies.** Only the project's direct dependencies are
  resolved. There is no transitive graph and no version solving across one.
- **A lockfile.** Nothing pins a git dependency to a commit yet, so two machines can
  resolve the same `nova.toml` to different revisions. That is the next piece.
- **The registry.** `search` and `get_latest_version` return mock data from a
  hard-coded list. `download` and `verify_token` return an explicit error:

  ```
  cannot download foo@1.0.0: the NOVA package registry is not implemented.
  Use a path dependency in `nova.toml` instead
  ```

  They used to build a URL under `https://registry.nova-lang.org` and call it. That
  host does not exist, so the only thing an HTTP client ever produced there was a
  connection error — while dragging in roughly a hundred crates and the only
  non-permissive licence in the tree (`webpki-roots`, MPL-2.0). The dependency is
  gone and the error is honest.
- **Building dependencies.** Dependencies are recorded and not compiled. A project
  with a dependency does not build it.

## Why there is no registry client

A registry is a product decision, not a compiler feature: it needs hosting, signing,
a moderation story and a story for what happens when it goes away. Git and path
dependencies cover the cases a pre-release language actually has, and they needed no
server at all.

## Why fetching shells out to `git`

Because the alternative is an HTTP client, and this crate used to have one: it served
the mock registry, reached a host that does not exist, and dragged in the only
non-permissive licence in the tree. `git` is already installed wherever there is
source to fetch, it honours the user's own credentials and configuration, and it
means the package manager still contains no network code.

## The rule that follows

Do not describe this as a package manager without the qualifier. `DEV_PLAN.md`
carries it in the status table, `README.md` carries it, and this file exists so the
claim has somewhere to point.
