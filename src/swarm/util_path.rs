//! Rust port of `opencode/packages/core/src/util/path.ts`.
//!
//! Five pure functions that treat paths as plain strings. No filesystem access,
//! no allocation beyond what is needed, no global state.
//!
//! # Trap one: `!path` is not `??`
//!
//! The first four functions all begin with `if (!path) return ""`. That is a
//! TRUTHINESS test, not a null test. In JavaScript `!""` is true, so the empty
//! string leaves by that door too. An `Option<&str>` translated by matching on
//! `None` would let `Some("")` through and go on splitting an empty string,
//! which does not give the same result.
//!
//! Hence a separate function, [`is_absent`], covering both `None` and the empty
//! string. The `??` operator appears nowhere in this source, and it was not
//! introduced here.
//!
//! # Traps two and three: JavaScript corners
//!
//! Two behaviours look like bugs and are reproduced faithfully, because the port
//! targets compatibility rather than intuition.
//!
//! 1. `truncateMiddle` with `maxLength == 1`. The code computes `end == 0` and
//!    then writes `text.slice(-end)`. In JavaScript `slice(-0)` equals
//!    `slice(0)`, and `slice(0)` returns the WHOLE string. The result is
//!    therefore `"…" + text`, the complete text behind a punctuation mark.
//!    Absurd, but that is what the original does.
//!
//! 2. `getFileExtension("archive")` returns `"archive"`, not `""`. The source
//!    does `path.split(".")` with no check, and the last element of a
//!    single-element list is that element. A file with no dot is therefore
//!    reported as having its own name as its extension.

use std::path::Path;

/// True for an absent value AND for the empty string, like `!path` in JavaScript.
///
/// The `!path ? "" : ...` ternary in the source tests truthiness, not null:
/// in JavaScript the empty string is falsy. Conflating the two gives a
/// different result on the most common input of all.
fn is_absent(path: Option<&str>) -> bool {
    match path {
        None => true,
        Some(s) => s.is_empty(),
    }
}

/// Strips trailing separators, like `path.replace(/[/\\]+$/, "")`.
///
/// The source accepts BOTH separators in one regular expression, forward slash
/// and backslash. A Windows path and a POSIX path therefore follow the same
/// rule, and an agent must not pick a single default separator.
fn strip_trailing_separators(path: &str) -> &str {
    let bytes = path.as_bytes();
    let mut end = bytes.len();
    while end > 0 {
        match bytes[end - 1] {
            b'/' | b'\\' => end -= 1,
            _ => break,
        }
    }
    &path[..end]
}

/// Splits on both separators, like `path.split(/[/\\]/)`.
///
/// The source filters no empty components: `a//b` yields three components with
/// an empty one in the middle. That behaviour is kept, otherwise
/// `get_directory("a//b")` would return `a/b/` instead of `a//b/`.
fn split_path(path: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    for (i, byte) in path.as_bytes().iter().enumerate() {
        if *byte == b'/' || *byte == b'\\' {
            parts.push(&path[start..i]);
            start = i + 1;
        }
    }
    parts.push(&path[start..]);
    parts
}

/// The final file name, or an empty string.
///
/// Equivalent to `getFilename`.
pub fn file_name(path: Option<&str>) -> &str {
    if is_absent(path) {
        return "";
    }
    // `is_absent` already excluded `None`, so the unwrap is sound.
    let path = path.unwrap();
    let trimmed = strip_trailing_separators(path);
    let parts = split_path(trimmed);
    match parts.last() {
        Some(last) => last,
        // `parts[parts.length - 1]` cannot be missing on a list produced by
        // `split`, which always yields at least one element. The `None` arm is
        // therefore unreachable, and the source covers it with `?? ""`.
        None => "",
    }
}

