//! Rust port of `packages/core/src/skill/guidance.ts`.
//!
//! The source is an Effect service (`Service` + `layer`) that renders the
//! available-skill system-prompt block. Its only pure, wire-facing type is
//! `Summary`; everything else lives behind `Effect`, `SkillV2.Service`,
//! `PermissionV2.evaluate` and `Schema.toCodecJson`. That implementation is
//! out of scope for this port (Rule 7).
//!
//! # Dependencies not in this batch
//!
//! - `SystemContext` (returned by `SystemContext.make`) and its `Key` are from
//!   `packages/core/src/system-context/index.ts`, ported in a separate file.

use serde::{Deserialize, Serialize};

/// `Summary` from the source: one `{ name, description }` entry rendered into
/// the skill list the model sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub name: String,
    pub description: String,
}

/// Documentation-only mirror of the source `Interface` (`load`): the real
/// implementation is an `Effect`, left out (Rule 7).
pub mod interface {
    use super::Summary;
    /// `load(agent: AgentV2.Selection) => Effect.Effect<SystemContext.SystemContext>`.
    pub fn load(_agent_selection: ()) -> Vec<Summary> {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_round_trips_and_has_no_camelcase_fields() {
        let s = Summary {
            name: "explain".to_string(),
            description: "Explains the codebase".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "{\"name\":\"explain\",\"description\":\"Explains the codebase\"}");
        let back: Summary = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
