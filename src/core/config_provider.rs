//! Port of `opencode/packages/core/src/config/provider.ts`.
//!
//! The configuration layer for providers, one step before the resolved models
//! of `core::model`: every field here is optional, and `cost` accepts either a
//! single entry or a list. That is why this file does not reuse
//! `core::model::ModelInfo` - different shape, not a duplicate - but it does
//! reuse `core::model::CostTier` and `core::provider::ProviderApi`, which are
//! identical.
//!
//! `ModelApi` has no `toTaggedUnion` in the TS, so it is a STRUCTURAL union:
//! three shapes distinguished by their fields, not by a tag. `serde(untagged)`
//! is the faithful translation, with the order of the variants kept as written.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use crate::core::model::CostTier;
pub use crate::core::provider::ProviderApi;

/// `ConfigV2.Provider.Request`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<BTreeMap<String, serde_json::Value>>,
}

/// `ConfigV2.Model.Cost.Cache`: both fields optional at config level.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CostCache {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write: Option<f64>,
}

/// `ConfigV2.Model.Cost`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<CostTier>,
    pub input: f64,
    pub output: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<CostCache>,
}

/// `cost` accepts one entry or a list of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CostOrCosts {
    One(Cost),
    Many(Vec<Cost>),
}

/// `ConfigV2.Model.Limit`, all optional.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Limit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<i64>,
}

/// `{ id?, ...AISDK.fields }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AisdkModelApi {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, serde_json::Value>>,
}

/// `{ id?, ...Native.fields }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeModelApi {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// `{ id }` alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdOnlyModelApi {
    pub id: String,
}

/// Structural union, in source order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelApi {
    Aisdk(AisdkModelApi),
    Native(NativeModelApi),
    Id(IdOnlyModelApi),
}

/// `Info.request` on a model: `Request` plus an optional variant.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModelRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// One entry of `Info.variants`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelVariant {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<BTreeMap<String, serde_json::Value>>,
}

/// `ConfigV2.Model`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Model {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<ModelApi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<ModelRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<Vec<ModelVariant>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostOrCosts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<Limit>,
}

/// `ModelV2.Capabilities` at config level: all three fields optional.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Vec<String>>,
}

/// `ConfigV2.Provider`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Info {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<ProviderApi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Request>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<BTreeMap<String, Model>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_empty_config_writes_nothing() {
        assert_eq!(serde_json::to_value(Info::default()).unwrap(), json!({}));
        assert_eq!(serde_json::to_value(Model::default()).unwrap(), json!({}));
    }

    #[test]
    fn cost_accepts_a_single_entry_or_a_list() {
        let one = Cost {
            tier: None,
            input: 1.0,
            output: 2.0,
            cache: Some(CostCache { read: Some(0.5), write: None }),
        };
        let v = serde_json::to_value(&one).unwrap();
        assert_eq!(v, json!({ "input": 1.0, "output": 2.0, "cache": { "read": 0.5 } }));

        let mut model = Model::default();
        model.cost = Some(CostOrCosts::Many(vec![one.clone()]));
        let text = serde_json::to_string(&model).unwrap();
        assert!(text.contains("\"cost\":["), "cost as a list stays a list: {}", text);

        model.cost = Some(CostOrCosts::One(one));
        let v = serde_json::to_value(&model).unwrap();
        assert!(v["cost"].is_object());
    }

    #[test]
    fn the_structural_api_union_is_read_by_shape() {
        let aisdk = json!({ "id": "m", "package": "@ai-sdk/x" });
        let api: ModelApi = serde_json::from_value(aisdk).unwrap();
        assert!(matches!(api, ModelApi::Aisdk(_)));

        let native = json!({ "id": "m", "settings": { "a": 1 } });
        let api: ModelApi = serde_json::from_value(native).unwrap();
        assert!(matches!(api, ModelApi::Native(_)));

        let bare = json!({ "id": "m" });
        let api: ModelApi = serde_json::from_value(bare).unwrap();
        assert!(matches!(api, ModelApi::Id(_)));
    }

    #[test]
    fn a_config_api_variant_is_untagged_so_it_has_no_type_key() {
        let api = ModelApi::Id(IdOnlyModelApi { id: "m".to_string() });
        assert_eq!(serde_json::to_value(api).unwrap(), json!({ "id": "m" }));
    }

    #[test]
    fn limit_and_capabilities_stay_all_optional() {
        assert_eq!(serde_json::to_value(Limit::default()).unwrap(), json!({}));
        assert_eq!(serde_json::to_value(Capabilities::default()).unwrap(), json!({}));
        let lim = Limit { context: Some(200_000), input: None, output: Some(64_000) };
        assert_eq!(
            serde_json::to_value(lim).unwrap(),
            json!({ "context": 200_000, "output": 64_000 })
        );
    }

    #[test]
    fn a_provider_info_round_trips() {
        let info = Info {
            name: Some("anthropic".to_string()),
            env: Some(vec!["ANTHROPIC_API_KEY".to_string()]),
            models: Some(BTreeMap::from([(
                "claude".to_string(),
                Model { name: Some("Claude".to_string()), ..Model::default() },
            )])),
            ..Info::default()
        };
        let back: Info = serde_json::from_str(&serde_json::to_string(&info).unwrap()).unwrap();
        assert_eq!(back, info);
    }
}