#!/usr/bin/env python3
"""A module that is declared and empty is a signpost to nowhere.

`compiler/src/semantic/mod.rs` contained one line — `//! Semantic analyzer scaffold` — and was
exported as `pub mod semantic;` next to the real analyser, `sema`. A reader looking for the
semantic analyser finds a module that says it is one and holds nothing, and the two names are
close enough that the mistake is easy to make twice.

The same defect is already recorded in `integration_tests/lib.rs`, from the other direction:
those tests imported `nova_compiler::frontend::semantic_analyzer` and
`nova_compiler::middle::sir`, "and none of those modules exist". A module that exists and is
empty is worse, because the import compiles.

    python3 tools/module-check.py

**What counts as empty**: a module file whose body is only comments and blank lines. A module
with one `pub use` is not empty; a module with a `//!` header and nothing else is.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Every crate root that declares modules. `lib.rs` and `main.rs` are the two places a module
# tree starts; a crate with both declares in both.
CRATE_ROOTS = sorted(
    [*(ROOT / "compiler" / "src").glob("lib.rs"), *(ROOT / "compiler" / "src").glob("main.rs")]
    + [*(ROOT / "crates").glob("*/src/lib.rs")]
    + [*(ROOT / "backends").glob("*/src/lib.rs")]
    + [*(ROOT / "lsp" / "src").glob("main.rs"), *(ROOT / "pkg" / "src").glob("main.rs")]
    + [*(ROOT / "dbg" / "src").glob("main.rs"), *(ROOT / "runtime" / "src").glob("lib.rs")]
)

MODULE_DECL = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;", re.M)

# A line that is an item rather than a comment or whitespace.
ITEM = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:fn|struct|enum|trait|impl|use|const|static|type|mod|macro_rules!|extern)\b",
    re.M,
)


def module_path(crate_root: Path, name: str) -> Path | None:
    """Where `mod name;` resolves to, in the 2018 layout."""
    base = crate_root.parent
    for candidate in (base / f"{name}.rs", base / name / "mod.rs"):
        if candidate.exists():
            return candidate
    return None


def is_empty_module(path: Path) -> bool:
    text = path.read_text()
    return ITEM.search(text) is None


def main(argv: list[str]) -> int:
    problems: list[str] = []
    checked = 0
    for crate_root in CRATE_ROOTS:
        text = crate_root.read_text()
        for match in MODULE_DECL.finditer(text):
            name = match.group(1)
            path = module_path(crate_root, name)
            if path is None:
                # A missing file is a compile error, not this gate's business.
                continue
            checked += 1
            if is_empty_module(path):
                line = text[: match.start()].count("\n") + 1
                problems.append(
                    f"{crate_root.relative_to(ROOT)}:{line}: `mod {name};` resolves to "
                    f"{path.relative_to(ROOT)}, which declares nothing — a module that is "
                    f"declared and empty reads as a module that exists"
                )

    if problems:
        print(f"{len(problems)} empty module(s):\n")
        for problem in problems:
            print(f"  {problem}")
        print(
            "\nEither delete the module and its declaration, or give it the items its name "
            "promises. `compiler/src/semantic/` was one line of prose beside a working "
            "analyser called `sema`."
        )
        return 1

    print(f"every declared module has items ({checked} checked)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
