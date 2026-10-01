//! Rust port of `packages/core/src/observability/logging.ts`.
//!
//! The source builds Effect `Logger` values (`fileLogger`, `stderrLogger`,
//! `minimumLogLevel`) from a structured formatter and an env-var level parser.
//! The `Logger.make` / `Logger.toFile` machinery is Effect-side and
//! side-effectful, so it is out of scope (Rule 7). This port carries the pure,
//! deterministic parts that are observable on the wire: the field formatter,
//! the value flattener, the plain-object test, the scalar formatter, and the
//! `OPENCODE_LOG_LEVEL` env parsing.
//!
//! # Dependencies not in this batch
//!
//! `runID` (from `./shared`) and `Global.Path.log` are omitted; `fileLogger`
//! accepts its path as an argument instead.
//!
//! # Field-name discipline (Rule 6)
//!
//! No camelCase fields occur in the pure surface; env keys
//! (`OPENCODE_LOG_LEVEL`) are matched verbatim.

use std::collections::BTreeMap;

/// Environment variable read by the source.
pub const LOG_LEVEL_ENV: &str = "OPENCODE_LOG_LEVEL";

/// Lowercased env value -> `Level`, matching the source's `levels` map.
fn from_env(value: &str) -> Option<Level> {
    match value {
        "DEBUG" => Some(Level::Debug),
        "INFO" => Some(Level::Info),
        "WARN" => Some(Level::Warn),
        "ERROR" => Some(Level::Error),
        _ => None,
    }
}

/// Log severity, mirroring `LogLevel.LogLevel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

/// `minimumLogLevel()` from the source: reads `OPENCODE_LOG_LEVEL`,
/// upper-cases it, and defaults to `Info`.
pub fn minimum_log_level(env: Option<&str>) -> Level {
    match env.and_then(|v| from_env(&v.to_uppercase())) {
        Some(level) => level,
        None => Level::Info,
    }
}

/// `plain(input)` from the source: true for bare object records.
pub fn is_plain(input: &serde_json::Value) -> bool {
    matches!(input, serde_json::Value::Object(_))
}

/// `flatten(input, prefix)` from the source. Produces `key.path` pairs into
/// `out`. Cycle guard omitted: `serde_json::Value` cannot be self-referential,
/// so recursion cannot loop.
pub fn flatten(
    input: &serde_json::Value,
    prefix: &str,
    out: &mut Vec<(String, serde_json::Value)>,
) {
    if is_plain(input) {
        let obj = input.as_object().unwrap();
        if obj.is_empty() && !prefix.is_empty() {
            out.push((prefix.to_string(), input.clone()));
            return;
        }
        for (key, value) in obj {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten(value, &path, out);
        }
    } else if !prefix.is_empty() {
        out.push((prefix.to_string(), input.clone()));
    }
}

/// `format(value)` from the source: strings pass through unquoted unless they
/// contain whitespace, `=`, `"`, or `\` and are non-empty; non-strings are
/// JSON-serialized. Mirrors the source's `/^[^\s="\\]+$/` predicate without the
/// `regex` crate (which is not a dependency of this crate).
pub fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => {
            let needs_quote = s.is_empty()
                || s.chars()
                    .any(|c| c.is_whitespace() || c == '=' || c == '"' || c == '\\');
            if needs_quote {
                format!("{s:?}")
            } else {
                s.clone()
            }
        }
        other => serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()),
    }
}

/// `render_pair`: `key=value`, matching the source's map step.
pub fn render_pair(key: &str, value: &serde_json::Value) -> String {
    format!("{key}={}", format_value(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn level_parsing_defaults_to_info() {
        assert_eq!(minimum_log_level(None), Level::Info);
        assert_eq!(minimum_log_level(Some("GARBAGE")), Level::Info);
        assert_eq!(minimum_log_level(Some("debug")), Level::Debug);
        assert_eq!(minimum_log_level(Some("WARN")), Level::Warn);
        assert_eq!(minimum_log_level(Some("error")), Level::Error);
        assert_eq!(minimum_log_level(Some("INFO")), Level::Info);
    }

    #[test]
    fn format_value_plain_strings_pass_unquoted() {
        let plain = json!("hello");
        assert_eq!(format_value(&plain), "hello");
        let spaced = json!("hello world");
        let q = format_value(&spaced);
        assert!(q.starts_with('"') && q.ends_with('"'), "expected quoted, got {q}");
    }

    #[test]
    fn format_value_string_with_equals_is_quoted() {
        let v = json!("a=b");
        assert_eq!(format_value(&v), "\"a=b\"");
    }

    #[test]
    fn flatten_matches_source_shape() {
        let input = json!({ "a": 1, "b": { "c": 2 } });
        let mut out = vec![];
        flatten(&input, "", &mut out);
        let map: BTreeMap<String, serde_json::Value> = out.into_iter().collect();
        assert_eq!(map["a"], json!(1));
        assert_eq!(map["b.c"], json!(2));
    }

    #[test]
    fn render_pair_formats_a_scalar() {
        let rendered = render_pair("level", &json!("info"));
        assert_eq!(rendered, "level=info");
    }
}
