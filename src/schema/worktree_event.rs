//! Port of `opencode/packages/schema/src/worktree-event.ts`.
//!
//! Two events: `worktree.ready` (name, optional branch) and `worktree.failed`
//! (message).

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of `Ready`.
pub const READY: &str = "worktree.ready";
/// Wire type string of `Failed`.
pub const FAILED: &str = "worktree.failed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyData {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedData {
    pub message: String,
}

pub type ReadyEvent = Payload<ReadyData>;
pub type FailedEvent = Payload<FailedData>;

pub fn ready(id: impl Into<String>, name: impl Into<String>, branch: Option<String>) -> ReadyEvent {
    Payload::new(id, READY, ReadyData { name: name.into(), branch })
}

pub fn failed(id: impl Into<String>, message: impl Into<String>) -> FailedEvent {
    Payload::new(id, FAILED, FailedData { message: message.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_ready_event_with_a_branch_carries_it() {
        let v = serde_json::to_value(ready("evt_1", "wt", Some("main".to_string()))).unwrap();
        assert_eq!(v["type"], json!("worktree.ready"));
        assert_eq!(v["data"], json!({ "name": "wt", "branch": "main" }));
    }

    #[test]
    fn a_ready_event_without_a_branch_omits_the_key() {
        let v = serde_json::to_value(ready("evt_2", "wt", None)).unwrap();
        assert_eq!(v["data"], json!({ "name": "wt" }));
    }

    #[test]
    fn a_failed_event_carries_only_its_message() {
        let v = serde_json::to_value(failed("evt_3", "boom")).unwrap();
        assert_eq!(v["type"], json!("worktree.failed"));
        assert_eq!(v["data"], json!({ "message": "boom" }));
    }
}