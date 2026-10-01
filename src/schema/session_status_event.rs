//! Port of `opencode/packages/schema/src/session-status-event.ts`.
//!
//! `Info` is a three-way union tagged on `type`: `idle`, `busy`, and `retry`
//! (which carries the attempt number, a message, the sequence of the next
//! step, and an optional action block). `session.status` wraps it with the
//! session id; `session.idle` is the deprecated one-field variant.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of `Status`.
pub const STATUS: &str = "session.status";
/// Wire type string of the deprecated `Idle`.
pub const IDLE: &str = "session.idle";

/// Session identifier.
pub type SessionId = String;

/// The optional `action` block of a `retry` status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryAction {
    pub reason: String,
    pub provider: String,
    pub title: String,
    pub message: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// `SessionStatus`: idle, busy, or retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Info {
    #[serde(rename = "idle")]
    Idle {},
    #[serde(rename = "busy")]
    Busy {},
    #[serde(rename = "retry")]
    Retry {
        attempt: i64,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<RetryAction>,
        next: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusData {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub status: Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdleData {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
}

pub type StatusEvent = Payload<StatusData>;
pub type IdleEvent = Payload<IdleData>;

pub fn status(id: impl Into<String>, session_id: impl Into<SessionId>, status: Info) -> StatusEvent {
    Payload::new(id, STATUS, StatusData { session_id: session_id.into(), status })
}

pub fn idle(id: impl Into<String>, session_id: impl Into<SessionId>) -> IdleEvent {
    Payload::new(id, IDLE, IdleData { session_id: session_id.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn idle_and_busy_are_bare_tags() {
        let v = serde_json::to_value(status("evt_1", "ses_1", Info::Idle {})).unwrap();
        assert_eq!(v["data"]["status"], json!({ "type": "idle" }));
        let v = serde_json::to_value(status("evt_2", "ses_1", Info::Busy {})).unwrap();
        assert_eq!(v["data"]["status"], json!({ "type": "busy" }));
    }

    #[test]
    fn a_retry_status_carries_attempt_next_and_message() {
        let info = Info::Retry { attempt: 2, message: "rate limited".to_string(), action: None, next: 5 };
        let v = serde_json::to_value(status("evt_3", "ses_1", info)).unwrap();
        assert_eq!(
            v["data"]["status"],
            json!({ "type": "retry", "attempt": 2, "message": "rate limited", "next": 5 })
        );
        assert!(v["data"]["status"].get("action").is_none());
    }

    #[test]
    fn a_retry_action_is_written_when_present() {
        let info = Info::Retry {
            attempt: 1,
            message: "m".to_string(),
            action: Some(RetryAction {
                reason: "quota".to_string(),
                provider: "anthropic".to_string(),
                title: "t".to_string(),
                message: "m".to_string(),
                label: "l".to_string(),
                link: None,
            }),
            next: 1,
        };
        let v = serde_json::to_value(status("evt_4", "ses_1", info)).unwrap();
        assert_eq!(v["data"]["status"]["action"]["provider"], json!("anthropic"));
        assert!(v["data"]["status"]["action"].get("link").is_none());
    }

    #[test]
    fn the_three_status_variants_round_trip() {
        let cases = [
            Info::Idle {},
            Info::Busy {},
            Info::Retry { attempt: 0, message: "m".to_string(), action: None, next: 1 },
        ];
        for info in cases {
            let ev = status("evt_5", "ses_1", info.clone());
            let back: StatusEvent = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
            assert_eq!(back.data.status, info);
        }
    }

    #[test]
    fn the_deprecated_idle_event_keeps_its_own_type_string() {
        let v = serde_json::to_value(idle("evt_6", "ses_1")).unwrap();
        assert_eq!(v["type"], json!("session.idle"));
        assert_eq!(v["data"], json!({ "sessionID": "ses_1" }));
    }
}