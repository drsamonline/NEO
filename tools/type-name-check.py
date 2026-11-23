#!/usr/bin/env python3
"""Two idioms that have cost this repository ten defects. Fail if either returns.

**1. A type read through `.name` instead of `type_display`.**

`TypeExpr` stores `Option<int>` as name `Option` plus a `generics` list. Reading `.name`
gives `Option`, and `c_type` maps that to `int` — so the argument disappears without a
diagnostic. That single mistake produced:

    resolve_generic_call        a generic call's reported return type
    lower_instance              a generic instantiation's declared return type
    expr_type's call arm        `let a = find(..)` inferred `int`
    lower_method                a method returning `Option<T>` declared `int`
    struct fields               `Holder { maybe: Option<int> }` declared `int`
    local `const`               `const LIMIT: Option<int>` declared `int`
    sema, seven sites           every parameter, return, field and cast type

**2. A helper that inspects a `SirExpr` without unwrapping `Typed`.**

The lowerer annotates expressions with their type, so `range(0, 4)` arrives as
`Typed { expr: Call { callee: "range" } }`. A helper matching the call directly never
matches, and `for i in range(..)` emitted an empty body — an infinite loop, in a module
that compiled and passed every gate.

    python3 tools/type-name-check.py          # check
    python3 tools/type-name-check.py --list   # print what is checked

**This is a heuristic, not a type checker.** It matches the shapes the bug has actually
taken here, so it catches the tenth occurrence and not every conceivable one. A compiler
warning would be better and this repository has no lint infrastructure; the alternative to
a heuristic gate is no gate, and ten defects say no gate is not enough.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCES = ROOT / "compiler" / "src"

# (pattern, why). Each is a shape the defect has actually taken.
TYPE_NAME_PATTERNS = [
    (re.compile(r"type_from_name\(&\s*([A-Za-z_][A-Za-z0-9_]*)\.name\s*\)"),
     "type_from_name(&x.name) drops the generic arguments — use type_display(x)"),
    (re.compile(r"substitute_type_name\(&\s*([A-Za-z_][A-Za-z0-9_]*)\.name\b"),
     "substitute_type_name(&x.name) drops the arguments — use type_display(x)"),
    (re.compile(r"\b(?:param|field|const_def)\.typ\.name\b"),
     "x.typ.name drops the arguments — use type_display(&x.typ)"),
]

# `.name` is legitimate inside the function that renders a type in full.
TYPE_DISPLAY_FN = re.compile(r"fn type_display\(")

# A function whose signature inspects a SirExpr must unwrap `Typed`.
SIR_EXPR_SIGNATURE = re.compile(r"fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\([^)]*&SirExpr")
SIR_EXPR_MATCH = re.compile(r"SirExpr::[A-Z][A-Za-z]*")
TYPED_ARM = re.compile(r"SirExpr::Typed")


def strip_doc_comments(text: str) -> str:
    """Blank out `///` lines so examples in documentation are not reported."""
    return "\n".join(
        "" if line.lstrip().startswith("///") else line for line in text.splitlines()
    )


def find_function_body(text: str, start: int) -> tuple[int, int] | None:
    """The (start, end) of a function body beginning at `start`, by brace counting."""
    open_at = text.find("{", start)
    if open_at == -1:
        return None
    depth = 0
    for index in range(open_at, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return (open_at, index + 1)
    return None


def check_type_names(path: Path, text: str) -> list[str]:
    problems: list[str] = []
    lines = text.splitlines()

    # The span of `type_display` itself, where `.name` is the whole point.
    exempt: set[int] = set()
    for match in TYPE_DISPLAY_FN.finditer(text):
        body = find_function_body(text, match.start())
        if body:
            start_line = text[: body[0]].count("\n")
            end_line = text[: body[1]].count("\n")
            exempt.update(range(start_line, end_line + 1))

    stripped = strip_doc_comments(text).splitlines()
    for number, line in enumerate(stripped, start=1):
        if (number - 1) in exempt:
            continue
        for pattern, why in TYPE_NAME_PATTERNS:
            if pattern.search(line):
                problems.append(
                    f"{path.relative_to(ROOT)}:{number}: {why}\n    {lines[number - 1].strip()}"
                )
    return problems


def check_typed_unwrap(path: Path, text: str) -> list[str]:
    problems: list[str] = []
    for match in SIR_EXPR_SIGNATURE.finditer(text):
        body = find_function_body(text, match.start())
        if body is None:
            continue
        body_text = text[body[0] : body[1]]
        # A helper that only calls `expr_to_c` is not inspecting anything.
        if not SIR_EXPR_MATCH.search(body_text):
            continue
        if TYPED_ARM.search(body_text):
            continue
        line = text[: match.start()].count("\n") + 1
        problems.append(
            f"{path.relative_to(ROOT)}:{line}: `{match.group(1)}` inspects a SirExpr "
            f"without unwrapping `Typed` — the annotation hides the shape"
        )
    return problems


def main(argv: list[str]) -> int:
    if "--list" in argv:
        print("A type must be rendered with `type_display`, never `.name`:")
        for _, why in TYPE_NAME_PATTERNS:
            print(f"  - {why}")
        print("A function that inspects a `SirExpr` must unwrap `SirExpr::Typed` first.")
        return 0

    problems: list[str] = []
    files = sorted(SOURCES.rglob("*.rs"))
    for path in files:
        text = path.read_text()
        problems.extend(check_type_names(path, text))
        problems.extend(check_typed_unwrap(path, text))

    if problems:
        print(f"{len(problems)} type-information hazard(s):\n")
        for problem in problems:
            print(f"  {problem}")
        print(
            "\nBoth idioms are recorded in docs/HANDOVER.md under 'The audit those rows "
            "earned'. `type_display` keeps a type's arguments; `Typed` carries the type "
            "the lowerer already worked out."
        )
        return 1

    print(f"no type-information hazards in {len(files)} file(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
