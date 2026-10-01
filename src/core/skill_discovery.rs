//! Rust port of `packages/core/src/skill/discovery.ts`.
//!
//! The source is an Effect service (`Service` + `Layer.effect`) that downloads
//! and extracts skill packages over HTTP, coalescing concurrency and doing
//! atomic rename staging with `FSUtil`, `Global`, `httpClient` and `path`.
//!
//! This port carries only the wire-facing input/output structs: `IndexSkill`
//! and `Index`. The HTTP download pipeline, the staging/rename logic, the
//! `Layer` wiring and the `Effect.fn` body are out of scope and intentionally
//! omitted (Rule 7).
//!
//! # Dependencies not in this batch
//!
//! - `AbsolutePath` is a branded string from `packages/schema/src/schema.ts`
//!   (not yet a named Rust type); treated as an opaque `String` alias.
//!
//! # Field-name discipline (Rule 6)
//!
//! No camelCase fields appear in these two structs; `version` stays a plain
//! field name.

use serde::{Deserialize, Serialize};

/// Opaque alias for the schema `AbsolutePath` branded string.
pub type AbsolutePath = String;

/// `IndexSkill` from the source. Optional `version` disappears at `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "SkillDiscovery.IndexSkill")]
pub struct IndexSkill {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub files: Vec<String>,
}

/// `Index` from the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "SkillDiscovery.Index")]
pub struct Index {
    pub skills: Vec<IndexSkill>,
}

/// Documentation-only mirror of the source `Interface` (`pull`): the real
/// implementation is an `Effect` over HTTP and FS, left out (Rule 7).
pub mod interface {
    use super::AbsolutePath;
    /// `pull(url) => Effect.Effect<AbsolutePath[]>`.
    pub fn pull(_url: &str) -> Vec<AbsolutePath> {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn index_skill_optional_version_disappears() {
        let with = IndexSkill {
            name: "skill-a".to_string(),
            version: Some("1.2.3".to_string()),
            files: vec!["a.ts".to_string(), "b.ts".to_string()],
        };
        let v = serde_json::to_value(&with).unwrap();
        assert_eq!(v["name"], json!("skill-a"));
        assert_eq!(v["version"], json!("1.2.3"));
        assert_eq!(v["files"], json!(["a.ts", "b.ts"]));

        let without = IndexSkill {
            version: None,
            ..with
        };
        let v = serde_json::to_value(&without).unwrap();
        assert!(!v.as_object().unwrap().contains_key("version"));
        assert!(!v.to_string().contains("null"));
    }

    #[test]
    fn index_round_trips() {
        let index = Index {
            skills: vec![IndexSkill {
                name: "x".to_string(),
                version: None,
                files: vec!["x.js".to_string()],
            }],
        };
        let json = serde_json::to_string(&index).unwrap();
        assert!(json.contains("\"skills\""));
        let back: Index = serde_json::from_str(&json).unwrap();
        assert_eq!(back, index);
    }
}
