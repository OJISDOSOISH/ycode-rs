//! Port of the pure helpers in `opencode/packages/core/src/fs-util.ts`.
//!
//! Not ported: the file service itself, and the three functions that need a
//! real filesystem - `normalizePath` and `resolve` call `realpathSync`, so
//! their result depends on what is on disk. What is ported is everything that
//! is pure, and one of those functions is worth more than the others.
//!
//! `contains` is the permission boundary check: `true` means `child` is inside
//! `parent`. It is built on `relative`, and the three ways the result can prove
//! the child escaped are all in one expression:
//!
//! - an ABSOLUTE relative result means the two paths share nothing, and
//!   `path.relative` produces one on Windows when the drives differ;
//! - a result of exactly `..` means the child is the parent, one level up;
//! - a result STARTING WITH `..` and a separator means somewhere above.
//!
//! The first two are easy to forget because they look redundant - `..` does
//! start with `..` - but the check is for `..` plus a SEPARATOR, so a bare `..`
//! has to be excluded separately. Getting that wrong makes every sibling
//! directory look like a child, and that is a permission bug, not a cosmetic
//! one.
//!
//! The drive-letter rewriting in `windowsPath` is the other part worth pinning.
//! Four different prefixes map to a Windows drive - `/c:/x`, `/c/x`,
//! `/cygdrive/c/x` and `/mnt/c/x` - and the uppercase matters: `/c/x` becomes
//! `C:/x`, not `C:` or `c:/x`. On a non-Windows host the function returns its
//! argument untouched, and that branch is a real branch, so the platform is a
//! parameter here rather than a compile-time constant.
//!
//! Known divergence, recorded rather than hidden: `normalizePathPattern` joins
//! with the host separator through `path.join`, so a POSIX run joins with `/`
//! and a Windows run with `\`. The trailing `*` is appended after the join in
//! both, so the pattern ends up `/dir/*` on POSIX and `\dir\*` on Windows.

use std::path::MAIN_SEPARATOR_STR;

// `MAIN_SEPARATOR` is referenced by a test only, so importing it here would be
// an unused import in the library build - the same trap as the other five I hit
// earlier. The test module imports it itself.

/// The `platform` the TS reads from `process.platform`.
///
/// A parameter rather than `cfg!`, so both branches are reachable from a test
/// on either host - the same reason `util_wildcard` takes `ignore_case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Posix,
    Windows,
}

impl Platform {
    /// The host this is running on.
    pub fn host() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Posix
        }
    }

    fn is_windows(self) -> bool {
        matches!(self, Platform::Windows)
    }
}

/// The separator `contains` compares a parent-relative result against.
///
/// Always `/`, and that is not an oversight. Node uses `path.sep`, because
/// `path.relative` there returns NATIVE separators - so on Windows its result
/// contains a backslash and comparing against `..\\` is what works. The
/// `relative` in this module always joins with `/`, on every platform, so the
/// check has to match the path it is handed: comparing a `/`-joined result
/// against `..\\` never matches, and a path that climbs out of its parent
/// would be silently reported as contained. That is a permission bug, and it
/// would only appear on Windows - which is where this crate's CI runs.
const SEP: &str = "/";

/// The native separator, for callers that build a platform path.
pub fn native_separator(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "\\",
        Platform::Posix => "/",
    }
}

/// Whether a path is ROOTED, which is a per-platform question.
///
/// Node answers it inside `path.win32.relative` and `path.posix.relative`, and
/// the two disagree on a leading backslash: `path.win32` reads `\a` as the root
/// of the current drive, while `path.posix` reads it as a relative path whose
/// first segment happens to contain a backslash.
///
/// That disagreement is not cosmetic here, because `relative` only consults the
/// root flags on one branch - the case where the two paths share no leading
/// segment - and that branch returns `to` unchanged when neither side is
/// rooted. So for `relative("\\base", "other")` the old test, which recognised
/// a root only by a leading `/` or a `:`, saw two unrooted paths and returned
/// `other`, i.e. "the path `other` is inside the root `\base`". On Windows it is
/// not: it is a sibling. That is a permission decision, and it was being made
/// by a parameter the function never read.
fn has_root(path: &str, platform: Platform) -> bool {
    if path.starts_with('/') {
        return true;
    }
    match platform {
        // A drive letter and colon, or a backslash-led root.
        Platform::Windows => path.starts_with('\\') || path.get(1..2) == Some(":"),
        // Nothing else roots a POSIX path, and a colon is an ordinary character
        // in a segment name - which is why this is not the `contains(':')` the
        // first draft used.
        Platform::Posix => false,
    }
}

