#!/usr/bin/env python3
"""Nothing in this repository may be a documentation stub.

Four things go wrong with documentation and all four are silent:

- a file is created as a placeholder and never filled, so a reader who follows a
  link learns nothing and stops following links;
- a page is written and never linked, so it may as well not exist;
- a workflow or a script has no explanation, so the first person who finds it
  inconvenient deletes it;
- a document drifts away from the code it describes.

This checks the first three. The fourth is what `grammar-check.py`,
`error-code-check.py` and `extension-check.py` do for their own subjects.

    python3 tools/docs-check.py
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# An index page and a signpost are both legitimately short. Below this, a file is a
# placeholder — and every one that existed when this check was written has been
# filled.
MIN_BYTES = 120
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")

# Two front doors, because there are two: `README.md` is what a visitor lands on and
# `docs/README.md` is the index. A document reachable from either has been found by
# somebody.
ROOTS = ("README.md", "docs/README.md")

# GitHub renders these in its own UI and links them itself; they are configuration
# with prose in them, not documentation a reader browses to.
EXEMPT_PREFIXES = (".github/",)


def tracked_markdown() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "*.md"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout
    return [ROOT / line for line in out.splitlines() if line]


def links_from(path: Path) -> list[Path]:
    """Relative links in a document that point at another markdown file."""
    found = []
    for target in LINK.findall(path.read_text(encoding="utf-8")):
        target = target.split("#", 1)[0].strip()
        if not target or target.startswith(("http://", "https://", "mailto:")):
            continue
        resolved = (path.parent / target).resolve()
        if resolved.suffix == ".md" and resolved.is_file():
            found.append(resolved)
    return found


def reachable_from(start: Path) -> set[Path]:
    seen = {start}
    queue = [start]
    while queue:
        for link in links_from(queue.pop()):
            if link not in seen:
                seen.add(link)
                queue.append(link)
    return seen


def main() -> int:
    problems: list[str] = []
    roots = [ROOT / name for name in ROOTS]
    for root in roots:
        if not root.is_file():
            sys.exit(f"{root.relative_to(ROOT)} does not exist — it is a reachability root")

    markdown = tracked_markdown()
    if not markdown:
        sys.exit("git ls-files returned no markdown — wrong directory?")

    # 1. no stubs
    for path in markdown:
        size = path.stat().st_size
        if size < MIN_BYTES:
            problems.append(
                f"{path.relative_to(ROOT)} is {size} bytes — a placeholder, not a document"
            )

    # 2 and 3. everything is reachable from a front door
    reachable: set[Path] = set()
    for root in roots:
        reachable |= reachable_from(root)
    for path in markdown:
        relative = path.relative_to(ROOT).as_posix()
        if relative in ROOTS or relative.startswith(EXEMPT_PREFIXES):
            continue
        if path not in reachable:
            problems.append(
                f"{relative} is not reachable from README.md or docs/README.md — link it, or it is a page nobody finds"
            )

    # 4. every workflow says what it guards
    for workflow in sorted((ROOT / ".github" / "workflows").glob("*.yml")):
        header = []
        for line in workflow.read_text(encoding="utf-8").splitlines():
            if line.startswith("on:"):
                break
            header.append(line)
        if not any(line.lstrip().startswith("#") for line in header):
            problems.append(
                f".github/workflows/{workflow.name} has no comment saying what it guards"
            )

    # 5. every check says what it checks — in its own docstring, and in the page that
    #    explains the checks. A tool missing from `docs/tooling.md` is a check nobody can
    #    find, which is how six gates were added and the index still listed eight.
    tooling = (ROOT / "docs" / "tooling.md").read_text(encoding="utf-8")
    for tool in sorted((ROOT / "tools").glob("*.py")):
        text = tool.read_text(encoding="utf-8")
        if not re.match(r'(?:#![^\n]*\n|\s*\n)*?"""', text.split("from __future__")[0]):
            problems.append(f"tools/{tool.name} has no module docstring")
        if tool.name.endswith("-check.py") and f"tools/{tool.name}" not in tooling:
            problems.append(f"tools/{tool.name} is not explained in docs/tooling.md")

    print(f"{len(markdown)} document(s), {len(reachable)} reachable from a front door")

    if problems:
        print()
        for problem in problems:
            print(f"FAIL {problem}")
        print(
            "\nA placeholder should be filled or deleted, an orphan should be linked, "
            "and a check should say what it guards."
        )
        return 1

    print("\nevery document is written, reachable and explained")
    return 0


if __name__ == "__main__":
    sys.exit(main())
