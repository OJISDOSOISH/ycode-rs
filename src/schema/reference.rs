//! Port of `opencode/packages/schema/src/reference.ts`.
//!
//! A reference is either a local path or a git repository, tagged on `type`.
//! `Reference.Info` carries the source inline, so the union appears nested in a
//! required field rather than as an option.
//!
//! The module also owns the `reference.updated` event, whose payload is empty:
//! `schema: {}` means `data` is `{}` on the wire, not a missing key.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Absolute path string.
pub type AbsolutePath = String;

/// Wire type string of the only event of this module.
pub const UPDATED: &str = "reference.updated";

/// `define({ type: "reference.updated", schema: {} })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedData {}

pub type UpdatedEvent = Payload<UpdatedData>;

pub fn updated(id: impl Into<String>) -> UpdatedEvent {
    Payload::new(id, UPDATED, UpdatedData {})
}

/// `Reference.LocalSource`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSource {
    pub path: AbsolutePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

/// `Reference.GitSource`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSource {
    pub repository: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

/// `toTaggedUnion("type")` over the two sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Source {
    #[serde(rename = "local")]
    Local(LocalSource),
    #[serde(rename = "git")]
    Git(GitSource),
}

/// `Reference.Info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub path: AbsolutePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    pub source: Source,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_local_source_carries_only_its_path_when_nothing_else_is_set() {
        let s = Source::Local(LocalSource {
            path: "/repo/refs".into(),
            description: None,
            hidden: None,
        });
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v, json!({ "type": "local", "path": "/repo/refs" }));
    }

    #[test]
    fn a_git_source_keeps_its_optional_fields() {
        let s = Source::Git(GitSource {
            repository: "https://github.com/o/r".into(),
            branch: Some("main".into()),
            description: None,
            hidden: Some(true),
        });
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(
            v,
            json!({ "type": "git", "repository": "https://github.com/o/r", "branch": "main", "hidden": true })
        );
    }

    #[test]
    fn info_nests_the_source_in_a_required_field() {
        let info = Info {
            name: "specs".into(),
            path: "/repo/specs".into(),
            description: None,
            hidden: None,
            source: Source::Local(LocalSource {
                path: "/repo/refs".into(),
                description: None,
                hidden: None,
            }),
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["source"]["type"], json!("local"));
        assert!(v.get("description").is_none());
        // the nested source must not repeat the parent's optionals
        assert!(v["source"].get("hidden").is_none());
    }

    #[test]
    fn an_unknown_source_kind_is_refused() {
        let raw = json!({ "type": "svn", "url": "x" });
        assert!(serde_json::from_value::<Source>(raw).is_err());
    }

    #[test]
    fn the_updated_event_has_an_empty_payload_not_a_missing_one() {
        let v = serde_json::to_value(updated("evt_1")).unwrap();
        assert_eq!(v["type"], json!("reference.updated"));
        assert_eq!(v["data"], json!({}));
    }
}