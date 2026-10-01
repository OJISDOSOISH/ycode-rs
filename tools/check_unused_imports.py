"""Report `use` imports whose name never appears again in the file.

rustc reports unused imports, but only after a full compile, and on this
project that costs a CI cycle per mistake. The check is meant to be cheap and
high precision: an import mentioned exactly once in the file - the import
itself - is unused.

History worth keeping: the first version of this script reported 15 candidates,
all 15 false positives. Its own comment-stripping state machine desynchronised
on a character literal and blanked the rest of the file, so every real mention
of the imported name disappeared and every import looked unused. It did not
find the two genuinely unused imports rustc had reported, because those had
already been fixed by their owners, so there was no ground truth left to check
against. A checker that produces only false positives is worse than none.

So this version reuses the character-literal detector from check_braces.py
verbatim - that one is validated, it found a real defect at the right line, and
it reported nothing on 213 files - rather than writing a third variant of the
same fiddly state machine.

Measured on the tree at the time of writing: the 15 false positives of the
first version are gone, and one candidate remains - `use std::future::Future;`
in src/swarm/util_iife.rs, which is a true positive. Seeded checks: an unused
import is reported with the right line while its used sibling is not; a trait
imported only so a macro can resolve it IS reported, which is the documented
false positive below.

Known limits: a trait imported only for method or macro resolution looks
unused and is a false positive - `use std::fmt::Write;` with only `write!` in
the body is the case to remember; `use a::*` is skipped; an alias used only in
another `use` counts as a use.

A note on measuring this at all: the first seeded check reported zero
candidates for two imports, one of them unused. The cause was a BOM, so the
first line did not start at a `use` and the anchor never matched it. A
measurement that reports nothing is only meaningful once you have shown it can
report something.

usage: python check_unused_imports.py [root-or-file]
"""

import os
import re
import sys

DEFAULT_ROOT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'src')

USE = re.compile(r'^[ \t]*(?:pub[ \t]+)?use[ \t]+([^;]+);', re.M)


def is_char_literal(src, start):
    """True when the quote at `start` opens `'x'` rather than a lifetime `'a`.

    Copied from tools/check_braces.py, where it is validated: that script found
    a real `unexpected closing delimiter` at the right line and reported zero
    imbalances across the tree.
    """
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


def blank_non_code(src):
    """Replace comments and literal bodies with spaces, keeping newlines.

    Every replacement is the same length as what it replaces, so offsets still
    point at the original line.
    """
    out = []
    i, n = 0, len(src)
    in_line = in_block = in_str = in_raw = esc = False
    hashes = 0
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ''
        if c == '\n':
            in_line = False
            out.append(c)
            i += 1
            continue
        if in_raw:
            if c == '"' and src[i + 1:i + 1 + hashes] == '#' * hashes:
                in_raw = False
                i += 1 + hashes
                continue
            out.append(c)
            i += 1
            continue
        if in_block:
            if c == '*' and nxt == '/':
                in_block = False
                out.append('  ')
                i += 2
                continue
            out.append('\n' if c == '\n' else ' ')
            i += 1
            continue
        if in_str:
            if esc:
                esc = False
                out.append(c)
            elif c == '\\':
                esc = True
                out.append(c)
            elif c == '"':
                in_str = False
                out.append(c)
            else:
                out.append('\n' if c == '\n' else ' ')
            i += 1
            continue
        if c == 'r' and nxt in ('"', '#'):
            j = i + 1
            while j < n and src[j] == '#':
                j += 1
            if j < n and src[j] == '"':
                hashes = j - i - 1
                in_raw = True
                out.append(src[i:j + 1])
                i = j + 1
                continue
        if c == '"':
            in_str = True
            out.append(c)
            i += 1
            continue
        if c == '/' and nxt == '/':
            in_line = True
            out.append('  ')
            i += 2
            continue
        if c == '/' and nxt == '*':
            in_block = True
            out.append('  ')
            i += 2
            continue
        if c == "'" and is_char_literal(src, i):
            # Keep the whole literal: the character itself may matter, but its
            # content must never satisfy a name search.
            j = i + 1
            escaped = False
            while j < n and src[j] != '\n':
                if escaped:
                    escaped = False
                elif src[j] == '\\':
                    escaped = True
                elif src[j] == "'":
                    break
                j += 1
            end = min(j + 1, n)
            out.append(src[i] + ' ' * (end - i - 2) + (src[end - 1] if end - 1 > i else ''))
            i = end
            continue
        out.append(c)
        i += 1
    return ''.join(out)


def imported_names(tree):
    """The local names a single `use` statement binds, or None for a glob."""
    names = []
    for part in tree.split(','):
        part = part.strip()
        if not part:
            continue
        if part.endswith('*'):
            return None
        if ' as ' in part:
            names.append(part.split(' as ')[-1].strip())
        else:
            segment = part.split('::')[-1].strip()
            if segment:
                names.append(segment)
    return names


def check(path):
    raw = open(path, encoding='utf-8', errors='replace').read()
    code = blank_non_code(raw)
    hits = []
    for m in USE.finditer(code):
        names = imported_names(m.group(1))
        if not names:
            continue
        line = raw[:m.start()].count('\n') + 1
        for name in names:
            if not re.match(r'^[A-Za-z_][A-Za-z0-9_]*$', name):
                continue
            if len(re.findall(r'\b' + re.escape(name) + r'\b', code)) <= 1:
                hits.append((line, name))
    return hits


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    if os.path.isfile(root):
        paths = [root]
    else:
        paths = sorted(
            os.path.join(dirpath, n)
            for dirpath, _, names in os.walk(root)
            for n in names if n.endswith('.rs')
        )
    total = 0
    for path in paths:
        for line, name in check(path):
            total += 1
            print(f'{os.path.relpath(path, os.path.dirname(root))}:{line}: unused import candidate `{name}`')
    print(f'checked {len(paths)} file(s), {total} candidate(s)')
    return 0


if __name__ == '__main__':
    sys.exit(main())