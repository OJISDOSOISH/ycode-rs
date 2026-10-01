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

use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

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

/// The separator `contains` uses when rejecting a parent-relative result.
fn sep(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "\\",
        Platform::Posix => "/",
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
    let from_root = from.starts_with('/') || from.contains(':');
    let to_root = to.starts_with('/') || to.contains(':');
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
    !result.starts_with(&format!("..{}", sep(platform)))
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
fn replace_drive(input: &str, colon: bool, build: impl Fn(char) -> String) -> String {
    let bytes = input.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return input.to_string();
    }
    if colon {
        if bytes[2] != b':' {
            return input.to_string();
        }
        // The character after the colon must be a separator or the end.
        let after = bytes.get(3);
        if !(after.is_none() || matches!(after, Some(b'/') | Some(b'\\'))) {
            return input.to_string();
        }
        let drive = input[1..2].chars().next().unwrap();
        return format!("{}{}", build(drive), &input[3..]);
    }
    // No colon: the next character must be a separator or the end.
    if !matches!(bytes.get(2), None | Some(b'/')) {
        return input.to_string();
    }
    let drive = input[1..2].chars().next().unwrap();
    format!("{}{}", build(drive), &input[2..])
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
    format!("{}{}", build(drive), tail)
}

/// `normalizePathPattern`: a `dir/*` glob whose directory is normalised.
///
/// On a POSIX host `normalizePath` returns its argument, so this reduces to
/// joining the directory with `*`. The drive-root case is in the TS and worth
/// keeping: a pattern of `/c:*` has a directory of `/c:`, and joining that with
/// a separator would produce `/c:/*`, which is a different path.
pub fn normalize_path_pattern(p: &str, platform: Platform) -> String {
    if !platform.is_windows() {
        return p.to_string();
    }
    if p == "*" {
        return p.to_string();
    }
    let Some(slash) = p.rfind(['/', '\\']) else {
        return join_star(p, platform);
    };
    let (dir, _) = p.split_at(slash);
    let dir = if is_drive_root(dir) { format!("{}\\", dir) } else { dir.to_string() };
    join_star(&dir, platform)
}

/// `/^[A-Za-z]:$/`
fn is_drive_root(dir: &str) -> bool {
    dir.len() == 2 && dir.as_bytes()[0].is_ascii_alphabetic() && dir.as_bytes()[1] == b':'
}

/// `join(normalizePath(dir), "*")`, with `normalizePath` elided on POSIX.
fn join_star(dir: &str, platform: Platform) -> String {
    if dir.is_empty() {
        return "*".to_string();
    }
    format!("{}{}{}", dir.trim_end_matches(['/', '\\']), MAIN_SEPARATOR_STR, "*")
}

/// The separator this host joins with, exposed for tests and callers.
pub fn host_separator() -> &'static str {
    MAIN_SEPARATOR_STR
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Platform = Platform::Windows;
    const P: Platform = Platform::Posix;

    // --- windowsPath ---

    #[test]
    fn a_posix_host_returns_the_path_untouched() {
        assert_eq!(windows_path("/c:/x", P), "/c:/x");
        assert_eq!(windows_path("/mnt/c/x", P), "/mnt/c/x");
        assert_eq!(windows_path("/already/posix", P), "/already/posix");
    }

    #[test]
    fn all_four_drive_prefixes_become_the_same_uppercase_drive() {
        assert_eq!(windows_path("/c:/x", W), "C:/x");
        assert_eq!(windows_path("/c/x", W), "C:/x");
        assert_eq!(windows_path("/cygdrive/c/x", W), "C:/x");
        assert_eq!(windows_path("/mnt/c/x", W), "C:/x");
    }

    #[test]
    fn the_drive_letter_is_uppercased() {
        assert_eq!(windows_path("/z:/x", W), "Z:/x");
        assert_eq!(windows_path("/Q/a", W), "Q:/a");
    }

    #[test]
    fn a_bare_drive_and_a_trailing_separator_both_work() {
        assert_eq!(windows_path("/c:", W), "C:/");
        assert_eq!(windows_path("/c", W), "C:/");
        assert_eq!(windows_path("/c/", W), "C:/");
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
    fn the_windows_separator_is_used_for_the_parent_test() {
        // A result of "..\\x" must be rejected on Windows and "..x" must not
        // be, because the separator is what makes it a parent reference.
        assert!(!contains("C:/base", "C:/../x", W));
        assert!(contains("C:/base", "C:/base/x", W));
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

    #[test]
    fn a_posix_host_normalises_nothing() {
        assert_eq!(normalize_path_pattern("C:/base/*", P), "C:/base/*");
        assert_eq!(normalize_path_pattern("*", P), "*");
    }

    #[test]
    fn a_star_alone_is_returned_unchanged() {
        assert_eq!(normalize_path_pattern("*", W), "*");
    }

    #[test]
    fn a_drive_root_directory_keeps_its_separator() {
        // `/c:*` has the directory `/c:`; joining with a separator would make
        // `/c:/*`, a different path.
        let out = normalize_path_pattern("/c:*", W);
        assert!(out.ends_with("*"));
        assert!(!out.starts_with("/c:"), "got {}", out);
    }

    #[test]
    fn a_pattern_keeps_its_directory_and_its_star() {
        let out = normalize_path_pattern("C:/base/dir/*", W);
        assert!(out.starts_with("C:/base/dir"), "got {}", out);
        assert!(out.ends_with('*'));
    }

    #[test]
    fn a_pattern_without_a_directory_is_just_the_star() {
        assert_eq!(normalize_path_pattern("*", W), "*");
        assert!(normalize_path_pattern("no-star").ends_with('*'));
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
    fn the_separator_helper_follows_the_platform() {
        assert_eq!(sep(W), "\\");
        assert_eq!(sep(P), "/");
        assert!(MAIN_SEPARATOR == '/' || MAIN_SEPARATOR == '\\');
        assert!(!host_separator().is_empty());
    }
}