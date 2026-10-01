//! Rust port of `packages/schema/src/v1/permission.ts`.
//!
//! The source defines the permission request/reply wire shapes plus a small
//! set of branded identifiers and enums. The Rust port carries only the data
//! shapes themselves; the Effect `Layer` wiring that assembles a live permission
//! service is out of scope for this module and is intentionally not ported
//! here.
//!
//! # Field-name discipline (Rule 6)
//!
//! `sessionID`, `requestID`, `callID`, `projectID` and `messageID` are kept
//! exactly in camelCase via `#[serde(rename)]`. No camelCase field is
//! accidentally renamed, no snake_case field (`id`, `tool`) is touched.

use serde::{Deserialize, Serialize};

/// `packages/schema/src/session-id.ts` `SessionID`, opaque here.
pub type SessionId = String;

/// `packages/schema/src/project.ts` `Project.ID`, opaque here.
pub type ProjectId = String;

/// `ID` from the source: any string starting with `"per"`, branded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PermissionId(pub String);

impl PermissionId {
    pub const PREFIX: &'static str = "per";

    pub fn make(id: impl Into<String>) -> Result<Self, InvalidPermissionId> {
        let id = id.into();
        if !id.starts_with(Self::PREFIX) {
            return Err(InvalidPermissionId { id, prefix: Self::PREFIX });
        }
        Ok(Self(id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ID {id:?} does not start with {prefix}")]
pub struct InvalidPermissionId {
    pub id: String,
    pub prefix: &'static str,
}

/// `Action`: closed literal union `allow | deny | ask`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    Allow,
    Deny,
    Ask,
}

/// `Rule` from the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: String,
    pub pattern: String,
    pub action: PermissionAction,
}

pub type PermissionRuleset = Vec<PermissionRule>;

/// Optional `tool` block on [`PermissionRequest`]: `{ messageID, callID }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRequestTool {
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "callID")]
    pub call_id: String,
}

/// `Request` (`PermissionRequest`). Optional `tool` disappears at `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "PermissionRequest")]
pub struct PermissionRequest {
    pub id: PermissionId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub permission: String,
    pub patterns: Vec<String>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub always: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<PermissionRequestTool>,
}

/// `Reply`: closed literal union `once | always | reject`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionReply {
    Once,
    Always,
    Reject,
}

/// `ReplyBody` (`PermissionReplyBody`). Optional `message` disappears at `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "PermissionReplyBody")]
pub struct PermissionReplyBody {
    pub reply: PermissionReply,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// `Approval` (`PermissionApproval`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "PermissionApproval")]
pub struct PermissionApproval {
    #[serde(rename = "projectID")]
    pub project_id: ProjectId,
    pub patterns: Vec<String>,
}

/// `AskInput` (`PermissionAskInput`): spreads `Request.fields`, adds
/// optional `id` and required `ruleset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "PermissionAskInput")]
pub struct PermissionAskInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<PermissionId>,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub permission: String,
    pub patterns: Vec<String>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub always: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<PermissionRequestTool>,
    pub ruleset: PermissionRuleset,
}

/// `ReplyInput` (`PermissionReplyInput`): spreads `ReplyBody.fields`,
/// adds `requestID`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "PermissionReplyInput")]
pub struct PermissionReplyInput {
    #[serde(rename = "requestID")]
    pub request_id: PermissionId,
    pub reply: PermissionReply,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// `Event` discriminants from the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionEvent {
    Asked,
    Replied,
}

impl PermissionEvent {
        pub const fn as_str(self) -> &'static str {
        match self {
            PermissionEvent::Asked => "permission.asked",
            PermissionEvent::Replied => "permission.replied",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape test for `PermissionRequest`: wire names exact, `tool`
    /// disappears when `None` instead of becoming `null` (Rule 5).
    #[test]
    fn request_wire_names_and_optional_tool_disappears() {
        let id = PermissionId::make("per_abc123").unwrap();
        let with_tool = PermissionRequest {
            id: id.clone(),
            session_id: "ses_456".to_string(),
            permission: "file.read".to_string(),
            patterns: vec!["/etc/*".to_string()],
            metadata: std::collections::HashMap::new(),
            always: vec!["once".to_string()],
            tool: Some(PermissionRequestTool {
                message_id: "msg_1".to_string(),
                call_id: "call_1".to_string(),
            }),
        };
        let json = serde_json::to_string(&with_tool).unwrap();
        assert!(json.contains("\"sessionID\":\"ses_456\""));
        assert!(!json.contains("session_id"));
        assert!(json.contains("\"messageID\":\"msg_1\""));
        assert!(json.contains("\"callID\":\"call_1\""));
        let back: PermissionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, with_tool);

        let no_tool = PermissionRequest {
            tool: None,
            ..with_tool
        };
        let json = serde_json::to_string(&no_tool).unwrap();
        assert!(!json.contains("\"tool\""));
        assert!(!json.contains("null"));
    }

