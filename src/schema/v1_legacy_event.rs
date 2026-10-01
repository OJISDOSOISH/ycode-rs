//! Port of `opencode/packages/schema/src/v1/legacy-event.ts`.
//!
//! One v1 event, `command.executed`, carrying the command name, the session,
//! the raw argument string and the message id. The top-level
//! `schema/src/legacy-event.ts` only re-exports this module, so there is
//! nothing else to port from it.

use serde::{Deserialize, Serialize};

use crate::schema::event::Payload;

/// Wire type string of the only event of this module.
pub const COMMAND_EXECUTED: &str = "command.executed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecutedData {
    pub name: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub arguments: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
}

pub type CommandExecutedEvent = Payload<CommandExecutedData>;

pub fn command_executed(
    id: impl Into<String>,
    name: impl Into<String>,
    session_id: impl Into<String>,
    arguments: impl Into<String>,
    message_id: impl Into<String>,
) -> CommandExecutedEvent {
    Payload::new(
        id,
        COMMAND_EXECUTED,
        CommandExecutedData {
            name: name.into(),
            session_id: session_id.into(),
            arguments: arguments.into(),
            message_id: message_id.into(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_payload_keeps_both_id_capitals() {
        let ev = command_executed("evt_1", "init", "ses_1", "{}", "msg_1");
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], json!("command.executed"));
        assert_eq!(
            v["data"],
            json!({ "name": "init", "sessionID": "ses_1", "arguments": "{}", "messageID": "msg_1" })
        );
        assert!(v["data"].get("sessionId").is_none());
        assert!(v["data"].get("messageId").is_none());
    }

    #[test]
    fn the_payload_round_trips() {
        let ev = command_executed("evt_2", "run", "ses_2", "{\"a\":1}", "msg_2");
        let back: CommandExecutedEvent =
            serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(back, ev);
    }
}