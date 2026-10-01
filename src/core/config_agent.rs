//! Port of `opencode/packages/core/src/config/agent.ts`.
//!
//! Agent configuration: the model to use, an optional per-agent request
//! override, prompt text, visibility, colour, step budget and permissions.
//!
//! `Color` is a union of a named palette and a `#rrggbb` string. The TS
//! validates the hex form with a regex; a regex engine is out of scope for a
//! pure data module, so `is_hex_color` performs the same check by hand and is
//! exercised by the tests - the wire format itself is an untagged string.

use serde::{Deserialize, Serialize};

use crate::core::config_provider::Request;
use crate::permission::Ruleset;

/// The named palette entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedColor {
    #[serde(rename = "primary")]
    Primary,
    #[serde(rename = "secondary")]
    Secondary,
    #[serde(rename = "accent")]
    Accent,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "info")]
    Info,
}

/// Either a palette name or a `#rrggbb` string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    Named(NamedColor),
    Hex(String),
}

impl Color {
    /// Same acceptance as the TS `isPattern(/^#[0-9a-fA-F]{6}$/)`.
    pub fn is_hex_color(s: &str) -> bool {
        let bytes = s.as_bytes();
        bytes.len() == 7
            && bytes[0] == b'#'
            && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
    }

    /// Build a colour from a string: palette name if it matches one, hex form
    /// if it matches the pattern, and `None` for anything else - the same
    /// three-way outcome the TS union produces.
    pub fn parse(raw: &str) -> Option<Self> {
        let named = match raw {
            "primary" => Some(NamedColor::Primary),
            "secondary" => Some(NamedColor::Secondary),
            "accent" => Some(NamedColor::Accent),
            "success" => Some(NamedColor::Success),
            "warning" => Some(NamedColor::Warning),
            "error" => Some(NamedColor::Error),
            "info" => Some(NamedColor::Info),
            _ => None,
        };
        if named.is_some() {
            return named.map(Color::Named);
        }
        if Self::is_hex_color(raw) {
            return Some(Color::Hex(raw.to_string()));
        }
        None
    }
}

/// Agent kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "subagent")]
    Subagent,
    #[serde(rename = "primary")]
    Primary,
    #[serde(rename = "all")]
    All,
}

/// `ConfigV2.Agent`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Info {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Request>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Ruleset>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_palette_names_and_the_hex_form_both_parse() {
        assert_eq!(Color::parse("accent"), Some(Color::Named(NamedColor::Accent)));
        assert_eq!(Color::parse("#A1b2C3"), Some(Color::Hex("#A1b2C3".to_string())));
        assert_eq!(Color::parse("#fff"), None, "three digits are not accepted");
        assert_eq!(Color::parse("#gggggg"), None);
        assert_eq!(Color::parse("chartreuse"), None);
    }

    #[test]
    fn a_colour_is_written_as_a_bare_string() {
        let v = serde_json::to_value(Color::Named(NamedColor::Primary)).unwrap();
        assert_eq!(v, json!("primary"));
        let v = serde_json::to_value(Color::Hex("#000000".to_string())).unwrap();
        assert_eq!(v, json!("#000000"));
    }

    #[test]
    fn an_empty_agent_config_writes_nothing() {
        assert_eq!(serde_json::to_value(Info::default()).unwrap(), json!({}));
    }

    #[test]
    fn modes_keep_their_literals() {
        for (m, text) in [(Mode::Subagent, "subagent"), (Mode::Primary, "primary"), (Mode::All, "all")] {
            assert_eq!(serde_json::to_value(m).unwrap(), json!(text));
        }
    }

    #[test]
    fn an_agent_config_round_trips_with_permissions() {
        let info = Info {
            model: Some("claude".to_string()),
            mode: Some(Mode::Primary),
            color: Some(Color::Hex("#ff0000".to_string())),
            steps: Some(12),
            permissions: Some(vec![crate::permission::Rule::new(
                "edit",
                "*",
                crate::permission::Effect::Ask,
            )]),
            ..Info::default()
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: Info = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
        assert!(json.contains("\"steps\":12"));
    }
}