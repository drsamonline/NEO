#!/usr/bin/env python3
"""Numbers in the documents must match the things they count.

Five documents quoted the size of the test suite as 199, 316 and 193 while it was
342 — each number true on the day it was typed, and none of them true together. The
repository has fixed this by hand four times; this is the check that stops the fifth.

It reads `docs/status.json` for the measured value and fails when a document states a
different one. Only claims with a machine-checkable shape are checked:

    N `#[test]` functions        against status.json tests.functions
    N passing tests              against status.json tests.functions
    N of M chapters written      against docs/language/README.md
    N parity programs            against the files in tests/parity/

A number in a historical document is still checked: a superseded plan that says "199"
is wrong about the workspace, and the banner above it does not make the number true.

    python3 tools/count-check.py
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STATUS = ROOT / "docs" / "status.json"

# Documents that are allowed to state a number from their own past, because they are
# archives of a moment rather than descriptions of the workspace. Each is named here so
# the exemption is a decision rather than an oversight.
ARCHIVES = {"CHANGELOG.md"}


def measured() -> dict[str, int]:
    status = json.loads(STATUS.read_text())
    chapters = (ROOT / "docs" / "language" / "README.md").read_text()
    written = chapters.count("✅ written")
    total = len(re.findall(r"(?m)^\| \d\d \|", chapters))
    parity = len(list((ROOT / "tests" / "parity").glob("*.nova")))
    return {
        "tests": status["tests"]["functions"],
        "chapters_written": written,
        "chapters_total": total,
        "parity": parity,
    }


PATTERNS = [
    (re.compile(r"(\d+)\s+`#\[test\]`\s+functions"), "tests"),
    (re.compile(r"(\d+)\s+passing tests"), "tests"),
    (re.compile(r"(\d+)\s+of\s+(\d+)\s+chapters"), None),
    (re.compile(r"parity-(\d+)%20programs"), "parity"),
]


def main() -> int:
    values = measured()
    problems: list[str] = []

    for path in sorted(ROOT.rglob("*.md")):
        if "target" in path.parts or path.name in ARCHIVES:
            continue
        if "node_modules" in path.parts:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        for pattern, key in PATTERNS:
            for match in pattern.finditer(text):
                if key is None:
                    written, total = int(match.group(1)), int(match.group(2))
                    if written != values["chapters_written"] or total != values["chapters_total"]:
                        problems.append(
                            f"{path.relative_to(ROOT)}: says {written} of {total} chapters; "
                            f"{values['chapters_written']} of {values['chapters_total']} are written"
                        )
                else:
                    claimed = int(match.group(1))
                    if claimed != values[key]:
                        problems.append(
                            f"{path.relative_to(ROOT)}: says {claimed} {key}; measured {values[key]}"
                        )

    print(f"tests={values['tests']} chapters={values['chapters_written']}/{values['chapters_total']} parity={values['parity']}")
    if problems:
        print("FAIL a document states a number the workspace does not have:")
        for problem in problems:
            print(f"     {problem}")
        return 1
    print("every stated count matches the thing it counts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
