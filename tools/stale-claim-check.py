#!/usr/bin/env python3
"""Words that were true once and are not any more.

Three documents described the package registry, the LSP's `references` handler and the
debugger's stack frames as **mocked** — each had been made real in a pull request, and
none of those pull requests updated the sentence. `ARCHITECTURE.md` opened its "Current
Stage" with all three wrong, and `SECURITY.md` told a vulnerability reporter that two of
them were out of scope.

A claim about a capability is checkable when the capability has a name in the source:

    "mocked registry"       -> `pkg/src/registry.rs` must still contain `not implemented`
    "references missing"    -> `lsp/src/main.rs` must not define `async fn references`
    "stack frames mocked"   -> `dbg/src/session.rs` must not define `pub fn backtrace`

This checks those three, and the list is meant to grow: adding a pair is how a fixed
claim stays fixed.

    python3 tools/stale-claim-check.py
"""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (document phrase, the file that decides it, a marker in that file, what the code now does)
#
# The phrase is stale when the marker is **present**: `async fn references` in the language
# server means `references` exists, so a document calling it missing is wrong. For the
# registry the marker is `not implemented`, because a registry that refuses is not a
# registry that mocks.
CLAIMS = [
    (
        "the registry is mocked",
        ROOT / "pkg" / "src" / "registry.rs",
        "not implemented",
        "the registry refuses rather than mocking",
    ),
    (
        "`references` is missing",
        ROOT / "lsp" / "src" / "main.rs",
        "async fn references",
        "the language server answers `references`",
    ),
    (
        "the debugger's stack frames are mocked",
        ROOT / "dbg" / "src" / "session.rs",
        "pub fn backtrace",
        "the debugger walks a real frame chain",
    ),
]


def main() -> int:
    problems: list[str] = []
    for phrase, source, marker, truth in CLAIMS:
        if not source.is_file():
            problems.append(f"{source.relative_to(ROOT)} does not exist — is the claim list stale?")
            continue
        if marker not in source.read_text(errors="ignore"):
            continue  # the marker is gone, so the phrase may be right again
        for document in ROOT.rglob("*.md"):
            if "target" in document.parts or "node_modules" in document.parts:
                continue
            # Case-insensitive: a claim is a claim at the start of a sentence too. The first
            # version of this check missed `The registry is mocked.` because the pattern was
            # lower-case — a false pass on the exact sentence it was written for.
            if phrase.lower() in document.read_text(errors="ignore").lower():
                problems.append(
                    f"{document.relative_to(ROOT)} says \"{phrase}\", but {truth}"
                )

    if problems:
        print("FAIL a document describes a capability that changed:")
        for problem in problems:
            print(f"     {problem}")
        return 1
    print(f"{len(CLAIMS)} capability claim(s) checked against the source")
    return 0


if __name__ == "__main__":
    sys.exit(main())
