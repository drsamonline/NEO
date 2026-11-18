#!/usr/bin/env python3
"""The editor extension must describe the language the compiler actually accepts.

Two things drift silently in an editor extension, because nothing fails when they
do: the syntax highlighting keeps colouring a keyword the parser no longer has, and
the manifest points at a file that was renamed. Both are cosmetic-looking and both
make the extension wrong.

This checks:

- every keyword in the TextMate grammar is a keyword in `crates/nova-common/src/token.rs`,
  and every keyword there is highlighted;
- every path the manifest contributes exists;
- the manifest declares what VS Code requires to load an extension at all.

    python3 tools/extension-check.py
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXT = ROOT / "vscode-extension"


def compiler_keywords() -> set[str]:
    """The canonical spelling of every keyword, from the compiler's own table."""
    source = (ROOT / "crates" / "nova-common" / "src" / "token.rs").read_text()
    pairs = re.findall(r'TokenKind::(\w+) => "([^"]+)"', source)
    # Keywords are the lowercase alphabetic spellings; the rest of that match is
    # operators and punctuation.
    return {spelling for _, spelling in pairs if spelling.isalpha() and spelling.islower()}


def grammar_keywords(grammar: dict) -> set[str]:
    """Every literal alternative inside the grammar's `keywords` patterns."""
    words: set[str] = set()
    for pattern in grammar["repository"]["keywords"]["patterns"]:
        for group in re.findall(r"\(\?:([^)]*)\)", pattern["match"]):
            words |= {word.strip() for word in group.split("|") if word.strip()}
    return words


def main() -> int:
    problems: list[str] = []

    manifest_path = EXT / "package.json"
    grammar_path = EXT / "syntaxes" / "nova.tmLanguage.json"
    for required in (manifest_path, grammar_path):
        if not required.exists():
            sys.exit(f"missing {required.relative_to(ROOT)}")

    manifest = json.loads(manifest_path.read_text())
    grammar = json.loads(grammar_path.read_text())

    # --- what VS Code needs before it will load an extension at all
    for key in ("name", "publisher", "engines", "main", "activationEvents", "contributes"):
        if key not in manifest:
            problems.append(f"package.json has no `{key}`")
    if "vscode" not in manifest.get("engines", {}):
        problems.append("package.json declares no `engines.vscode`")

    # --- the entry point the manifest names must be the one that is built
    main = manifest.get("main", "")
    if main and not (EXT / main.lstrip("./")).with_suffix(".ts").exists():
        source = EXT / "src" / "extension.ts"
        if not source.exists():
            problems.append(f"`main` is {main} but src/extension.ts does not exist")

    # --- every contributed path must be real
    for language in manifest.get("contributes", {}).get("languages", []):
        configuration = language.get("configuration")
        if configuration and not (EXT / configuration).exists():
            problems.append(f"languages[{language.get('id')}] points at {configuration}, which does not exist")
    for contributed in manifest.get("contributes", {}).get("grammars", []):
        path = contributed.get("path")
        if path and not (EXT / path).exists():
            problems.append(f"grammars[{contributed.get('scopeName')}] points at {path}, which does not exist")

    # --- the grammar must describe the compiler's keyword set exactly
    compiler = compiler_keywords()
    highlighted = grammar_keywords(grammar)
    for keyword in sorted(compiler - highlighted):
        problems.append(f"`{keyword}` is a keyword in token.rs but is not highlighted")
    for keyword in sorted(highlighted - compiler):
        problems.append(f"`{keyword}` is highlighted but is not a keyword in token.rs")

    print(f"compiler keywords   ({len(compiler)}): {' '.join(sorted(compiler))}")
    print(f"grammar highlights  ({len(highlighted)}): {' '.join(sorted(highlighted))}")

    if problems:
        print()
        for problem in problems:
            print(f"FAIL {problem}")
        return 1

    print("\nthe extension describes the language the compiler accepts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
