//! Port of `opencode/packages/schema/src/ide-event.ts`.
//!
//! One event, `ide.installed`, carrying the ide name.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of the only event of this module.
pub const INSTALLED: &str = "ide.installed";

/// `Installed = Event.define({ type: "ide.installed", schema: { ide } })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledData {
    pub ide: String,
}

pub type InstalledEvent = Payload<InstalledData>;

pub fn installed(id: impl Into<String>, ide: impl Into<String>) -> InstalledEvent {
    Payload::new(id, INSTALLED, InstalledData { ide: ide.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_payload_is_the_ide_name() {
        let v = serde_json::to_value(installed("evt_1", "vscode")).unwrap();
        assert_eq!(v["type"], json!("ide.installed"));
        assert_eq!(v["data"], json!({ "ide": "vscode" }));
    }

    #[test]
    fn the_payload_round_trips() {
        let ev = installed("evt_2", "cursor");
        let back: InstalledEvent = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(back, ev);
    }
}