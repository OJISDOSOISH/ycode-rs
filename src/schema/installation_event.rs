//! Port of `opencode/packages/schema/src/installation-event.ts`.
//!
//! Two events whose payloads are the same single field: `installation.updated`
//! and `installation.update-available`. The hyphen in the second type string is
//! load-bearing.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of `Updated`.
pub const UPDATED: &str = "installation.updated";
/// Wire type string of `UpdateAvailable`.
pub const UPDATE_AVAILABLE: &str = "installation.update-available";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionData {
    pub version: String,
}

pub type UpdatedEvent = Payload<VersionData>;
pub type UpdateAvailableEvent = Payload<VersionData>;

pub fn updated(id: impl Into<String>, version: impl Into<String>) -> UpdatedEvent {
    Payload::new(id, UPDATED, VersionData { version: version.into() })
}

pub fn update_available(id: impl Into<String>, version: impl Into<String>) -> UpdateAvailableEvent {
    Payload::new(id, UPDATE_AVAILABLE, VersionData { version: version.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_available_type_keeps_its_hyphen() {
        let v = serde_json::to_value(update_available("evt_1", "1.2.3")).unwrap();
        assert_eq!(v["type"], json!("installation.update-available"));
        assert!(!v["type"].as_str().unwrap().contains('_'));
    }

    #[test]
    fn both_payloads_are_the_version_string() {
        assert_eq!(
            serde_json::to_value(updated("evt_2", "1.2.3")).unwrap()["data"],
            json!({ "version": "1.2.3" })
        );
        assert_eq!(
            serde_json::to_value(update_available("evt_3", "1.2.4")).unwrap()["data"],
            json!({ "version": "1.2.4" })
        );
    }
}