/// `path.relative`, for the absolute paths this module is given.
///
/// Same shape as `config_plugin_path::relative`, which is the other place the
/// same Node function is needed; both are ports of one behaviour, kept apart
/// because one takes no platform parameter and this one does.
pub fn relative(from: &str, to: &str, platform: Platform) -> String {
    if from == to {
        return String::new();
    }
    let from_parts = segments(from);
    let to_parts = segments(to);
    let from_root = has_root(from, platform);
    let to_root = has_root(to, platform);
    let mut shared = 0usize;
    while shared < from_parts.len() && shared < to_parts.len() && from_parts[shared] == to_parts[shared] {
        shared += 1;
    }
    if shared == 0 && !from_root && !to_root {
        return to_parts.join("/");
    }
    let mut out: Vec<String> = Vec::new();
    for _ in shared..from_parts.len() {
        out.push("..".to_string());
    }
    for part in &to_parts[shared..] {
        out.push((*part).to_string());
    }
    out.join("/")
}

fn segments(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|s| !s.is_empty() && *s != ".").collect()
}

/// `path.isAbsolute`: a leading separator, or a Windows drive or UNC root.
pub fn is_absolute(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || (path.len() >= 3 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':'
            && matches!(path.as_bytes()[2], b'/' | b'\\'))
        || path.starts_with("//")
}

/// `contains`: whether `child` lies inside `parent`.
///
/// The three rejections are spelled out rather than folded into one predicate,
/// because each closes a different escape route.
pub fn contains(parent: &str, child: &str, platform: Platform) -> bool {
    let result = relative(parent, child, platform);
    if result.is_empty() {
        return true;
    }
    if is_absolute(&result) {
        return false;
    }
    if result == ".." {
        return false;
    }
    // `concat!` accepts LITERALS only, so `concat!("..", SEP)` is rejected - and the
    // headline names the wrong place. It says
    //
    //     error: expected a literal
    //        --> src/core/fs_util.rs:175:39
    //         |  !result.starts_with(concat!("..", SEP))
    //         |                                       ^^^
    //         = note: only literals (like "foo", -42 and 3.14) can be passed to concat!()
    //
    // pointing at the argument of `starts_with`, which would have been perfectly
    // happy with whatever `concat!` produced. Two rounds of diagnosis went the
    // wrong way from that headline: the first removed the `&`, reasoning that
    // `&&str` is not a `Pattern` - true, and irrelevant; the second concluded
    // that `starts_with` is a builtin macro needing a literal pattern - also
    // true of many of its siblings, and not the cause here, since
    // `starts_with(concat!("..", "/"))` compiles.
    //
    // The note is the whole answer, and it is one line further down. Read the
    // note.
    //
    // `strip_prefix` is an ordinary generic taking a `Pattern`, so the check
    // goes through it and the separator stays written down once, in `SEP`.
    let climbs_out = result.strip_prefix("..").and_then(|rest| rest.strip_prefix(SEP)).is_some();
    !climbs_out
}

/// `overlaps`: either path contains the other.
pub fn overlaps(a: &str, b: &str, platform: Platform) -> bool {
    contains(a, b, platform) || contains(b, a, platform)
}

/// `windowsPath`: the four drive-letter prefixes, uppercased.
pub fn windows_path(p: &str, platform: Platform) -> String {
    if !platform.is_windows() {
        return p.to_string();
    }
    let mut out = p.to_string();
    out = replace_drive(&out, true, |drive| format!("{}:/", drive.to_uppercase()));
    out = replace_drive(&out, false, |drive| format!("{}:/", drive.to_uppercase()));
    out = replace_prefixed_drive(&out, "/cygdrive/", |drive| format!("{}:/", drive.to_uppercase()));
    out = replace_prefixed_drive(&out, "/mnt/", |drive| format!("{}:/", drive.to_uppercase()));
    out
}

