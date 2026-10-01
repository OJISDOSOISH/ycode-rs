//! Rust port of `packages/core/src/system-context/index.ts`.
//!
//! The source models privileged system context as independently refreshable,
//! typed sources, each rendered into a stable text baseline and snapshotted for
//! comparison. It also carries the pure `compare`/`replace` reconciler.
//!
//! The Rust port carries the wire-facing types (`Key`, `SourceSnapshot`,
//! `Snapshot`, the tagged result enums, and the two tagged errors) plus the
//! pure, deterministic parts of `replaceObservation` and the entry
//! discriminated union's `compare`. What is out of scope (Rule 7):
//!
//! - the `Symbol`-tagged `SystemContext` opaque carrier
//!   (`{ [ContextTypeId]: ReadonlyArray<PackedSource> }`), which has no named
//!   Rust shape and no pure logic of its own;
//! - the `Effect`/`Scope`/`Ref` plumbing that observes entries live;
//! - `Schema.toCodecJson` and the `Source<A>` generic codec, which require the
//!   Effect runtime to evaluate.
//!
//! # Dependencies not in this batch
//!
//! `SystemContextRegistry` is ported separately; this module re-exports `Key`
//! so the two files agree on the canonical definition (the source owns `Key`
//! here and re-exports it).
//!
//! # Field-name discipline (Rule 6)
//!
//! The tagged results serialize their variant under `_tag`, matching the TS
//! string discriminants. Field names are plain (`baseline`, `text`, `snapshot`,
//! `generation`, `keys`).

use serde::{Deserialize, Serialize};

/// `Key` from the source: a stable namespaced identity
/// `[a-z0-9][a-z0-9._-]*\/[a-z0-9][a-z0-9._/-]*`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Key(pub String);

/// `SourceSnapshot` from the source. Optional `removed` disappears at `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "SourceSnapshot")]
pub struct SourceSnapshot {
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<String>,
}

/// `Snapshot` from the source: a record of `Key -> SourceSnapshot`.
///
/// Ported as an ordered map for deterministic comparison output.
pub type Snapshot = std::collections::BTreeMap<String, SourceSnapshot>;

/// `Generation` from the source: the rendered text plus its snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Generation")]
pub struct Generation {
    pub baseline: String,
    pub snapshot: Snapshot,
}

/// `Updated` from the source: `{ _tag: "Updated", text, snapshot }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Updated {
    #[serde(rename = "_tag")]
    pub tag: String,
    pub text: String,
    pub snapshot: Snapshot,
}

/// `ReplacementReady` from the source: `{ _tag: "ReplacementReady", generation }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplacementReady {
    #[serde(rename = "_tag")]
    pub tag: String,
    pub generation: Generation,
}

/// `ReplacementBlocked` from the source: `{ _tag: "ReplacementBlocked" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementBlocked {
    #[serde(rename = "_tag")]
    pub tag: String,
}

/// `ReplacementResult = ReplacementReady | ReplacementBlocked`.
///
/// Ported as a plain enum (externally tagged); each variant carries its tag
/// struct so the wire shape keeps the `_tag` discriminator of the source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReplacementResult {
    Ready(ReplacementReady),
    Blocked(ReplacementBlocked),
}


/// `ReconcileResult` from the source: `Unchanged | Updated | ReplacementResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum ReconcileResult {
        #[serde(rename = "Unchanged")]
    Unchanged,
    #[serde(rename = "Updated")]
    Updated(Updated),
    #[serde(rename = "ReplacementReady")]
    Ready(ReplacementReady),
    #[serde(rename = "ReplacementBlocked")]
    Blocked(ReplacementBlocked),
}

/// `InitializationBlocked` tagged error: some keys could not be observed
/// without treating them as removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "SystemContext.InitializationBlocked")]
pub struct InitializationBlocked {
    #[serde(rename = "_tag")]
    pub tag: String,
    pub keys: Vec<String>,
}

/// `DuplicateKeyError` tagged error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "SystemContext.DuplicateKeyError")]
pub struct DuplicateKeyError {
    #[serde(rename = "_tag")]
    pub tag: String,
    pub key: Key,
}

