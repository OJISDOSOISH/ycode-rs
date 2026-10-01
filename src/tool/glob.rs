//! Port of the portable part of `opencode/packages/core/src/tool/glob.ts`.
//!
//! The tool's contract and its formatter. The formatter is a single join, and
//! its one interesting behaviour is the empty case: a glob that matched
//! nothing prints "No files found" rather than an empty string, so the model
//! can tell "no matches" from "empty output".

use serde::{Deserialize, Serialize};

use super::read_filesystem::DirectoryEntry;

/// Tool name.
pub const NAME: &str = "glob";

/// `GlobTool.Input`, mirroring `FileSystem.GlobInput`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// The exact text printed when nothing matched.
pub const NO_FILES_FOUND: &str = "No files found";

/// `toModelOutput`: the entry paths, one per line.
pub fn to_model_output(entries: &[DirectoryEntry]) -> String {
    if entries.is_empty() {
        return NO_FILES_FOUND.to_string();
    }
    entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::read_filesystem::EntryKind;
    use serde_json::json;

    fn e(name: &str) -> DirectoryEntry {
        DirectoryEntry { name: name.to_string(), kind: EntryKind::File }
    }

    #[test]
    fn no_entry_prints_no_files_found() {
        assert_eq!(to_model_output(&[]), "No files found");
    }

    #[test]
    fn entries_are_printed_one_per_line_in_order() {
        assert_eq!(
            to_model_output(&[e("a.rs"), e("b.rs"), e("c.rs")]),
            "a.rs\nb.rs\nc.rs"
        );
    }

    #[test]
    fn a_single_entry_has_no_trailing_newline() {
        assert_eq!(to_model_output(&[e("a.rs")]), "a.rs");
    }

    #[test]
    fn the_input_omits_absent_optionals() {
        assert_eq!(
            serde_json::to_value(Input { pattern: "*.ts".into(), ..Input::default() }).unwrap(),
            json!({ "pattern": "*.ts" })
        );
        assert_eq!(
            serde_json::to_value(Input {
                pattern: "*.ts".into(),
                path: Some("src".into()),
                limit: Some(50),
            })
            .unwrap(),
            json!({ "pattern": "*.ts", "path": "src", "limit": 50 })
        );
    }
}