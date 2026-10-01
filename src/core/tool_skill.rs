//! Port of the portable part of `opencode/packages/core/src/tool/skill.ts`.
//!
//! The template the skill tool wraps its payload in, and the rule that decides
//! which files accompany it.
//!
//! Two things are easy to lose here:
//!
//! - the skill's own content is TRIMMED before it goes into the template, so
//!   stray blank lines in a `SKILL.md` do not change the block the model sees;
//! - the file list is only populated when the skill lives in a file literally
//!   named `SKILL.md`. Any other filename yields an EMPTY list, and the
//!   template still prints the `<skill_files>` wrapper. The model cannot tell
//!   "no files" from "not sampled" from the wire, so the distinction has to
//!   live in this function.
//!
//! `SkillV2.Info` is defined locally because `core/src/skill.ts` - the module
//! that owns it - is not ported yet; once it is, these three fields should be
//! re-exported from there instead.

use serde::{Deserialize, Serialize};

/// Tool name.
pub const NAME: &str = "skill";

/// `FILE_LIMIT`: how many companion files are listed at most.
pub const FILE_LIMIT: usize = 10;

/// The filename that makes a skill eligible for a file list.
pub const SKILL_FILENAME: &str = "SKILL.md";

/// The slice of `SkillV2.Info` this tool reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub location: String,
    pub content: String,
}

/// `SkillTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
}

/// `SkillTool.Output`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub directory: String,
    pub output: String,
}

/// `SkillTool.description`, joined with newlines exactly as the array is.
///
/// Five lines with a blank between each pair of paragraphs, so the newline
/// has to live inside the pieces: `concat!` glues literals without adding
/// anything, and an empty piece contributes nothing at all.
pub const DESCRIPTION: &str = concat!(
    "Load a specialized skill when the task at hand matches one of the available ",
    "skills in the system context.\n",
    "\n",
    "Use this tool to inject the skill's instructions and resources into the current ",
    "conversation. The output may contain detailed workflow guidance as well as ",
    "references to scripts, files, etc. in the same directory as the skill.\n",
    "\n",
    "The skill name must match one of the available skills in the system context.",
);

/// `path.posix.dirname`, because the skill locations are POSIX paths.
///
/// Faithful to the Node implementation, including the two quirks a naive
/// `rsplit('/')` gets wrong. Trailing slashes are collapsed rather than
/// obeyed, so `dirname("a/b/")` is `"a"` - not `"a/b"`, and not `"."`. And a
/// rooted path whose parent would be empty returns the root, keeping the
/// double slash for a path like `//a`. Tests pin both, because they are the
/// cases a short implementation answers differently.
pub fn dirname(location: &str) -> String {
    if location.is_empty() {
        return ".".to_string();
    }
    let has_root = location.starts_with('/');
    let bytes = location.as_bytes();
    let mut end: isize = -1;
    let mut matched_slash = true;
    let mut i = bytes.len() as isize - 1;
    while i >= 1 {
        if bytes[i as usize] == b'/' {
            if !matched_slash {
                end = i;
                break;
            }
        } else {
            matched_slash = false;
        }
        i -= 1;
    }
    if end == -1 {
        return if has_root { "/".to_string() } else { ".".to_string() };
    }
    let end = end as usize;
    if has_root && end == 1 {
        return "//".to_string();
    }
    location[..end].to_string()
}

/// `basename`, POSIX flavour, for the `SKILL.md` test.
pub fn basename(location: &str) -> &str {
    let trimmed = location.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    }
}

/// The companion files that accompany a skill.
///
/// `None` in, empty list out: a skill that does not live in a `SKILL.md` is
/// never sampled, whatever the caller found on disk.
pub fn sample_files(location: &str, found: &[String]) -> Vec<String> {
    if basename(location) != SKILL_FILENAME {
        return Vec::new();
    }
    let mut files: Vec<String> = found
        .iter()
        .filter(|file| basename(file) != SKILL_FILENAME)
        .cloned()
        .collect();
    files.sort();
    files.truncate(FILE_LIMIT);
    files
}

/// `toModelOutput`: the `<skill_content>` block.
pub fn to_model_output(skill: &SkillInfo, files: &[String]) -> String {
    let directory = dirname(&skill.location);
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("<skill_content name=\"{}\">", skill.name));
    lines.push(format!("# Skill: {}", skill.name));
    lines.push(String::new());
    lines.push(skill.content.trim().to_string());
    lines.push(String::new());
    lines.push(format!("Base directory for this skill: {}", directory));
    lines.push(
        "Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory."
            .to_string(),
    );
    lines.push("Note: file list is sampled.".to_string());
    lines.push(String::new());
    lines.push("<skill_files>".to_string());
    for file in files {
        lines.push(format!("<file>{}</file>", file));
    }
    lines.push("</skill_files>".to_string());
    lines.push("</skill_content>".to_string());
    lines.join("\n")
}

