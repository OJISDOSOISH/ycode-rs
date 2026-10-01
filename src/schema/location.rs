//! Port of `opencode/packages/schema/src/location.ts`.
//!
//! Two shapes only: a `Ref` carried by events, and an `Info` that also names
//! the project. `AbsolutePath` is a plain string here, exactly like the TS
//! branded string: the branding is a compile-time check in TypeScript and has
//! no runtime representation.
//!
//! Wire names: `directory`, `workspaceID`, `project.id`, `project.directory`.
//! `workspaceID` keeps the capital ID - it is not `workspaceId`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use serde_json::Value;

/// Absolute path string (`AbsolutePath` in the TS source).
pub type AbsolutePath = String;
/// Project identifier string.
pub type ProjectId = String;
/// Workspace identifier string.
pub type WorkspaceId = String;

/// `Location.Ref`: the minimal location carried by events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationRef {
    pub directory: AbsolutePath,
    #[serde(rename = "workspaceID", skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<WorkspaceId>,
}

impl LocationRef {
    pub fn new(directory: impl Into<AbsolutePath>) -> Self {
        Self { directory: directory.into(), workspace_id: None }
    }

    pub fn with_workspace(directory: impl Into<AbsolutePath>, workspace_id: impl Into<WorkspaceId>) -> Self {
        Self { directory: directory.into(), workspace_id: Some(workspace_id.into()) }
    }
}

/// Project block nested in `Location.Info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationProject {
    pub id: ProjectId,
    pub directory: AbsolutePath,
}

/// `Location.Info`: location plus its project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationInfo {
    pub directory: AbsolutePath,
    #[serde(rename = "workspaceID", skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<WorkspaceId>,
    pub project: LocationProject,
}

/// `response(data)`: `{ location, data }` wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationResponse<D> {
    pub location: LocationInfo,
    pub data: D,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_ref_keeps_the_workspace_id_capitals() {
        let v = serde_json::to_value(LocationRef::with_workspace("/repo", "wrk_1")).unwrap();
        assert_eq!(v, json!({ "directory": "/repo", "workspaceID": "wrk_1" }));
        assert!(v.get("workspaceId").is_none());
        assert!(v.get("workspace_id").is_none());
    }

    #[test]
    fn an_omitted_workspace_disappears_from_the_payload() {
        let v = serde_json::to_value(LocationRef::new("/repo")).unwrap();
        assert_eq!(v, json!({ "directory": "/repo" }));
    }

    #[test]
    fn info_round_trips_with_its_project_block() {
        let info = LocationInfo {
            directory: "/repo".to_string(),
            workspace_id: Some("wrk_1".to_string()),
            project: LocationProject { id: "prj_1".to_string(), directory: "/repo/proj".to_string() },
        };
        let json = serde_json::to_string(&info).unwrap();
        let relue: LocationInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(relue, info);
        assert!(json.contains(r#""workspaceID":"wrk_1""#));
        assert!(json.contains(r#""project":{"id":"prj_1""#));
    }

    #[test]
    fn response_wraps_data_next_to_the_location() {
        let r = LocationResponse {
            location: LocationInfo {
                directory: "/repo".to_string(),
                workspace_id: None,
                project: LocationProject { id: "prj_1".to_string(), directory: "/repo".to_string() },
            },
            data: BTreeMap::from([("k".to_string(), Value::from(1))]),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("location").is_some());
        assert_eq!(v["data"]["k"], json!(1));
    }
}