/// Rewrites `/<drive>:` and `/<drive>` at the start, when `colon` selects which.
///
/// Two details of the TypeScript regex, both of which this got wrong.
///
/// The regex CONSUMES the separator it matched. `^\/([a-zA-Z]):(?:[\\/]|$)` is
/// replaced by `C:/`, and the separator that followed the colon is part of what
/// the match removed - so `/c:/x` becomes `C:/x` and not `C://x`. Emitting the
/// built prefix and then appending the tail INCLUDING its leading separator gave
/// a doubled separator on every one of the four prefixes.
///
/// And the alternation ends in `$`, so a drive with nothing after it matches too:
/// `/c` and `/c:` are both rewritten, to `C:/`. Requiring three bytes rejected
/// `/c`, which came back untouched - a bare drive is a legitimate thing for a
/// permission root to be, so it is not an exotic input.
fn replace_drive(input: &str, colon: bool, build: impl Fn(char) -> String) -> String {
    let bytes = input.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return input.to_string();
    }
    let drive = input[1..2].chars().next().unwrap();
    if colon {
        if bytes.get(2) != Some(&b':') {
            return input.to_string();
        }
        // The character after the colon must be a separator or the end.
        if !matches!(bytes.get(3), None | Some(b'/') | Some(b'\\')) {
            return input.to_string();
        }
        // Skip the separator the match consumed, if there was one.
        let tail = input[3..].strip_prefix(['/', '\\']).unwrap_or(&input[3..]);
        return format!("{}{}", build(drive), tail);
    }
    // No colon: the next character must be a separator or the end.
    if !matches!(bytes.get(2), None | Some(b'/')) {
        return input.to_string();
    }
    let tail = input[2..].strip_prefix('/').unwrap_or(&input[2..]);
    format!("{}{}", build(drive), tail)
}

fn replace_prefixed_drive(input: &str, prefix: &str, build: impl Fn(char) -> String) -> String {
    let Some(rest) = input.strip_prefix(prefix) else {
        return input.to_string();
    };
    let mut chars = rest.chars();
    let Some(drive) = chars.next() else {
        return input.to_string();
    };
    if !drive.is_ascii_alphabetic() {
        return input.to_string();
    }
    let tail = &rest[drive.len_utf8()..];
    if !(tail.is_empty() || tail.starts_with('/')) {
        return input.to_string();
    }
    // Same as in `replace_drive`: the separator belongs to the match, not to the
    // tail, and `build` already ends with one.
    let tail = tail.strip_prefix('/').unwrap_or(tail);
    format!("{}{}", build(drive), tail)
}

/// `normalizePathPattern`: normalise a `dir/*` glob's directory.
///
/// Three things happen in the TypeScript, in this order, and the order is the
/// whole function:
///
/// ```ts
/// if (process.platform !== "win32") return p
/// if (p === "*") return p
/// const match = p.match(/^(.*)[\\/]\*$/)
/// if (!match) return normalizePath(p)
/// const dir = /^[A-Za-z]:$/.test(match[1]) ? match[1] + "\\" : match[1]
/// return join(normalizePath(dir), "*")
/// ```
///
/// The first draft of this port missed that the regex is ANCHORED at both ends.
/// It searched for the last separator anywhere in the string, so `"C:/base/dir"`
/// - which has a separator but does not end in `/*` - came back as
/// `"C:\\base\\*"`: a glob for a completely different directory, built out of a
/// path that was never a glob. Same for `"no-star"`, which has no separator at
/// all and so fell through to the join branch and grew a star it was never
/// given. Both mistakes are Windows-only, and this crate is built on
/// `windows-latest`, so they were waiting to be found by a failing test rather
/// than by a failing build.
///
/// A pattern that does not match does not get a star; it goes to
/// `normalizePath`. That is why the step is a parameter here.
pub fn normalize_path_pattern(p: &str, platform: Platform) -> String {
    normalize_path_pattern_with(p, platform, |p| p.to_string())
}