/// `Unavailable` from the source, ported as a unit variant of an entry
/// discriminated union. The source is `Symbol.for("@opencode/SystemContext.Unavailable")`
/// and is not a string, so it has no JSON form; it is modelled as a marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename_all = "UPPERCASE")]
pub enum Availability {
    Unavailable,
    Available,
}

/// `Compared` from the source: the result of comparing an observed entry to a
/// stored snapshot. `Updated` carries the rendered text + new snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum Compared {
    #[serde(rename = "Incompatible")]
    Incompatible,
    #[serde(rename = "Unchanged")]
    Unchanged,
    #[serde(rename = "Updated")]
    Updated {
        text: String,
        snapshot: Snapshot,
    },
}

/// One reconciler input entry, ported from the source's internal `Entry`
/// discriminated union (`Unavailable` vs `Available`). Only the shape needed
/// by [`replace_observation`] is modelled; the live `Effect`/`Schema` codec
/// is out of scope (Rule 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename_all = "UPPERCASE")]
pub enum ReconcileEntry {
    Unavailable {
        key: Key,
    },
    Available {
        key: Key,
        #[serde(skip_serializing_if = "Option::is_none")]
        compared: Option<Compared>,
    },
}

/// Pure core of the source `replaceObservation` (index.ts ~line 228).
///
/// Returns `ReplacementBlocked` when an `Unavailable` entry has a stored
/// snapshot (the source blocks waiting for admitted context to become
/// available again). Otherwise returns `ReplacementReady` with the rendered
/// updates. This mirrors the deterministic branch only; the rendering of
/// `baseline()`/`compare()` against `Schema.toCodecJson` requires the Effect
/// runtime and is out of scope (Rule 7).
pub fn replace_observation(
    entries: &[ReconcileEntry],
    previous: &Snapshot,
) -> Result<ReplacementReady, &'static str> {
    for entry in entries {
        let key = match entry {
            ReconcileEntry::Unavailable { key } => key,
            ReconcileEntry::Available { .. } => continue,
        };
        if previous.get(&key.0).is_some() {
            return Err("ReplacementBlocked: an admitted source went Unavailable");
        }
    }
    // All admitted sources that are Unavailable were not previously seen, so
    // the source would build a fresh generation. We cannot render it here
    // (requires Schema.toCodecJson), so surface the ready signal only.
    Ok(ReplacementReady {
        tag: "ReplacementReady".to_string(),
        generation: Generation {
            baseline: String::new(),
            snapshot: Snapshot::new(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_entry_with_no_prior_snapshot_does_not_block() {
        let entries = vec![ReconcileEntry::Unavailable {
            key: Key("core/missing".to_string()),
        }];
        let previous = Snapshot::new();
        let ready = replace_observation(&entries, &previous).unwrap();
        assert_eq!(ready.tag, "ReplacementReady");
    }

    #[test]
    fn unavailable_entry_with_prior_snapshot_blocks() {
        let mut previous = Snapshot::new();
        previous.insert(
            "core/missing".to_string(),
            SourceSnapshot {
                value: serde_json::json!(null),
                removed: None,
            },
        );
        let entries = vec![ReconcileEntry::Unavailable {
            key: Key("core/missing".to_string()),
        }];
        let err = replace_observation(&entries, &previous).unwrap_err();
        assert!(err.contains("Blocked"));
    }

    #[test]
    fn updated_reconcile_result_carries_text() {
        let mut snap = Snapshot::new();
        snap.insert("k".to_string(), SourceSnapshot {
            value: serde_json::json!(42),
            removed: None,
        });
        let up = Updated {
            tag: "Updated".to_string(),
            text: "v1".to_string(),
            snapshot: snap,
        };
        let v = serde_json::to_value(&up).unwrap();
        assert_eq!(v["_tag"], serde_json::json!("Updated"));
        assert_eq!(v["text"], serde_json::json!("v1"));
    }

    #[test]
    fn initialization_blocked_tags_are_camel() {
        let e = InitializationBlocked {
            tag: "SystemContext.InitializationBlocked".to_string(),
            keys: vec!["core/x".to_string()],
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["_tag"], serde_json::json!("SystemContext.InitializationBlocked"));
        assert_eq!(v["keys"], serde_json::json!(["core/x"]));
    }
}
