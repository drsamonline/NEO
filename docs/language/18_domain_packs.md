# Domain Packs

**NOVA has no domain packs.** The term appears nowhere in the compiler, the runtime, the standard
library or the package tool:

```text
$ grep -rl "domain pack" --include=*.rs --include=*.nova --include=*.toml .
0 files
```

It is a title in this specification's chapter list and nothing else. This chapter says so in one
place, so that a reader who saw the title does not go looking for a manifest section, a `use` form,
or a package kind that would install one.

## What the term would have to mean

A domain pack is a bundle of code and conventions for one problem area — a set of types, functions
and defaults that a program in that domain starts from. NOVA already has the two things such a
bundle would be built out of:

| A domain pack would need | NOVA has |
|---|---|
| a way to ship code together | **packages** — `Nova.toml`, `path` and `git` dependencies ([Packages](17_packages.md)) |
| a way to bring names into scope | **modules** — `use name;`, with qualified calls when two modules collide ([Modules](08_modules.md)) |

So a domain pack today is **a package that happens to be about one domain**. Nothing distinguishes
it from any other package, and nothing needs to: a package of geometry helpers is already usable by
any program that depends on it.

## Why this chapter is not a design

What would make a domain pack more than a package is a decision nobody has taken: whether it can
contribute *syntax* (an operator, a literal form), whether it can constrain the type checker, and
whether the compiler knows the difference between a domain pack and a library. Each of those
changes the language, and the third one is the trap — a package the compiler treats specially is a
second kind of dependency with its own rules.

Until that is decided and recorded in `docs/adr/`, there are no domain packs. The honest statement
is the one at the top of this chapter: the word appears nowhere in the implementation.
