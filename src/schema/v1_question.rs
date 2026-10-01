//! Rust port of `packages/schema/src/v1/question.ts`.
//!
//! Pure wire shapes: the question request/reply payloads and the closed
//! literal enums they use. The Effect `Layer` that dispatches a live question
//! to an agent is out of scope and intentionally not ported here (Rule 7).

use serde::{Deserialize, Serialize};

/// `packages/schema/src/session-id.ts` `SessionID`, opaque here.
pub type SessionId = String;

/// `packages/schema/src/session/v1/session.ts` `SessionV1.MessageID`, opaque.
pub type MessageId = String;

/// `ID` from the source: a string starting with `"que"`, branded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuestionId(pub String);

impl QuestionId {
    pub const PREFIX: &'static str = "que";

    pub fn make(id: impl Into<String>) -> Result<Self, InvalidQuestionId> {
        let id = id.into();
        if !id.starts_with(Self::PREFIX) {
            return Err(InvalidQuestionId { id, prefix: Self::PREFIX });
        }
        Ok(Self(id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ID {id:?} does not start with {prefix}")]
pub struct InvalidQuestionId {
    pub id: String,
    pub prefix: &'static str,
}

/// A single answer choice, ported from the source `Option` struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionOption")]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// One question with its available choices. `custom` is optional and
/// disappears at `None` (Rule 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionInfo")]
pub struct QuestionInfo {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<bool>,
}

/// The prompt form of a question: base minus the optional `custom`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionPrompt")]
pub struct QuestionPrompt {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,
}

/// `Tool` reference: `{ messageID, callID }`. Both camelCase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionTool {
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
}

/// `Request` (`QuestionRequest`). Optional `tool` disappears at `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionRequest")]
pub struct QuestionRequest {
    pub id: QuestionId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub questions: Vec<QuestionInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<QuestionTool>,
}

/// `Answer` is an array of selected labels.
pub type QuestionAnswer = Vec<String>;

/// `Reply` (`QuestionReply`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionReply")]
pub struct QuestionReply {
    pub answers: Vec<QuestionAnswer>,
}

/// `Replied` (`QuestionReplied`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionReplied")]
pub struct QuestionReplied {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "requestID")]
    pub request_id: QuestionId,
    pub answers: Vec<QuestionAnswer>,
}

/// `Rejected` (`QuestionRejected`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "QuestionRejected")]
pub struct QuestionRejected {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "requestID")]
    pub request_id: QuestionId,
}

/// `Event` discriminants from the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionEvent {
    Asked,
    Replied,
    Rejected,
}

impl QuestionEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            QuestionEvent::Asked => "question.asked",
            QuestionEvent::Replied => "question.replied",
            QuestionEvent::Rejected => "question.rejected",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_names_and_optional_tool_disappears() {
        let id = QuestionId::make("que_abc").unwrap();
        let with_tool = QuestionRequest {
            id: id.clone(),
            session_id: "ses_1".to_string(),
            questions: vec![QuestionInfo {
                question: "q".to_string(),
                header: "h".to_string(),
                options: vec![QuestionOption {
                    label: "a".to_string(),
                    description: "d".to_string(),
                }],
                multiple: None,
                custom: None,
            }],
            tool: Some(QuestionTool {
                message_id: "msg_1".to_string(),
                call_id: "call_1".to_string(),
            }),
        };
        let json = serde_json::to_string(&with_tool).unwrap();
        assert!(json.contains("\"sessionID\":\"ses_1\""));
        assert!(!json.contains("session_id"));
        assert!(json.contains("\"messageID\":\"msg_1\""));
        assert!(json.contains("\"callID\":\"call_1\""));
        assert!(!json.contains("\"tool\":null"));

        let no_tool = QuestionRequest {
            tool: None,
            ..with_tool
        };
        let json = serde_json::to_string(&no_tool).unwrap();
        assert!(!json.contains("\"tool\""));
        assert!(!json.contains("null"));
        let back: QuestionRequest = serde_json::from_str(&serde_json::to_string(&QuestionRequest {
            tool: Some(QuestionTool { message_id: "m".into(), call_id: "c".into() }),
            ..no_tool
        }).unwrap()).unwrap();
        assert_eq!(back.tool.unwrap().call_id, "c");
    }

    #[test]
    fn replied_and_rejected_keep_camelcase_request_id() {
        let id = QuestionId::make("que_r").unwrap();
        let replied = QuestionReplied {
            session_id: "ses_2".to_string(),
            request_id: id.clone(),
            answers: vec![vec!["a".to_string()]],
        };
        let json = serde_json::to_string(&replied).unwrap();
        assert!(json.contains("\"requestID\":\"que_r\""));
        assert!(!json.contains("request_id"));
        assert!(json.contains("\"sessionID\":\"ses_2\""));

        let rejected = QuestionRejected {
            session_id: "ses_3".to_string(),
            request_id: id,
        };
        let json = serde_json::to_string(&rejected).unwrap();
        assert!(json.contains("\"requestID\":\"que_r\""));
    }

    #[test]
    fn info_custom_field_disappears_when_none() {
        let info = QuestionInfo {
            question: "q".to_string(),
            header: "h".to_string(),
            options: vec![],
            multiple: None,
            custom: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("\"multiple\""));
        assert!(!json.contains("\"custom\""));
        assert!(!json.contains("null"));

        let with = QuestionInfo {
            multiple: Some(true),
            custom: Some(false),
            ..info
        };
        let json = serde_json::to_string(&with).unwrap();
        assert!(json.contains("\"multiple\":true"));
        assert!(json.contains("\"custom\":false"));
    }

    #[test]
    fn invalid_question_id_rejects_wrong_prefix() {
        assert!(QuestionId::make("ses_x").is_err());
        assert!(QuestionId::make("").is_err());
        assert_eq!(QuestionId::make("per_1").unwrap_err().prefix, "que");
    }

    #[test]
    fn event_discriminants_match_source() {
        assert_eq!(QuestionEvent::Asked.as_str(), "question.asked");
        assert_eq!(QuestionEvent::Replied.as_str(), "question.replied");
        assert_eq!(QuestionEvent::Rejected.as_str(), "question.rejected");
    }
}
