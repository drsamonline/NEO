#!/usr/bin/env python3
"""The files MANIFEST.json claims must still be the files on disk.

`tools/manifest.py check` reports a status per file but its exit code cannot be used
as a gate: a file that is recorded and correct comes back `protected` (exit 4), and so
does one that has drifted — the code says "this file is protected", not "this file is
what the manifest says". So this reads the statuses and fails only on drift:

    committed-drift   the file changed since it was recorded, and the change is committed
    uncommitted       the file changed since it was recorded, and the change is not
    unrecorded        the manifest does not know this file
    deleted           the manifest records a file that is gone

A repository with no MANIFEST.json is not checked, and says so — an absent manifest is
not a failure, it is a decision someone made.

    python3 tools/manifest-check.py
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "MANIFEST.json"
DRIFT = {"committed-drift", "uncommitted", "unrecorded", "deleted"}


def main() -> int:
    if not MANIFEST.is_file():
        print("no MANIFEST.json: nothing claims anything, so nothing can drift")
        return 0

    paths = sorted(json.loads(MANIFEST.read_text()).get("files", {}))
    if not paths:
        print("MANIFEST.json records no files")
        return 0

    result = subprocess.run(
        [sys.executable, str(ROOT / "tools" / "manifest.py"), "check", *paths],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )

    drifted: list[tuple[str, str]] = []
    unverified: list[str] = []
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        entry = json.loads(line)
        status = entry.get("status")
        if status in DRIFT:
            drifted.append((entry["path"], status))
        elif status == "protected":
            # `check_one` returns `protected` for a `hand` or `agent` file **without
            # comparing anything** — that is the design: those classes are owned by a
            # person, so a change is expected. It also means the manifest cannot say
            # whether the file is the one it recorded, and a gate that printed "none
            # drifted" for them would be a false assurance. It says so instead.
            unverified.append(entry["path"])

    if drifted:
        print("FAIL MANIFEST.json no longer describes these files:")
        for path, status in drifted:
            print(f"     {path}: {status}")
        print("     re-record with: python3 tools/manifest.py record --producer <you> --force <path>")
        return 1

    if unverified:
        print(
            f"{len(paths)} file(s) recorded; {len(unverified)} is `hand`/`agent` and "
            f"cannot be drift-checked by design"
        )
        for path in unverified:
            print(f"     {path}: protected — the manifest records who owns it, not what it contains")
    else:
        print(f"{len(paths)} file(s) recorded in MANIFEST.json, none drifted")
    return 0


if __name__ == "__main__":
    sys.exit(main())
