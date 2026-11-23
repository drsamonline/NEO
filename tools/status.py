#!/usr/bin/env python3
"""Publish the repository's own numbers, so nothing else has to copy them by hand.

`docs/status.json` is the machine-readable answer to "what is NEO, right now". The
`drsamonline/skills` refresh reads it to keep the NEO skill's status current; a dashboard or
the fleet audit can read the same file. Copying these numbers into prose by hand is how the
skill came to claim v0.6.0 for a week after v0.7.0 shipped.

    python3 tools/status.py            # write docs/status.json
    python3 tools/status.py --print    # print it, change nothing
    python3 tools/status.py --check    # fail if the committed file is stale

**Only facts that are cheap and deterministic go in here**, so `--check` can gate CI without
a toolchain: the version, two line counts, the ADR count, the gate list, and what the stdlib
ratchet says. Test counts and coverage are deliberately absent — they need a full run and
they move on every commit, so a file that carried them could never be checked. They live in
`docs/HANDOVER.md`, which a person keeps current and which CI cannot verify.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STATUS = ROOT / "docs" / "status.json"

# Every member of the cargo workspace, so "workspace" means what it says.
WORKSPACE_MEMBERS = (
    "compiler",
    "crates",
    "backends",
    "runtime",
    "pkg",
    "lsp",
    "dbg",
    "integration_tests",
)


def version() -> str:
    """The released version, from the file the release workflow reads."""
    candidate = ROOT / "VERSION"
    if candidate.is_file():
        return candidate.read_text(encoding="utf-8").strip()
    manifest = ROOT / "Cargo.toml"
    match = re.search(r'^version\s*=\s*"([^"]+)"', manifest.read_text(encoding="utf-8"), re.M)
    return match.group(1) if match else "unknown"


def rust_lines(*directories: str) -> int:
    """Lines of Rust under the given directories, excluding build output."""
    total = 0
    for directory in directories:
        for path in (ROOT / directory).rglob("*.rs"):
            if "target" in path.parts:
                continue
            total += len(path.read_text(encoding="utf-8", errors="replace").splitlines())
    return total


def adr_count() -> int:
    """Numbered ADRs. `docs/adr/README.md` is the index, not a decision."""
    return len(list((ROOT / "docs" / "adr").glob("[0-9][0-9][0-9][0-9]-*.md")))


def gates() -> list[str]:
    """Every `tools/*-check.py`, which is what CI runs without a toolchain."""
    return sorted(
        path.stem.replace("-check", "")
        for path in (ROOT / "tools").glob("*-check.py")
    )


def stdlib() -> dict[str, int]:
    """Modules that exist, and how many the ratchet still lists as not compiling."""
    modules = sorted({path.parent.name for path in (ROOT / "stdlib").glob("*/*.nova")})
    listed = 0
    ratchet = ROOT / "stdlib" / "not-compiling.txt"
    if ratchet.is_file():
        for line in ratchet.read_text(encoding="utf-8").splitlines():
            if line.strip() and not line.lstrip().startswith("#"):
                listed += 1
    return {"modules": len(modules), "listed_as_not_compiling": listed}


def collect() -> dict:
    return {
        "version": version(),
        "loc": {
            "compiler_src": rust_lines("compiler/src"),
            "workspace": rust_lines(*WORKSPACE_MEMBERS),
        },
        "adrs": adr_count(),
        "gates": gates(),
        "stdlib": stdlib(),
    }


def render(status: dict) -> str:
    return json.dumps(status, indent=2) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--print", dest="print_only", action="store_true")
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args(argv)

    status = collect()
    rendered = render(status)

    if arguments.print_only:
        print(rendered, end="")
        return 0

    if arguments.check:
        if not STATUS.is_file():
            print("docs/status.json is missing — run python3 tools/status.py")
            return 1
        if STATUS.read_text(encoding="utf-8") != rendered:
            print(
                "docs/status.json is stale — run python3 tools/status.py and commit the result.\n"
                "It carries the version, the line counts, the ADR count, the gate list and the\n"
                "stdlib ratchet. Everything else about this repository is prose."
            )
            return 1
        print("docs/status.json matches the repository")
        return 0

    STATUS.write_text(rendered, encoding="utf-8")
    print(f"wrote {STATUS.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
