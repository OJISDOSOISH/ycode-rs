//! Port of `opencode/packages/schema/src/file-diff.ts`.
//!
//! `Info` describes one changed file in a snapshot diff. Two details worth
//! naming: `additions` and `deletions` are `Schema.Finite`, so they are `f64`
//! here, not integers; and `file`, `patch` and `status` are all optional.

use serde::{Deserialize, Serialize};

/// `status`: the closed set of git statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "modified")]
    Modified,
}

/// `SnapshotFileDiff` (`FileDiff.Info`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    pub additions: f64,
    pub deletions: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<FileStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_the_two_counts_are_always_present() {
        let info = Info {
            file: None,
            patch: None,
            additions: 3.0,
            deletions: 0.0,
            status: None,
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap(),
            json!({ "additions": 3.0, "deletions": 0.0 })
        );
    }

    #[test]
    fn the_full_shape_matches_the_ts() {
        let info = Info {
            file: Some("src/a.rs".to_string()),
            patch: Some("@@ -1 +1 @@".to_string()),
            additions: 10.0,
            deletions: 2.0,
            status: Some(FileStatus::Modified),
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap(),
            json!({
                "file": "src/a.rs",
                "patch": "@@ -1 +1 @@",
                "additions": 10.0,
                "deletions": 2.0,
                "status": "modified"
            })
        );
    }

    #[test]
    fn every_status_literal_is_stable() {
        for (v, text) in [(FileStatus::Added, "added"), (FileStatus::Deleted, "deleted"), (FileStatus::Modified, "modified")] {
            assert_eq!(serde_json::to_value(v).unwrap(), json!(text));
            assert_eq!(serde_json::from_value::<FileStatus>(json!(text)).unwrap(), v);
        }
    }

    #[test]
    fn the_info_round_trips() {
        let info = Info {
            file: Some("a".to_string()),
            patch: None,
            additions: 1.0,
            deletions: 1.0,
            status: Some(FileStatus::Added),
        };
        let back: Info = serde_json::from_str(&serde_json::to_string(&info).unwrap()).unwrap();
        assert_eq!(back, info);
    }
}