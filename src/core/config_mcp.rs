//! Port of `opencode/packages/core/src/config/mcp.ts`.
//!
//! MCP server configuration: a local process (argv array, cwd, environment)
//! and a remote URL (headers, optional OAuth), tagged on `type`. Note the
//! OAuth block is already snake_case in the TS (`client_id`, `callback_port`)
//! and keeps it on the wire; and `oauth` accepts either an object or the
//! literal `false` to disable it, which is why it is not a plain Option.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Timeouts in milliseconds, both optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Timeout {
    /// Maximum time to establish and initialize the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup: Option<i64>,
    /// Maximum time for each request after initialization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<i64>,
}

/// A locally spawned MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Local {
    pub command: Vec<String>,
    /// Working directory; relative paths resolve from the workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
}

/// OAuth settings. Field names stay snake_case, exactly as in the TS.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Oauth {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

/// `oauth: Union([OAuth, Literal(false)])`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OauthOrDisabled {
    Settings(Oauth),
    Disabled(bool),
}

/// A remote MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OauthOrDisabled>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
}

/// `Schema.toTaggedUnion("type")` over Local and Remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Server {
    #[serde(rename = "local")]
    Local(Local),
    #[serde(rename = "remote")]
    Remote(Remote),
}

/// `ConfigV2.MCP`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Info {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<BTreeMap<String, Server>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn local() -> Server {
        Server::Local(Local {
            command: vec!["npx".to_string(), "-y".to_string(), "server".to_string()],
            cwd: Some("/repo".to_string()),
            environment: Some(BTreeMap::from([("K".to_string(), "V".to_string())])),
            disabled: None,
            timeout: Some(Timeout { startup: Some(10_000), request: None }),
        })
    }

    #[test]
    fn a_local_server_is_tagged_local() {
        let v = serde_json::to_value(local()).unwrap();
        assert_eq!(v["type"], json!("local"));
        assert_eq!(v["command"], json!(["npx", "-y", "server"]));
        assert_eq!(v["timeout"], json!({ "startup": 10_000 }));
    }

    #[test]
    fn a_remote_server_keeps_url_and_headers() {
        let s = Server::Remote(Remote {
            url: "https://mcp.example".to_string(),
            headers: Some(BTreeMap::from([("Authorization".to_string(), "Bearer x".to_string())])),
            ..Remote { oauth: None, disabled: None, timeout: None, url: "https://mcp.example".to_string(), headers: None }
        });
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["type"], json!("remote"));
        assert_eq!(v["url"], json!("https://mcp.example"));
        let back: Server = serde_json::from_value(v).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn oauth_keeps_its_snake_case_names_and_accepts_false() {
        let o = Oauth { client_id: Some("id".into()), callback_port: Some(8080), ..Oauth::default() };
        let v = serde_json::to_value(OauthOrDisabled::Settings(o)).unwrap();
        assert_eq!(v, json!({ "client_id": "id", "callback_port": 8080 }));
        assert!(v.get("clientId").is_none());

        let v = serde_json::to_value(OauthOrDisabled::Disabled(false)).unwrap();
        assert_eq!(v, json!(false));
    }

    #[test]
    fn a_server_map_reads_both_kinds() {
        let raw = json!({
            "timeout": { "startup": 1000 },
            "servers": {
                "local": { "type": "local", "command": ["a"] },
                "remote": { "type": "remote", "url": "https://x", "oauth": false }
            }
        });
        let info: Info = serde_json::from_value(raw).unwrap();
        let servers = info.servers.unwrap();
        assert!(matches!(servers["local"], Server::Local(_)));
        assert!(matches!(servers["remote"], Server::Remote(_)));
        assert_eq!(serde_json::to_value(Info::default()).unwrap(), json!({}));
    }

    #[test]
    fn an_unknown_server_kind_is_refused() {
        let raw = json!({ "type": "socket", "url": "x" });
        assert!(serde_json::from_value::<Server>(raw).is_err());
    }
}