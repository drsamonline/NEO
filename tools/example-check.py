#!/usr/bin/env python3
"""Check that every program shipped in examples/ is a program NOVA accepts.

The repository ships example programs that a reader is told to compile. Nothing
ever read them back: `grammar-check.py` compares `grammar/grammar.ebnf` with the
Rust sources, and the Rust tests parse snippets written inside `#[test]`
functions -- so an example could say anything at all and the build stayed green.

This is the test that notices. It is driven by `grammar/grammar.ebnf` itself, not
by a second hand-written parser: the grammar is read, turned into a recogniser,
and every `examples/**/*.nova` is required to parse from `program` to EOF. The
two checks therefore compose -- `grammar-check.py` proves the grammar describes
the parser, and this proves the examples are described by the grammar.

It fails, naming the file, the line and the token, when an example

  * is not lexable (an unterminated string, a stray character), or
  * is not a program (a file of comments and nothing else), or
  * uses syntax the grammar does not have -- `print "hi"` with no parentheses,
    a type annotation the parser cannot read, and so on.

Run from the repository root:  python3 tools/example-check.py
Exit code 0 means every shipped example is a NOVA program. 1 means one is not.

`--selftest` runs the recogniser against a handful of known-good and known-bad
sources, so this check cannot pass by being vacuous.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "grammar" / "grammar.ebnf"
EXAMPLES = ROOT / "examples"

# --------------------------------------------------------------------------- #
# The lexer -- a faithful mirror of compiler/src/lexer/mod.rs
# --------------------------------------------------------------------------- #

KEYWORDS = {
    "fn", "let", "mut", "if", "else", "while", "for", "return", "true", "false",
    "struct", "enum", "match", "use", "mod", "pub", "const", "type", "as", "in",
    "loop", "break", "continue", "impl", "static",
}

# Longest first: the Rust lexer dispatches on the first character and then peeks
# one ahead, which for this set is the same thing.
OPERATORS = [
    "->", "=>", "::", "..", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>",
    "+=", "-=", "*=", "/=",
    "+", "-", "*", "/", "%", "=", "!", "<", ">", "&", "|", "^", "~",
    "(", ")", "{", "}", "[", "]", ",", ";", ":", ".", "?",
]


class LexError(Exception):
    def __init__(self, message, line):
        super().__init__(message)
        self.line = line


class Token:
    __slots__ = ("kind", "text", "line")

    def __init__(self, kind, text, line):
        self.kind = kind      # ident | int | float | string | char | kw | sym | eof
        self.text = text
        self.line = line

    def __repr__(self):
        return f"{self.kind}({self.text!r})@{self.line}"


def lex(source):
    tokens = []
    i, line = 0, 1
    n = len(source)
    while i < n:
        ch = source[i]
        if ch in " \t\r\n":
            if ch == "\n":
                line += 1
            i += 1
            continue
        # comments: // to end of line, /* ... */ to the closing pair
        if ch == "/" and i + 1 < n and source[i + 1] == "/":
            while i < n and source[i] != "\n":
                i += 1
            continue
        if ch == "/" and i + 1 < n and source[i + 1] == "*":
            start_line = line
            i += 2
            prev = "/"
            while i < n:
                if source[i] == "\n":
                    line += 1
                if prev == "*" and source[i] == "/":
                    i += 1
                    break
                prev = source[i]
                i += 1
            else:
                raise LexError("unterminated block comment", start_line)
            continue
        if ch.isalpha() or ch == "_":
            j = i
            while j < n and (source[j].isalnum() or source[j] == "_"):
                j += 1
            word = source[i:j]
            tokens.append(Token("kw" if word in KEYWORDS else "ident", word, line))
            i = j
            continue
        if ch.isdigit():
            j, is_float = i, False
            if ch == "0" and i + 1 < n and source[i + 1] in "xXbBoO":
                radix = {"x": 16, "b": 2, "o": 8}[source[i + 1].lower()]
                valid = {16: lambda c: c in "0123456789abcdefABCDEF",
                         2: lambda c: c in "01",
                         8: lambda c: c in "01234567"}[radix]
                j = i + 2
                while j < n and (valid(source[j]) or source[j] == "_"):
                    j += 1
                tokens.append(Token("int", source[i:j], line))
                i = j
                continue
            while j < n:
                c = source[j]
                if c.isdigit() or c == "_":
                    j += 1
                elif c == "." and not is_float and j + 1 < n and source[j + 1].isdigit():
                    is_float = True
                    j += 1
                elif c in "eE":
                    is_float = True
                    j += 1
                    if j < n and source[j] in "+-":
                        j += 1
                else:
                    break
            tokens.append(Token("float" if is_float else "int", source[i:j], line))
            i = j
            continue
        if ch == '"':
            start_line = line
            j = i + 1
            while j < n:
                if source[j] == "\\":
                    j += 2
                    continue
                if source[j] == "\n":
                    raise LexError("unterminated string", start_line)
                if source[j] == '"':
                    break
                j += 1
            else:
                raise LexError("unterminated string", start_line)
            tokens.append(Token("string", source[i:j + 1], start_line))
            i = j + 1
            continue
        if ch == "'":
            start_line = line
            j = i + 1
            if j < n and source[j] == "\\":
                j += 2
            elif j < n:
                j += 1
            if j < n and source[j] == "'":
                tokens.append(Token("char", source[i:j + 1], start_line))
                i = j + 1
                continue
            raise LexError("unterminated character literal", start_line)
        for op in OPERATORS:
            if source.startswith(op, i):
                tokens.append(Token("sym", op, line))
                i += len(op)
                break
        else:
            raise LexError(f"stray character {ch!r}", line)
    tokens.append(Token("eof", "", line))
    return tokens


# --------------------------------------------------------------------------- #
# Reading the grammar into a rule table
# --------------------------------------------------------------------------- #

def read_grammar(text):
    """Return {production_name: element}. Directives and comments are skipped."""
    # Strip comment lines first: a line whose first non-space character is ';'.
    body = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith(";") or stripped.startswith("@"):
            continue
        body.append(stripped)
    joined = " ".join(body)

    # Tokenise: quoted literals, ? prose ?, words, and the structural symbols.
    parts, i, n = [], 0, len(joined)
    while i < n:
        c = joined[i]
        if c in " \t":
            i += 1
        elif c == '"':
            j = i + 1
            while j < n and joined[j] != '"':
                j += 1
            parts.append(("lit", joined[i + 1:j]))
            i = j + 1
        elif c == "?":
            j = joined.find("?", i + 1)
            if j < 0:
                raise SystemExit("grammar: unterminated ? prose ?")
            parts.append(("class", joined[i + 1:j].strip()))
            i = j + 1
        elif c in "=|()[]{};":
            parts.append(("sym", c))
            i += 1
        else:
            j = i
            while j < n and (joined[j].isalnum() or joined[j] == "_"):
                j += 1
            if j == i:
                raise SystemExit(f"grammar: unexpected {c!r}")
            parts.append(("word", joined[i:j]))
            i = j

    # Split into productions at top level: name = expr ;
    rules, pos = {}, 0
    while pos < len(parts):
        if parts[pos][0] != "word" or pos + 1 >= len(parts) or parts[pos + 1] != ("sym", "="):
            raise SystemExit(f"grammar: expected 'name =' at {parts[pos]}")
        name = parts[pos][1]
        pos += 2
        depth, start = 0, pos
        while pos < len(parts):
            kind, value = parts[pos]
            if kind == "sym" and value in "([{":
                depth += 1
            elif kind == "sym" and value in ")]}":
                depth -= 1
            elif kind == "sym" and value == ";" and depth == 0:
                break
            pos += 1
        if pos >= len(parts):
            raise SystemExit(f"grammar: production {name!r} is not terminated by ';'")
        rules[name] = parse_alt(parts[start:pos], name)
        pos += 1
    return rules


def parse_alt(parts, where):
    """alt := seq ('|' seq)*   -- returns ('alt', [seq, ...])."""
    seqs, current, depth = [], [], 0
    for kind, value in parts:
        if kind == "sym" and value in "([{":
            depth += 1
        elif kind == "sym" and value in ")]}":
            depth -= 1
        if depth == 0 and kind == "sym" and value == "|":
            seqs.append(parse_seq(current, where))
            current = []
        else:
            current.append((kind, value))
    seqs.append(parse_seq(current, where))
    return ("alt", seqs) if len(seqs) > 1 else seqs[0]


def parse_seq(parts, where):
    """seq := item+   -- returns ('seq', [item, ...])."""
    items, i = [], 0
    while i < len(parts):
        kind, value = parts[i]
        if kind == "sym" and value in "{[":
            close = "}" if value == "{" else "]"
            depth, j = 0, i
            while j < len(parts):
                k, v = parts[j]
                if k == "sym" and v in "([{":
                    depth += 1
                elif k == "sym" and v in ")]}":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            if j >= len(parts) or parts[j][1] != close:
                raise SystemExit(f"grammar: unbalanced {value!r} in {where!r}")
            inner = parse_alt(parts[i + 1:j], where)
            items.append(("rep" if value == "{" else "opt", inner))
            i = j + 1
        elif kind == "sym" and value == "(":
            depth, j = 0, i
            while j < len(parts):
                k, v = parts[j]
                if k == "sym" and v in "([{":
                    depth += 1
                elif k == "sym" and v in ")]}":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            items.append(parse_alt(parts[i + 1:j], where))
            i = j + 1
        elif kind == "lit":
            items.append(("lit", value))
            i += 1
        elif kind == "class":
            items.append(("class", value))
            i += 1
        elif kind == "word":
            items.append(("ref", value))
            i += 1
        else:
            raise SystemExit(f"grammar: unexpected {parts[i]!r} in {where!r}")
    return ("seq", items)


# --------------------------------------------------------------------------- #
# The recogniser -- packrat: memoised (rule, position) -> end positions
# --------------------------------------------------------------------------- #

CAP = 256   # a rule may end in at most this many places; beyond that, give up


class Recogniser:
    def __init__(self, rules, tokens):
        self.rules = rules
        self.tokens = tokens
        self.memo = {}
        self.eof = len(tokens) - 1

    def match_rule(self, name, pos):
        key = (name, pos)
        if key in self.memo:
            return self.memo[key]
        if name not in self.rules:
            raise SystemExit(f"grammar: non-terminal {name!r} is used but not defined")
        self.memo[key] = set()          # guards against a cyclic rule
        ends = self.match(self.rules[name], pos)
        self.memo[key] = ends
        return ends

    def match(self, node, pos):
        kind = node[0]
        if kind == "alt":
            out = set()
            for seq in node[1]:
                out |= self.match(seq, pos)
            return out
        if kind == "seq":
            current = {pos}
            for item in node[1]:
                if not current:
                    return set()
                nxt = set()
                for p in current:
                    nxt |= self.match(item, p)
                current = nxt if len(nxt) <= CAP else set(list(nxt)[:CAP])
            return current
        if kind == "rep":
            seen = {pos}
            frontier = {pos}
            while frontier:
                nxt = set()
                for p in frontier:
                    for q in self.match(node[1], p):
                        if q not in seen and q != p:
                            seen.add(q)
                            nxt.add(q)
                frontier = nxt if len(nxt) <= CAP else set(list(nxt)[:CAP])
            return seen
        if kind == "opt":
            return {pos} | self.match(node[1], pos)
        if kind == "ref":
            return self.match_rule(node[1], pos)
        if kind == "lit":
            tok = self.tokens[pos]
            if tok.kind in ("kw", "sym") and tok.text == node[1]:
                return {pos + 1}
            return set()
        if kind == "class":
            return self.match_class(node[1], pos)
        raise SystemExit(f"grammar: unknown node {node!r}")

    def match_class(self, prose, pos):
        tok = self.tokens[pos]
        p = prose.lower()
        if "identifier" in p:
            return {pos + 1} if tok.kind == "ident" else set()
        if "integer" in p:
            return {pos + 1} if tok.kind == "int" else set()
        if "float" in p or "floating" in p:
            return {pos + 1} if tok.kind == "float" else set()
        if "string" in p:
            return {pos + 1} if tok.kind == "string" else set()
        if "character" in p or "char" in p:
            return {pos + 1} if tok.kind == "char" else set()
        raise SystemExit(f"grammar: don't know the token class {prose!r}")


def check_source(source, rules=None):
    """Return (ok, message). Parses `program` to EOF with at least one token."""
    if rules is None:
        rules = read_grammar(GRAMMAR.read_text(encoding="utf-8"))
    tokens = lex(source)
    if len(tokens) == 1:                       # only EOF: comments and whitespace
        return False, "no program: the file holds no tokens at all"
    rec = Recogniser(rules, tokens)
    ends = rec.match_rule("program", 0)
    if rec.eof not in ends:
        bad = _first_unconsumed(tokens, ends)
        return False, (f"not a program: parsing stopped at line {bad.line} "
                       f"on {bad.kind} {bad.text!r}")
    return True, "ok"


def _first_unconsumed(tokens, ends):
    """The token just past the longest partial parse -- where it went wrong."""
    best = max(ends) if ends else 0
    return tokens[min(best, len(tokens) - 1)]


# --------------------------------------------------------------------------- #
# The check
# --------------------------------------------------------------------------- #

SELFTEST = [
    # (source, should_parse, why)
    ('print("Hello, NOVA");', True, "a call with parentheses and a semicolon"),
    ('print "Hello from NOVA"', False, "a call with no parentheses is not a call"),
    ("fn f(a: int) -> int { return a; }", True, "typed parameter and return"),
    ("fn f(a: [int]) {}", True, "a slice type"),
    ("fn id<T>(x: T) -> T { x }", True, "generic parameters"),
    ("fn main() { match 1 { 1 => print(1), _ => print(0) } }", True, "match arms"),
    ("fn main() { let r = Range { start, end }; }", True, "a struct literal"),
    ("let x: int = 1;", True, "a top-level let"),
    ("let x = 1", False, "a let needs its ';'"),
    ("// only a comment\n", False, "a file of comments is not a program"),
    ("fn main() { if true { print(1); } }", True, "if with a block body"),
    ("struct Point { x: Int, y: Int }", True, "a struct"),
    ("fn main() { let a = [1, 2, 3]; print(a[0]); }", True, "array literal and index"),
    ("fn main() { for i in 0..10 { print(i); } }", True, "for over a range"),
]


def selftest(rules):
    bad = 0
    for source, expected, why in SELFTEST:
        ok, message = check_source(source, rules)
        if ok != expected:
            bad += 1
            print(f"  SELFTEST FAIL: expected {'parse' if expected else 'reject'} "
                  f"({why}) for {source!r}; got: {message}")
    if bad:
        print(f"selftest: {bad} of {len(SELFTEST)} case(s) wrong -- the recogniser "
              f"itself is broken, so nothing below it can be trusted")
        return False
    print(f"selftest ok: {len(SELFTEST)} cases -- the recogniser accepts and rejects")
    return True


def main(argv):
    rules = read_grammar(GRAMMAR.read_text(encoding="utf-8"))

    if "--selftest" in argv:
        return 0 if selftest(rules) else 1
    if not selftest(rules):
        return 1

    files = sorted(EXAMPLES.rglob("*.nova"))
    if not files:
        print(f"example-check: no .nova files under {EXAMPLES} -- nothing was checked, "
              f"which is a failure, not a pass")
        return 1

    failures = []
    for path in files:
        rel = path.relative_to(ROOT)
        try:
            ok, message = check_source(path.read_text(encoding="utf-8"), rules)
        except LexError as exc:
            ok, message = False, f"does not lex: {exc} (line {exc.line})"
        if not ok:
            failures.append((rel, message))

    if failures:
        for rel, message in failures:
            print(f"FAIL {rel}: {message}")
        print(f"\nexample-check: {len(failures)} of {len(files)} example(s) are not "
              f"NOVA programs. A reader is told to compile these.")
        return 1

    print(f"example-check ok: {len(files)} example(s) parse from `program` to EOF "
          f"against grammar/grammar.ebnf")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