/// `normalizePathPattern` with the `normalizePath` step supplied.
///
/// `normalizePath` is not pure in the TypeScript - it is
/// `path.resolve(windowsPath(p))` followed by `realpathSync.native`, falling
/// back to the resolved path when the file does not exist - so it touches the
/// filesystem and cannot be written down here. Injecting it keeps this function
/// testable and lets a caller that does have a filesystem pass a real one. The
/// wrapper above substitutes the identity, which is exact on POSIX (where the
/// real function returns its argument) and for any path that is already
/// absolute and normalised.
pub fn normalize_path_pattern_with<F>(p: &str, platform: Platform, normalize: F) -> String
where
    F: Fn(&str) -> String,
{
    if !platform.is_windows() {
        return p.to_string();
    }
    if p == "*" {
        return p.to_string();
    }
    match trailing_glob_dir(p) {
        Some(dir) => {
            let dir = if is_drive_root(dir) { format!("{}\\", dir) } else { dir.to_string() };
            join_star(&normalize(&dir), platform)
        }
        None => normalize(p),
    }
}

/// `/^(.*)[\\/]\*$/`, keeping capture group 1.
///
/// `None` for anything that does not END with a separator and a star, which is
/// the branch that sends the TypeScript to `normalizePath`. Note that `"C:*"`
/// does not match either: the character before the star has to be a separator,
/// and there it is a colon.
fn trailing_glob_dir(p: &str) -> Option<&str> {
    p.strip_suffix('*')?.strip_suffix(['/', '\\'])
}

/// `/^[A-Za-z]:$/`
fn is_drive_root(dir: &str) -> bool {
    dir.len() == 2 && dir.as_bytes()[0].is_ascii_alphabetic() && dir.as_bytes()[1] == b':'
}

/// `join(normalizePath(dir), "*")`, with `normalizePath` elided on POSIX.
///
/// Joins with the separator of the platform it was HANDED, not the host's. An
/// API that takes an explicit platform and then ignores it answers a different
/// question than the one it was asked - and this one is how a permission glob is
/// built, so the separator is part of the rule that gets stored.
fn join_star(dir: &str, platform: Platform) -> String {
    if dir.is_empty() {
        return "*".to_string();
    }
    format!("{}{}{}", dir.trim_end_matches(['/', '\\']), native_separator(platform), "*")
}

