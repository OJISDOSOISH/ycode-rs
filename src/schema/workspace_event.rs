//! Port of `opencode/packages/schema/src/workspace-event.ts`.
//!
//! Three events plus the `ConnectionStatus` struct, which is reused as the
//! payload of `workspace.status` (`schema: ConnectionStatus.fields`). The four
//! status literals are a closed set, so they are an enum rather than a string.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type strings of the three events.
pub const READY: &str = "workspace.ready";
pub const FAILED: &str = "workspace.failed";
pub const STATUS: &str = "workspace.status";

/// Workspace identifier.
pub type WorkspaceId = String;

/// `Schema.Literals(["connected", "connecting", "disconnected", "error"])`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "error")]
    Error,
}

/// `WorkspaceEvent.ConnectionStatus`, also the payload of `workspace.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionStatus {
    #[serde(rename = "workspaceID")]
    pub workspace_id: WorkspaceId,
    pub status: ConnectionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyData {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedData {
    pub message: String,
}

pub type ReadyEvent = Payload<ReadyData>;
pub type FailedEvent = Payload<FailedData>;
pub type StatusEvent = Payload<ConnectionStatus>;

pub fn ready(id: impl Into<String>, name: impl Into<String>) -> ReadyEvent {
    Payload::new(id, READY, ReadyData { name: name.into() })
}

pub fn failed(id: impl Into<String>, message: impl Into<String>) -> FailedEvent {
    Payload::new(id, FAILED, FailedData { message: message.into() })
}

pub fn status(id: impl Into<String>, workspace_id: impl Into<WorkspaceId>, state: ConnectionState) -> StatusEvent {
    Payload::new(id, STATUS, ConnectionStatus { workspace_id: workspace_id.into(), status: state })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_status_payload_keeps_workspace_id_capitals() {
        let v = serde_json::to_value(status("evt_1", "wrk_1", ConnectionState::Connected)).unwrap();
        assert_eq!(v["type"], json!("workspace.status"));
        assert_eq!(v["data"], json!({ "workspaceID": "wrk_1", "status": "connected" }));
        assert!(v["data"].get("workspaceId").is_none());
    }

    #[test]
    fn all_four_status_literals_survive_the_round_trip() {
        for (state, text) in [
            (ConnectionState::Connected, "connected"),
            (ConnectionState::Connecting, "connecting"),
            (ConnectionState::Disconnected, "disconnected"),
            (ConnectionState::Error, "error"),
        ] {
            assert_eq!(serde_json::to_value(state).unwrap(), json!(text));
            assert_eq!(serde_json::from_value::<ConnectionState>(json!(text)).unwrap(), state);
        }
    }

    #[test]
    fn an_unknown_status_is_refused() {
        assert!(serde_json::from_value::<ConnectionState>(json!("sleeping")).is_err());
    }

    #[test]
    fn ready_and_failed_carry_one_field_each() {
        assert_eq!(serde_json::to_value(ready("evt_2", "w")).unwrap()["data"], json!({ "name": "w" }));
        assert_eq!(serde_json::to_value(failed("evt_3", "e")).unwrap()["data"], json!({ "message": "e" }));
    }
}