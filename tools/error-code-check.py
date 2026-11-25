#!/usr/bin/env python3
"""The error codes in the compiler, the binary and the docs must be the same set.

Three places name the codes a user can see:

- `compiler/src/sema/` attaches them with `.with_code("Exxxx")` — what the
  compiler actually emits;
- `ERROR_CODES` in `compiler/src/main.rs` — what `explain` can describe;
- `docs/errors.md` — what a reader is told exists;
- `COMPATIBILITY.md` — what a *program* is told it can rely on, which promised the same
  set and was missing `E0010` until 2026-11-26. A list in two places drifts; this makes
  the second place part of the check.

Any pair disagreeing is a bug: a code the compiler emits but `explain` cannot
describe, or a documented code that no longer fires. This checks all three, and it
reads the sources rather than a built binary so it runs anywhere.

    python3 tools/error-code-check.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CODE = r"E\d{3,5}"

EMITTED = re.compile(r'\.with_code\(\s*"(' + CODE + r')"')
DESCRIBED = re.compile(r'\(\s*"(' + CODE + r')"\s*,')
DOCUMENTED = re.compile(r"^##+\s+(" + CODE + r")\b", re.MULTILINE)
# `COMPATIBILITY.md` lists the codes inline — "`E0002`, `E0010`, ..." — because it is
# promising a set, not documenting one code per section. Every mention there is part of
# that promise, so the pattern is simply the shape of a code.
# Only the line that promises the set, not prose that mentions a code: a sentence like
# "`E0010` was missing from this list" must not count as promising `E0010`.
PROMISED = re.compile(r"(?m)^- \*\*Error codes\.\*\*(.*)$")


def scan(paths: list[Path], pattern: re.Pattern[str]) -> set[str]:
    found: set[str] = set()
    for path in paths:
        found |= set(pattern.findall(path.read_text(encoding="utf-8")))
    return found


def main() -> int:
    sema = sorted((ROOT / "compiler" / "src" / "sema").rglob("*.rs"))
    if not sema:
        sys.exit("compiler/src/sema has no sources — wrong directory?")

    main_rs = ROOT / "compiler" / "src" / "main.rs"
    docs = ROOT / "docs" / "errors.md"
    for required in (main_rs, docs):
        if not required.exists():
            sys.exit(f"missing {required.relative_to(ROOT)}")

    emitted = scan(sema, EMITTED)
    described = scan([main_rs], DESCRIBED)
    documented = scan([docs], DOCUMENTED)
    promise_text = (ROOT / "COMPATIBILITY.md").read_text()
    promised = set()
    for match in PROMISED.finditer(promise_text):
        promised |= set(re.findall(r"\b(" + CODE + r")\b", match.group(1)))

    print(f"emitted     ({len(emitted)}): {', '.join(sorted(emitted)) or '-'}")
    print(f"described   ({len(described)}): {', '.join(sorted(described)) or '-'}")
    print(f"documented  ({len(documented)}): {', '.join(sorted(documented)) or '-'}")
    print(f"promised    ({len(promised)}): {', '.join(sorted(promised)) or '-'}")

    problems = []
    if promised != emitted:
        problems.append(
            f"COMPATIBILITY.md promises {sorted(promised)}; the compiler emits {sorted(emitted)}"
        )
    for label, a, a_name, b, b_name in [
        ("emitted but not described", emitted - described, "emitted", described, "explain"),
        ("described but not emitted", described - emitted, "explain", emitted, "emitted"),
        ("emitted but not documented", emitted - documented, "emitted", documented, "docs/errors.md"),
        ("documented but not emitted", documented - emitted, "docs/errors.md", emitted, "emitted"),
    ]:
        for code in sorted(a):
            problems.append(f"{code}: {label} ({a_name} has it, {b_name} does not)")

    if problems:
        print()
        for problem in problems:
            print(f"FAIL {problem}")
        print(
            "\nEvery code must be attached with `.with_code`, described in "
            "`ERROR_CODES`, given a section in `docs/errors.md`, and promised in "
            "`COMPATIBILITY.md`'s error-codes line."
        )
        return 1

    print("\nall three agree")
    return 0


if __name__ == "__main__":
    sys.exit(main())
