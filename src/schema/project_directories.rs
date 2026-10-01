//! Port of `opencode/packages/schema/src/project-directories.ts`.
//!
//! One event, `project.directories.updated`, carrying `projectID` only.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of the only event of this module.
pub const UPDATED: &str = "project.directories.updated";

/// Project identifier string (`Project.ID`).
pub type ProjectId = String;

/// `define({ type: "project.directories.updated", schema: { projectID } })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedData {
    #[serde(rename = "projectID")]
    pub project_id: ProjectId,
}

pub type UpdatedEvent = Payload<UpdatedData>;

pub fn updated(id: impl Into<String>, project_id: impl Into<ProjectId>) -> UpdatedEvent {
    Payload::new(id, UPDATED, UpdatedData { project_id: project_id.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_payload_keeps_project_id_capitals() {
        let ev = updated("evt_1", "prj_1");
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], json!("project.directories.updated"));
        assert_eq!(v["data"], json!({ "projectID": "prj_1" }));
        assert!(v["data"].get("projectId").is_none());
        assert!(v["data"].get("project_id").is_none());
    }

    #[test]
    fn the_payload_round_trips() {
        let ev = updated("evt_2", "prj_2");
        let back: UpdatedEvent = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(back, ev);
    }
}