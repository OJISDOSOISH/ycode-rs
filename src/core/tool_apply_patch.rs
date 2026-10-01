//! Port of the portable part of `opencode/packages/core/src/tool/apply-patch.ts`.
//!
//! The patch parser, the target resolution and the Effect wiring are not
//! ported. What is ported is everything the model reads back, and the rules
//! that decide when the tool refuses.
//!
//! Three things here are easy to get wrong, and each has a test:
//!
//! - the failure message has TWO shapes, not one. With nothing applied yet it
//!   reads `Unable to apply patch at <path>`; once something has been applied
//!   it becomes `Patch partially applied before failing at <path>. Applied:
//!   <resources>`. The second shape is the only signal the model gets that the
//!   patch was left half-applied, so it has to name what did land.
//! - the per-operation letters in the summary are A, D and M - a different
//!   vocabulary from the wire, which says `add`, `delete`, `update`, and from
//!   the diff status, which says `added`, `deleted`, `modified`. Three sets of
//!   words for one operation.
//! - an added file gets a trailing newline appended exactly once, and an EMPTY
//!   body is left empty rather than becoming a lone newline. The condition is
//!   `endsWith("\n") || contents === ""`, so both edges are deliberate.
//!
//! BOM handling is not repeated here: `tool_edit::split_bom` and
//! `tool_edit::join_bom` already port it, and this tool uses the same pair.

use serde::{Deserialize, Serialize};

use crate::schema::file_diff::{FileStatus, Info as FileDiffInfo};

/// Tool name. Note it is `apply_patch`, with an underscore, unlike the file.
pub const NAME: &str = "apply_patch";

/// `ApplyPatchTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    /// The wire name is `patchText`; the Rust name is snake_case. Without the
    /// rename this serialises as `patch_text`, which no model ever sends.
    #[serde(rename = "patchText")]
    pub patch_text: String,
}

/// The `type` of one applied operation, as it goes on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppliedType {
    Add,
    Update,
    Delete,
}

impl AppliedType {
    /// The letter the summary uses.
    pub fn letter(self) -> &'static str {
        match self {
            AppliedType::Add => "A",
            AppliedType::Delete => "D",
            AppliedType::Update => "M",
        }
    }

    /// The `FileDiff` status, which is a different set of words again.
    pub fn file_status(self) -> FileStatus {
        match self {
            AppliedType::Add => FileStatus::Added,
            AppliedType::Delete => FileStatus::Deleted,
            AppliedType::Update => FileStatus::Modified,
        }
    }
}

/// One applied operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    #[serde(rename = "type")]
    pub applied_type: AppliedType,
    pub resource: String,
    pub target: String,
}

/// `ApplyPatchTool.Output`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub applied: Vec<Applied>,
    pub files: Vec<FileDiffInfo>,
}

impl Output {
    /// The resources that landed, in the order they landed.
    pub fn resources(&self) -> Vec<&str> {
        self.applied.iter().map(|item| item.resource.as_str()).collect()
    }
}

/// `toModelOutput`: a fixed header then one letter-prefixed line per operation.
pub fn to_model_output(output: &Output) -> String {
    let mut lines: Vec<String> = vec!["Applied patch sequentially:".to_string()];
    for item in &output.applied {
        lines.push(format!("{} {}", item.applied_type.letter(), item.resource));
    }
    lines.join("\n")
}

/// `fail(path)`: the two shapes, chosen by whether anything was applied.
pub fn fail_message(applied: &[Applied], path: &str) -> String {
    if applied.is_empty() {
        return format!("Unable to apply patch at {}", path);
    }
    let resources: Vec<&str> = applied.iter().map(|item| item.resource.as_str()).collect();
    format!(
        "Patch partially applied before failing at {}. Applied: {}",
        path,
        resources.join(", ")
    )
}

/// The refusal when `patchText` is empty or only whitespace.
pub const ERR_PATCH_TEXT_REQUIRED: &str = "patchText is required";
/// The refusal when the parser found no operation at all.
pub const ERR_EMPTY_PATCH: &str = "patch rejected: empty patch";
/// The refusal when a hunk carries a move.
pub const ERR_MOVE_UNSUPPORTED: &str = "apply_patch moves are not supported yet";

/// `contents.endsWith("\n") || contents === ""` - the trailing newline rule.
pub fn added_content(contents: &str) -> String {
    if contents.is_empty() || contents.ends_with('\n') {
        contents.to_string()
    } else {
        format!("{}\n", contents)
    }
}

/// `patchText.trim()` is empty.
pub fn patch_text_is_blank(patch_text: &str) -> bool {
    patch_text.trim().is_empty()
}

/// Whether the patch contains a move, which the tool refuses outright.
///
/// A move is an `update` hunk that carries a `movePath`; a hunk that merely
/// has the field set to nothing is not a move.
pub fn has_move(patches: &[(AppliedType, Option<String>)]) -> bool {
    patches
        .iter()
        .any(|(kind, move_path)| *kind == AppliedType::Update && move_path.is_some())
}

