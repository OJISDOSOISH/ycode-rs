"""Flag literal-only macros that have been handed a name.

THE CASE THAT BUILT THIS, and it cost two wrong diagnoses from two people.

    error: expected a literal
       --> src/core/fs_util.rs:175:39
        |
    175 |     !result.starts_with(concat!("..", SEP))
        |                                       ^^^
        |
        = note: only literals (like `"foo"`, `-42` and `3.14`) can be passed to `concat!()`

The headline blames the argument of `starts_with`, and the caret points at a
token three levels inside a nested macro call. Two rounds of reasoning went from
there in the wrong direction:

  - `&&str` is not a `Pattern`, so the `&` was removed. True, and irrelevant;
    the `&` was never the problem.
  - `str::starts_with` is a builtin macro whose pattern must be a literal, so
    `concat!` cannot appear there. True of `starts_with`, and not the cause:
    `starts_with(concat!("..", "/"))` compiles. The real rule is one level down,
    in the note, and it is about `concat!` rather than about `starts_with`.

So the class worth checking is `concat!` and friends being given a NAME, not
string methods being given a non-literal.

WHICH MACROS, and the exclusions are the interesting part.

Taken:   concat! stringify! env! option_env! include_str! include_bytes!
         include! compile_error!
Excluded, each for a reason:
  - `cfg!` and `format_args!` take a PREDICATE or a token tree, so `cfg!(windows)`
    is legal. Flagging it would be the same error as the two wrong diagnoses
    above: a correct-sounding rule pointed at the wrong node. A first draft of
    this file included them and reported sixteen sites, all of them fine.
  - `line!`, `file!`, `column!`, `module_path!` take NO arguments.
  - `format!`, `panic!`, `assert!`, `vec!`, `write!` are ordinary macros and take
    any expression. They belong to a different question, one this file does not
    answer.

TEN FALSE POSITIVES PAID FOR ON THE WAY, recorded because they are the same
shape as the two wrong diagnoses above - each was a plausible rule aimed at the
wrong node:

  1. `cfg!(windows)` reported sixteen times. `cfg!` takes a predicate, not a
     literal. Removed from the list.
  2. The whole argument list classified as ONE argument, so `concat!("..", SEP)`
     graded "unrecognisable" instead of naming `SEP`. `concat!` almost always
     has several arguments; they are now split on top-level commas.
  3. A multi-line `concat!` of raw strings graded "unrecognisable", because the
     literal pattern did not allow a newline between arguments.
  4. `r#"..."#` not recognised as a literal at all, because the closing delimiter
     is `"#` and the pattern expected a bare `"`. Fixed with a backreference on
     the hash count, and `r##"..."##` too - which matters here because a raw
     string in this tree contains `"#` inside it, which is the reason the hashes
     are there.
  5. Two bugs found by testing the pattern IN ISOLATION rather than by reading it,
     which is the only reason they were found at all:
       - `(?:b)'` means "the letter b, then a quote", not "optionally b", so a
         plain char literal `'c'` matched nothing;
       - `(?:(?!\1).)*` with an EMPTY backreference is `(?!)`, a negative
         lookahead of the empty pattern, which fails at every position. So
         `r"..."` and `br"..."`, which carry no hashes, matched nothing. The
         hashed form needs `#` at least once and a different inner guard; the
         unhashed form needs its own alternative;
     - and the closing delimiter of a raw string was built as quote + hashes +
         quote, THREE characters, where `r#"..."#` closes on quote-then-hash -
         TWO. `find` returned -1, the scan ran to the end of the argument list,
         and every raw string came back as one unclassifiable blob. All three
         were invisible to reading the code and obvious to running it.

  6. comment stripping by REGEX, which cannot tell a comment from the `//` of a
     URL. Five of the tool descriptions in this tree go through `concat!` and
     contain `https://...`, so the strip ate the remainder of the string and left
     it unterminated - five false positives, all in real code. The stripper now
     walks the text the way a lexer does;
  7. and the line numbers those five reported were counted on the STRIPPED text,
     which is why they all pointed at line 4 or 13 - near the top of the file,
     nowhere near the code. Blanking comments to spaces instead of deleting them
     keeps offsets valid, so the line numbers are now right.

The shape of all seven, and of the two wrong diagnoses before them: each was a
rule that sounded right, aimed at a node that was not the problem. The only thing
that separated the working version from the broken ones was running it against a
seed with the expected answer written down first.
  8. and the one that made this tool useless on real Rust: `'static` and `'a`
     are LIFETIMES, not character literals. The scanner took the quote as an
     opener and ran to the next quote ANYWHERE in the file, crossing newlines
     and whole comments - so every line comment after the first `+ 'static` came
     back unblanked. On `src/core/fs_util.rs` that is most of the file, and the
     result was two hits inside a comment that was quoting the very error this
     tool exists to catch. Now a quote is only a string opener when a character
     literal actually matches there; otherwise nothing is consumed.
  9. and the SAME lifetime trap a second time, in `split_args` rather than in
     `strip_comments`. `skill's` - an apostrophe inside a word, in one of my own
     tool descriptions - put the argument splitter into "inside a string" mode
     until the next apostrophe in the file, which swallowed four multi-line
     `concat!` calls whole and reported each with an empty argument name. Fixing
     one function and not the other is the same mistake twice: the shape test
     belongs to the lexer, not to each caller that happens to scan strings.
 10. and the trailing comma of a multi-line `concat!` leaves the whitespace
     after it as a final "argument", which is not a literal - so all four
     multi-line `concat!` calls in this tree reported an EMPTY argument name.
     That reads like a parse failure rather than the trailing newline it is,
     which is why it survived two rounds of "the real tree is clean".

WHAT IT CANNOT SEE, stated plainly:

  - it does not resolve names. A `const &str` really is rejected by `concat!`,
    and so is a `static` and so is a local variable; none of the three are
    distinguishable without type information, so all are reported alike;
  - it does not know a name may be shadowed by a local `macro_rules!`, which
    would make a report wrong;
  - it strips line and block comments, so a commented-out call is not reported -
    but a `concat!` inside a STRING LITERAL is invisible to it in the other
    direction, and that one really is an error;
  - it is a text scanner, so an unbalanced bracket inside a string it failed to
    recognise can make it read the wrong argument list.

usage: python check_literal_macros.py [paths...]
"""

