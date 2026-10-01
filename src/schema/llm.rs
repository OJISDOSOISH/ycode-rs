//! Port of `opencode/packages/schema/src/llm.ts`.
//!
//! The provider metadata map and the tool result content union. `ToolContent`
//! is a two-variant tagged union: a text part and a file part that carries a
//! `uri` (never a `path`) plus a `mime`, with an optional `name`.
//!
//! The same content union also exists in `core::session_event` for session
//! messages; the two are separate contracts in the TS as well, and this file
//! covers the LLM-facing one.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// `LLM.ProviderMetadata`: provider name to arbitrary metadata.
pub type ProviderMetadata = BTreeMap<String, BTreeMap<String, Value>>;

/// `Tool.TextContent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTextContent {
    pub text: String,
}

/// `Tool.FileContent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolFileContent {
    pub uri: String,
    pub mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `toTaggedUnion("type")` over the two content shapes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text(ToolTextContent),
    #[serde(rename = "file")]
    File(ToolFileContent),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_content_writes_its_tag() {
        let c = ToolContent::Text(ToolTextContent { text: "hello".into() });
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            json!({ "type": "text", "text": "hello" })
        );
    }

    #[test]
    fn file_content_uses_uri_and_omits_an_absent_name() {
        let c = ToolContent::File(ToolFileContent {
            uri: "file:///a.txt".into(),
            mime: "text/plain".into(),
            name: None,
        });
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(
            v,
            json!({ "type": "file", "uri": "file:///a.txt", "mime": "text/plain" })
        );
        assert!(v.get("name").is_none());
        assert!(v.get("path").is_none(), "this contract never had a path field");
    }

    #[test]
    fn both_variants_round_trip() {
        let cases = vec![
            ToolContent::Text(ToolTextContent { text: "t".into() }),
            ToolContent::File(ToolFileContent {
                uri: "u".into(),
                mime: "m".into(),
                name: Some("n".into()),
            }),
        ];
        for c in cases {
            let back: ToolContent =
                serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
            assert_eq!(back, c);
        }
    }

    #[test]
    fn an_unknown_content_kind_is_refused() {
        assert!(serde_json::from_value::<ToolContent>(json!({ "type": "image" })).is_err());
    }

    #[test]
    fn provider_metadata_is_nested_by_provider() {
        let m: ProviderMetadata = serde_json::from_str(r#"{"copilot":{"reasoningOpaque":"x"}}"#).unwrap();
        assert_eq!(m["copilot"]["reasoningOpaque"], json!("x"));
    }
}