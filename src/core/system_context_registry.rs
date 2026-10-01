//! Rust port of `packages/core/src/system-context/registry.ts`.
//!
//! The source is an Effect service backed by `Ref` (`ReadonlyArray<Entry>`)
//! with `register`/`load` effects. The Rust port carries only the wire-facing
//! `Entry` description; the `Ref` state, `Effect.acquireRelease`, the
//! `Scope` dependency and the `Layer` wiring are out of scope (Rule 7) because
//! they are effectful shared state with no pure-logic equivalent to port.
//!
//! # Dependencies not in this batch
//!
//! - `SystemContext.Key` and `SystemContext.SystemContext` come from
//!   `../system-context/index.ts`, ported in `system_context.rs`.

use serde::{Deserialize, Serialize};

/// Canonical re-export of `Key`, owned by `system_context.rs` per the source.
pub use super::system_context::Key;

/// `Entry` from the source: one registered context source.
///
/// `load` is an `Effect` in the source; here it is documented only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "SystemContextRegistry.Entry")]
pub struct Entry {
    #[serde(rename = "key")]
    pub key: Key,
    /// Document-only: the real member is `load: Effect<...>`. The pure Rust
    /// port records only that a loader exists; the effect itself is out of
    /// scope (Rule 7).
    pub has_loader: bool,
}

/// Documentation-only mirror of the source `Interface` (`register`, `load`).
pub mod interface {
    use super::Entry;
    /// `register(entry) => Effect<void, never, Scope.Scope>`.
    pub fn register(_entry: &Entry) {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }
    /// `load() => Effect<SystemContext.SystemContext>`.
    pub fn load() {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn entry_wires_key_by_name() {
        let e = Entry {
            key: Key("core/environment".to_string()),
            has_loader: true,
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["key"], json!("core/environment"));
        let back: Entry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn key_is_a_plain_branded_string() {
        let v = serde_json::to_value(&Key("core/date".to_string())).unwrap();
        assert_eq!(v, json!("core/date"));
    }
}
