//! Port of `opencode/packages/schema/src/mcp-event.ts`.
//!
//! Two events: `mcp.tools.changed` (server name) and
//! `mcp.browser.open.failed` (mcp name and url). Note the camelCase field
//! `mcpName` inside an otherwise lowercase dotted type string.

use serde::{Deserialize, Serialize};

use super::event::Payload;

/// Wire type string of `ToolsChanged`.
pub const TOOLS_CHANGED: &str = "mcp.tools.changed";
/// Wire type string of `BrowserOpenFailed`.
pub const BROWSER_OPEN_FAILED: &str = "mcp.browser.open.failed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsChangedData {
    pub server: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserOpenFailedData {
    #[serde(rename = "mcpName")]
    pub mcp_name: String,
    pub url: String,
}

pub type ToolsChangedEvent = Payload<ToolsChangedData>;
pub type BrowserOpenFailedEvent = Payload<BrowserOpenFailedData>;

pub fn tools_changed(id: impl Into<String>, server: impl Into<String>) -> ToolsChangedEvent {
    Payload::new(id, TOOLS_CHANGED, ToolsChangedData { server: server.into() })
}

pub fn browser_open_failed(
    id: impl Into<String>,
    mcp_name: impl Into<String>,
    url: impl Into<String>,
) -> BrowserOpenFailedEvent {
    Payload::new(
        id,
        BROWSER_OPEN_FAILED,
        BrowserOpenFailedData { mcp_name: mcp_name.into(), url: url.into() },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn both_type_strings_use_the_mcp_prefix() {
        assert_eq!(
            serde_json::to_value(tools_changed("evt_1", "docs")).unwrap()["type"],
            json!("mcp.tools.changed")
        );
        assert_eq!(
            serde_json::to_value(browser_open_failed("evt_2", "browser", "https://x")).unwrap()["type"],
            json!("mcp.browser.open.failed")
        );
    }

    #[test]
    fn the_browser_failure_keeps_mcp_name_camel_case() {
        let v = serde_json::to_value(browser_open_failed("evt_3", "b", "https://x")).unwrap();
        assert_eq!(v["data"], json!({ "mcpName": "b", "url": "https://x" }));
        assert!(v["data"].get("mcp_name").is_none());
    }

    #[test]
    fn both_payloads_round_trip() {
        let a = tools_changed("evt_4", "docs");
        assert_eq!(
            serde_json::from_str::<ToolsChangedEvent>(&serde_json::to_string(&a).unwrap()).unwrap(),
            a
        );
        let b = browser_open_failed("evt_5", "b", "https://x");
        assert_eq!(
            serde_json::from_str::<BrowserOpenFailedEvent>(&serde_json::to_string(&b).unwrap()).unwrap(),
            b
        );
    }
}