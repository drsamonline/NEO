# Package manager architecture

`nova-pkg` is `pkg/`. It has a manifest, a resolver and a CLI. It does not have a
working registry, and it no longer pretends to.

## What is real

- **`nova.toml`** — package name, version, dependencies. Parsed with `toml` and
  validated.
- **Dependency resolution** — version constraints resolved with `semver`, with
  errors that name the conflict.
- **`init` / `build` / `run`** — they work.

## What is not

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
a moderation story and a story for what happens when it goes away. The plan
(`DEV_PLAN.md` Stage 5) is git and path dependencies plus a lockfile, which is a
week of work and removes the asterisk entirely.

## The rule that follows

Do not describe this as a package manager without the qualifier. `DEV_PLAN.md`
carries it in the status table, `README.md` carries it, and this file exists so the
claim has somewhere to point.