/// The permission failure text, verbatim.
pub fn unable_to_load(name: &str) -> String {
    format!("Unable to load skill {}", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, location: &str, content: &str) -> SkillInfo {
        SkillInfo {
            name: name.to_string(),
            location: location.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn dirname_matches_the_posix_rules() {
        assert_eq!(dirname("/a/b/SKILL.md"), "/a/b");
        assert_eq!(dirname("a/SKILL.md"), "a");
        assert_eq!(dirname("SKILL.md"), ".", "no separator at all");
        assert_eq!(dirname("/SKILL.md"), "/");
        assert_eq!(dirname("a/b/"), "a", "trailing slashes collapse to the parent");
        assert_eq!(dirname("a/b//"), "a", "a run of trailing slashes is one");
        assert_eq!(dirname("a"), ".");
        assert_eq!(dirname("/"), "/");
        assert_eq!(dirname(""), ".");
    }

    #[test]
    fn dirname_keeps_the_double_root_quirk() {
        assert_eq!(dirname("//a"), "//");
        assert_eq!(dirname("//a/b"), "//a");
    }

    #[test]
    fn basename_ignores_trailing_slashes() {
        assert_eq!(basename("/a/b/SKILL.md"), "SKILL.md");
        assert_eq!(basename("/a/b/"), "b");
        assert_eq!(basename("SKILL.md"), "SKILL.md");
    }

    #[test]
    fn only_a_skill_md_location_gets_a_file_list() {
        let found = vec!["/a/b/scripts/run.sh".to_string()];
        assert_eq!(sample_files("/a/b/SKILL.md", &found).len(), 1);
        assert!(
            sample_files("/a/b/other.md", &found).is_empty(),
            "a skill not named SKILL.md is never sampled"
        );
    }

    #[test]
    fn the_skill_file_itself_is_never_listed() {
        let found = vec![
            "/a/b/SKILL.md".to_string(),
            "/a/b/scripts/run.sh".to_string(),
        ];
        let listed = sample_files("/a/b/SKILL.md", &found);
        assert_eq!(listed, vec!["/a/b/scripts/run.sh".to_string()]);
    }

    #[test]
    fn files_are_sorted_and_capped_at_ten() {
        let mut found: Vec<String> = (0..25).map(|i| format!("/a/b/f{:02}.txt", i)).collect();
        found.push("/a/b/SKILL.md".to_string());
        let listed = sample_files("/a/b/SKILL.md", &found);
        assert_eq!(listed.len(), FILE_LIMIT);
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted, "the order is the sorted order");
        assert_eq!(listed[0], "/a/b/f00.txt");
    }

    #[test]
    fn the_block_carries_the_name_the_content_and_the_directory() {
        let out = to_model_output(&skill("pdf", "/a/b/SKILL.md", "do the thing"), &[]);
        assert!(out.starts_with("<skill_content name=\"pdf\">\n# Skill: pdf\n"), "{}", out);
        assert!(out.contains("\ndo the thing\n"));
        assert!(out.contains("Base directory for this skill: /a/b"));
        assert!(out.ends_with("</skill_files>\n</skill_content>"));
    }

    #[test]
    fn the_content_is_trimmed() {
        let out = to_model_output(&skill("s", "/a/SKILL.md", "\n\n  body  \n\n"), &[]);
        assert!(out.contains("\nbody\n"), "{}", out);
        assert!(!out.contains("  body  "));
    }

    #[test]
    fn the_files_wrapper_is_present_even_with_no_files() {
        let out = to_model_output(&skill("s", "/a/other.md", "x"), &[]);
        assert!(out.contains("\n<skill_files>\n</skill_files>\n"), "{}", out);
    }

    #[test]
    fn each_file_is_wrapped_in_its_own_element() {
        let files = vec!["/a/b/one.sh".to_string(), "/a/b/two.sh".to_string()];
        let out = to_model_output(&skill("s", "/a/b/SKILL.md", "x"), &files);
        assert!(out.contains("<file>/a/b/one.sh</file>"));
        assert!(out.contains("<file>/a/b/two.sh</file>"));
    }

    #[test]
    fn the_output_field_holds_the_whole_block() {
        let s = skill("s", "/a/b/SKILL.md", "x");
        let out = Output {
            name: s.name.clone(),
            directory: dirname(&s.location),
            output: to_model_output(&s, &[]),
        };
        assert!(out.output.starts_with("<skill_content"));
        assert_eq!(out.directory, "/a/b");
    }

    #[test]
    fn the_failure_message_names_the_skill() {
        assert_eq!(unable_to_load("pdf"), "Unable to load skill pdf");
    }

    #[test]
    fn the_description_is_five_lines_with_a_blank_between_paragraphs() {
        let lines: Vec<&str> = DESCRIPTION.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[1], "");
        assert_eq!(lines[3], "");
        assert!(lines[0].starts_with("Load a specialized skill"));
        assert!(lines[2].starts_with("Use this tool to inject"));
        assert!(lines[4].starts_with("The skill name must match"));
    }
}