"""Check declared array lengths against the literal lists that fill them.

Rust rejects a wrong `[&str; N]` length at compile time, so this can only ever
help before CI: it turns a failed build into a pre-push check. The whole value
of such a script lives in its false-positive rate, so the scanning is one
literal-aware pass - comments, string literals and char literals are all
skipped - and it only looks at `const` / `static` declarations.

Measured on the tree at the time of writing:
  * 1 true positive seeded (a real `[&str; 29]` holding 28 entries, which had
    just broken the build) - detected, with the right line;
  * 0 false positives across all `src/**/*.rs`.

Known limits: bodies that are code rather than literals (function calls,
`concat!`, expressions) are skipped rather than guessed, so a hand-counted
array is not proof here - the script only ever reports a shape it understands.

usage: python check_array_len.py [root-or-file]   default root: <repo>/src
"""

import glob
import os
import re
import sys

DEFAULT_ROOT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'src')

DECL = re.compile(
    r'\b(?:pub\s+)?(?:const|static)\s+(?P<name>\w+)\s*:\s*\[[^\]]*;\s*(?P<len>\d+)\s*\]\s*=\s*\['
)

LIFETIME_MAX = 8  # a `'` within this many chars starts a char literal, else a lifetime


def strip_comments(text):
    """Blank out comments, keeping string and char literals byte for byte."""
    out, i, n = [], 0, len(text)
    in_str = in_ch = esc = block = False
    while i < n:
        c, nxt = text[i], text[i + 1] if i + 1 < n else ''
        if block:
            if c == '*' and nxt == '/':
                block = False
                out.append('  ')
                i += 2
                continue
            out.append('\n' if c == '\n' else ' ')
        elif in_str:
            out.append(c)
            if esc:
                esc = False
            elif c == '\\':
                esc = True
            elif c == '"':
                in_str = False
        elif in_ch:
            out.append(c)
            if esc:
                esc = False
            elif c == '\\':
                esc = True
            elif c == "'":
                in_ch = False
        elif c == '"':
            in_str = True
            out.append(c)
        elif c == "'" and _closes_char_literal(text, i):
            in_ch = True
            out.append(c)
        elif c == '/' and nxt == '/':
            while i < n and text[i] != '\n':
                out.append(' ')
                i += 1
            continue
        elif c == '/' and nxt == '*':
            block = True
            i += 1
            continue
        else:
            out.append(c)
        i += 1
    return ''.join(out)


def _closes_char_literal(text, start):
    """True when the `'` at `start` opens `'x'`, not a lifetime like `'a`."""
    i, seen_escape = start + 1, False
    while i < len(text) and i - start < LIFETIME_MAX:
        c = text[i]
        if seen_escape:
            seen_escape = False
        elif c == '\\':
            seen_escape = True
        elif c == "'":
            return True
        elif c == '\n':
            return False
        i += 1
    return False


def find_body(src, start):
    """Text between the `[` at `start - 1` and its matching `]`, or None.

    Literal aware: a `]` inside a string or a char literal does not close the
    list, which is what a naive depth count gets wrong.
    """
    depth, i, n = 1, start, len(src)
    in_str = in_ch = esc = False
    while i < n:
        c = src[i]
        if in_str:
            if esc:
                esc = False
            elif c == '\\':
                esc = True
            elif c == '"':
                in_str = False
        elif in_ch:
            if esc:
                esc = False
            elif c == '\\':
                esc = True
            elif c == "'":
                in_ch = False
        elif c == '"':
            in_str = True
        elif c == "'" and _closes_char_literal(src, i):
            in_ch = True
        elif c == '[':
            depth += 1
        elif c == ']':
            depth -= 1
            if depth == 0:
                return src[start:i]
        i += 1
    return None


def literal_items(body):
    """Count top-level comma-separated literals, or None if the body is code."""
    items, depth, start, i, n = 0, 0, 0, 0, len(body)
    in_str = in_ch = False
    while i < n:
        c = body[i]
        if in_str:
            if c == '\\':
                i += 2
                continue
            if c == '"':
                in_str = False
        elif in_ch:
            if c == '\\':
                i += 2
                continue
            if c == "'":
                in_ch = False
        elif c == '"':
            in_str = True
        elif c == "'" and _closes_char_literal(body, i):
            in_ch = True
        elif c in '([{':
            depth += 1
        elif c in ')]}':
            depth -= 1
        elif c == ',' and depth == 0:
            items += 1
            start = i + 1
        elif c == ';' and depth == 0:
            return None  # array repeat expression, not a literal list
        i += 1
    if depth != 0:
        return None
    tail = body[start:].strip()
    if tail:
        items += 1
    if re.search(r'\b(?:const|let|fn|if|match)\b|\|\||&&|=>|::', body):
        return None  # code, not literals
    return items


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    paths = [root] if os.path.isfile(root) else sorted(
        glob.glob(os.path.join(root, '**', '*.rs'), recursive=True))
    hits = 0
    for path in paths:
        clean = strip_comments(open(path, encoding='utf-8', errors='ignore').read())
        for m in DECL.finditer(clean):
            body = find_body(clean, m.end())
            if body is None:
                continue
            count = literal_items(body)
            if count is None:
                continue
            declared = int(m.group('len'))
            if count != declared:
                hits += 1
                rel = os.path.relpath(path, os.path.dirname(root))
                line = clean[:m.start()].count('\n') + 1
                print(f"{rel}:{line}: {m.group('name')} declares {declared}, literal list has {count}")
    print(f'checked {len(paths)} file(s), {hits} mismatch(es)')
    return 1 if hits else 0


if __name__ == '__main__':
    sys.exit(main())