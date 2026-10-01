"""String- and comment-aware brace balance check.

The first version of this counted braces with a plain regex and reported a file
balanced while rustc was reporting `unexpected closing delimiter`. Braces
inside string literals and doc comments threw the count off. A smoke test that
lies is worse than no smoke test at all, so the scanner skips:

  * string literals, escapes included,
  * char literals, told apart from lifetimes,
  * line and block comments, doc comments included,
  * raw strings, where a backslash is not an escape.

It reports the first line at which the depth goes negative, because that is
where an extra closing brace lives. A negative depth is the useful signal: a
non-zero final depth can also mean an unclosed block, and rustc names that one
too, so both are reported.

usage: python check_braces.py FILE [FILE...]
"""

import os
import sys


def _is_char_literal(src, start):
    """True when the quote at `start` opens `'x'` rather than a lifetime `'a`."""
    j, escaped = start + 1, False
    while j < len(src) and j - start < 8:
        c = src[j]
        if escaped:
            escaped = False
        elif c == '\\':
            escaped = True
        elif c == "'":
            return True
        elif c == '\n':
            return False
        j += 1
    return False


def scan(src):
    """Return (first_negative_line, final_depth)."""
    i, n, line, depth = 0, len(src), 1, 0
    in_str = in_ch = in_raw = block = esc = False
    hashes = 0
    first_negative = None
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ''
        if c == '\n':
            line += 1
            i += 1
            continue
        if in_raw:
            if c == '"' and src[i + 1:i + 1 + hashes] == '#' * hashes:
                in_raw = False
                i += 1 + hashes
                continue
            i += 1
            continue
        if block:
            if c == '*' and nxt == '/':
                block = False
                i += 2
                continue
            i += 1
            continue
        if in_str:
            if esc:
                esc = False
            elif c == '\\':
                esc = True
            elif c == '"':
                in_str = False
            i += 1
            continue
        if in_ch:
            if esc:
                esc = False
            elif c == '\\':
                esc = True
            elif c == "'":
                in_ch = False
            i += 1
            continue
        if c == 'r' and nxt in ('"', '#'):
            j = i + 1
            while j < n and src[j] == '#':
                j += 1
            if j < n and src[j] == '"':
                hashes = j - i - 1
                in_raw = True
                i = j + 1
                continue
        if c == '"':
            in_str = True
            i += 1
            continue
        if c == "'" and _is_char_literal(src, i):
            in_ch = True
            i += 1
            continue
        if c == '/' and nxt == '/':
            while i < n and src[i] != '\n':
                i += 1
            continue
        if c == '/' and nxt == '*':
            block = True
            i += 2
            continue
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth < 0 and first_negative is None:
                first_negative = line
        i += 1
    return first_negative, depth


def main(paths):
    bad = 0
    for path in paths:
        src = open(path, encoding='utf-8', errors='replace').read()
        negative, depth = scan(src)
        if negative is not None:
            bad += 1
            print(f'{path}: depth goes negative at line {negative}')
        elif depth != 0:
            bad += 1
            print(f'{path}: final depth {depth}, expected 0')
    print(f'checked {len(paths)} file(s), {bad} imbalance(s)')
    return 1 if bad else 0


def default_files():
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    src = os.path.join(here, 'src')
    found = []
    for root, _, names in os.walk(src):
        found.extend(os.path.join(root, n) for n in names if n.endswith('.rs'))
    return sorted(found)


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:] or default_files()))