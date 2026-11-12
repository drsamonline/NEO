#!/usr/bin/env python3
"""Check that grammar/grammar.ebnf still describes the compiler it claims to.

The grammar file used to be a three-line scaffold that described nothing, and
nothing noticed. This is the test that notices. It reads the real sources --
crates/nova-common/src/token.rs and compiler/src/parser/mod.rs -- and fails if
the grammar and the code have drifted apart in any of five ways:

  1. shape     -- every non-terminal is defined exactly once and is reachable
                  from `program` (no dead rules, no typos, no dangling names).
  2. keywords  -- every keyword the lexer knows is either used by a production
                  or declared `@reserved-keyword`; a word literal in the grammar
                  must be a keyword the lexer actually has (no invented syntax);
                  a token cannot be both used and reserved.
  3. operators -- the same rule for every operator and delimiter the lexer can
                  produce.
  4. precedence-- the expression chain in the grammar is in the same order as
                  the parser's parse_* functions, so a precedence level cannot
                  be added on one side only.
  5. honesty   -- the grammar may not claim a production the parser does not
                  have: the entry rules named here must exist as functions.

Run from the repository root:  python3 tools/grammar-check.py
Exit code 0 means the grammar is true. 1 means it is not, and the message says
which token or rule broke it.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "grammar" / "grammar.ebnf"
TOKENS = ROOT / "crates" / "nova-common" / "src" / "token.rs"
PARSER = ROOT / "compiler" / "src" / "parser" / "mod.rs"

# grammar rule -> parser function that implements it, loosest binding first.
PRECEDENCE = [
    ("assignment", "parse_assignment"),
    ("or_expr", "parse_or"),
    ("and_expr", "parse_and"),
    ("equality", "parse_equality"),
    ("comparison", "parse_comparison"),
    ("term", "parse_term"),
    ("shift", "parse_shift"),
    ("range", "parse_range"),
    ("factor", "parse_factor"),
    ("unary", "parse_unary"),
    ("cast", "parse_cast"),
    ("call", "parse_call"),
    ("primary", "parse_primary"),
]

failures = []


def fail(msg):
    failures.append(msg)


# --------------------------------------------------------------------------- #
# reading the grammar
# --------------------------------------------------------------------------- #

def read_grammar(text):
    """Return (productions, reserved_keywords, reserved_operators).

    A line whose first non-space character is ';' is a comment. A line starting
    with '@' is a directive. Everything else is production text; a ';' outside a
    quoted literal ends a production, so a production may wrap across lines.
    """
    directives, body = [], []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith(";"):
            continue
        if stripped.startswith("@"):
            directives.append(stripped)
            continue
        body.append(stripped)

    joined = " ".join(body)
    chunks, buf, i, in_string = [], [], 0, False
    while i < len(joined):
        ch = joined[i]
        if ch == '"':
            in_string = not in_string
            buf.append(ch)
        elif ch == ";" and not in_string:
            chunks.append("".join(buf).strip())
            buf = []
        else:
            buf.append(ch)
        i += 1
    tail = "".join(buf).strip()
    if tail:
        chunks.append(tail)

    productions = {}
    for chunk in chunks:
        m = re.match(r"^([a-z_][a-z0-9_]*)\s*=\s*(.+)$", chunk, re.S)
        if not m:
            fail(f"grammar: cannot read the production {chunk[:60]!r} — "
                 f"expected `name = ... ;`")
            continue
        name, rhs = m.group(1), m.group(2)
        if name in productions:
            fail(f"grammar: `{name}` is defined twice")
        productions[name] = rhs

    reserved_kw, reserved_op = set(), set()
    for d in directives:
        m = re.match(r"^@reserved-(keyword|operator)\s+(\S+)$", d)
        if not m:
            fail(f"grammar: cannot read the directive {d!r}")
            continue
        (reserved_kw if m.group(1) == "keyword" else reserved_op).add(m.group(2))

    return productions, reserved_kw, reserved_op


def literals_of(rhs):
    return set(re.findall(r'"([^"]*)"', rhs))


def references_of(rhs):
    """Non-terminals on the right-hand side: bare names, not literals, not prose."""
    cleaned = re.sub(r'"[^"]*"', " ", rhs)
    cleaned = re.sub(r"\?.*?\?", " ", cleaned, flags=re.S)
    return set(re.findall(r"\b([a-z_][a-z0-9_]*)\b", cleaned))


# --------------------------------------------------------------------------- #
# reading the sources
# --------------------------------------------------------------------------- #

def read_tokens(text):
    """Return (keywords, lexemes) from token.rs.

    keywords: the set of TokenKind variants inside `is_keyword()`.
    lexemes:  TokenKind variant -> the string the lexer prints for it.
    """
    m = re.search(r"fn is_keyword\(&self\) -> bool \{(.*?)\n    \}", text, re.S)
    if not m:
        fail("token.rs: could not find is_keyword() — has it been renamed?")
        keywords = set()
    else:
        keywords = set(re.findall(r"TokenKind::(\w+)", m.group(1)))

    lexemes = {}
    for name, lex in re.findall(r'TokenKind::(\w+)(?:\([^)]*\))? => "([^"]*)"\.to_string\(\)', text):
        lexemes[name] = lex
    if not lexemes:
        fail("token.rs: could not find the Token::new lexeme table")
    return keywords, lexemes


def read_parser_functions(text):
    """Function name -> line number, in source order."""
    found = {}
    for i, line in enumerate(text.splitlines(), 1):
        m = re.match(r"\s*(?:pub )?fn (parse_[a-z_]+)\b", line)
        if m and m.group(1) not in found:
            found[m.group(1)] = i
    return found


# --------------------------------------------------------------------------- #
# the checks
# --------------------------------------------------------------------------- #

def check_shape(productions):
    if "program" not in productions:
        fail("grammar: there is no `program` production — nothing to start from")
        return
    for name, rhs in productions.items():
        for ref in references_of(rhs):
            if ref not in productions:
                fail(f"grammar: `{name}` refers to `{ref}`, which is never defined")
    seen, queue = {"program"}, ["program"]
    while queue:
        for ref in references_of(productions.get(queue.pop(), "")):
            if ref in productions and ref not in seen:
                seen.add(ref)
                queue.append(ref)
    for name in sorted(set(productions) - seen):
        fail(f"grammar: `{name}` is defined but unreachable from `program`")


def check_tokens(productions, reserved_kw, reserved_op):
    token_text = TOKENS.read_text(encoding="utf-8")
    keywords, lexemes = read_tokens(token_text)

    used = set()
    for rhs in productions.values():
        used |= literals_of(rhs)

    # a keyword with no lexeme cannot be matched against the grammar at all, so
    # it would otherwise slip through silently
    for k in sorted(keywords):
        if not lexemes.get(k):
            fail(f"token.rs: keyword `{k}` has no lexeme in Token::new — the "
                 f"grammar cannot be checked against a keyword with no spelling")

    keyword_words = {lexemes.get(k, "") for k in keywords}
    keyword_words.discard("")

    # every keyword is used or reserved, and not both
    for word in sorted(keyword_words):
        if word in used and word in reserved_kw:
            fail(f"keyword `{word}` is both used by a production and declared "
                 f"@reserved-keyword — it is one or the other")
        elif word not in used and word not in reserved_kw:
            fail(f"keyword `{word}` is in token.rs but the grammar neither uses it "
                 f"nor declares it @reserved-keyword")
    for word in sorted(reserved_kw - keyword_words):
        fail(f"`{word}` is declared @reserved-keyword but is not a keyword in token.rs")

    # no invented keywords: a word literal must be a real keyword
    for word in sorted(w for w in used if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", w)):
        if word not in keyword_words:
            fail(f"grammar: `\"{word}\"` is used as a keyword but token.rs has no such "
                 f"keyword — the grammar may not invent syntax")

    # operators and delimiters, same rule
    operator_lexemes = {lex for lex in lexemes.values() if lex and not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", lex)}
    for lex in sorted(operator_lexemes):
        if lex in used and lex in reserved_op:
            fail(f"operator `{lex}` is both used by a production and declared "
                 f"@reserved-operator — it is one or the other")
        elif lex not in used and lex not in reserved_op:
            fail(f"operator `{lex}` is in token.rs but the grammar neither uses it "
                 f"nor declares it @reserved-operator")
    for lex in sorted(reserved_op - operator_lexemes):
        fail(f"`{lex}` is declared @reserved-operator but token.rs has no such operator")


def check_precedence(productions, parser_fns):
    for name, fn in PRECEDENCE:
        if name not in productions:
            fail(f"grammar: the expression rule `{name}` is missing")
            continue
        if fn not in parser_fns:
            fail(f"parser: `{fn}` is missing, but the grammar still expects the "
                 f"`{name}` level")
    for (name, _), (nxt, _) in zip(PRECEDENCE, PRECEDENCE[1:]):
        if name in productions and nxt not in references_of(productions[name]):
            fail(f"grammar: `{name}` does not descend into `{nxt}` — the expression "
                 f"chain is broken")
    order = [(parser_fns[fn], name) for name, fn in PRECEDENCE if fn in parser_fns]
    for (line_a, a), (line_b, b) in zip(order, order[1:]):
        if line_a > line_b:
            fail(f"parser: `{b}` is parsed before `{a}` in the source but binds "
                 f"tighter in the grammar — one of the two was edited alone")


def main():
    for path in (GRAMMAR, TOKENS, PARSER):
        if not path.exists():
            fail(f"missing file: {path.relative_to(ROOT)}")
    if failures:
        report()
        return 1

    productions, reserved_kw, reserved_op = read_grammar(GRAMMAR.read_text(encoding="utf-8"))
    check_shape(productions)
    check_tokens(productions, reserved_kw, reserved_op)
    check_precedence(productions, read_parser_functions(PARSER.read_text(encoding="utf-8")))

    if failures:
        report()
        return 1

    print(f"grammar ok: {len(productions)} productions, "
          f"{len(reserved_kw)} reserved keyword(s), {len(reserved_op)} reserved operator(s)")
    print(f"  {GRAMMAR.relative_to(ROOT)} describes {PARSER.relative_to(ROOT)} and {TOKENS.relative_to(ROOT)}")
    return 0


def report():
    print(f"grammar-check: {len(failures)} problem(s)\n")
    for f in failures:
        print(f"  - {f}")
    print("\nThe grammar is the contract; fix the grammar or fix the code, not this test.")


if __name__ == "__main__":
    sys.exit(main())
