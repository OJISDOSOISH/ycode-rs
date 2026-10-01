//! Port of the portable part of
//! `opencode/packages/core/src/config/plugin/provider.ts`.
//!
//! The plugin pushes configured providers, models and the default model into a
//! catalog, and configured integrations into an integration registry. The
//! Effect wiring is not ported; the merge rules are, and they are the part
//! that is easy to get wrong.
//!
//! The rule that decides this file: `Object.assign(target, patch)` MERGES key
//! by key, it does not replace the map. A provider config that sets one
//! request header must leave the other headers in place, and a model config
//! that sets `family` must leave `name`, `api` and `cost` untouched. A port
//! that assigns whole maps would pass every "is this field set" test and still
//! be wrong, so the tests below assert what survives, not just what changed.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::core::config_provider::{Capabilities, Model, ModelRequest, Request};

/// Ids of the providers that declare an `env` list, across every document.
///
/// A provider with no `env` never produces a configured integration, which is
/// what the TS checks before touching the registry.
pub fn configured_integration_ids(
    files: &[BTreeMap<String, ProviderEntry>],
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for providers in files {
        for (id, entry) in providers {
            if entry.env.is_some() {
                out.insert(id.clone());
            }
        }
    }
    out
}

/// The parts of a configured provider this plugin reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProviderEntry {
    pub name: Option<String>,
    pub env: Option<Vec<String>>,
    pub api: Option<Value>,
    pub request: Option<Request>,
    pub models: BTreeMap<String, Model>,
}

/// `Object.assign(headers, patch.headers)` and the same for the body.
pub fn merge_request(base: &Request, patch: &Request) -> Request {
    let mut headers = base.headers.clone().unwrap_or_default();
    let mut body = base.body.clone().unwrap_or_default();
    if let Some(h) = &patch.headers {
        headers.extend(h.clone());
    }
    if let Some(b) = &patch.body {
        body.extend(b.clone());
    }
    Request { headers: Some(headers), body: Some(body) }
}

/// Merge a request in place, for the catalog update path.
pub fn merge_request_into(base: &mut Request, patch: &Request) {
    if let Some(h) = &patch.headers {
        base.headers.get_or_insert_with(BTreeMap::new).extend(h.clone());
    }
    if let Some(b) = &patch.body {
        base.body.get_or_insert_with(BTreeMap::new).extend(b.clone());
    }
}

/// Capabilities are replaced as a block in the TS, but with every field taken
/// from the config when it defines one.
pub fn merge_capabilities(
    base: Option<&Capabilities>,
    patch: Option<&Capabilities>,
) -> Option<Capabilities> {
    match (base, patch) {
        (None, None) => None,
        (None, Some(p)) => Some(p.clone()),
        (Some(b), None) => Some(b.clone()),
        (Some(b), Some(p)) => Some(Capabilities {
            tools: p.tools.unwrap_or(b.tools),
            input: p.input.clone().or_else(|| b.input.clone()),
            output: p.output.clone().or_else(|| b.output.clone()),
        }),
    }
}

/// The default model named by the config, already parsed into its two parts.
pub fn parse_default_model(raw: &str) -> Option<(String, String)> {
    let (provider, model) = raw.split_once('/')?;
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some((provider.to_string(), model.to_string()))
}

/// Apply one configured model onto the catalog entry, field by field.
pub fn apply_model_config(target: &mut Model, patch: &Model) {
    if let Some(v) = &patch.family {
        target.family = Some(v.clone());
    }
    if let Some(v) = &patch.name {
        target.name = Some(v.clone());
    }
    if let Some(v) = &patch.api {
        // The TS spreads into the existing api, so a partial config keeps the
        // rest. Representing that faithfully needs the merge below.
        target.api = merge_model_api(target.api.take(), v.clone());
    }
    if let Some(v) = &patch.capabilities {
        target.capabilities = merge_capabilities(target.capabilities.as_ref(), Some(v));
    }
    if let Some(v) = &patch.request {
        let base = target.request.clone().unwrap_or_default();
        let mut merged = ModelRequest {
            headers: base.headers,
            body: base.body,
            variant: base.variant,
        };
        if let Some(h) = &v.headers {
            merged.headers.get_or_insert_with(BTreeMap::new).extend(h.clone());
        }
        if let Some(b) = &v.body {
            merged.body.get_or_insert_with(BTreeMap::new).extend(b.clone());
        }
        target.request = Some(merged);
    }
    if let Some(v) = &patch.variants {
        target.variants = Some(v.clone());
    }
    if let Some(v) = &patch.cost {
        target.cost = Some(v.clone());
    }
    if let Some(v) = patch.disabled {
        target.disabled = Some(v);
    }
    if let Some(v) = &patch.limit {
        target.limit = Some(v.clone());
    }
}

