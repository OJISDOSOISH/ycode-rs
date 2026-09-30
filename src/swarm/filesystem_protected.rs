//! Rust port of `opencode/packages/core/src/filesystem/protected.ts`.
//!
//! # Scope: Windows only, and the dead branches are left out on purpose
//!
//! The source branches three ways on `process.platform`: `darwin`, `win32`, and
//! an empty fallback for everything else. **This project targets Windows only**,
//! so the `darwin` branch and the fallback are not translated. Carrying them
//! would mean shipping eighteen directory names for a platform that cannot run
//! this binary, and every one of them would be untested on the only platform
//! that exists.
//!
//! What survives is therefore the Windows list and the joining logic.
//!
//! # The home directory is a parameter, not a call
//!
//! The source evaluates `os.homedir()` once at module load and captures the
//! result. That is untestable: a test cannot choose a home directory, so it
//! would have to assert against whatever the machine running the test happens to
//! use. Here the home directory is a **parameter**, and the module-level
//! constant is derived from `USERPROFILE`. The joining logic, which is the part
//! with bugs in it, becomes a pure function.
//!
//! No test in this file touches the real filesystem: every case runs on strings.

/// Directory basenames to skip when scanning the home directory, on Windows.
///
/// The source calls this `WIN32_HOME`. Order is preserved from the source,
/// because it is the order a caller iterating the set would produce.
pub const WIN32_HOME: [&str; 8] = [
    "AppData", "Downloads", "Desktop", "Documents", "Pictures", "Music", "Videos", "OneDrive",
];

/// Directory basenames to skip, for the platform this build targets.
///
/// The source returns a `ReadonlySet<string>`. A set would lose the order,
/// which matters here only for reporting, so a slice is used and the duplicate
/// case is covered by a test instead.
///
/// The `darwin` list and the empty fallback of the source are NOT here. See the
/// module header.
pub fn names() -> &'static [&'static str] {
    &WIN32_HOME
}

/// Joins a home directory and a basename the way `path.join` does on Windows.
///
/// `path.join("C:\\Users\\a", "Documents")` yields `C:\Users\a\Documents`, with a
/// single separator, and no trailing separator is added. A basename that is
/// already absolute would REPLACE the first argument in Node, which this
/// reproduces, because a home directory followed by an absolute path is
/// nonsense and silently joining instead would hide the bug.
pub fn join_home(home: &str, name: &str) -> String {
    if has_drive_prefix(name) || name.starts_with('\\') {
        return name.to_string();
    }
    let base = home.trim_end_matches(['\\', '/']);
    if base.is_empty() {
        return name.to_string();
    }
    format!("{base}\\{name}")
}

/// True when a path starts with a drive letter, as `path.isAbsolute` sees it.
fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Absolute paths that should never be watched, stated, or scanned.
///
/// `home` is the home directory to build them under. On the source this is the
/// captured `os.homedir()`; here it is supplied, so the result is testable.
///
/// Returns the eight Windows paths, in source order. The `darwin` list, which
/// the source concatenates in two separate groups, is not ported.
pub fn paths(home: &str) -> Vec<String> {
    WIN32_HOME.iter().map(|name| join_home(home, name)).collect()
}

