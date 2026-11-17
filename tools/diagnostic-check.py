#!/usr/bin/env python3
"""Golden-file checks for the compiler's diagnostics.

A diagnostic is part of the compiler's interface. Its code, its message, the
span it points at and the `help:` line under it are what a user acts on, and
nothing pinned any of them: the message could be reworded, the span could drift
to the wrong line, or a diagnostic could disappear entirely, and every test in
the repository would still pass.

This runs each case under `tests/diagnostics/`, compares the compiler's stderr
and exit code against a `.stderr` golden file beside it, and cross-checks the
`//~ ERROR` annotations in the source.

Why both a golden file and an annotation, when either alone would catch a change:
they fail differently, and the redundancy is the point. The golden pins the exact
text, span and help. The annotation says which diagnostics are *expected*, so an
extra diagnostic appearing on a line nobody touched is a failure rather than
something to re-bless without noticing. rustc's UI tests work the same way for
the same reason.

    python3 tools/diagnostic-check.py            # check
    python3 tools/diagnostic-check.py --bless    # rewrite the goldens
    python3 tools/diagnostic-check.py --nova PATH   # use a specific binary

`--bless` writes what the compiler actually said. Read the diff before you
commit it: a blessed golden that nobody looked at is a test that agrees with the
bug.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CASES = ROOT / "tests" / "diagnostics"

# `//~ ERROR` or `//~ ERROR E0308` or `//~ WARNING`. The code is optional: a
# parse error has no code today (see docs/errors.md).
ANNOTATION = re.compile(r"//~\s*(ERROR|WARNING|INFO|NOTE|HELP)\b\s*(E\d{3,5})?")
CODE_IN_OUTPUT = re.compile(r"\[(E\d{3,5})\]")
# What the CLI prints when it successfully writes a file. It names a temp path,
# so it is the one line that cannot be part of a golden.
WROTE = re.compile(r"^✓ wrote ")


def find_nova(explicit: str | None) -> str:
    if explicit:
        return explicit
    env = os.environ.get("NOVA_COMPILER")
    if env:
        return env
    for profile in ("debug", "release"):
        candidate = ROOT / "target" / profile / "nova-compiler"
        if candidate.exists():
            return str(candidate)
    sys.exit(
        "no nova-compiler binary found. Build one with `cargo build -p nova-compiler`, "
        "or pass --nova PATH."
    )


def normalise(stderr: str, rel_path: str) -> str:
    """Make the output independent of where the repository is checked out."""
    lines = []
    for raw in stderr.replace(rel_path, "<case>").splitlines():
        if WROTE.match(raw):
            continue
        lines.append(raw.rstrip())
    while lines and not lines[-1]:
        lines.pop()
    return "".join(line + "\n" for line in lines)


def run_case(nova: str, case: Path) -> tuple[int, str]:
    rel = case.relative_to(ROOT).as_posix()
    outdir = Path(tempfile.mkdtemp(prefix="nova-diag-"))
    try:
        proc = subprocess.run(
            [nova, "emit", rel, "-o", str(outdir / "out.c")],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        return proc.returncode, normalise(proc.stderr, rel)
    finally:
        shutil.rmtree(outdir, ignore_errors=True)


def run_case_json(nova: str, case: Path) -> tuple[int, list[str], str]:
    """Run the same case with `--error-format=json`.

    Returns (exit code, stdout lines, stderr). The contract under test: in JSON
    mode stdout carries one JSON object per diagnostic and nothing else, and
    stderr is empty — an editor reading stdout never has to skip a human header
    line.
    """
    rel = case.relative_to(ROOT).as_posix()
    outdir = Path(tempfile.mkdtemp(prefix="nova-diag-json-"))
    try:
        proc = subprocess.run(
            [nova, "emit", rel, "-o", str(outdir / "out.c"), "--error-format=json"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        lines = [line for line in proc.stdout.splitlines() if line.strip()]
        return proc.returncode, lines, proc.stderr
    finally:
        shutil.rmtree(outdir, ignore_errors=True)


def check_json(nova: str, case: Path, expected_codes: set[str], should_fail: bool) -> list[str]:
    """Problems with the machine-readable form of this case, if any."""
    problems: list[str] = []
    code, lines, stderr = run_case_json(nova, case)

    if stderr.strip():
        problems.append(f"--error-format=json wrote to stderr: {stderr.strip()[:200]!r}")

    seen: set[str] = set()
    for index, line in enumerate(lines):
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as error:
            problems.append(f"stdout line {index + 1} is not JSON ({error}): {line[:120]!r}")
            continue
        if not isinstance(payload, dict):
            problems.append(f"stdout line {index + 1} is not a JSON object")
            continue
        for key in ("level", "code", "message", "file", "span", "notes", "helps"):
            if key not in payload:
                problems.append(f"stdout line {index + 1} has no `{key}` key")
        if payload.get("code"):
            seen.add(payload["code"])

    if seen != expected_codes:
        problems.append(
            f"JSON codes {sorted(seen) or '[]'} != annotated {sorted(expected_codes) or '[]'}"
        )
    if should_fail and code == 0:
        problems.append("JSON mode exited 0 on a case that should fail")
    if not should_fail and lines:
        problems.append("JSON mode produced diagnostics for a valid program")
    return problems


def golden_text(code: int, stderr: str) -> str:
    return f"exit: {code}\n---\n{stderr}"


def read_golden(path: Path) -> tuple[int, str] | None:
    if not path.exists():
        return None
    text = path.read_text()
    head, _, body = text.partition("\n---\n")
    match = re.fullmatch(r"exit: (\d+)\n?", head)
    if not match:
        sys.exit(f"{path.relative_to(ROOT)}: first line must be `exit: N`")
    return int(match.group(1)), body


def annotations(case: Path) -> tuple[set[str], set[str]]:
    """Return (levels, codes) named by `//~` comments in the source."""
    levels, codes = set(), set()
    for line in case.read_text().splitlines():
        for match in ANNOTATION.finditer(line):
            levels.add(match.group(1))
            if match.group(2):
                codes.add(match.group(2))
    return levels, codes


def report_block(label: str, content: str) -> None:
    """Print content in a form that survives being read out of a CI log.

    A failing run is how a golden gets written when you have no compiler to hand:
    the actual output is right there, base64 so nothing reflows it.
    """
    print(f"{label}-BEGIN")
    print(base64.b64encode(content.encode()).decode())
    print(f"{label}-END")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bless", action="store_true", help="rewrite the goldens")
    parser.add_argument("--nova", default=None, help="path to nova-compiler")
    args = parser.parse_args()

    if not CASES.is_dir():
        sys.exit(f"{CASES} does not exist")
    cases = sorted(CASES.glob("*.nova"))
    if not cases:
        sys.exit(f"no *.nova cases under {CASES.relative_to(ROOT)}")

    nova = find_nova(args.nova)
    failures = 0
    blessed = 0

    for case in cases:
        golden_path = case.with_suffix(".stderr")
        code, stderr = run_case(nova, case)
        levels, expected_codes = annotations(case)
        actual_codes = set(CODE_IN_OUTPUT.findall(stderr))
        rel = case.relative_to(ROOT).as_posix()

        problems: list[str] = []

        # 1. The annotation is a claim about what should happen, independent of
        #    what the compiler happened to say.
        if levels and code == 0:
            problems.append(
                f"the source annotates {sorted(levels)} but the compiler exited 0"
            )
        if not levels and code != 0:
            problems.append(
                f"the source has no //~ annotation but the compiler exited {code}"
            )
        if expected_codes != actual_codes:
            problems.append(
                f"annotated codes {sorted(expected_codes) or '[]'} "
                f"!= codes in the output {sorted(actual_codes) or '[]'}"
            )
        if not levels and stderr.strip():
            problems.append("a case with no annotation produced output")

        # 2. The golden is the exact text, span and help.
        expected = golden_text(code, stderr)
        if args.bless:
            if read_golden(golden_path) != (code, stderr):
                golden_path.write_text(expected)
                print(f"blessed {golden_path.relative_to(ROOT)}")
                blessed += 1
            continue

        actual_golden = read_golden(golden_path)
        if actual_golden is None:
            problems.append(f"no golden at {golden_path.relative_to(ROOT)} — run with --bless")
        elif actual_golden != (code, stderr):
            want_code, want_stderr = actual_golden
            if want_code != code:
                problems.append(f"golden exit {want_code}, compiler exited {code}")
            if want_stderr != stderr:
                problems.append("golden text differs from the compiler's output")

        # The machine-readable form is part of the contract too: the LSP is
        # meant to read it instead of re-deriving diagnostics on its own.
        problems += check_json(nova, case, expected_codes, should_fail=bool(levels))

        if problems:
            failures += 1
            print(f"FAIL {rel}")
            for problem in problems:
                print(f"     {problem}")
            if actual_golden is not None and actual_golden[1] != stderr:
                print("     --- golden ---")
                for line in actual_golden[1].splitlines():
                    print(f"     | {line}")
            report_block("ACTUAL", expected)
        else:
            print(f"ok   {rel}")

    if args.bless:
        print(f"\nblessed {blessed} golden(s), {len(cases)} case(s)")
        return 0

    if failures:
        print(f"\n{failures} of {len(cases)} case(s) failed")
        print("Re-run with --bless to accept the compiler's output, then read the diff.")
        return 1

    print(f"\nall {len(cases)} case(s) match their goldens")
    return 0


if __name__ == "__main__":
    sys.exit(main())
