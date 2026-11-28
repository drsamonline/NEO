#!/usr/bin/env python3
"""Every construct a backend refuses is listed in `COMPATIBILITY.md`.

The refusals live in the backends as `Err("...")` strings, and a user meets them by hitting one. The
document that says what works where did not mention any of them: the only list was a `//!` comment in
`backends/wasm/src/lib.rs`, which is invisible unless you are reading that file.

This check closes that gap the way the others here do — by making the second place machine-checked
rather than remembered. It extracts every literal passed to `Err(` in a backend's source, normalises
the whitespace (these literals continue across lines with a trailing backslash), and requires each to
appear in `COMPATIBILITY.md`.

What it cannot check, and says so: a row in the document for a construct no backend refuses. That
would be a claim about a limit that does not exist, and the table is prose — a human reading it
against the source is the only way to catch it.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOC = ROOT / "COMPATIBILITY.md"
BACKENDS = sorted((ROOT / "backends").glob("*/src/lib.rs"))

# `Err("...")`, honouring a backslash-newline continuation: `\\.` alone does not match one, and
# several of these literals are written across lines.
ERR_LITERAL = re.compile(r'Err\(\s*"((?:[^"\\]|\\[\s\S])*)"')


def normalise(text: str) -> str:
    text = re.sub(r"\\\s*\n\s*", "", text)
    text = text.replace('\\"', '"').replace("\\\\", "\\")
    return " ".join(text.split())


def refusals(source: str) -> list[str]:
    return [normalise(m.group(1)) for m in ERR_LITERAL.finditer(source)]


def main() -> int:
    if not DOC.is_file():
        print("FAIL COMPATIBILITY.md is missing")
        return 1
    document = " ".join(DOC.read_text().split())

    missing: list[str] = []
    checked = 0
    for path in BACKENDS:
        backend = path.parts[-3]
        for refusal in refusals(path.read_text()):
            checked += 1
            if refusal not in document:
                missing.append(f"{backend}: {refusal!r}")

    if missing:
        print("FAIL a backend refuses something COMPATIBILITY.md does not mention:")
        for entry in missing:
            print(f"     {entry}")
        print()
        print("     A refusal a user cannot look up is one they meet as a surprise. Add it to the")
        print('     "What each backend refuses" table, with the message it produces.')
        return 1

    print(f"{checked} refusal(s) across {len(BACKENDS)} backend(s) are documented in COMPATIBILITY.md")
    print("every construct a backend refuses is one a user can look up before hitting it")
    return 0


if __name__ == "__main__":
    sys.exit(main())
