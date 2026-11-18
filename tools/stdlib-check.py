#!/usr/bin/env python3
"""How much of the standard library can actually run.

The stdlib declares about 250 functions. Many of them call `__builtin_*` functions,
and only a small set of those exists: the compiler knows fourteen, and a call to any
other name is a function the lowerer cannot emit and the C shim cannot provide.

Nothing recorded which was which, so the only way to find out was to write a program
and watch it fail. This derives the answer from the source and compares it against
the table in `stdlib/README.md`, so the table cannot drift.

**"Usable" is a weak claim and it is the strongest one available here.** It means the
function's body calls no builtin the compiler does not implement. It does not mean the
function is correct — the stdlib has no tests, which is a separate gap.

    python3 tools/stdlib-check.py            # compare
    python3 tools/stdlib-check.py --print    # print the table to paste into the README
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
README = ROOT / "stdlib" / "README.md"
BEGIN = "<!-- stdlib-check:begin -->"
END = "<!-- stdlib-check:end -->"
BUILTIN = re.compile(r"__builtin_[a-z_0-9]+")
FUNCTION = re.compile(r"(?m)^fn\s+([A-Za-z_][A-Za-z0-9_]*)")


def known_builtins() -> set[str]:
    """Builtins the compiler knows: those it names in quotes in sema or the lowerer."""
    names: set[str] = set()
    for directory in ("compiler/src/sema", "compiler/src/lowerer"):
        for path in (ROOT / directory).rglob("*.rs"):
            names |= set(re.findall(r'"(__builtin_[a-z_0-9]+)"', path.read_text()))
    return names


def survey(known: set[str]) -> tuple[dict[str, tuple[int, int, int]], dict[str, int]]:
    """Per module: (functions, usable, blocked). Plus how often each missing builtin appears."""
    per_module: dict[str, tuple[int, int, int]] = {}
    missing: dict[str, int] = {}

    for path in sorted((ROOT / "stdlib").glob("*/*.nova")):
        text = path.read_text()
        starts = [match.start() for match in FUNCTION.finditer(text)]
        names = [match.group(1) for match in FUNCTION.finditer(text)]

        usable = blocked = 0
        for index, _name in enumerate(names):
            end = starts[index + 1] if index + 1 < len(starts) else len(text)
            body = text[starts[index]:end]
            used = set(BUILTIN.findall(body))
            gaps = used - known
            for gap in gaps:
                missing[gap] = missing.get(gap, 0) + 1
            if gaps:
                blocked += 1
            else:
                usable += 1

        per_module[path.parent.name] = (len(names), usable, blocked)

    return per_module, missing


def table(per_module: dict[str, tuple[int, int, int]]) -> str:
    total = [0, 0, 0]
    lines = [
        "| Module | Functions | Usable | Blocked |",
        "|---|---|---|---|",
    ]
    for module, (functions, usable, blocked) in per_module.items():
        lines.append(f"| `{module}` | {functions} | {usable} | {blocked} |")
        total[0] += functions
        total[1] += usable
        total[2] += blocked
    lines.append(f"| **total** | **{total[0]}** | **{total[1]}** | **{total[2]}** |")
    return "\n".join(lines)


def main() -> int:
    known = known_builtins()
    per_module, missing = survey(known)
    generated = table(per_module)

    if "--print" in sys.argv:
        print(generated)
        return 0

    if not README.is_file():
        sys.exit("stdlib/README.md does not exist")
    text = README.read_text()
    if BEGIN not in text or END not in text:
        sys.exit(f"stdlib/README.md has no `{BEGIN}` … `{END}` block to compare against")

    documented = text.split(BEGIN, 1)[1].split(END, 1)[0].strip()
    print(f"known builtins: {len(known)}")
    print(generated)

    if documented != generated:
        print()
        print("FAIL the table in stdlib/README.md does not match the source")
        print("     regenerate it with: python3 tools/stdlib-check.py --print")
        return 1

    print(f"\nthe table matches: {sum(v[2] for v in per_module.values())} function(s) call a")
    print("builtin the compiler does not implement, and the README says so")
    if missing:
        worst = sorted(missing.items(), key=lambda item: -item[1])[:5]
        print("most-called missing builtins: " + ", ".join(f"{n} ({c})" for n, c in worst))
    return 0


if __name__ == "__main__":
    sys.exit(main())