/// The separator this host joins with, exposed for tests and callers.
pub fn host_separator() -> &'static str {
    MAIN_SEPARATOR_STR
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::MAIN_SEPARATOR;

    const W: Platform = Platform::Windows;
    const P: Platform = Platform::Posix;

    // --- relative, and what "rooted" means per platform ---

    #[test]
    fn a_leading_backslash_is_a_root_on_windows_and_a_name_on_posix() {
        // The whole reason `relative` takes a platform. `path.win32` reads `\a`
        // as the root of the current drive; `path.posix` reads it as a relative
        // path. When neither side is rooted, `relative` returns `to` unchanged -
        // so the flag decides whether `other` is inside `\a` or beside it.
        assert_eq!(relative("\\base", "other", W), "../other", "Windows roots the backslash path");
        assert_eq!(relative("\\base", "other", P), "other", "POSIX treats it as a plain name");
        assert_eq!(relative("other", "\\a", W), "../a");
    }

    #[test]
    fn a_permission_root_with_a_leading_backslash_does_not_contain_a_sibling() {
        // The consequence of the test above, stated as the permission decision
        // it actually is. Before the platform was read, this returned true: the
        // root flags said both paths were unrooted, so `relative` answered
        // `other` and `contains` read that as containment.
        assert!(!contains("\\base", "other", W), "a Windows root does not contain a bare sibling");
        assert!(!contains("\\base", "sub/file", W));
        assert!(!contains("C:/base", "other", W), "a drive root does not either");
        // A rooted child is still contained, on either platform.
        assert!(contains("\\base", "\\base/sub", W));
        assert!(contains("/base", "/base/sub", P));
    }

    #[test]
    fn a_colon_is_an_ordinary_character_in_a_posix_segment() {
        // Why `has_root` does not look for a colon on POSIX: `weird:name` is a
        // legal relative path there, and the first draft treated it as rooted.
        // `a:b` and `a:c` are two different single-segment NAMES, not a parent
        // and a child: neither is rooted, they share no leading segment, so
        // `relative` returns the target unchanged. A bare relative name is not
        // inside an absolute root either - `contains` is asked about `/base`
        // here, and `weird:name` climbs out of it.
        assert_eq!(relative("a:b", "a:c", P), "a:c");
        assert!(!contains("/base", "weird:name", P), "a bare name is not inside /base");
        // The contrast that shows the colon is what stopped it rooting: with a
        // drive-shaped second segment the Windows rule applies and it IS rooted.
        assert_eq!(relative("x", "C:b", W), "../C:b");
        assert_eq!(relative("x", "C:b", P), "C:b", "POSIX reads the colon as part of the name");
    }

    #[test]
    fn the_common_shapes_are_unaffected_by_the_root_change() {
        // Everything the first draft got right, restated, so a future edit to
        // `has_root` cannot quietly pass by breaking only the exotic cases.
        assert_eq!(relative("/base", "/base/a", P), "a");
        assert_eq!(relative("/base", "/other", P), "../other");
        assert_eq!(relative("/base", "/base/../x", P), "../x");
        assert_eq!(relative("C:/a", "C:/a/b", W), "b");
        assert_eq!(relative("C:/a", "C:/b", W), "../b");
        assert_eq!(relative("/base", "/base", P), "");
        assert_eq!(relative("a/b", "c/d", P), "c/d");
    }

    // --- windowsPath ---

    #[test]
    fn a_posix_host_returns_the_path_untouched() {
        assert_eq!(windows_path("/c:/x", P), "/c:/x");
        assert_eq!(windows_path("/mnt/c/x", P), "/mnt/c/x");
        assert_eq!(windows_path("/already/posix", P), "/already/posix");
    }

    #[test]
    fn all_four_drive_prefixes_become_the_same_uppercase_drive() {
        // These four are the only prefixes the TypeScript rewrites, and each one
        // CONSUMES the separator it matched - which is why none of them may come
        // back with a doubled slash. Every expectation below was checked against
        // the four regexes themselves, not against my reading of them: the first
        // draft emitted the built prefix and then appended a tail that still
        // began with the separator, and CI caught it as `C://x`.
        assert_eq!(windows_path("/c:/x", W), "C:/x");
        assert_eq!(windows_path("/c/x", W), "C:/x");
        assert_eq!(windows_path("/cygdrive/c/x", W), "C:/x");
        assert_eq!(windows_path("/mnt/c/x", W), "C:/x");
    }

    #[test]
    fn a_bare_drive_and_a_trailing_separator_both_work() {
        // The alternation in each regex ends in `$`, so a drive with nothing
        // after it matches too. Requiring three bytes rejected `/c` outright,
        // which matters because a bare drive is a perfectly ordinary thing for a
        // permission root to be.
        assert_eq!(windows_path("/c", W), "C:/");
        assert_eq!(windows_path("/c:", W), "C:/");
        assert_eq!(windows_path("/z", W), "Z:/");
        assert_eq!(windows_path("/cygdrive/c", W), "C:/");
        assert_eq!(windows_path("/mnt/c", W), "C:/");
        assert_eq!(windows_path("/c/", W), "C:/", "a separator already there is consumed too");
    }

    #[test]
    fn only_the_matched_prefix_is_rewritten() {
        assert_eq!(windows_path("/mnt/z/x/y", W), "Z:/x/y", "the tail is kept");
        assert_eq!(windows_path("/cygdrive/c/deep/path", W), "C:/deep/path");
        assert_eq!(windows_path("/cat", W), "/cat", "a directory named cat, not the C drive");
        assert_eq!(windows_path("/c:x", W), "/c:x", "a colon that is not followed by a separator");
        assert_eq!(windows_path("/1:/x", W), "/1:/x", "a digit is not a drive letter");
        assert_eq!(windows_path("/cygdrive/1/x", W), "/cygdrive/1/x");
        assert_eq!(windows_path("/mnt/", W), "/mnt/", "no drive letter after the prefix");
        assert_eq!(windows_path("C:/already", W), "C:/already", "already a Windows path");
        assert_eq!(windows_path("", W), "");
        assert_eq!(windows_path("/", W), "/");
        assert_eq!(windows_path("relative/path", W), "relative/path");
    }

    #[test]
    fn the_drive_letter_is_uppercased() {
        assert_eq!(windows_path("/z:/x", W), "Z:/x");
        assert_eq!(windows_path("/Q/a", W), "Q:/a");
    }

    #[test]
    fn a_drive_that_is_not_followed_by_a_separator_is_left_alone() {
        // `/cat` is a directory named `cat`, not the C: drive.
        assert_eq!(windows_path("/cat", W), "/cat");
        assert_eq!(windows_path("/c:x", W), "/c:x");
    }

    #[test]
    fn a_non_letter_first_character_is_left_alone() {
        assert_eq!(windows_path("/1:/x", W), "/1:/x");
        assert_eq!(windows_path("/cygdrive/1/x", W), "/cygdrive/1/x");
    }

    // --- contains ---

    #[test]
    fn a_child_inside_its_parent_is_contained() {
        assert!(contains("/base", "/base/a", P));
        assert!(contains("/base", "/base/a/b/c", P));
    }

    #[test]
    fn a_directory_contains_itself() {
        assert!(contains("/base", "/base", P), "an empty relative result is containment");
    }

    #[test]
    fn a_sibling_is_not_contained() {
        assert!(!contains("/base", "/other", P));
        assert!(!contains("/base", "/baseother", P));
    }

    #[test]
    fn a_parent_is_not_contained_by_its_child() {
        assert!(!contains("/base/a", "/base", P));
    }

    #[test]
    fn climbing_out_is_never_contained() {
        assert!(!contains("/base", "/base/../etc", P));
        assert!(!contains("/a/b", "/a/../c", P));
    }

    #[test]
    fn a_bare_parent_relative_result_is_rejected_separately() {
        // This is the case a single `starts_with("..")` test gets wrong: ".."
        // has no separator after it, so it must be excluded on its own.
        assert_eq!(relative("/base/a", "/base", P), "..");
        assert!(!contains("/base/a", "/base", P));
    }

    #[test]
    fn a_sibling_with_a_common_prefix_is_not_contained() {
        assert!(!contains("/base", "/base-other/x", P));
    }

    #[test]
    fn the_parent_test_works_on_both_platforms() {
        // The separator compared against is ALWAYS `/` here, because this
        // module's `relative` always joins with `/`. Node compares against
        // `path.sep` because its `relative` returns native separators; porting
        // that comparison without porting that property would make every
        // climbing path look contained on Windows - which is where this crate
        // is tested, so the bug would be caught rather than shipped, but it
        // would be a real one.
        assert!(!contains("C:/base", "C:/../x", W), "a Windows climbing path is refused");
        assert!(!contains("/base", "/base/../x", P));
        assert!(contains("C:/base", "C:/base/x", W));
        assert!(contains("/base", "/base/x", P));
    }

    #[test]
    fn is_absolute_recognises_both_shapes() {
        assert!(is_absolute("/a"));
        assert!(is_absolute("C:/a"));
        assert!(is_absolute("C:\\a"));
        assert!(is_absolute("//server/share"));
        assert!(!is_absolute("a/b"));
        assert!(!is_absolute("a:b"), "a colon alone is not a drive");
    }

    // --- overlaps ---

    #[test]
    fn overlaps_is_symmetric_and_covers_both_directions() {
        assert!(overlaps("/base", "/base/a", P), "a contains b");
        assert!(overlaps("/base/a", "/base", P), "and b contains a");
        assert!(overlaps("/base", "/base", P));
    }

    #[test]
    fn two_disjoint_trees_do_not_overlap() {
        assert!(!overlaps("/a", "/b", P));
        assert!(!overlaps("/a/b", "/a/c", P));
    }

    #[test]
    fn two_siblings_do_not_overlap_either() {
        assert!(!overlaps("/a/b", "/a/c", P), "a common parent is not containment");
    }

    // --- normalizePathPattern ---

    // The TypeScript, for reference, because every expectation below is a
    // reading of these five lines rather than a guess about them:
    //
    //   if (process.platform !== "win32") return p
    //   if (p === "*") return p
    //   const match = p.match(/^(.*)[\\/]\*$/)
    //   if (!match) return normalizePath(p)
    //   const dir = /^[A-Za-z]:$/.test(match[1]) ? match[1] + "\\" : match[1]
    //   return join(normalizePath(dir), "*")

    #[test]
    fn a_posix_platform_normalises_nothing() {
        // The first branch returns the argument untouched - not "the directory
        // with a star", which is what this port used to do.
        assert_eq!(normalize_path_pattern("C:/base/*", P), "C:/base/*");
        assert_eq!(normalize_path_pattern("*", P), "*");
        assert_eq!(normalize_path_pattern("no-star", P), "no-star");
        assert_eq!(normalize_path_pattern("", P), "");
        assert_eq!(
            normalize_path_pattern("C:/base/dir", P),
            "C:/base/dir",
            "a directory with a separator in it is still left alone on POSIX"
        );
    }

    #[test]
    fn a_star_alone_is_returned_unchanged() {
        assert_eq!(normalize_path_pattern("*", W), "*");
    }

    #[test]
    fn a_glob_keeps_its_directory_and_gains_a_windows_separator() {
        // Exact assertions rather than `ends_with`, because the separator is
        // part of the permission glob that gets stored and the answer must not
        // depend on which host runs the test. Joining with the HOST separator
        // made the POSIX expectation in the test above fail on `windows-latest`,
        // which is where this crate is built.
        //
        // Note what is NOT here: the forward slashes in the directory survive.
        // This function does not rewrite separators - that is `windowsPath`, and
        // it runs inside the `normalizePath` step this port injects. The only
        // separator it chooses itself is the one it joins before the star.
        assert_eq!(normalize_path_pattern("C:/base/dir/*", W), "C:/base/dir\\*");
        assert_eq!(normalize_path_pattern("C:/base/dir\\*", W), "C:/base/dir\\*");
        assert_eq!(normalize_path_pattern("/base/*", W), "/base\\*");
    }

    #[test]
    fn a_string_that_does_not_end_in_a_glob_is_not_turned_into_one() {
        // The anchored regex is the whole point. Each of these has a separator
        // somewhere, or none at all, and the first draft of this port built a
        // `dir/*` glob out of every one of them - including a glob for a
        // directory the caller never named.
        assert_eq!(normalize_path_pattern("C:/base/dir", W), "C:/base/dir");
        assert_eq!(normalize_path_pattern("no-star", W), "no-star");
        assert_eq!(normalize_path_pattern("", W), "");
        assert_eq!(normalize_path_pattern("C:*", W), "C:*", "a colon is not a separator");
        assert_eq!(
            normalize_path_pattern("**", W),
            "**",
            "a star before the star is not a separator, so nothing is stripped"
        );
    }

    #[test]
    fn a_drive_root_directory_keeps_its_separator() {
        // `C:/*` has the directory `C:`, which matches `/^[A-Za-z]:$/`, so the
        // TS appends a backslash before normalising - otherwise joining would
        // produce `C:/*`, a different path.
        assert_eq!(normalize_path_pattern("C:/*", W), "C:\\*");
        assert_eq!(normalize_path_pattern("z:/*", W), "z:\\*");
        // The same shape with a leading slash is NOT a drive root, so it keeps
        // its own separator instead of gaining one.
        assert_eq!(normalize_path_pattern("/c:/*", W), "/c:\\*");
    }

    #[test]
    fn the_normalize_step_is_the_caller_s_to_supply() {
        // `normalizePath` calls `realpathSync.native` in the TypeScript, so it
        // cannot be written down here. It is a parameter instead, which makes
        // the branch observable: a caller that resolves the path sees its
        // result in both places the TS calls it.
        let shout = |p: &str| p.to_uppercase();
        assert_eq!(normalize_path_pattern_with("no-star", W, shout), "NO-STAR");
        assert_eq!(normalize_path_pattern_with("c:/base/*", W, shout), "C:/BASE\\*");
        // And it is not consulted at all on POSIX, or for a bare star.
        assert_eq!(normalize_path_pattern_with("c:/base/*", P, shout), "c:/base/*");
        assert_eq!(normalize_path_pattern_with("*", W, shout), "*");
    }


    // --- platform ---

    #[test]
    fn the_host_reports_itself_consistently() {
        let host = Platform::host();
        assert_eq!(host.is_windows(), cfg!(target_os = "windows"));
        assert!(!Platform::Posix.is_windows());
        assert!(Platform::Windows.is_windows());
    }

    #[test]
    fn the_separator_helpers_follow_the_platform() {
        assert_eq!(native_separator(W), "\\");
        assert_eq!(native_separator(P), "/");
        assert_eq!(SEP, "/", "the parent check always compares against a slash");
        assert!(MAIN_SEPARATOR == '/' || MAIN_SEPARATOR == '\\');
        assert!(!host_separator().is_empty());
    }
}