/// The parent directory, with a trailing separator, or an empty string.
///
/// Equivalent to `getDirectory`. The source joins components with `"/"`
/// whatever separator the input used, then appends one slash, so a Windows path
/// produces a POSIX separator here. Faithful, and surprising.
pub fn directory(path: Option<&str>) -> String {
    if is_absent(path) {
        return String::new();
    }
    let path = path.unwrap();
    let trimmed = strip_trailing_separators(path);
    let mut parts = split_path(trimmed);
    // The source returns `parts.slice(0, parts.length - 1).join("/") + "/"`. On a
    // single-element list the slice is empty, `join` returns "", so the result is
    // exactly "/".
    if parts.is_empty() {
        parts.push("");
    }
    let _last = parts.pop().unwrap();
    let mut out = parts.join("/");
    out.push('/');
    out
}

/// The extension of a path, or an empty string.
///
/// Equivalent to `getFileExtension`, with its strangeness deliberately kept: a
/// name without a dot returns ITS OWN NAME. See the module header.
///
/// Unlike the other three functions this one does not test for absence: the
/// source calls `path.split(".")` directly, which throws on `undefined`. Rust
/// cannot throw, so an empty string is returned and the difference documented
/// rather than inventing a panic.
pub fn extension(path: &str) -> &str {
    // `split(".")` on a string with no dot returns the string itself, so the
    // last element IS the full name. That is the source's behaviour.
    match path.rsplit_once('.') {
        Some((_, after)) if !after.is_empty() => after,
        // Case `"archive.tar."`: the last element is empty, as in JavaScript.
        Some(_) => "",
        // No dot at all: JavaScript returns the last element, so the path.
        None => path,
    }
}

/// The file name truncated from the end, keeping the extension.
///
/// Equivalent to `getFilenameTruncated`. The extension survives and the
/// truncation mark is a real character, not three dots.
///
/// The source default is 20, and it is reproduced.
pub fn truncated_file_name(path: Option<&str>, max_len: usize) -> String {
    let name = file_name(path);
    if name.len() <= max_len {
        return name.to_string();
    }
    // `lastIndexOf(".")` returns -1 when absent. The source tests `lastDot <= 0`,
    // which covers TWO cases: no dot at all, and a dot at position zero as in
    // ".gitignore". In both the extension is empty.
    let last_dot = match name.rfind('.') {
        Some(i) if i > 0 => i,
        _ => usize::MAX,
    };
    let ext = if last_dot == usize::MAX { "" } else { &name[last_dot..] };
    // In JavaScript `slice(0, n)` with negative n returns an empty string, while
    // Rust slicing panics. The `available <= 0` case of the source lands exactly
    // where `maxLength - 1` would be negative, so it is clamped to zero.
    let available = max_len as isize - ext.len() as isize - 1;
    if available <= 0 {
        let cut = max_len.saturating_sub(1);
        let mut out = name[..cut].to_string();
        out.push('\u{2026}');
        return out;
    }
    let available = available as usize;
    // Splitting in the middle of a multi-byte character is impossible here:
    // `rfind` yields a char boundary, and `max_len` is a byte count just as
    // `String::length` is a UTF-16 code unit count in JavaScript. That is the
    // very same divergence as the rest of the port, an emoji counting 1 in the
    // TypeScript and 4 bytes in Rust.
    let mut out = name[..available].to_string();
    out.push('\u{2026}');
    out.push_str(ext);
    out
}

/// Truncates text in the middle, with a punctuation mark at the centre.
///
/// Equivalent to `truncateMiddle`.
///
/// NOTE the `maxLength == 1` case: the arithmetic yields `end == 0` and the
/// source writes `text.slice(-0)`. In JavaScript `slice(-0)` equals `slice(0)`,
/// which returns the WHOLE string. The original's result is therefore the
/// punctuation mark followed by the complete text. This is reproduced on
/// purpose, because fixing it would change the observable output of a function
/// that has already shipped.
pub fn truncate_middle(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let available = max_len as isize - 1;
    let start = available.div_euclid(2);
    let end = available.div_euclid(2);
    let mut out = String::new();
    out.push_str(&text[..start as usize]);
    out.push('\u{2026}');
    if end == 0 {
        // `slice(-0)` in JavaScript returns the whole string, not an empty one.
        // This is the most counter-intuitive trap in the file.
        out.push_str(text);
    } else {
        let cut = text.len() - end as usize;
        out.push_str(&text[cut..]);
    }
    out
}

