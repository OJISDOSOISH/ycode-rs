//! Port of `opencode/packages/schema/src/event.ts`.
//!
//! The shared event envelope. `define({ type, schema })` in the TS builds
//! `Schema.Struct({ id, metadata?, type, durable?, location?, data })`, and
//! every event module in `src/schema/` reuses that exact field set - so it
//! lives here once instead of being copied into twenty modules.
//!
//! `type` is a literal per event in the TS. Rust cannot carry that in the type
//! system without unstable const generics, so the envelope keeps it as a
//! `String` and each event module exposes its own constant plus a constructor
//! that fills it in. Every module's test asserts the exact wire string, which
//! is what the literal gave us for free in the source.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::location::LocationRef;

/// Event identifier (`evt_...` in the TS source).
pub type EventId = String;

/// `durable: { version, aggregate }` as passed to `define`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableOptions {
    pub version: i64,
    pub aggregate: String,
}

/// `durable: { aggregateID, seq, version }` as carried on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableRef {
    #[serde(rename = "aggregateID")]
    pub aggregate_id: String,
    pub seq: i64,
    pub version: i64,
}

impl DurableRef {
    pub fn new(aggregate_id: impl Into<String>, seq: i64, version: i64) -> Self {
        Self { aggregate_id: aggregate_id.into(), seq, version }
    }
}

/// The event envelope: `Payload` in the TS source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payload<D> {
    pub id: EventId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, Value>>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durable: Option<DurableRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationRef>,
    pub data: D,
}

impl<D> Payload<D> {
    /// Envelope for an event whose payload is `data`.
    pub fn new(id: impl Into<EventId>, kind: impl Into<String>, data: D) -> Self {
        Self {
            id: id.into(),
            metadata: None,
            kind: kind.into(),
            durable: None,
            location: None,
            data,
        }
    }

    pub fn with_location(mut self, location: LocationRef) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_durable(mut self, durable: DurableRef) -> Self {
        self.durable = Some(durable);
        self
    }

    pub fn with_metadata(mut self, metadata: BTreeMap<String, Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// `versionedType(type, version)`: the manifest key of a durable event.
pub fn versioned_type(kind: &str, version: i64) -> String {
    format!("{}.{}", kind, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Demo {
        name: String,
    }

    const KIND: &str = "demo.updated";

    #[test]
    fn the_envelope_carries_the_five_fields_of_the_ts() {
        let p = Payload::new("evt_1", KIND, Demo { name: "x".to_string() });
        let v = serde_json::to_value(&p).unwrap();
        // id, type and data are always present; the three optionals are absent.
        assert_eq!(v, json!({ "id": "evt_1", "type": "demo.updated", "data": { "name": "x" } }));
    }

    #[test]
    fn optional_parts_are_written_when_present() {
        let p = Payload::new("evt_1", KIND, Demo { name: "x".to_string() })
            .with_location(LocationRef::new("/repo"))
            .with_durable(DurableRef::new("ses_1", 7, 1))
            .with_metadata(BTreeMap::from([("k".to_string(), json!(1))]));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["location"]["directory"], json!("/repo"));
        assert_eq!(v["durable"]["aggregateID"], json!("ses_1"));
        assert_eq!(v["durable"]["seq"], json!(7));
        assert_eq!(v["metadata"]["k"], json!(1));
    }

    #[test]
    fn the_envelope_round_trips() {
        let p = Payload::new("evt_1", KIND, Demo { name: "x".to_string() }).with_durable(DurableRef::new("ses_1", 2, 1));
        let json = serde_json::to_string(&p).unwrap();
        let back: Payload<Demo> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn versioned_type_appends_the_version() {
        assert_eq!(versioned_type("session.next.prompted", 1), "session.next.prompted.1");
    }

    #[test]
    fn an_unknown_optional_field_is_ignored_on_read() {
        let v = json!({
            "id": "evt_9",
            "type": "demo.updated",
            "unknownField": 12,
            "data": { "name": "y" }
        });
        let p: Payload<Demo> = serde_json::from_value(v).unwrap();
        assert_eq!(p.data.name, "y");
        assert_eq!(p.id, "evt_9");
    }
}