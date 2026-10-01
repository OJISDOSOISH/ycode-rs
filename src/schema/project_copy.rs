//! Port of `opencode/packages/schema/src/project-copy.ts`.
//!
//! Inputs of the project copy and remove operations. `StrategyID` is trimmed
//! and checked non-empty in the TS; the check is reproduced as a constructor
//! that returns an error, because a branded string with a runtime check has no
//! direct Rust equivalent.
//!
//! `force` on `RemoveInput` is REQUIRED in the TS, not optional: a removal
//! that does not say whether it may delete untracked work is refused.

use serde::{Deserialize, Serialize};

/// Absolute path string.
pub type AbsolutePath = String;

/// Project identifier string.
pub type ProjectId = String;

/// `Schema.Trim + isNonEmpty`: a trimmed, non-empty strategy name.
pub fn parse_strategy_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `ProjectCopy.CreateInput`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateInput {
    #[serde(rename = "projectID")]
    pub project_id: ProjectId,
    pub strategy: String,
    #[serde(rename = "sourceDirectory")]
    pub source_directory: AbsolutePath,
    pub directory: AbsolutePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `ProjectCopy.RemoveInput`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveInput {
    #[serde(rename = "projectID")]
    pub project_id: ProjectId,
    pub directory: AbsolutePath,
    /// Required, not optional.
    pub force: bool,
}

/// `ProjectCopy.Copy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub directory: AbsolutePath,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_strategy_id_is_trimmed_and_must_not_be_empty() {
        assert_eq!(parse_strategy_id("  copy  "), Some("copy".to_string()));
        assert_eq!(parse_strategy_id("copy"), Some("copy".to_string()));
        assert_eq!(parse_strategy_id("   "), None);
        assert_eq!(parse_strategy_id(""), None);
    }

    #[test]
    fn create_input_keeps_its_id_capitals_and_omits_an_absent_name() {
        let input = CreateInput {
            project_id: "prj_1".into(),
            strategy: "copy".into(),
            source_directory: "/a".into(),
            directory: "/b".into(),
            name: None,
        };
        let v = serde_json::to_value(&input).unwrap();
        assert_eq!(
            v,
            json!({
                "projectID": "prj_1",
                "strategy": "copy",
                "sourceDirectory": "/a",
                "directory": "/b"
            })
        );
        assert!(v.get("name").is_none());
    }

    #[test]
    fn remove_input_requires_force_and_keeps_it_when_false() {
        let input = RemoveInput {
            project_id: "prj_1".into(),
            directory: "/b".into(),
            force: false,
        };
        let v = serde_json::to_value(&input).unwrap();
        assert_eq!(v["force"], json!(false), "false is a value, not an absence");
        assert!(v.get("force").is_some());
    }

    #[test]
    fn a_remove_input_without_force_does_not_deserialise() {
        let raw = json!({ "projectID": "prj_1", "directory": "/b" });
        assert!(serde_json::from_value::<RemoveInput>(raw).is_err());
    }

    #[test]
    fn copy_carries_only_its_directory() {
        let c = Copy { directory: "/b".into() };
        assert_eq!(serde_json::to_value(&c).unwrap(), json!({ "directory": "/b" }));
    }
}