/// The module-level home directory, read from the environment.
///
/// `os.homedir()` reads `USERPROFILE` on Windows. Reading it here keeps the same
/// source of truth, but late enough that a test can set the variable and get a
/// different answer. When it is unset the function returns an empty string, and
/// [`paths`] then yields bare basenames rather than absolute paths, which is
/// visibly wrong instead of silently plausible.
pub fn home_directory() -> String {
    std::env::var("USERPROFILE").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // The names
    // -----------------------------------------------------------------------

    #[test]
    fn the_eight_windows_names_are_present_in_source_order() {
        // Order is the source's, and it is not alphabetical: AppData first,
        // OneDrive last. A set would have hidden that.
        assert_eq!(
            names(),
            &["AppData", "Downloads", "Desktop", "Documents", "Pictures", "Music", "Videos", "OneDrive"]
        );
    }

    #[test]
    fn the_darwin_names_are_absent() {
        // The source has nine darwin home names and nine library names. They are
        // deliberately not ported, because this project targets Windows only.
        for absent in ["Library", "Applications", "Public", "AddressBook", "Spotlight"] {
            assert!(
                !names().contains(&absent),
                "{absent} belongs to the darwin branch and must not be ported"
            );
        }
    }

    #[test]
    fn the_names_are_unique() {
        // The source builds a Set, which silently drops duplicates. Asserting
        // uniqueness keeps the list honest: if a name were added twice, the
        // count below would catch it.
        let mut sorted = names().to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "the source list has a duplicate");
    }

    // -----------------------------------------------------------------------
    // Joining
    // -----------------------------------------------------------------------

    #[test]
    fn joining_produces_a_single_separator() {
        assert_eq!(join_home("C:\\Users\\a", "Documents"), "C:\\Users\\a\\Documents");
        // A trailing separator on the home does not produce a double one.
        assert_eq!(join_home("C:\\Users\\a\\", "Documents"), "C:\\Users\\a\\Documents");
        assert_eq!(join_home("C:\\Users\\a/", "Documents"), "C:\\Users\\a\\Documents");
    }

    #[test]
    fn an_empty_home_yields_the_bare_name_rather_than_a_leading_separator() {
        // `path.join("", "Documents")` gives "Documents", not "\Documents".
        assert_eq!(join_home("", "Documents"), "Documents");
    }

    #[test]
    fn an_absolute_name_replaces_the_home_rather_than_being_appended() {
        // `path.join` gives the second argument when it is absolute. Joining
        // anyway would produce "C:\Users\a\D:\other", which is not a path.
        assert_eq!(join_home("C:\\Users\\a", "D:\\other"), "D:\\other");
        assert_eq!(join_home("C:\\Users\\a", "\\\\server\\share"), "\\\\server\\share");
        assert_eq!(join_home("C:\\Users\\a", "\\rooted"), "\\rooted");
    }

    #[test]
    fn drive_detection_matches_the_windows_rule() {
        assert!(has_drive_prefix("C:\\"));
        assert!(has_drive_prefix("D:/x"));
        assert!(has_drive_prefix("z:"));
        assert!(!has_drive_prefix("C"), "a lone drive letter is not absolute");
        assert!(!has_drive_prefix("CD:\\"), "the colon must be at index 1");
        assert!(!has_drive_prefix("/usr/bin"), "a POSIX path is not a drive");
        assert!(!has_drive_prefix(""));
    }

    // -----------------------------------------------------------------------
    // The paths
    // -----------------------------------------------------------------------

    #[test]
    fn the_paths_are_the_eight_names_under_the_home() {
        let p = paths("C:\\Users\\alice");
        assert_eq!(p.len(), 8);
        assert_eq!(p[0], "C:\\Users\\alice\\AppData");
        assert_eq!(p[7], "C:\\Users\\alice\\OneDrive");
        assert!(p.iter().all(|x| x.starts_with("C:\\Users\\alice\\")), "every path is under the home");
    }

    #[test]
    fn a_home_already_ending_in_a_separator_does_not_double_it() {
        // The bug this guards against is invisible until the path is used: a
        // doubled separator still resolves on Windows, so nothing complains.
        for home in ["C:\\Users\\alice", "C:\\Users\\alice\\", "C:\\Users\\alice\\\\"] {
            for p in paths(home) {
                assert!(!p.contains("\\\\"), "doubled separator in {p} for home {home}");
            }
        }
    }

    #[test]
    fn paths_are_distinct() {
        // Two home entries that collided would silently shorten the list.
        let mut p = paths("C:\\Users\\alice");
        let before = p.len();
        p.sort();
        p.dedup();
        assert_eq!(before, p.len());
    }

    // -----------------------------------------------------------------------
    // The prefix trap, which is the reason this module is worth testing
    // -----------------------------------------------------------------------

    #[test]
    fn a_protected_path_is_not_a_string_prefix_of_a_longer_one() {
        // "C:\Users\alice\Documents" is protected. "C:\Users\alice\Documents2" is
        // a DIFFERENT directory, and it must not be treated as inside the
        // protected one. A naive starts_with says it is. This is the trap the
        // reviewer asked to be covered.
        let protected = paths("C:\\Users\\alice");
        let sibling = "C:\\Users\\alice\\Documents2";

        let naive = protected.iter().any(|p| sibling.starts_with(p.as_str()));
        assert!(naive, "a naive prefix match does produce the false positive");

        let boundary_aware = protected.iter().any(|p| is_within(sibling, p));
        assert!(!boundary_aware, "a boundary-aware check must reject the sibling");
    }

    /// True when `candidate` is `parent` itself or lies under it, comparing path
    /// SEGMENTS rather than characters.
    ///
    /// Not in the source: the source only builds a list for its caller to use,
    /// and the source carries no containment test at all. This is here because
    /// without it the list is untestable against the bug it is meant to prevent.
    pub fn is_within(candidate: &str, parent: &str) -> bool {
        let c = normalise(candidate);
        let p = normalise(parent);
        if !c.starts_with(&p) {
            return false;
        }
        // Equal, or the next character must be a separator.
        c.len() == p.len() || matches!(c.as_bytes().get(p.len()), Some(b'\\') | Some(b'/'))
    }

    /// Lowercases and strips trailing separators, without touching the disk.
    fn normalise(path: &str) -> String {
        let mut s = path.trim_end_matches(['\\', '/']).to_lowercase();
        while s.contains("\\\\") {
            s = s.replace("\\\\", "\\");
        }
        s
    }

    #[test]
    fn boundary_aware_containment_accepts_the_real_children() {
        let home = "C:\\Users\\alice";
        assert!(is_within("C:\\Users\\alice\\Documents", &format!("{home}\\Documents")));
        assert!(is_within("C:\\Users\\alice\\Documents\\notes", &format!("{home}\\Documents")));
        assert!(is_within("C:\\Users\\alice\\Documents", &home));
    }

    #[test]
    fn boundary_aware_containment_is_case_insensitive_like_windows() {
        assert!(is_within("C:\\USERS\\ALICE\\Documents", "c:\\users\\alice\\documents"));
    }

    #[test]
    fn an_empty_home_is_not_a_parent_of_everything() {
        // With an empty home, normalise gives an empty string, and every path
        // starts with it. The length check saves us here; this test proves it.
        assert!(!is_within("C:\\anything", ""));
    }

    // -----------------------------------------------------------------------
    // The environment read
    // -----------------------------------------------------------------------

    #[test]
    fn the_home_directory_comes_from_userprofile_or_is_empty() {
        // Either value is acceptable, but it must be one of the two, and the
        // empty case must not silently produce plausible absolute paths.
        let h = home_directory();
        let from_env = std::env::var("USERPROFILE").ok();
        assert_eq!(Some(h.clone()), from_env.or(Some(String::new())));
    }
}
