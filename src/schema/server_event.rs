//! Port of `opencode/packages/schema/src/server-event.ts`.
//!
//! Two events with empty payloads: `server.connected` and - note the prefix -
//! `global.disposed`. The second one is declared in the server module but its
//! type string belongs to the `global` namespace; copying the file name onto
//! the tag would be the obvious mistake here.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of `Connected`.
pub const CONNECTED: &str = "server.connected";
/// Wire type string of `Disposed`, which is `global.*`, not `server.*`.
pub const DISPOSED: &str = "global.disposed";

/// `Connected = Event.define({ type: "server.connected", schema: {} })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedData {}

/// `Disposed = Event.define({ type: "global.disposed", schema: {} })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisposedData {}

pub type ConnectedEvent = Payload<ConnectedData>;
pub type DisposedEvent = Payload<DisposedData>;

pub fn connected(id: impl Into<String>) -> ConnectedEvent {
    Payload::new(id, CONNECTED, ConnectedData {})
}

pub fn disposed(id: impl Into<String>) -> DisposedEvent {
    Payload::new(id, DISPOSED, DisposedData {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn connected_keeps_the_server_prefix() {
        let v = serde_json::to_value(connected("evt_1")).unwrap();
        assert_eq!(v["type"], json!("server.connected"));
        assert_eq!(v["data"], json!({}));
    }

    #[test]
    fn disposed_keeps_the_global_prefix() {
        // The file is server-event.ts but the tag is global.disposed.
        let v = serde_json::to_value(disposed("evt_2")).unwrap();
        assert_eq!(v["type"], json!("global.disposed"));
        assert!(v.get("type").unwrap().as_str().unwrap().starts_with("global."));
    }
}