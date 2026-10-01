//! The `node:path` subset shared by `config/plugin/agent.ts` and
//! `config/plugin/command.ts`.
//!
//! Both plugins turn a discovered file into a plugin-relative name with the
//! same three steps - `path.relative`, backslashes to slashes, then one prefix
//! and one suffix - and the TS shares them only through Node's `path`. Putting
//! the three steps here keeps them identical, and keeps `relative` in one place
//! instead of two.
//!
//! `relative` is a real `path.relative`, not `strip_prefix`. The difference is
//! the whole point of the function: `relative("/base", "/base/a.md")` is
//! `"a.md"` with no leading separator, and a path that is not under the base
//! gains one `..` per remaining base segment instead of keeping the base
//! prefix. A `strip_prefix` version returns `"/a.md"` and `"/base/a.md"`,
//! which then defeats the prefix-stripping step - it looks for `agent/` at the
//! start of a string that begins with `/`.
//!
//! Known limitation, recorded: this is the POSIX behaviour and expects the
//! absolute POSIX paths these plugins discover. Node's Windows branch (drive
//! letters, UNC roots, backslash separators) is not reproduced, and neither is
//! its case-insensitive comparison.

/// `path.posix.relative` for absolute POSIX paths.
///
/// Returns `""` when both sides are the same path, as Node does, and
/// interpolates `..` segments for whatever part of the base is not shared.
pub fn relative(from: &str, to: &str) -> String {
    if from == to {
        return String::new();
    }
    let from_parts = segments(from);
    let to_parts = segments(to);
    if from_parts == to_parts {
        return String::new();
    }
    let mut shared = 0usize;
    while shared < from_parts.len() && shared < to_parts.len() && from_parts[shared] == to_parts[shared] {
        shared += 1;
    }
    // Node's loop only remembers a shared SEPARATOR, not a shared prefix. When
    // nothing in common was ever separated, `lastCommonSep` is still -1 and it
    // returns the target untouched - no `..` at all. That is what happens to a
    // Windows-shaped path on a POSIX host: `C:\base` holds no separator, so
    // `C:\base\agent\plan.md` shares no segment and comes back whole.
    if shared == 0 && !from.starts_with('/') && !to.starts_with('/') {
        return to_parts.join("/");
    }
    let mut out: Vec<String> = Vec::with_capacity(from_parts.len() + to_parts.len());
    for _ in shared..from_parts.len() {
        out.push("..".to_string());
    }
    for part in &to_parts[shared..] {
        out.push((*part).to_string());
    }
    out.join("/")
}

/// The path split into segments, dropping empties and `.`.
fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty() && *s != ".").collect()
}

/// `replaceAll("\\", "/")`.
pub fn to_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

/// The name both plugins derive, parameterised by the prefixes to strip.
///
/// The order matters and is the source's: relative path, then slashes, then at
/// most ONE leading `<prefix>/`, then at most one trailing `.md`. The two
/// strips are independent, which is worth stating because it is easy to assume
/// they come as a pair: a path whose prefix does not survive still loses its
/// `.md`. A name that is exactly `agent/` keeps its prefix, since `relative`
/// collapses the trailing slash and leaves nothing to strip.
pub fn plugin_name(directory: &str, filepath: &str, prefixes: &[&str]) -> String {
    let mut name = to_slashes(&relative(directory, filepath));
    for prefix in prefixes {
        let with_slash = format!("{}/", prefix);
        if let Some(rest) = name.strip_prefix(&with_slash) {
            name = rest.to_string();
            break;
        }
    }
    if let Some(rest) = name.strip_suffix(".md") {
        name = rest.to_string();
    }
    name
}

