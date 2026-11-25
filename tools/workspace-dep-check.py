#!/usr/bin/env python3
"""Every dependency declared for the workspace must be used by a member.

`[workspace.dependencies]` is a list of versions to share, not a list of things the
workspace needs. An entry nothing uses is a claim about the project that is not true —
`nova-runtime` sat there while no crate depended on it, which reads as "NOVA has a Rust
runtime" to anyone opening `Cargo.toml`. `cargo` warns about it; a warning is not a gate.

    python3 tools/workspace-dep-check.py
"""
from __future__ import annotations

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def main() -> int:
    manifest = ROOT / "Cargo.toml"
    workspace = tomllib.loads(manifest.read_text()).get("workspace", {})
    declared = workspace.get("dependencies", {})
    if not declared:
        print("no [workspace.dependencies]: nothing to check")
        return 0

    # Cargo's rule, not a looser one: an entry is used only when a member opts into it with
    # `name = { workspace = true }`. A member that declares the same crate by version or by path
    # does not use the entry — it duplicates it — and cargo warns about exactly that. Counting a
    # name match would make this gate pass while `cargo build` printed six warnings, which is the
    # false assurance this file exists to avoid.
    used: set[str] = set()
    for member in workspace.get("members", []):
        member_manifest = ROOT / member / "Cargo.toml"
        if not member_manifest.is_file():
            print(f"FAIL workspace member `{member}` has no Cargo.toml")
            return 1
        parsed = tomllib.loads(member_manifest.read_text())
        for table in TABLES:
            for name, spec in parsed.get(table, {}).items():
                if isinstance(spec, dict) and spec.get("workspace") is True:
                    used.add(name)

    unused = sorted(name for name in declared if name not in used)
    print(f"{len(declared)} workspace dependencies, {len(used)} names used by a member")
    if unused:
        print("FAIL declared for the workspace, used by no member:")
        for name in unused:
            print(f"     {name}")
        print("     remove the entry, or make a member depend on it")
        return 1
    print("every workspace dependency is used")
    return 0


if __name__ == "__main__":
    sys.exit(main())