/// `{ ...model.api, ...config.api }`: merge two object-shaped api configs.
pub fn merge_model_api(
    base: Option<crate::core::config_provider::ModelApi>,
    patch: crate::core::config_provider::ModelApi,
) -> Option<crate::core::config_provider::ModelApi> {
    use crate::core::config_provider::ModelApi as Api;
    let merged = match (base, &patch) {
        (Some(Api::Aisdk(mut a)), Api::Aisdk(b)) => {
            if b.id.is_some() {
                a.id = b.id.clone();
            }
            if b.url.is_some() {
                a.url = b.url.clone();
            }
            if b.settings.is_some() {
                a.settings = b.settings.clone();
            }
            Api::Aisdk(a)
        }
        (Some(Api::Native(mut a)), Api::Native(b)) => {
            if b.id.is_some() {
                a.id = b.id.clone();
            }
            if b.url.is_some() {
                a.url = b.url.clone();
            }
            a.settings = b.settings.clone();
            Api::Native(a)
        }
        (None, other) => other.clone(),
        (Some(_), other) => other.clone(),
    };
    Some(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config_provider::{ModelApi, ModelRequest};
    use serde_json::json;

    #[test]
    fn only_providers_with_env_become_configured_integrations() {
        let a = BTreeMap::from([
            (
                "x".to_string(),
                ProviderEntry { env: Some(vec!["X_KEY".into()]), ..ProviderEntry::default() },
            ),
            ("y".to_string(), ProviderEntry::default()),
        ]);
        let b = BTreeMap::from([(
            "z".to_string(),
            ProviderEntry { env: Some(vec![]), ..ProviderEntry::default() },
        )]);
        let out = configured_integration_ids(&[a, b]);
        assert!(out.contains("x"), "an empty env list still counts as configured");
        assert!(out.contains("z"));
        assert!(!out.contains("y"));
    }

    #[test]
    fn a_request_patch_merges_key_by_key_instead_of_replacing() {
        let base = Request {
            headers: Some(BTreeMap::from([("a".into(), "1".into()), ("b".into(), "2".into())])),
            body: Some(BTreeMap::from([("k".into(), json!(1))])),
        };
        let patch = Request {
            headers: Some(BTreeMap::from([("b".into(), "9".into()), ("c".into(), "3".into())])),
            body: Some(BTreeMap::from([("k2".into(), json!(2))])),
        };
        let merged = merge_request(&base, &patch);
        let h = merged.headers.unwrap();
        assert_eq!(h["a"], "1", "an untouched header survives");
        assert_eq!(h["b"], "9", "a patched header wins");
        assert_eq!(h["c"], "3", "a new header is added");
        let b = merged.body.unwrap();
        assert_eq!(b.len(), 2, "the body merges too");
    }

    #[test]
    fn merging_in_place_keeps_the_untouched_keys() {
        let mut base = Request {
            headers: Some(BTreeMap::from([("a".into(), "1".into())])),
            body: None,
        };
        merge_request_into(&mut base, &Request { headers: None, body: Some(BTreeMap::from([("z".into(), json!(true))])) });
        assert_eq!(base.headers.unwrap()["a"], "1");
        assert_eq!(base.body.unwrap()["z"], json!(true));
    }

    #[test]
    fn capabilities_take_the_patch_field_by_field() {
        let base = Capabilities {
            tools: Some(true),
            input: Some(vec!["text".into()]),
            output: Some(vec!["text".into()]),
        };
        let patch = Capabilities { tools: Some(false), input: Some(vec!["image".into()]), output: None };
        let merged = merge_capabilities(Some(&base), Some(&patch)).unwrap();
        assert_eq!(merged.tools, Some(false), "a defined field wins");
        assert_eq!(merged.input, Some(vec!["image".to_string()]));
        assert_eq!(merged.output, Some(vec!["text".to_string()]), "an absent field keeps the base");
        assert_eq!(merge_capabilities(None, None), None);
    }

    #[test]
    fn the_default_model_is_split_provider_first() {
        assert_eq!(
            parse_default_model("anthropic/claude-sonnet-4"),
            Some(("anthropic".to_string(), "claude-sonnet-4".to_string()))
        );
        assert_eq!(parse_default_model("nope"), None);
    }

    #[test]
    fn applying_a_model_config_touches_only_what_it_names() {
        let mut target = Model {
            family: Some("anthropic".into()),
            name: Some("Claude".into()),
            request: Some(ModelRequest {
                headers: Some(BTreeMap::from([("a".into(), "1".into())])),
                ..ModelRequest::default()
            }),
            ..Model::default()
        };
        let patch = Model {
            name: Some("Claude 4".into()),
            request: Some(ModelRequest {
                headers: Some(BTreeMap::from([("b".into(), "2".into())])),
                ..ModelRequest::default()
            }),
            ..Model::default()
        };
        apply_model_config(&mut target, &patch);
        assert_eq!(target.family.as_deref(), Some("anthropic"), "unnamed fields survive");
        assert_eq!(target.name.as_deref(), Some("Claude 4"));
        let h = target.request.unwrap().headers.unwrap();
        assert_eq!(h["a"], "1");
        assert_eq!(h["b"], "2");
    }

    #[test]
    fn an_api_patch_merges_into_the_existing_api_object() {
        use crate::core::config_provider::{AisdkModelApi, NativeModelApi};
        let target = ModelApi::Aisdk(AisdkModelApi {
            id: Some("m".into()),
            package: "@ai-sdk/x".into(),
            url: Some("https://old".into()),
            settings: Some(BTreeMap::from([("k".into(), json!(1))])),
        });
        let patch = ModelApi::Aisdk(AisdkModelApi {
            id: None,
            package: "@ai-sdk/x".into(),
            url: Some("https://new".into()),
            settings: None,
        });
        let merged = merge_model_api(Some(target), patch).unwrap();
        match merged {
            ModelApi::Aisdk(a) => {
                assert_eq!(a.id.as_deref(), Some("m"), "an absent id keeps the old one");
                assert_eq!(a.url.as_deref(), Some("https://new"));
                assert_eq!(a.settings.unwrap()["k"], json!(1), "an absent settings map survives");
            }
            _ => panic!("expected the aisdk variant"),
        }
        // a native patch replaces the variant, exactly like the TS spread on a
        // different shape
        let native = ModelApi::Native(NativeModelApi {
            id: None,
            url: None,
            settings: BTreeMap::new(),
        });
        assert!(matches!(merge_model_api(None, native).unwrap(), ModelApi::Native(_)));
    }
}