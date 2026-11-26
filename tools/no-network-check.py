#!/usr/bin/env python3
"""The compiler, language server and debugger must not reach the network.

`PRIVACY.md` and `FAQ.md` both promise that no request carries anything the user types
or stores, and that the compiler contains no network code at all. That is a claim about
the dependency graph and the sources, so it can be checked rather than trusted.

It fails when:

- a crate in `compiler/`, `lsp/`, `dbg/`, `crates/` or `backends/` depends on an HTTP
  client or a raw socket crate;
- their sources mention `TcpStream`, `UdpSocket`, `std::net` or a `reqwest::`/`hyper::`
  call outside a comment.

`pkg/` is deliberately out of scope: it runs `git clone` for a `git` dependency, which
`PRIVACY.md` states plainly.

    python3 tools/no-network-check.py
"""
from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Crates that reach the network. `tokio` is not here: the language server uses it as a
# runtime and never opens a socket, which the source scan below confirms.
FORBIDDEN_CRATES = {
    "reqwest", "hyper", "ureq", "isahc", "surf", "curl", "curl-sys", "attohttpc",
    "tiny_http", "axum", "warp", "tungstenite", "tokio-tungstenite",
}

SCOPE = ["compiler", "lsp", "dbg", "crates", "backends"]
TABLES = ("dependencies", "dev-dependencies", "build-dependencies")

FORBIDDEN_SOURCE = re.compile(r"\b(TcpStream|UdpSocket|std::net|reqwest::|hyper::)\b")


def manifests() -> list[Path]:
    found: list[Path] = []
    for entry in SCOPE:
        base = ROOT / entry
        if base.is_dir():
            found.extend(sorted(base.rglob("Cargo.toml")))
    return [p for p in found if "target" not in p.parts]


def main() -> int:
    problems: list[str] = []

    for manifest in manifests():
        parsed = tomllib.loads(manifest.read_text())
        for table in TABLES:
            for name in parsed.get(table, {}):
                if name in FORBIDDEN_CRATES:
                    problems.append(
                        f"{manifest.relative_to(ROOT)} depends on `{name}`, which reaches the network"
                    )

    for entry in SCOPE:
        base = ROOT / entry
        if not base.is_dir():
            continue
        for source in sorted(base.rglob("*.rs")):
            if "target" in source.parts:
                continue
            for number, line in enumerate(source.read_text(errors="ignore").splitlines(), 1):
                code = line.split("//")[0]
                if FORBIDDEN_SOURCE.search(code):
                    problems.append(
                        f"{source.relative_to(ROOT)}:{number} uses a socket: {line.strip()}"
                    )

    print(f"{len(manifests())} manifest(s) and their sources checked for network use")
    if problems:
        print("FAIL the compiler, language server and debugger are promised to be offline:")
        for problem in problems:
            print(f"     {problem}")
        print("     either remove the use, or correct PRIVACY.md and FAQ.md")
        return 1
    print("no HTTP client crate and no socket call: the offline claim holds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