/// `patchFile`'s status field, for a change of the given kind.
pub fn file_status(kind: AppliedType) -> FileStatus {
    kind.file_status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn applied(kind: AppliedType, resource: &str) -> Applied {
        Applied {
            applied_type: kind,
            resource: resource.to_string(),
            target: format!("/repo/{}", resource),
        }
    }

    #[test]
    fn the_summary_names_its_header_and_letters_each_operation() {
        let out = Output {
            applied: vec![
                applied(AppliedType::Add, "a.ts"),
                applied(AppliedType::Update, "b.ts"),
                applied(AppliedType::Delete, "c.ts"),
            ],
            files: vec![],
        };
        assert_eq!(
            to_model_output(&out),
            "Applied patch sequentially:\nA a.ts\nM b.ts\nD c.ts"
        );
    }

    #[test]
    fn an_empty_patch_still_prints_the_header() {
        let out = Output { applied: vec![], files: vec![] };
        assert_eq!(to_model_output(&out), "Applied patch sequentially:");
    }

    #[test]
    fn the_three_vocabularies_stay_distinct() {
        // wire: add/update/delete
        assert_eq!(serde_json::to_value(AppliedType::Add).unwrap(), json!("add"));
        assert_eq!(serde_json::to_value(AppliedType::Update).unwrap(), json!("update"));
        assert_eq!(serde_json::to_value(AppliedType::Delete).unwrap(), json!("delete"));
        // summary letters
        assert_eq!(AppliedType::Add.letter(), "A");
        assert_eq!(AppliedType::Update.letter(), "M");
        assert_eq!(AppliedType::Delete.letter(), "D");
        // diff status
        assert_eq!(file_status(AppliedType::Add), FileStatus::Added);
        assert_eq!(file_status(AppliedType::Delete), FileStatus::Deleted);
        assert_eq!(file_status(AppliedType::Update), FileStatus::Modified);
    }

    #[test]
    fn a_failure_before_anything_lands_says_so_plainly() {
        assert_eq!(fail_message(&[], "src/a.ts"), "Unable to apply patch at src/a.ts");
    }

    #[test]
    fn a_failure_after_a_partial_apply_names_what_landed() {
        let done = vec![applied(AppliedType::Add, "a.ts"), applied(AppliedType::Update, "b.ts")];
        assert_eq!(
            fail_message(&done, "src/c.ts"),
            "Patch partially applied before failing at src/c.ts. Applied: a.ts, b.ts"
        );
    }

    #[test]
    fn the_partial_list_keeps_the_order_of_application() {
        let done = vec![applied(AppliedType::Update, "z.ts"), applied(AppliedType::Add, "a.ts")];
        assert!(fail_message(&done, "x").ends_with("Applied: z.ts, a.ts"));
    }

    #[test]
    fn an_added_file_gets_one_trailing_newline() {
        assert_eq!(added_content("body"), "body\n");
        assert_eq!(added_content("body\n"), "body\n", "an existing newline is not doubled");
        assert_eq!(added_content("body\n\n"), "body\n\n", "only the missing one is added");
    }

    #[test]
    fn an_empty_body_stays_empty_and_does_not_become_a_newline() {
        assert_eq!(added_content(""), "");
    }

    #[test]
    fn a_blank_patch_text_is_refused() {
        assert!(patch_text_is_blank(""));
        assert!(patch_text_is_blank("   \n\t "));
        assert!(!patch_text_is_blank(" *** Begin Patch"));
        assert_eq!(ERR_PATCH_TEXT_REQUIRED, "patchText is required");
    }

    #[test]
    fn only_an_update_with_a_move_path_counts_as_a_move() {
        assert!(has_move(&[(AppliedType::Update, Some("b.ts".into()))]));
        assert!(!has_move(&[(AppliedType::Update, None)]));
        assert!(!has_move(&[(AppliedType::Add, Some("b.ts".into()))]), "an add cannot move");
        assert!(!has_move(&[(AppliedType::Delete, Some("b.ts".into()))]));
        assert!(!has_move(&[]));
    }

    #[test]
    fn the_refusals_are_verbatim() {
        assert_eq!(ERR_EMPTY_PATCH, "patch rejected: empty patch");
        assert_eq!(ERR_MOVE_UNSUPPORTED, "apply_patch moves are not supported yet");
    }

    #[test]
    fn an_applied_entry_keeps_the_type_field_name() {
        let v = serde_json::to_value(applied(AppliedType::Add, "a.ts")).unwrap();
        assert_eq!(
            v,
            json!({ "type": "add", "resource": "a.ts", "target": "/repo/a.ts" })
        );
    }

    #[test]
    fn the_input_is_the_patch_text_alone() {
        let v = serde_json::to_value(Input { patch_text: "p".into() }).unwrap();
        assert_eq!(v, json!({ "patchText": "p" }));
    }

    #[test]
    fn the_output_carries_the_applied_list_and_the_diffs() {
        let out = Output {
            applied: vec![applied(AppliedType::Update, "a.ts")],
            files: vec![FileDiffInfo {
                file: Some("a.ts".into()),
                patch: None,
                additions: 2.0,
                deletions: 1.0,
                status: Some(FileStatus::Modified),
            }],
        };
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v["applied"][0]["type"], json!("update"));
        assert_eq!(v["files"][0]["status"], json!("modified"));
        assert_eq!(out.resources(), vec!["a.ts"]);
    }
}