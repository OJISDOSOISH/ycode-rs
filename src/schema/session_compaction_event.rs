//! Port of `opencode/packages/schema/src/session-compaction-event.ts`.
//!
//! One event, `session.compacted`, carrying the session id.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of the only event of this module.
pub const COMPACTED: &str = "session.compacted";

/// Session identifier string (`SessionID`).
pub type SessionId = String;

/// `Compacted = Event.define({ type, schema: { sessionID } })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactedData {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
}

pub type CompactedEvent = Payload<CompactedData>;

pub fn compacted(id: impl Into<String>, session_id: impl Into<SessionId>) -> CompactedEvent {
    Payload::new(id, COMPACTED, CompactedData { session_id: session_id.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_payload_keeps_session_id_capitals() {
        let v = serde_json::to_value(compacted("evt_1", "ses_1")).unwrap();
        assert_eq!(v["type"], json!("session.compacted"));
        assert_eq!(v["data"], json!({ "sessionID": "ses_1" }));
        assert!(v["data"].get("sessionId").is_none());
    }

    #[test]
    fn the_payload_round_trips() {
        let ev = compacted("evt_2", "ses_2");
        let back: CompactedEvent = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(back, ev);
    }
}