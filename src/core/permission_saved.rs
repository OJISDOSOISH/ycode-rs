//! Rust port of `packages/core/src/permission/saved.ts`.
//!
//! The source is an Effect service whose `Layer` reads its state from a Drizzle
//! database (`PermissionTable`). The Rust port carries only the wire-facing
//! shapes: the two input structs and the read signatures of the `Interface`.
//! The `Layer` itself, the `Database.Service` dependency, and the Drizzle query
//! builders are out of scope and intentionally not ported here (Rule 7).
//!
//! # Dependencies not in this batch
//!
//! - `PermissionSaved.ID` and `PermissionSaved.Info` come from
//!   `packages/schema/src/permission-saved.ts`, which is not part of this port
//!   batch. It is treated as an opaque branded string here (`PermissionSavedId`)
//!   and should be replaced by the real type once that schema file is ported.
//! - `ProjectV2.ID` is likewise an opaque branded string (`ProjectV2Id`).
//!
//! # Field-name discipline (Rule 6)
//!
//! `projectID` is kept in camelCase. `id`, `action`, `resource` stay
//! snake_case. No numeric field is mis-typed as `String`.

use serde::{Deserialize, Serialize};

/// `packages/schema/src/project-id.ts` `ProjectV2.ID`, opaque here.
pub type ProjectV2Id = String;

/// `packages/schema/src/permission-saved.ts` `ID`, opaque here.
pub type PermissionSavedId = String;

/// `ListInput` from the source. Optional `projectID` disappears at `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename = "PermissionSaved.ListInput")]
pub struct PermissionSavedListInput {
    #[serde(rename = "projectID", skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectV2Id>,
}

/// `AddInput` from the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "PermissionSaved.AddInput")]
pub struct PermissionSavedAddInput {
    #[serde(rename = "projectID")]
    pub project_id: ProjectV2Id,
    pub action: String,
    pub resources: Vec<String>,
}

/// Pure-shape mirror of `PermissionSaved.Info` (from the not-yet-ported
/// `permission-saved.ts` schema). Kept here so callers in this module do not
/// need the schema crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "PermissionSaved.Info")]
pub struct PermissionSavedInfo {
    pub id: PermissionSavedId,
    #[serde(rename = "projectID")]
    pub project_id: ProjectV2Id,
    pub action: String,
    pub resource: String,
}

/// Read-only projection of the service `Interface` (`list`, `add`, `remove`).
///
/// Each variant is documented against the source's `Effect.fn` signatures; the
/// `Effect`/`Layer`/Drizzle implementations are left out (Rule 7).
pub mod interface {
    use super::{PermissionSavedAddInput, PermissionSavedId, PermissionSavedInfo, PermissionSavedListInput};

    /// `list(input?)` -> `ReadonlyArray<Info>`.
    pub fn list(_input: &PermissionSavedListInput) -> Vec<PermissionSavedInfo> {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }

    /// `add(input)` -> `void`.
    pub fn add(_input: &PermissionSavedAddInput) {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }

    /// `remove(id)` -> `void`.
    pub fn remove(_id: PermissionSavedId) {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_input_project_id_disappears_when_none() {
        let input = PermissionSavedListInput {
            project_id: None,
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(!json.contains("projectID"));
        assert!(!json.contains("null"));

        let with = PermissionSavedListInput {
            project_id: Some("prj_1".to_string()),
        };
        let json = serde_json::to_string(&with).unwrap();
        assert!(json.contains("\"projectID\":\"prj_1\""));
    }

    #[test]
    fn add_input_wire_names() {
        let input = PermissionSavedAddInput {
            project_id: "prj_2".to_string(),
            action: "allow".to_string(),
            resources: vec!["file.read".to_string(), "file.write".to_string()],
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"projectID\":\"prj_2\""));
        assert!(json.contains("\"resources\":["));
        assert!(!json.contains("project_id"));
    }

    #[test]
    fn info_round_trips_camelcase() {
        let info = PermissionSavedInfo {
            id: "psv_abc".to_string(),
            project_id: "prj_3".to_string(),
            action: "deny".to_string(),
            resource: "dir/*".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"projectID\":\"prj_3\""));
        assert!(!json.contains("project_id"));
        let back: PermissionSavedInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
    }
}
