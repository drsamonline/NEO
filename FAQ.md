# FAQ

### Is this production ready?

No. It is pre-release: Phase 1 complete, Phase 3 partial, Phase 4 in progress
(`DEV_PLAN.md`). What that means concretely is in "Known limitations" — read that
before filing a bug, because several things that look broken are deliberate.

### Why compile to C, JavaScript, Rust and WebAssembly instead of machine code?

Because each of those already has a mature compiler and runtime. NOVA's job is to
decide what a program means; LLVM's or `rustc`'s job is to make it fast on a
particular chip. C is the reference target and the only one `build`/`run` can link.

The cost is real: four backends is four times the surface to keep honest, and the
project is honest that three of them run a subset. `test --parity` exists because of
that cost.

### Why does `print` work without importing anything?

It is a compiler builtin (`__builtin_print`), lowered to the runtime shim. The
`core` module wraps it in a generic `print<T>` for source that prefers a function.

### Is there a formatter?

Yes, and it is deliberately narrow:

```bash
nova-compiler fmt file.nova          # rewrite in place
nova-compiler fmt file.nova -o out   # or write elsewhere
nova-compiler fmt file.nova --check  # exit 1 if it is not formatted
```

It normalises whitespace — one blank line between top-level items, no trailing
whitespace, one newline at end of file — and **changes no token**. The output lexes
to exactly the same tokens, with the same text, as the input.

That is the promise a formatter can keep. "Not one byte changes" cannot be true,
because removing trailing whitespace changes bytes; "not one token changes" can be
true, and is what matters: the program is the same program.

It **does** re-indent every line of code to its bracket depth, four spaces per
level, from the token stream — so it needs no parse and cannot disagree with the
parser about anything.

It does **not** break a long line, align arguments or reflow an expression. Those need
to know what the expression *means*, which needs a lossless tree over every
expression — `docs/adr/0001` has the reasoning.

Two things it leaves exactly as they are: a line inside a string literal, where the
leading spaces are content, and a comment-only line, so an indented example inside a
doc comment survives. The cost of the second is that a misplaced comment keeps its
own indentation.

Because it never parses, it is total: it works on a file that is half-typed, which
is when you most want to run it. `compiler/tests/format.rs` checks against every
shipped program that the token stream is identical, no comment is lost, the item
structure is unchanged, and formatting twice changes nothing.

### Why do two identifiers with the same name resolve to each other?

Go-to-definition and find-references work on tokens, so they match text rather than
scopes. It is exact about *where* and honest that it is not a resolver. Fixing it
needs a scope map over a parsed body.

### Why does the compiler print error codes now, and why only three?

`E0002`, `E0061`, `E0308`. The CLI used to drop the code and print only the message,
so a code existed everywhere except in front of the person who needed it. Three is
what the analyser actually has; `docs/errors.md` documents those and nothing else,
because a page listing codes the compiler does not emit teaches readers to distrust
it.

### Is my source sent anywhere?

No. The compiler and language server are local processes with no network code in
them at all. The package manager is the only part that ever had an HTTP client, and
it was removed when the licence gate found what it dragged in.

### Can I use Hindi or Gujarati in my source?

Yes, including in comments, identifiers and string literals — and it is tested:
`tests/parity/unicode_string.nova` prints `नमस्ते` and `કેમ છો` and compares the
output byte-for-byte across all four backends. It did not work until 2026-11-18; the
lexer was slicing a `String` with character offsets.

### How do I know the claims in this repository are true?

Every number is produced by a command or a CI run, and eleven workflows check the
claims rather than trusting them: sanitisers on generated C, WebAssembly validation,
diagnostic goldens, a generated differential corpus, fuzzing, a coverage ratchet,
mutation testing, a licence policy, extension packaging. `docs/tooling.md` lists
them and what each one would catch.
