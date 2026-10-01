//! Port of `opencode/packages/schema/src/lsp-event.ts`.
//!
//! One event, `lsp.updated`, with an empty payload: the TS `schema: {}` means
//! `data` is an empty object, not an absent field.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of the only event of this module.
pub const UPDATED: &str = "lsp.updated";

/// `Updated = Event.define({ type: "lsp.updated", schema: {} })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedData {}

/// Envelope for `lsp.updated`.
pub type UpdatedEvent = Payload<UpdatedData>;

/// Build the envelope with the right type string.
pub fn updated(id: impl Into<String>) -> UpdatedEvent {
    Payload::new(id, UPDATED, UpdatedData {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_wire_type_is_lsp_updated() {
        let ev = updated("evt_1");
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], json!("lsp.updated"));
        // `schema: {}` is an empty object on the wire, not a missing key.
        assert_eq!(v["data"], json!({}));
    }

    #[test]
    fn the_payload_round_trips() {
        let ev = updated("evt_2");
        let json = serde_json::to_string(&ev).unwrap();
        let back: UpdatedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
    }
}