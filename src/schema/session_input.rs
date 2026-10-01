//! Port of `opencode/packages/schema/src/session-input.ts`.
//!
//! `Admitted` is a prompt that entered a session's input queue: the sequence
//! number it was admitted at, the message id, the session, the prompt itself,
//! how it is delivered, when it was created, and the sequence number it was
//! promoted at once a turn picked it up.
//!
//! `Prompt` and `Delivery` are re-exported rather than copied: the prompt
//! contract lives in `core::session_event` and the delivery enum in
//! `session_delivery`.

use serde::{Deserialize, Serialize};

pub use super::session_delivery::Delivery;
pub use crate::core::session_event::Prompt;

/// Message identifier (`SessionMessage.ID`).
pub type MessageId = String;
/// Session identifier.
pub type SessionId = String;

/// `SessionInput.Admitted`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Admitted {
    pub admitted_seq: i64,
    pub id: MessageId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub prompt: Prompt,
    pub delivery: Delivery,
    #[serde(rename = "timeCreated")]
    pub time_created: i64,
    #[serde(rename = "promotedSeq", skip_serializing_if = "Option::is_none")]
    pub promoted_seq: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn exemple() -> Admitted {
        Admitted {
            admitted_seq: 3,
            id: "msg_1".to_string(),
            session_id: "ses_1".to_string(),
            prompt: Prompt { text: "go".to_string(), files: None, agents: None },
            delivery: Delivery::Queue,
            time_created: 1_700_000_000_000_i64,
            promoted_seq: None,
        }
    }

    #[test]
    fn the_wire_names_keep_their_capitals() {
        let v = serde_json::to_value(exemple()).unwrap();
        assert_eq!(v["admittedSeq"], json!(3));
        assert_eq!(v["sessionID"], json!("ses_1"));
        assert_eq!(v["timeCreated"], json!(1_700_000_000_000_i64));
        assert_eq!(v["delivery"], json!("queue"));
        assert_eq!(v["prompt"], json!({ "text": "go" }));
        assert!(v.get("promotedSeq").is_none());
        assert!(v.get("admitted_seq").is_none());
        assert!(v.get("session_id").is_none());
        assert!(v.get("time_created").is_none());
    }

    #[test]
    fn a_promoted_prompt_writes_its_sequence() {
        let mut a = exemple();
        a.promoted_seq = Some(4);
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["promotedSeq"], json!(4));
    }

    #[test]
    fn the_admitted_payload_round_trips() {
        let a = exemple();
        let back: Admitted = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a);
    }
}