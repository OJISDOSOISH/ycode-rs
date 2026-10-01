//! Rust port of `packages/core/src/control-plane/move-session.ts`.
//!
//! The source is an Effect service: a `Service` class, a `Layer.effect`
//! implementation built on `Git`, `EventV2`, `ProjectV2` and `SessionStore`,
//! and the `moveSession` effect that captures/clears git changes across a
//! directory move.
//!
//! This port carries only the **pure, wire-facing surface**: the two input
//! structs, the four `Schema.TaggedErrorClass` errors, the `Error` union, and
//! a documentation-only mirror of the `Interface`. The `Layer`, the
//! `Effect.fn` body, the `Database`/Drizzle dependencies and the git
//! capture/apply/discard calls are out of scope and intentionally omitted
//! (Rule 7).
//!
//! # Dependencies not in this batch
//!
//! - `AbsolutePath` (`packages/schema/src/schema.ts`) is a branded string in
//!   the source. It is not yet present as a named type in this crate, so it is
//!   an opaque alias here and should be replaced by the real shared type once
//!   the schema port lands. The existing `crate::core::session_event::RelativePath`
//!   precedent (`pub type RelativePath = String`) is the model.
//! - `ProjectV2.ID`, `SessionV2.NotFoundError` and `SessionSchema.ID` come from
//!   schema modules outside this batch and are likewise opaque aliases.
//!
//! # Field-name discipline (Rule 6)
//!
//! `sessionID`, `projectID` keep their capitals. `directory`, `message`,
//! `expected`, `actual`, `cause` stay snake_case.

use serde::{Deserialize, Serialize};

/// Opaque alias for the schema `AbsolutePath` branded string.
pub type AbsolutePath = String;

/// Opaque alias for `ProjectV2.ID`.
pub type ProjectV2Id = String;

/// Opaque alias for `SessionSchema.ID` / `SessionV2.ID`.
pub type SessionId = String;

/// `Destination` from the source: `{ directory: AbsolutePath }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "MoveSession.Destination")]
pub struct MoveSessionDestination {
    pub directory: AbsolutePath,
}

/// `Input` from the source (`MoveSession.Input`). Optional `moveChanges`
/// disappears at `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "MoveSession.Input")]
pub struct MoveSessionInput {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub destination: MoveSessionDestination,
    #[serde(rename = "moveChanges", skip_serializing_if = "Option::is_none")]
    pub move_changes: Option<bool>,
}

/// `DestinationProjectMismatchError`: `MoveSession.DestinationProjectMismatchError`.
/// Both ids keep camelCase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename = "MoveSession.DestinationProjectMismatchError")]
pub struct DestinationProjectMismatchError {
    #[serde(rename = "expected")]
    pub expected: ProjectV2Id,
    #[serde(rename = "actual")]
    pub actual: ProjectV2Id,
}

/// `ApplyChangesError`: `MoveSession.ApplyChangesError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename = "MoveSession.ApplyChangesError")]
pub struct ApplyChangesError {
    pub message: String,
}

/// `CaptureChangesError`: `MoveSession.CaptureChangesError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename = "MoveSession.CaptureChangesError")]
pub struct CaptureChangesError {
    pub message: String,
}

/// `ResetSourceChangesError`: `MoveSession.ResetSourceChangesError`.
/// `cause` mirrors `Schema.optional(Schema.Defect())`; the source's `Defect`
/// is `unknown`, so this is an opaque `Option<String>` here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename = "MoveSession.ResetSourceChangesError")]
pub struct ResetSourceChangesError {
    pub directory: AbsolutePath,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// `Error` from the source: the union of the four tagged errors above plus
/// `SessionV2.NotFoundError`. `SessionV2.NotFoundError` is not in this batch,
/// so it stays an opaque member of the document. Each concrete error carries
/// its own `_tag` so the set is recoverable on the wire even without a shared
/// base enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveSessionError {
    DestinationProjectMismatchError(DestinationProjectMismatchError),
    CaptureChangesError(CaptureChangesError),
    ApplyChangesError(ApplyChangesError),
    ResetSourceChangesError(ResetSourceChangesError),
    /// `SessionV2.NotFoundError`, from a schema module not in this batch.
    #[allow(dead_code)]
    NotFound(SessionNotFound),
}

/// Opaque stand-in for `SessionV2.NotFoundError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionNotFound {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
}

/// Documentation-only mirror of the source `Interface`:
/// `moveSession(input) => Effect<unknown, Error>`.
/// The real effect body is out of scope (Rule 7).
pub mod interface {
    use super::{MoveSessionError, MoveSessionInput};
    /// `moveSession(input)` — Effect implementation left out (Rule 7).
    pub fn move_session(_input: &MoveSessionInput) -> Result<(), MoveSessionError> {
        unimplemented!("Effect Layer wiring is out of scope for this port")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn input_wire_names_and_optional_disappears() {
        let input = MoveSessionInput {
            session_id: "ses_1".to_string(),
            destination: MoveSessionDestination {
                directory: "/workspace".to_string(),
            },
            move_changes: None,
        };
        let v: Value = serde_json::to_value(&input).unwrap();
        assert_eq!(v["sessionID"], json!("ses_1"));
        assert_eq!(v["destination"]["directory"], json!("/workspace"));
        assert!(!v.as_object().unwrap().contains_key("moveChanges"));
        assert!(!v.to_string().contains("null"));

        let with = MoveSessionInput {
            move_changes: Some(true),
            ..input
        };
        let v: Value = serde_json::to_value(&with).unwrap();
        assert_eq!(v["moveChanges"], json!(true));
    }

    #[test]
    fn tagged_errors_carry_tag_and_camelcase() {
        let mismatch = DestinationProjectMismatchError {
            expected: "prj_a".to_string(),
            actual: "prj_b".to_string(),
        };
        let v: Value = serde_json::to_value(&mismatch).unwrap();
        assert_eq!(v["_tag"], json!("MoveSession.DestinationProjectMismatchError"));
        assert_eq!(v["expected"], json!("prj_a"));
        assert_eq!(v["actual"], json!("prj_b"));

        let reset = ResetSourceChangesError {
            directory: "/src".to_string(),
            message: "src is not a repo".to_string(),
            cause: None,
        };
        let v: Value = serde_json::to_value(&reset).unwrap();
        assert_eq!(v["_tag"], json!("MoveSession.ResetSourceChangesError"));
        assert!(!v.as_object().unwrap().contains_key("cause"));

        let with_cause = ResetSourceChangesError {
            cause: Some("boom".to_string()),
            ..reset
        };
        let v: Value = serde_json::to_value(&with_cause).unwrap();
        assert_eq!(v["cause"], json!("boom"));
    }
}