    /// `AskInput` has optional `id` and `tool`; both must vanish at `None`.
    #[test]
    fn ask_input_optionals_disappear() {
        let id = PermissionId::make("per_xyz").unwrap();
        let input = PermissionAskInput {
            id: None,
            session_id: "ses_x".to_string(),
            permission: "tool.call".to_string(),
            patterns: vec![],
            metadata: std::collections::HashMap::new(),
            always: vec![],
            tool: None,
            ruleset: vec![],
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(!json.contains("\"tool\""));
        assert!(!json.contains("null"));

        let with_id = PermissionAskInput {
            id: Some(id),
            ..input
        };
        let json = serde_json::to_string(&with_id).unwrap();
        assert!(json.contains("\"id\":\"per_xyz\""));
        let back: PermissionAskInput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id.unwrap().0, "per_xyz");
    }

    /// `ReplyBody` optional `message` disappears at `None`.
    #[test]
    fn reply_body_message_disappears() {
        let body = PermissionReplyBody {
            reply: PermissionReply::Once,
            message: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(!json.contains("\"message\""));
        assert!(!json.contains("null"));

        let with_msg = PermissionReplyBody {
            message: Some("ok".to_string()),
            ..body
        };
        let json = serde_json::to_string(&with_msg).unwrap();
        assert!(json.contains("\"message\":\"ok\""));
    }

    #[test]
    fn reply_input_request_id_is_camelcase() {
        let id = PermissionId::make("per_q").unwrap();
        let input = PermissionReplyInput {
            request_id: id,
            reply: PermissionReply::Always,
            message: None,
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"requestID\":\"per_q\""));
        assert!(!json.contains("request_id"));
    }

    #[test]
    fn approval_project_id_is_camelcase() {
        let a = PermissionApproval {
            project_id: "prj_1".to_string(),
            patterns: vec!["*".to_string()],
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"projectID\":\"prj_1\""));
        assert!(!json.contains("project_id"));
    }

    #[test]
    fn invalid_permission_id_rejects_wrong_prefix() {
        assert!(PermissionId::make("ses_abc").is_err());
        assert!(PermissionId::make("").is_err());
        assert_eq!(PermissionId::make("evt_1").unwrap_err().prefix, "per");
    }

    #[test]
    fn enums_serialize_lowercase() {
        assert_eq!(serde_json::to_string(&PermissionAction::Allow).unwrap(), "\"allow\"");
        assert_eq!(serde_json::to_string(&PermissionAction::Deny).unwrap(), "\"deny\"");
        assert_eq!(serde_json::to_string(&PermissionAction::Ask).unwrap(), "\"ask\"");
        assert_eq!(serde_json::to_string(&PermissionReply::Once).unwrap(), "\"once\"");
        assert_eq!(serde_json::to_string(&PermissionReply::Always).unwrap(), "\"always\"");
        assert_eq!(serde_json::to_string(&PermissionReply::Reject).unwrap(), "\"reject\"");
    }

    #[test]
    fn event_discriminants_match_source() {
        assert_eq!(PermissionEvent::Asked.as_str(), "permission.asked");
        assert_eq!(PermissionEvent::Replied.as_str(), "permission.replied");
    }

    /// Round-trips `PermissionId`'s newtype wrapper through JSON.
    #[test]
    fn permission_id_serializes_as_plain_string() {
        let id = PermissionId::make("per_deadbeef").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"per_deadbeef\"");
        let back: PermissionId = serde_json::from_str("\"per_deadbeef\"").unwrap();
        assert_eq!(back, id);
    }
}