import os
import re
import sys

LITERAL_ONLY = [
    'concat', 'stringify', 'env', 'option_env',
    'include_str', 'include_bytes', 'include', 'compile_error',
]

CALL = re.compile(r'(?<![\w:!])(' + '|'.join(LITERAL_ONLY) + r')!\s*\(')

# `r"`, `r#"`, `r##"`, and the byte forms. Group 2 is the hash run, or None for
# a plain `r"`.
CHAR_LITERAL = re.compile(r"'(?:\\.|[^\\'])'", re.S)

RAW_OPEN = re.compile(r'(?:b?r)(#*)"')

# Optional whitespace lives OUTSIDE the alternatives. An earlier version led
# with a `[-+]?\s*` alternative that can match the empty string, and an
# empty-matching alternative inside a `+` group made the regex give up before it
# reached the alternatives that can consume a character literal - so `concat!("a",
# 'c', 1)` reported the `'c'`.
LITERAL = re.compile(r'^\s*(?:\s*(?:'
                      r'[-+]'
                      r'|(?:b?r)(\#+)"(?:(?!\1").)*"\1'
                      r'|(?:b)r"(?:[^"])*"'
                      r'|(?:b)?"(?:[^"\\]|\\.)*"'
                      r"|b?'(?:[^'\\]|\\.)*'"
                      r'|-?\d[\d_]*(?:\.\d+)?(?:[iuf](?:8|16|32|64|128|size))?'
                      r'|true|false'
                      r')\s*)+\s*$', re.X)

NESTED = re.compile(r'^\s*(' + '|'.join(LITERAL_ONLY) + r')!\s*\(')

NAME = re.compile(r'^\s*(?:&|&&)?\s*(?:mut\s+)?'
                  r'(?:[A-Za-z_][A-Za-z0-9_]*::)*'
                  r'[A-Za-z_][A-Za-z0-9_]*'
                  r'\s*(?:\([^()]*\))?\s*$', re.S)


def strip_comments(code):
    """Blank out comments, preserving every byte offset and every newline.

    A REGEX cannot do this. `re.sub(r'//[^\n]*', '', code)` destroys the `//`
    inside a string literal, and in this tree that is not hypothetical: the tool
    descriptions handed to `concat!` are full of `https://...`, so stripping
    "comments" ate the rest of the string and left it unterminated. Five false
    positives, every one in real code, none visible to reading the checker.

    So this walks the text the way a lexer does, tracking normal strings, char
    literals and raw strings. Comment bodies become spaces and newlines are kept,
    so an offset into the result is also an offset into the original and line
    numbers counted on the original are right. Getting that wrong was the seventh
    bug: those five all reported line 4 or 13, near the top of the file and
    nowhere near the code, because the line was counted on the STRIPPED text.
    """
    out = list(code)
    i, n = 0, len(code)
    while i < n:
        m = RAW_OPEN.match(code, i)
        if m:
            close = '"' + (m.group(1) or '')
            end = code.find(close, m.end())
            i = n if end == -1 else end + len(close)
            continue
        ch = code[i]
        if ch == '"':
            j = i + 1
            while j < n and code[j] != '"':
                j += 2 if code[j] == '\\' else 1
            i = j + 1
            continue
        if ch == "'":
            # A LIFETIME, not a character literal. `'static` and `'a` are the
            # common case in Rust and they are not strings: treating the quote as
            # an opener made the scan run to the next quote anywhere in the file,
            # crossing newlines and whole comments, so every line comment after
            # the first `+ 'static` came back unblanked. Decide by shape - a
            # character literal is a quote, ONE character or an escape, then a
            # quote - and if that does not match, consume nothing.
            m2 = CHAR_LITERAL.match(code, i)
            i = m2.end() if m2 else i + 1
            continue
        if ch == '/' and i + 1 < n and code[i + 1] == '/':
            while i < n and code[i] != '\n':
                out[i] = ' '
                i += 1
            continue
        if ch == '/' and i + 1 < n and code[i + 1] == '*':
            depth = 1
            out[i] = out[i + 1] = ' '
            i += 2
            while i < n and depth:
                if code.startswith('/*', i):
                    depth += 1
                    out[i] = out[i + 1] = ' '
                    i += 2
                elif code.startswith('*/', i):
                    depth -= 1
                    out[i] = out[i + 1] = ' '
                    i += 2
                else:
                    if code[i] != '\n':
                        out[i] = ' '
                    i += 1
            continue
        i += 1
    return ''.join(out)


