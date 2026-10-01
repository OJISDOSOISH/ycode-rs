//! Port of `opencode/packages/schema/src/tui-event.ts`.
//!
//! Four events. Two details are easy to get wrong:
//! - `tui.command.execute` accepts EITHER one of seventeen known command names
//!   OR any string, so it is not a closed enum. The known names are still worth
//!   a list, and a value outside it must stay legal.
//! - `tui.toast.show` has `duration` with a decoding default of 5000 ms: an
//!   absent duration decodes to 5000 rather than failing.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type strings of the four events.
pub const PROMPT_APPEND: &str = "tui.prompt.append";
pub const COMMAND_EXECUTE: &str = "tui.command.execute";
pub const TOAST_SHOW: &str = "tui.toast.show";
pub const SESSION_SELECT: &str = "tui.session.select";

/// Default decoding value of `ToastShow.duration`.
pub const DEFAULT_TOAST_DURATION: i64 = 5000;

/// The sixteen command names the TUI knows. Not exhaustive on the wire: any
/// string is accepted, this list is what the UI itself can trigger.
pub const KNOWN_COMMANDS: [&str; 16] = [
    "session.list",
    "session.new",
    "session.share",
    "session.interrupt",
    "session.compact",
    "session.page.up",
    "session.page.down",
    "session.line.up",
    "session.line.down",
    "session.half.page.up",
    "session.half.page.down",
    "session.first",
    "session.last",
    "prompt.clear",
    "prompt.submit",
    "agent.cycle",
];

/// Toast severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToastVariant {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptAppendData {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandExecuteData {
    /// Any string is legal, known or not.
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToastShowData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub message: String,
    pub variant: ToastVariant,
    /// Decoding default: absent means 5000 ms, not an error.
    #[serde(default = "default_toast_duration")]
    pub duration: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSelectData {
    #[serde(rename = "sessionID")]
    pub session_id: String,
}

fn default_toast_duration() -> i64 {
    DEFAULT_TOAST_DURATION
}

pub type PromptAppendEvent = Payload<PromptAppendData>;
pub type CommandExecuteEvent = Payload<CommandExecuteData>;
pub type ToastShowEvent = Payload<ToastShowData>;
pub type SessionSelectEvent = Payload<SessionSelectData>;

pub fn prompt_append(id: impl Into<String>, text: impl Into<String>) -> PromptAppendEvent {
    Payload::new(id, PROMPT_APPEND, PromptAppendData { text: text.into() })
}

pub fn command_execute(id: impl Into<String>, command: impl Into<String>) -> CommandExecuteEvent {
    Payload::new(id, COMMAND_EXECUTE, CommandExecuteData { command: command.into() })
}

pub fn toast_show(
    id: impl Into<String>,
    title: Option<String>,
    message: impl Into<String>,
    variant: ToastVariant,
    duration: Option<i64>,
) -> ToastShowEvent {
    Payload::new(
        id,
        TOAST_SHOW,
        ToastShowData {
            title,
            message: message.into(),
            variant,
            duration: duration.unwrap_or(DEFAULT_TOAST_DURATION),
        },
    )
}

pub fn session_select(id: impl Into<String>, session_id: impl Into<String>) -> SessionSelectEvent {
    Payload::new(id, SESSION_SELECT, SessionSelectData { session_id: session_id.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_command_outside_the_known_list_stays_legal() {
        let v = serde_json::to_value(command_execute("evt_1", "session.brand.new")).unwrap();
        assert_eq!(v["type"], json!("tui.command.execute"));
        assert_eq!(v["data"]["command"], json!("session.brand.new"));
        let back: CommandExecuteEvent = serde_json::from_value(v).unwrap();
        assert_eq!(back.data.command, "session.brand.new");
    }

    #[test]
    fn the_known_command_list_has_sixteen_entries() {
        assert_eq!(KNOWN_COMMANDS.len(), 16);
        assert_eq!(KNOWN_COMMANDS[0], "session.list");
        assert_eq!(KNOWN_COMMANDS[15], "agent.cycle");
    }

    #[test]
    fn an_absent_toast_duration_decodes_to_five_seconds() {
        let raw = json!({
            "id": "evt_2",
            "type": "tui.toast.show",
            "data": { "message": "saved", "variant": "success" }
        });
        let ev: ToastShowEvent = serde_json::from_value(raw).unwrap();
        assert_eq!(ev.data.duration, DEFAULT_TOAST_DURATION);
        assert_eq!(ev.data.duration, 5000);
    }

    #[test]
    fn an_explicit_toast_duration_survives() {
        let ev = toast_show("evt_3", Some("t".into()), "m", ToastVariant::Error, Some(250));
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["data"], json!({ "title": "t", "message": "m", "variant": "error", "duration": 250 }));
    }

    #[test]
    fn every_toast_variant_keeps_its_spelling() {
        for (variant, text) in [
            (ToastVariant::Info, "info"),
            (ToastVariant::Success, "success"),
            (ToastVariant::Warning, "warning"),
            (ToastVariant::Error, "error"),
        ] {
            assert_eq!(serde_json::to_value(variant).unwrap(), json!(text));
        }
    }

    #[test]
    fn session_select_keeps_session_id_capitals() {
        let v = serde_json::to_value(session_select("evt_4", "ses_1")).unwrap();
        assert_eq!(v["type"], json!("tui.session.select"));
        assert_eq!(v["data"], json!({ "sessionID": "ses_1" }));
    }
}