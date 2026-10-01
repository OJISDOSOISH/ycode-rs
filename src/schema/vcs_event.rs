//! Port of `opencode/packages/schema/src/vcs-event.ts`.
//!
//! One event, `vcs.branch.updated`, whose `branch` is optional: an absent
//! branch is a real state (detached head, branch deleted), so the field is
//! skipped on the wire rather than emitted as null.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of the only event of this module.
pub const BRANCH_UPDATED: &str = "vcs.branch.updated";

/// `BranchUpdated = Event.define({ type, schema: { branch? } })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchUpdatedData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

pub type BranchUpdatedEvent = Payload<BranchUpdatedData>;

pub fn branch_updated(id: impl Into<String>, branch: Option<String>) -> BranchUpdatedEvent {
    Payload::new(id, BRANCH_UPDATED, BranchUpdatedData { branch })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_present_branch_is_written() {
        let ev = branch_updated("evt_1", Some("main".to_string()));
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], json!("vcs.branch.updated"));
        assert_eq!(v["data"], json!({ "branch": "main" }));
    }

    #[test]
    fn an_absent_branch_disappears_instead_of_turning_into_null() {
        let ev = branch_updated("evt_2", None);
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["data"], json!({}));
        assert!(v["data"].get("branch").is_none());
    }

    #[test]
    fn the_payload_round_trips_both_shapes() {
        for branch in [Some("main".to_string()), None] {
            let ev = branch_updated("evt_3", branch);
            let back: BranchUpdatedEvent =
                serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
            assert_eq!(back, ev);
        }
    }
}