/// The prefixes `agent.ts` strips.
pub const AGENT_PREFIXES: [&str; 4] = ["agent", "agents", "mode", "modes"];
/// The prefixes `command.ts` strips.
pub const COMMAND_PREFIXES: [&str; 2] = ["command", "commands"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_is_relative_without_a_leading_separator() {
        assert_eq!(relative("/base", "/base/a.md"), "a.md");
        assert_eq!(relative("/base", "/base/agent/plan.md"), "agent/plan.md");
    }

    #[test]
    fn an_identical_path_is_empty() {
        assert_eq!(relative("/base", "/base"), "");
        assert_eq!(relative("/base/", "/base"), "");
        assert_eq!(relative("/base/./", "/base"), "");
    }

    #[test]
    fn a_path_outside_the_base_gains_one_dotdot_per_extra_segment() {
        assert_eq!(relative("/base", "/other/x.md"), "../other/x.md");
        assert_eq!(relative("/a/b", "/x.md"), "../../x.md");
        assert_eq!(relative("/a", "/a/b/c.md"), "b/c.md");
    }

    #[test]
    fn empty_and_dot_segments_are_ignored() {
        assert_eq!(relative("/base//sub", "/base/sub/./a.md"), "a.md");
        assert_eq!(relative("/base", "/base//a.md"), "a.md");
    }

    #[test]
    fn backslashes_become_slashes() {
        assert_eq!(to_slashes("a\\b\\c"), "a/b/c");
        assert_eq!(to_slashes("a/b"), "a/b");
    }

    #[test]
    fn an_agent_name_drops_one_prefix_and_the_suffix() {
        for prefix in AGENT_PREFIXES {
            assert_eq!(
                plugin_name("/base", &format!("/base/{}/plan.md", prefix), &AGENT_PREFIXES),
                "plan",
                "{}",
                prefix
            );
        }
    }

    #[test]
    fn only_the_first_segment_is_a_prefix() {
        assert_eq!(
            plugin_name("/base", "/base/agents/deep/plan.md", &AGENT_PREFIXES),
            "deep/plan"
        );
    }

    #[test]
    fn a_command_name_drops_the_command_prefixes() {
        for prefix in COMMAND_PREFIXES {
            assert_eq!(
                plugin_name("/base", &format!("/base/{}/plan.md", prefix), &COMMAND_PREFIXES),
                "plan",
                "{}",
                prefix
            );
        }
    }

    #[test]
    fn an_unexpected_prefix_is_left_alone() {
        assert_eq!(
            plugin_name("/base", "/base/other/plan.md", &COMMAND_PREFIXES),
            "other/plan"
        );
    }

    #[test]
    fn the_suffix_is_stripped_once_and_only_at_the_end() {
        assert_eq!(plugin_name("/base", "/base/agent/a.md.md", &AGENT_PREFIXES), "a.md");
        assert_eq!(plugin_name("/base", "/base/agent/a.txt", &AGENT_PREFIXES), "a.txt");
        assert_eq!(plugin_name("/base", "/base/agent/.md", &AGENT_PREFIXES), "");
    }

    #[test]
    fn a_name_that_is_only_the_prefix_collapses_to_nothing() {
        // `relative("/base", "/base/agent/")` is "agent": the trailing slash
        // collapses and leaves no `agent/` to strip. The name is the prefix.
        assert_eq!(plugin_name("/base", "/base/agent/", &AGENT_PREFIXES), "agent");
    }

    #[test]
    fn a_windows_shaped_path_has_no_segments_to_compare() {
        // On a POSIX host a backslash is not a separator, so `C:\base` and
        // `C:\base\agent\plan.md` share no separator and Node's `relative`
        // returns the target whole. Folding the backslashes then puts `agent/`
        // no longer at the start, so the PREFIX survives - while the `.md`
        // suffix, stripped independently, does not.
        // A Windows host would use the win32 branch of `path.relative` and get
        // "plan" instead - that divergence is documented, not papered over.
        assert_eq!(plugin_name("C:\\base", "C:\\base\\agent\\plan.md", &AGENT_PREFIXES), "C:/base/agent/plan");
    }

    #[test]
    fn two_unrooted_paths_sharing_no_segment_return_the_target() {
        assert_eq!(relative("a/b", "c/d.md"), "c/d.md");
    }
}