def split_args(text):
    """Split a macro argument list on its TOP-LEVEL commas.

    String, char and raw-string literals are tracked so a comma inside one is
    not a separator. Raw strings need their own case: `r#"..."#` closes at a
    quote followed by the same number of hashes, and the body in this tree
    contains `"#` itself, which is the whole reason the hashes are there.
    """
    args, depth, cur = [], 0, []
    i, n = 0, len(text)
    while i < n:
        m = RAW_OPEN.match(text, i)
        if m:
            # `r#"..."#` closes on quote-then-hash, so the closing delimiter is a
            # quote followed by the SAME hash run - two characters for one hash.
            # Building it as quote + hashes + quote made it three characters,
            # `find` returned -1, and the whole rest of the argument list came
            # back as one blob that then classified as "not a literal".
            close = '"' + (m.group(1) or '')
            end = text.find(close, m.end())
            end = n if end == -1 else end + len(close)
            cur.append(text[i:end])
            i = end
            continue
        ch = text[i]
        if ch == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == '\\' else 1
            cur.append(text[i:min(j + 1, n)])
            i = j + 1
            continue
        if ch == "'":
            # Same lifetime trap as in `strip_comments`: an apostrophe inside a
            # word - `skill's` - is not a character literal, and treating it as
            # one ran the scan to the next apostrophe anywhere in the file. A
            # character literal matches by shape; when it does not, consume
            # nothing.
            m2 = CHAR_LITERAL.match(text, i)
            if m2:
                cur.append(text[i:m2.end()])
                i = m2.end()
            else:
                cur.append(ch)
                i += 1
            continue
        if ch in '([{':
            depth += 1
        elif ch in ')]}':
            depth -= 1
        elif ch == ',' and depth == 0:
            args.append(''.join(cur))
            cur = []
            i += 1
            continue
        cur.append(ch)
        i += 1
    args.append(''.join(cur))
    return args


def classify(arg):
    """(grade, name, reason) or None when the argument is plainly a literal."""
    if NESTED.match(arg):
        return None
    if LITERAL.match(arg):
        return None
    if NAME.match(arg):
        return ('strong', arg.strip().replace('\n', ' ')[:40],
                'these macros take literals, not names')
    return ('weak', arg.strip().replace('\n', ' ')[:40],
            'not a recognisable literal; read by hand')


def check(path):
    raw = open(path, encoding='utf-8').read()
    code = strip_comments(raw)
    strong, weak = [], []
    for m in CALL.finditer(code):
        start = m.end()
        depth, i = 1, m.end()
        while i < len(code) and depth:
            if code[i] == '(':
                depth += 1
            elif code[i] == ')':
                depth -= 1
            i += 1
        line = raw[:m.start()].count('\n') + 1
        for arg in split_args(code[start:i - 1]):
            # A trailing comma leaves the whitespace after it as a final part,
            # and whitespace is not a literal. Every multi-line `concat!` in this
            # tree ends that way, so without this the tool reported all four of
            # them with an EMPTY argument name - which reads like a parse failure
            # rather than the trailing newline it is.
            if not arg.strip():
                continue
            found = classify(arg)
            if found is None:
                continue
            grade, name, reason = found
            (weak if grade == 'weak' else strong).append((line, m.group(1), name, reason))
            break
    return strong, weak


def walk(paths):
    for p in paths:
        if os.path.isdir(p):
            for root, _, names in os.walk(p):
                for n in sorted(names):
                    if n.endswith('.rs'):
                        yield os.path.join(root, n)
        elif p.endswith('.rs'):
            yield p


def main():
    roots = sys.argv[1:] or ['src']
    ts = tw = files = 0
    for path in walk(roots):
        files += 1
        strong, weak = check(path)
        for line, macro, name, reason in strong:
            print(f'{path}:{line}: {macro}!({name}) - {reason}')
            ts += 1
        for line, macro, name, reason in weak:
            print(f'{path}:{line}: maybe {macro}!({name}) - {reason}')
            tw += 1
    print(f'checked {files} file(s), {ts} strong, {tw} maybe')


if __name__ == '__main__':
    main()