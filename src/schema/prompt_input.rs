//! Port of `opencode/packages/schema/src/prompt-input.ts`.
//!
//! `PromptInput.FileAttachment` is `{uri, name?, description?, source?}` and
//! `PromptInput.Prompt` is `{text, files?, agents?}` - exactly the shapes
//! already carried by `core::session_event` (which mirrors `prompt.ts`, where
//! the same types originate). This module re-exports them instead of writing a
//! third copy: three definitions of one wire shape is how `sessionID` /
//! `callID` drifted once already.
//!
//! Known difference, recorded rather than hidden: `session_message::Prompt`
//! types `files` and `agents` as plain vectors and drops `source` and the
//! agent objects. That copy is the reduced one; consolidating it onto these
//! types is still open.

pub use crate::core::session_event::{AgentAttachment, FileAttachment, Prompt, PromptSource as Source};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_file_attachment_keeps_uri_and_drops_absent_optionals() {
        let f = FileAttachment {
            uri: "file:///a.txt".to_string(),
            mime: "text/plain".to_string(),
            name: Some("a.txt".to_string()),
            description: None,
            source: None,
        };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["uri"], json!("file:///a.txt"));
        assert_eq!(v["name"], json!("a.txt"));
        assert!(v.get("description").is_none());
        assert!(v.get("source").is_none());
        // `path` is the name this contract never had on the wire.
        assert!(v.get("path").is_none());
    }

    #[test]
    fn a_prompt_without_files_or_agents_writes_only_its_text() {
        let p = Prompt { text: "hello".to_string(), files: None, agents: None };
        assert_eq!(serde_json::to_value(&p).unwrap(), json!({ "text": "hello" }));
    }

    #[test]
    fn agents_are_objects_not_strings() {
        let p = Prompt {
            text: "go".to_string(),
            files: None,
            agents: Some(vec![AgentAttachment { name: "build".to_string(), source: None }]),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["agents"], json!([{ "name": "build" }]));
    }

    #[test]
    fn a_source_carries_its_three_fields() {
        let s = Source {
            start: 0.0,
            end: 12.0,
            text: "file:///a".to_string(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert!(v.get("start").is_some() && v.get("end").is_some() && v.get("text").is_some());
    }
}