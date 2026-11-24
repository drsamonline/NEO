#!/usr/bin/env python3
"""The paths and line counts in `ARCHITECTURE.md` are checked against the repository.

Constitution rule 4: **numbers come from a command**. `ARCHITECTURE.md` is the map a new
contributor reads to learn what the compiler is made of, and every section carries a line count
as a proxy for how much work lives there. Those numbers were typed by hand and nothing measured
them, so they drifted — the Lowerer said 3,190 lines when it held 4,940, an error of 55%, in the
one number a reader uses to judge where the complexity is. And the SIR section pointed at
`compiler/src/sir/mod.rs`, which does not exist; the SIR moved to `crates/nova-common` and the
document did not.

    python3 tools/architecture-check.py           # check
    python3 tools/architecture-check.py --fix     # rewrite the numbers from the files

**What is checked**: every `(N lines)` that can be tied to a path — a `### N. Name (X lines)`
heading followed by a `**Location**:` line, and a `- **file.rs** (X lines)` bullet resolved
inside the section's directory. A `**Location**` that does not resolve is **reported, not
skipped**: skipping it is how a dead path survived here.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC = ROOT / "ARCHITECTURE.md"

SECTION = re.compile(r"^### \d+\.\s+(?P<name>.+?)\s+\((?P<count>[\d,]+) lines\)", re.M)
LOCATION = re.compile(r"\*\*Location\*\*:\s*`(?P<path>[^`]+)`")
BULLET = re.compile(
    r"^- \*\*(?P<file>[A-Za-z_][A-Za-z0-9_./]*\.rs)\*\*\s+\((?P<count>[\d,]+) lines\)", re.M
)


def count_lines(path: Path) -> int | None:
    """Source lines in `path`; a directory counts the `.rs` files under it."""
    if path.is_file():
        return len(path.read_text().splitlines())
    if path.is_dir():
        files = sorted(path.rglob("*.rs"))
        if not files:
            return None
        return sum(len(f.read_text().splitlines()) for f in files)
    return None


def claims(text: str) -> list[tuple[str, Path, int, str]]:
    """(description, path, claimed_count, the literal text to rewrite) for each claim.

    A missing path carries a claimed count of 0 and is reported by `main`.
    """
    found: list[tuple[str, Path, int, str]] = []
    sections = list(SECTION.finditer(text))
    for index, match in enumerate(sections):
        end = sections[index + 1].start() if index + 1 < len(sections) else len(text)
        body = text[match.end() : end]
        location = LOCATION.search(body)
        if location is None:
            continue
        path = ROOT / location.group("path").strip()
        if not path.exists():
            # A `**Location**` that does not resolve is a signpost to a building that is not
            # there — the same defect as an empty module. Reported, never skipped.
            found.append((match.group("name").strip(), path, 0, match.group(0)))
            continue
        found.append(
            (
                match.group("name").strip(),
                path,
                int(match.group("count").replace(",", "")),
                match.group(0),
            )
        )
        for bullet in BULLET.finditer(body):
            base = path if path.is_dir() else path.parent
            found.append(
                (
                    f"{match.group('name').strip()} / {bullet.group('file')}",
                    base / bullet.group("file"),
                    int(bullet.group("count").replace(",", "")),
                    bullet.group(0),
                )
            )
    return found


def main(argv: list[str]) -> int:
    text = DOC.read_text()
    checked = claims(text)
    if not checked:
        print("no checkable claims in ARCHITECTURE.md")
        return 0

    stale: list[str] = []
    fixed = text
    for description, path, claimed, literal in checked:
        actual = count_lines(path)
        if actual is None:
            stale.append(f"  {description}: names {path.relative_to(ROOT)}, which does not exist")
            continue
        if actual == claimed:
            continue
        stale.append(f"  {description}: says {claimed:,}, is {actual:,} ({path.relative_to(ROOT)})")
        if "--fix" in argv:
            for spelling in (f"{claimed:,} lines", f"{claimed} lines"):
                if spelling in literal:
                    fixed = fixed.replace(
                        literal, literal.replace(spelling, f"{actual:,} lines"), 1
                    )
                    break

    if "--fix" in argv and stale:
        DOC.write_text(fixed)
        print(f"rewrote {len(stale)} claim(s) in ARCHITECTURE.md")
        return 0

    if stale:
        print(f"{len(stale)} stale claim(s) in ARCHITECTURE.md:\n")
        for problem in stale:
            print(problem)
        print(
            f"\n{len(checked)} claim(s) checked."
            "\nRun `python3 tools/architecture-check.py --fix` to take the numbers from the files."
        )
        return 1

    print(f"ARCHITECTURE.md matches the repository ({len(checked)} claim(s) checked)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
