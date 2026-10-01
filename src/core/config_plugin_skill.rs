//! Port of the portable part of
//! `opencode/packages/core/src/config/plugin/skill.ts`.
//!
//! The TS file is a plugin registration: it reads the config service and
//! pushes skill sources into a draft. What is pure - and what is ported here -
//! is the derivation itself: which sources a config produces, and how a skill
//! entry becomes either a URL source or a directory source.
//!
//! Not ported: the Effect wiring, `Config.Service`, `Global.Service`,
//! `Location.Service`, and `ctx.skill.transform`. The caller supplies the
//! entries, the home directory and the location directory instead.
//!
//! Two details that matter and are easy to get wrong:
//! - a directory entry produces TWO sources, `<dir>/skill` and `<dir>/skills`;
//! - only `http:` and `https:` count as URL protocols, and `~/x` is expanded
//!   against the home directory before the absolute/relative decision.

use serde::{Deserialize, Serialize};

/// `SkillV2.DirectorySource`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorySource {
    #[serde(rename = "type")]
    pub kind: DirectoryKind,
    pub path: String,
}

/// Tag literal of a directory source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectoryKind {
    #[serde(rename = "directory")]
    Directory,
}

/// `SkillV2.UrlSource`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlSource {
    #[serde(rename = "type")]
    pub kind: UrlKind,
    pub url: String,
}

/// Tag literal of a url source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UrlKind {
    #[serde(rename = "url")]
    Url,
}

/// Either source shape, tagged on `type` like the TS `DirectorySource.make` /
/// `UrlSource.make` calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SkillSource {
    #[serde(rename = "directory")]
    Directory { path: String },
    #[serde(rename = "url")]
    Url { url: String },
}

impl From<DirectorySource> for SkillSource {
    fn from(d: DirectorySource) -> Self {
        SkillSource::Directory { path: d.path }
    }
}

impl From<UrlSource> for SkillSource {
    fn from(u: UrlSource) -> Self {
        SkillSource::Url { url: u.url }
    }
}

/// The two config entry shapes this plugin reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Directory { path: String },
    Document { skills: Vec<String> },
}

/// `URL.canParse` plus `/^(https?:)$/` on the protocol: only http and https.
pub fn is_http_url(item: &str) -> bool {
    for prefix in ("https://", "http://") {
        if let Some(rest) = item.strip_prefix(prefix) {
            return !rest.is_empty();
        }
    }
    false
}

/// `~/x` expands against the home directory; anything else is returned as is.
pub fn expand_tilde(item: &str, home: &str) -> String {
    match item.strip_prefix("~/") {
        Some(rest) => join_path(home, rest),
        None => item.to_string(),
    }
}

/// Minimal path join with `/`, matching the forward-slash wire convention.
fn join_path(base: &str, rest: &str) -> String {
    if base.is_empty() {
        return rest.to_string();
    }
    if base.ends_with('/') {
        format!("{}{}", base, rest)
    } else {
        format!("{}/{}", base, rest)
    }
}

fn is_absolute(item: &str) -> bool {
    item.starts_with('/') || {
        let b = item.as_bytes();
        b.len() >= 3 && b[1] == b':' && b[2] == b'/' && b[0].is_ascii_alphabetic()
    }
}

/// One document entry becomes one or more sources.
pub fn sources_for_items(items: &[String], home: &str, location_directory: &str) -> Vec<SkillSource> {
    let mut out = Vec::new();
    for item in items {
        if is_http_url(item) {
            out.push(SkillSource::Url { url: item.clone() });
            continue;
        }
        let expanded = expand_tilde(item, home);
        let path = if is_absolute(&expanded) {
            expanded
        } else {
            join_path(location_directory, &expanded)
        };
        out.push(SkillSource::Directory { path });
    }
    out
}

/// The full derivation: directories first (two sources each), then documents.
pub fn sources_for_entries(
    entries: &[Entry],
    home: &str,
    location_directory: &str,
) -> Vec<SkillSource> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            Entry::Directory { path } => {
                out.push(SkillSource::Directory { path: join_path(path, "skill") });
                out.push(SkillSource::Directory { path: join_path(path, "skills") });
            }
            Entry::Document { skills } => {
                out.extend(sources_for_items(skills, home, location_directory));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_directory_entry_yields_skill_and_skills() {
        let out = sources_for_entries(&[Entry::Directory { path: "/cfg".into() }], "/home/u", "/repo");
        assert_eq!(
            out,
            vec![
                SkillSource::Directory { path: "/cfg/skill".into() },
                SkillSource::Directory { path: "/cfg/skills".into() },
            ]
        );
    }

    #[test]
    fn only_http_and_https_are_urls() {
        assert!(is_http_url("https://a/b"));
        assert!(is_http_url("http://a"));
        assert!(!is_http_url("ftp://a"));
        assert!(!is_http_url("file:///a"));
        assert!(!is_http_url("a/b"));
        assert!(!is_http_url("https://"), "an empty remainder is not a url");
    }

    #[test]
    fn tilde_expands_against_home_then_the_location_directory() {
        assert_eq!(expand_tilde("~/skills", "/home/u"), "/home/u/skills");
        assert_eq!(expand_tilde("skills", "/home/u"), "skills");

        let out = sources_for_items(&["~/s".into()], "/home/u", "/repo");
        assert_eq!(out, vec![SkillSource::Directory { path: "/home/u/s".into() }]);

        let out = sources_for_items(&["s".into()], "/home/u", "/repo");
        assert_eq!(out, vec![SkillSource::Directory { path: "/repo/s".into() }]);

        let out = sources_for_items(&["/abs/s".into()], "/home/u", "/repo");
        assert_eq!(out, vec![SkillSource::Directory { path: "/abs/s".into() }]);
    }

    #[test]
    fn a_windows_absolute_path_survives_the_join() {
        let out = sources_for_items(&["C:/x/s".into()], "/home/u", "/repo");
        assert_eq!(out, vec![SkillSource::Directory { path: "C:/x/s".into() }]);
        let out = sources_for_items(&["s".into()], "/home/u", "C:/repo");
        assert_eq!(out, vec![SkillSource::Directory { path: "C:/repo/s".into() }]);
    }

    #[test]
    fn sources_serialize_with_a_type_tag() {
        let out = sources_for_entries(
            &[
                Entry::Directory { path: "/cfg".into() },
                Entry::Document { skills: vec!["https://x/y".into(), "local".into()] },
            ],
            "/home/u",
            "/repo",
        );
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v[0], json!({ "type": "directory", "path": "/cfg/skill" }));
        assert_eq!(v[2], json!({ "type": "url", "url": "https://x/y" }));
        assert_eq!(v[3], json!({ "type": "directory", "path": "/repo/local" }));
    }

    #[test]
    fn document_entries_without_skills_add_nothing() {
        let out = sources_for_entries(&[Entry::Document { skills: vec![] }], "/home/u", "/repo");
        assert!(out.is_empty());
    }
}