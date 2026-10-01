//! Rust port of `packages/core/src/observability/otlp.ts`.
//!
//! The source reads two env-backed flags (`OTEL_EXPORTER_OTLP_ENDPOINT`,
//! `OTEL_EXPORTER_OTLP_HEADERS`), parses the header string, builds the OTLP
//! `resource` object, and constructs an `OtlpLogger` plus an async `tracingLayer`
//! that dynamically imports the OpenTelemetry SDK. The logger/tracing factories
//! are side-effectful and depend on a Node SDK not in this crate, so they are
//! out of scope (Rule 7). This port carries the pure, observable parts: the
//! header parser and the `resource` builder, including its exact attribute
//! keys.
//!
//! # Dependencies not in this batch
//!
//! - `Flag.OTEL_EXPORTER_OTLP_ENDPOINT`, `Flag.OTEL_EXPORTER_OTLP_HEADERS` and
//!   `Flag.OPENCODE_CLIENT` come from `../flag/flag.ts`.
//! - `InstallationChannel` and `InstallationVersion` come from
//!   `../installation/version.ts`.
//! - `runID` comes from `./shared`.
//!
//! These are passed in as parameters so the port has no hidden state.
//!
//! # Field-name discipline (Rule 6)
//!
//! The resource attribute keys are dotted strings (`"deployment.environment.name"`,
//! `"opencode.client"`, `"opencode.run"`, `"service.instance.id"`); these are
//! data, not Rust field names, so they are asserted verbatim in tests.

/// Parsed `OTEL_EXPORTER_OTLP_HEADERS` value, or an empty map when unset.
pub fn parse_headers(raw: Option<&str>) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let some = match raw {
        Some(s) if !s.is_empty() => s,
        _ => return out,
    };
    // The source splits on `,`, then on the FIRST `=` only (key, ...value).
    for entry in some.split(',') {
        let mut parts = entry.splitn(2, '=');
        let key = match parts.next() {
            Some(k) if !k.is_empty() => k.to_string(),
            _ => continue,
        };
        let value = match parts.next() {
            Some(v) => v.to_string(),
            None => String::new(),
        };
        out.insert(key, value);
    }
    out
}

/// `resource.attributes` parsing from `OTEL_RESOURCE_ATTRIBUTES`. Entries
/// before the first `=` are dropped (index < 1 guard in the source).
pub fn parse_resource_attributes(raw: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for entry in raw.split(',') {
        let index = entry.find('=');
        let index = match index {
            Some(i) if i >= 1 => i,
            _ => continue,
        };
        out.insert(entry[..index].to_string(), entry[index + 1..].to_string());
    }
    out
}

/// The OTel resource object the source returns from `resource()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub service_name: String,
    pub service_version: String,
    pub attributes: std::collections::BTreeMap<String, String>,
}

/// `resource()` from the source (minus the SDK), with pure inputs substituted.
pub fn resource(
    endpoint: &str,
    headers_raw: Option<&str>,
    resource_attributes_raw: Option<&str>,
    installation_channel: &str,
    installation_version: &str,
    open_code_client: &str,
    run_id: &str,
) -> Resource {
    let mut attributes = parse_resource_attributes(resource_attributes_raw.unwrap_or(""));
    attributes.insert("deployment.environment.name".to_string(), installation_channel.to_string());
    attributes.insert("opencode.client".to_string(), open_code_client.to_string());
    attributes.insert("opencode.run".to_string(), run_id.to_string());
    attributes.insert("service.instance.id".to_string(), run_id.to_string());
    let _ = headers_raw; // echoed only to keep the pure surface complete
        Resource {
        service_name: endpoint.to_string(),
        service_version: installation_version.to_string(),
        attributes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_attribute_keys_match_source_exactly() {
        let r = resource(
            "http://localhost:4318",
            None,
            None,
            "stable",
            "1.0.0",
            "cli",
            "run-1",
        );
        assert_eq!(r.service_name, "http://localhost:4318");
        assert_eq!(r.service_version, "1.0.0");
        assert_eq!(r.attributes["deployment.environment.name"], "stable");
        assert_eq!(r.attributes["opencode.client"], "cli");
        assert_eq!(r.attributes["opencode.run"], "run-1");
        assert_eq!(r.attributes["service.instance.id"], "run-1");
    }

    #[test]
    fn parse_headers_splits_on_first_equals_only() {
        let h = parse_headers(Some("key1=val=1,key2=val2"));
        assert_eq!(h.get("key1"), Some(&"val=1".to_string()));
        assert_eq!(h.get("key2"), Some(&"val2".to_string()));
    }

    #[test]
    fn parse_headers_unset_is_empty() {
        assert!(parse_headers(None).is_empty());
        assert!(parse_headers(Some("")).is_empty());
    }

    #[test]
    fn parse_resource_attributes_drops_entry_before_equals() {
        let m = parse_resource_attributes("=bad,k=v ok");
        assert!(!m.contains_key(""));
        assert_eq!(m.get("k"), Some(&"v ok".to_string()));
    }
}