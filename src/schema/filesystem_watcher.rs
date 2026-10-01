//! Port of `opencode/packages/schema/src/filesystem-watcher.ts`.
//!
//! One event, `file.watcher.updated`, carrying the path and a watcher event
//! kind. The kind is a closed set of three literals in the TS, so it is a
//! three-variant enum here rather than a string that could hold anything.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of the only event of this module.
pub const UPDATED: &str = "file.watcher.updated";

/// `Schema.Literals(["add", "change", "unlink"])`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchEvent {
    #[serde(rename = "add")]
    Add,
    #[serde(rename = "change")]
    Change,
    #[serde(rename = "unlink")]
    Unlink,
}

/// `define({ type, schema: { file, event } })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedData {
    pub file: String,
    pub event: WatchEvent,
}

pub type UpdatedEvent = Payload<UpdatedData>;

pub fn updated(id: impl Into<String>, file: impl Into<String>, event: WatchEvent) -> UpdatedEvent {
    Payload::new(id, UPDATED, UpdatedData { file: file.into(), event })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_literal_round_trips_through_the_enum() {
        for (variant, text) in [(WatchEvent::Add, "add"), (WatchEvent::Change, "change"), (WatchEvent::Unlink, "unlink")] {
            let v = serde_json::to_value(updated("evt_1", "/repo/a.txt", variant)).unwrap();
            assert_eq!(v["data"]["event"], json!(text));
            let back: UpdatedEvent = serde_json::from_value(v).unwrap();
            assert_eq!(back.data.event, variant);
        }
    }

    #[test]
    fn an_unknown_event_kind_is_refused() {
        let bad = json!({
            "id": "evt_2", "type": "file.watcher.updated",
            "data": { "file": "/a", "event": "rename" }
        });
        assert!(serde_json::from_value::<UpdatedEvent>(bad).is_err());
    }

    #[test]
    fn the_type_string_is_the_file_watcher_one() {
        let v = serde_json::to_value(updated("evt_3", "/a", WatchEvent::Add)).unwrap();
        assert_eq!(v["type"], json!("file.watcher.updated"));
    }
}