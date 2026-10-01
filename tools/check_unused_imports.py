"""Report `use` imports whose name never appears again in the file.

rustc reports unused imports, but only after a full compile, and on this
project that costs a CI cycle per mistake - about four minutes each, shared
with two other agents who keep pushing in between. The check is meant to be
cheap and, above all, honest about what it cannot see.

History, because the false positives are the interesting part:

  * The first version reported 15 candidates, all 15 false positives. Its own
    comment-stripping state machine desynchronised on a character literal and
    blanked the rest of the file, so every real mention of the imported name
    disappeared. This version reuses the character-literal detector from
    check_braces.py verbatim, which is validated.
  * The second version had one true positive left and zero false positives on
    this tree - and then kilocode proved it wrong. It flagged
    `use std::future::Future;` in src/swarm/util_iife.rs, which the file uses
    through method resolution: Pin<&mut F>::poll(). rustc agreed with the flag
    and the build broke with E0599. Counting occurrences cannot see a trait used
    only through a method call or an await.

So the output is graded rather than binary:

  strong  - the name is not a standard-library CamelCase item, where a textual
            count is decisive.
  weak    - a CamelCase name from std/core/alloc, which is where the standard
            library traits live. Reported and labelled, to be read as "check
            this by hand", because the tool cannot tell a trait from a struct.

One case is resolved outright instead of guessed: an import of Future from a
standard-library path counts as used when the file contains .await or .poll(,
since that is the only way its methods can be reached.

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
    """The local names a single `use` statement binds, or None for a glob.

    Returns `(name, path)` pairs, because the path decides how far the report can
    be trusted: a CamelCase item from the standard library may be a trait used
    only through a method call, which no amount of counting will reveal.
    """
    names = []
    for part in tree.split(','):
        part = part.strip()
        if not part:
            continue
        if part.endswith('*'):
            return None
        if ' as ' in part:
            path, local = part.split(' as ')
            names.append((local.strip(), path.strip()))
        else:
            segments = part.split('::')
            names.append((segments[-1].strip(), '::'.join(segments[:-1]).strip()))
    return names


STD_ROOTS = ('std', 'core', 'alloc')


def is_weak(name, path):
    """A CamelCase name from the standard library: possibly a trait."""
    return name[:1].isupper() and path.split('::')[0] in STD_ROOTS


def resolved_by_method_call(name, code):
    """The one trait case we settle instead of guessing.

    `Future`'s methods are reachable only through `.await` or `.poll()`, so an
    import of it alongside either is a use, whatever the name count says. This
    is the false positive kilocode proved on `util_iife.rs`, where removing the
    import broke the build with E0599.
    """
    if name != 'Future':
        return False
    return '.await' in code or '.poll(' in code


def check(path):
    """Return (strong, weak): high-confidence hits, then trait-shaped ones."""
    raw = open(path, encoding='utf-8', errors='replace').read()
    code = blank_non_code(raw)
    strong, weak = [], []
    for m in USE.finditer(code):
        entries = imported_names(m.group(1))
        if not entries:
            continue
        line = raw[:m.start()].count('\n') + 1
        for name, import_path in entries:
            if not re.match(r'^[A-Za-z_][A-Za-z0-9_]*$', name):
                continue
            if len(re.findall(r'\b' + re.escape(name) + r'\b', code)) > 1:
                continue
            if resolved_by_method_call(name, code):
                continue
            (weak if is_weak(name, import_path) else strong).append((line, name))
    return strong, weak


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
    strong_total = weak_total = 0
    for path in paths:
        strong, weak = check(path)
        rel = os.path.relpath(path, os.path.dirname(root))
        for line, name in strong:
            strong_total += 1
            print(f'{rel}:{line}: unused import `{name}`')
        for line, name in weak:
            weak_total += 1
            print(f'{rel}:{line}: maybe unused `{name}` (std item: a trait used through a '
                  f'method call would look the same - check by hand)')
    print(f'checked {len(paths)} file(s), {strong_total} unused, {weak_total} maybe unused')
    return 0


if __name__ == '__main__':
    sys.exit(main())