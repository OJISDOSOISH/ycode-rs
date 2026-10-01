//! Port of the portable part of
//! `opencode/packages/core/src/config/plugin/external.ts`.
//!
//! The plugin loads external plugin packages named in the config, from npm or
//! from the filesystem, and evaluates the module it finds. The evaluation is
//! not portable and is not ported: dynamic import, the npm service, the plugin
//! host.
//!
//! What is portable is the name resolution, and it has three cases in this
//! order: a `file://` URL becomes a path, a relative `./` or `../` reference
//! is resolved against the config entry's directory, and anything else is left
//! for the npm service untouched.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A plugin reference as it appears in a config document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginRef {
    /// Shorthand: just the package name.
    Name(String),
    /// Long form, with options forwarded to the plugin host.
    Detailed { package: String, options: Option<BTreeMap<String, serde_json::Value>> },
}

impl PluginRef {
    pub fn package(&self) -> &str {
        match self {
            PluginRef::Name(n) => n,
            PluginRef::Detailed { package, .. } => package,
        }
    }

    pub fn options(&self) -> Option<&BTreeMap<String, serde_json::Value>> {
        match self {
            PluginRef::Name(_) => None,
            PluginRef::Detailed { options, .. } => options.as_ref(),
        }
    }
}

/// `fileURLToPath` for the `file://` prefix, plus percent-decoding of the
/// path portion. Only what the config can realistically carry is handled.
fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    // file:///C:/x -> /C:/x on Windows-style URLs; file:///x -> /x
    let decoded = percent_decode(rest);
    Some(decoded)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Minimal `path.resolve`: absolutise a relative reference against a base.
fn resolve_against(directory: &str, reference: &str) -> String {
    if is_absolute(reference) {
        return reference.to_string();
    }
    let base = directory.trim_end_matches('/');
    if base.is_empty() {
        return reference.to_string();
    }
    if reference.starts_with("./") {
        format!("{}/{}", base, &reference[2..])
    } else {
        format!("{}/{}", base, reference)
    }
}

fn is_absolute(p: &str) -> bool {
    p.starts_with('/') || {
        let b = p.as_bytes();
        b.len() >= 3 && b[1] == b':' && b[2] == b'/' && b[0].is_ascii_alphabetic()
    }
}

/// The three-case resolution, in the order the TS applies them.
pub fn resolve_package_name(package: &str, directory: &str) -> String {
    if package.starts_with("file://") {
        if let Some(path) = file_url_to_path(package) {
            return path;
        }
    }
    if package.starts_with("./") || package.starts_with("../") {
        return resolve_against(directory, package);
    }
    package.to_string()
}

/// The resolved reference pushed to the loading list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfiguredPackage {
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<BTreeMap<String, serde_json::Value>>,
}

/// Resolve every plugin reference of one document entry.
pub fn configured_packages(
    refs: &[PluginRef],
    directory: &str,
) -> Vec<ConfiguredPackage> {
    refs.iter()
        .map(|r| ConfiguredPackage {
            package: resolve_package_name(r.package(), directory),
            options: r.options().cloned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_name_is_left_for_the_npm_service() {
        assert_eq!(resolve_package_name("my-plugin", "/cfg"), "my-plugin");
        assert_eq!(resolve_package_name("@scope/my-plugin", "/cfg"), "@scope/my-plugin");
    }

    #[test]
    fn a_relative_reference_resolves_against_the_entry_directory() {
        assert_eq!(resolve_package_name("./p.ts", "/cfg"), "/cfg/p.ts");
        assert_eq!(resolve_package_name("../p.ts", "/cfg/sub"), "/cfg/sub/../p.ts");
    }

    #[test]
    fn a_file_url_becomes_a_path() {
        assert_eq!(resolve_package_name("file:///a/p.js", "/cfg"), "/a/p.js");
        assert_eq!(resolve_package_name("file:///C:/a/p.js", "/cfg"), "/C:/a/p.js");
        assert_eq!(resolve_package_name("file:///a/my%20p.js", "/cfg"), "/a/my p.js");
    }

    #[test]
    fn the_shorthand_and_the_long_form_both_resolve() {
        let refs = vec![
            PluginRef::Name("./a.ts".to_string()),
            PluginRef::Detailed {
                package: "npm-pkg".to_string(),
                options: Some(BTreeMap::from([("k".to_string(), serde_json::json!(1))])),
            },
        ];
        let out = configured_packages(&refs, "/cfg");
        assert_eq!(out[0].package, "/cfg/a.ts");
        assert!(out[0].options.is_none());
        assert_eq!(out[1].package, "npm-pkg");
        assert_eq!(out[1].options.as_ref().unwrap()["k"], serde_json::json!(1));
    }

    #[test]
    fn a_plugin_reference_reads_from_both_shapes() {
        let court: PluginRef = serde_json::from_str(r#""a""#).unwrap();
        assert_eq!(court.package(), "a");
        let long: PluginRef = serde_json::from_str(r#"{"package":"b","options":{"x":1}}"#).unwrap();
        assert_eq!(long.package(), "b");
        assert!(long.options().is_some());
    }
}