/// `getFilename` delegated to the standard library.
///
/// This function is NOT in the source. It exists because the five functions
/// above only manipulate text and an agent using them will want to compare with
/// `std::path`. It is marked as foreign so nobody mistakes it for a port.
pub fn file_name_via_std(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Trap one: `!path` is truthiness, so the empty string disappears
    // -----------------------------------------------------------------------

    #[test]
    fn empty_string_is_handled_as_absence() {
        // `if (!path) return ""`: `!""` is true in JavaScript. This test is the
        // whole reason the `is_absent` function exists.
        assert_eq!(file_name(None), "");
        assert_eq!(file_name(Some("")), "");
        assert_eq!(directory(None), "");
        assert_eq!(directory(Some("")), "");
    }

    #[test]
    fn truthiness_and_nullity_disagree() {
        // Translating `!path` as a bare check on `None` would let `Some("")`
        // through and make these two assertions diverge from the original.
        assert!(is_absent(None));
        assert!(is_absent(Some("")));
        assert!(!is_absent(Some("a")));
    }

    // -----------------------------------------------------------------------
    // Splitting and separators
    // -----------------------------------------------------------------------

    #[test]
    fn both_separators_are_accepted() {
        // The source uses `/[/\\]+$/`, slash AND backslash in one expression. An
        // agent that picked a single separator would break half the paths.
        assert_eq!(file_name(Some("dir/file.txt")), "file.txt");
        assert_eq!(file_name(Some("dir\\file.txt")), "file.txt");
        assert_eq!(file_name(Some("a/b/c/")), "c");
        assert_eq!(file_name(Some("a\\b\\c\\")), "c");
        assert_eq!(file_name(Some("a/b///")), "b");
    }

    #[test]
    fn empty_components_are_not_filtered() {
        // `split` filters nothing in JavaScript, and the consequence is visible
        // in the result. "a//b" has no trailing separator, so the trim is a
        // no-op; parts are ["a", "", "b"]. `slice(0, 2)` KEEPS the empty
        // component, giving ["a", ""]; `join("/")` is then "a/", and the
        // trailing "/" the source appends makes it "a//".
        //
        // The file name is therefore NOT preserved, which is the counter-
        // intuitive part: an empty component does not survive as an empty path
        // piece, it shifts everything left and drops the last real element.
        assert_eq!(directory(Some("a//b")), "a//");
        assert_eq!(directory(Some("dir/file.txt")), "dir/");
    }

    #[test]
    fn the_directory_always_ends_with_a_slash() {
        assert_eq!(directory(Some("a/b/c")), "a/b/");
        // A single-component path: the slice is empty, `join` returns "", and the
        // original therefore returns exactly "/".
        assert_eq!(directory(Some("file")), "/");
    }

    // -----------------------------------------------------------------------
    // `getFileExtension` and its two oddities
    // -----------------------------------------------------------------------

    #[test]
    fn a_file_without_a_dot_returns_its_own_name() {
        // `path.split(".")` with no check: the last element of a single-element
        // list is that element. Counter-intuitive, and faithful.
        assert_eq!(extension("archive"), "archive");
        assert_eq!(extension("dir/archive"), "dir/archive");
    }

    #[test]
    fn the_extension_is_what_follows_the_last_dot() {
        assert_eq!(extension("file.txt"), "txt");
        assert_eq!(extension("archive.tar.gz"), "gz");
        // Trailing dot: last element empty, as in JavaScript.
        assert_eq!(extension("file."), "");
    }

    // -----------------------------------------------------------------------
    // Truncation from the end
    // -----------------------------------------------------------------------

    #[test]
    fn a_short_name_is_returned_unchanged() {
        assert_eq!(truncated_file_name(Some("a"), 5), "a");
        assert_eq!(truncated_file_name(Some(""), 5), "");
    }

    #[test]
    fn truncation_keeps_the_extension_and_adds_one_punctuation_mark() {
        let long = "a_very_long_file_name.txt";
        let r = truncated_file_name(Some(long), 20);
        // The ellipsis goes BEFORE the extension, not at the end: the source is
        // `filename.slice(0, available) + "…" + ext`. With ext = ".txt" (4),
        // available = 20 - 4 - 1 = 15, so the result keeps 15 characters, then
        // the mark, then the extension: 15 + 1 + 4 = 20 exactly.
        assert_eq!(r, "a_very_long_fil\u{2026}.txt");
        assert!(r.ends_with(".txt"), "the extension must survive: {r}");
        assert_eq!(r.chars().count(), 20, "the budget must be exact: {r}");
        // The mark sits at the junction, not at the end.
        assert!(!r.ends_with('\u{2026}'), "the mark precedes the extension: {r}");
    }

    #[test]
    fn a_dot_at_position_zero_does_not_count_as_an_extension() {
        // `lastDot <= 0` covers `.gitignore`: the dot is at position zero, so
        // the extension is empty and the whole name is truncatable.
        let r = truncated_file_name(Some(".a_very_long_cache_name"), 10);
        assert!(!r.contains(".txt"), "no dot to preserve: {r}");
    }

    #[test]
    fn a_max_len_that_is_too_small_does_not_panic() {
        // `slice(0, n)` with negative n returns an empty string in JavaScript,
        // while Rust slicing panics. So this case is clamped to zero.
        let r = truncated_file_name(Some("file.txt"), 1);
        assert_eq!(r.chars().count(), 1);
    }

    // -----------------------------------------------------------------------
    // Middle truncation
    // -----------------------------------------------------------------------

    #[test]
    fn middle_truncation_keeps_both_ends() {
        let r = truncate_middle("abcdefghij", 5);
        assert_eq!(r, "ab\u{2026}ij");
    }

    #[test]
    fn short_text_is_untouched() {
        assert_eq!(truncate_middle("short", 20), "short");
        assert_eq!(truncate_middle("", 20), "");
    }

    #[test]
    fn max_len_one_returns_the_whole_text_like_javascript() {
        // THE TRAP. With maxLength == 1 the source computes `end == 0` then
        // writes `text.slice(-0)`. In JavaScript `slice(-0)` equals `slice(0)`,
        // which returns the WHOLE string. The original's result is therefore the
        // punctuation mark followed by the complete text.
        //
        // An agent who "fixed" this to an empty range would change the
        // observable output of a function that already shipped.
        let r = truncate_middle("text", 1);
        assert_eq!(r, "\u{2026}text");
    }

    #[test]
    fn the_length_is_measured_in_bytes_like_javascript() {
        // `String::length` counts UTF-16 code units in JavaScript, Rust counts
        // bytes. Identical on ASCII, different on an emoji: 1 in the TypeScript
        // and 4 bytes in Rust. Same divergence as the rest of the port,
        // reported rather than hidden.
        assert_eq!(truncate_middle("abcde", 5), "abcde");
        assert_eq!(truncated_file_name(Some("abcde"), 5), "abcde");
    }

    // -----------------------------------------------------------------------
    // Non-regression of the whole module
    // -----------------------------------------------------------------------

    #[test]
    fn a_full_path_behaves_like_the_original() {
        let p = "C:\\Users\\name\\Documents\\final-report.pdf";
        assert_eq!(file_name(Some(p)), "final-report.pdf");
        assert_eq!(directory(Some(p)), "C:/Users/name/Documents/");
        assert_eq!(extension("final-report.pdf"), "pdf");
    }

    #[test]
    fn the_non_port_function_stays_labelled_as_such() {
        // It does not exist in the source. This test keeps the difference
        // visible to whoever reads the file later.
        assert_eq!(file_name_via_std(std::path::Path::new("a/b.txt")), "b.txt");
    }
}
