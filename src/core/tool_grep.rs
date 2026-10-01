//! Port of the portable part of `opencode/packages/core/src/tool/grep.ts`.
//!
//! The tool's contract, plus the formatter that decides what the model reads.
//! The formatter is the part that matters: it groups matches by file, prints
//! one header per file with a blank line before the next group, and indents
//! each match line by two spaces.
//!
//! Two behaviours are easy to lose in a port and are pinned by tests here: an
//! empty result prints "No files found" rather than an empty string, and
//! consecutive matches inside the SAME file do not repeat the file header.

use serde::{Deserialize, Serialize};

use super::tool_read_filesystem::{DirectoryEntry, EntryKind};

/// Tool name.
pub const NAME: &str = "grep";

/// `GrepTool.Input`, mirroring `FileSystem.GrepInput`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// One match, as `FileSystem.Match` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub entry: DirectoryEntry,
    pub line: u64,
    pub text: String,
}

/// The header printed when there is at least one match.
pub fn found_header(count: usize) -> String {
    format!("Found {} matches", count)
}

/// The exact text printed when there is none.
pub const NO_FILES_FOUND: &str = "No files found";

/// `toModelOutput`: grouped by file, two-space indent, blank line between groups.
pub fn to_model_output(matches: &[Match]) -> String {
    let mut lines: Vec<String> = if matches.is_empty() {
        vec![NO_FILES_FOUND.to_string()]
    } else {
        vec![found_header(matches.len())]
    };
    let mut current = String::new();
    for m in matches {
        if current != m.entry.name {
            if !current.is_empty() {
                lines.push(String::new());
            }
            current = m.entry.name.clone();
            lines.push(format!("{}:", current));
        }
        lines.push(format!("  Line {}: {}", m.line, m.text));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, line: u64, text: &str) -> Match {
        Match {
            entry: DirectoryEntry { name: name.to_string(), kind: EntryKind::File },
            line,
            text: text.to_string(),
        }
    }

    #[test]
    fn no_match_prints_no_files_found() {
        assert_eq!(to_model_output(&[]), "No files found");
    }

    #[test]
    fn matches_of_one_file_share_one_header() {
        let out = to_model_output(&[m("a.ts", 1, "one"), m("a.ts", 2, "two")]);
        assert_eq!(
            out,
            "Found 2 matches\na.ts:\n  Line 1: one\n  Line 2: two"
        );
    }

    #[test]
    fn a_new_file_gets_a_blank_line_and_its_own_header() {
        let out = to_model_output(&[m("a.ts", 1, "one"), m("b.ts", 5, "two")]);
        assert_eq!(
            out,
            "Found 2 matches\na.ts:\n  Line 1: one\n\nb.ts:\n  Line 5: two"
        );
    }

    #[test]
    fn a_file_reappearing_after_another_one_gets_a_new_group() {
        let out = to_model_output(&[m("a.ts", 1, "x"), m("b.ts", 1, "y"), m("a.ts", 2, "z")]);
        assert_eq!(
            out,
            "Found 3 matches\na.ts:\n  Line 1: x\n\nb.ts:\n  Line 1: y\n\na.ts:\n  Line 2: z"
        );
    }

    #[test]
    fn the_header_counts_matches_not_files() {
        let out = to_model_output(&[m("a.ts", 1, "x"), m("b.ts", 1, "y"), m("c.ts", 1, "z")]);
        assert!(out.starts_with("Found 3 matches"));
    }

    #[test]
    fn the_input_omits_absent_optionals() {
        let v = serde_json::to_value(Input { pattern: "x".into(), ..Input::default() }).unwrap();
        assert_eq!(v, serde_json::json!({ "pattern": "x" }));
        let v = serde_json::to_value(Input {
            pattern: "x".into(),
            path: Some("src".into()),
            include: Some("*.ts".into()),
            limit: Some(10),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "pattern": "x", "path": "src", "include": "*.ts", "limit": 10 })
        );
    }
}