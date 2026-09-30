//! Port of `packages/core/src/plugin/provider/openrouter.ts`.
//!
//! ## What the source says
//!
//! ```ts
//! export const OpenRouterPlugin = define({
//!   id: "openrouter",
//!   effect: Effect.fn(function* (ctx) {
//!     yield* ctx.catalog.transform(
//!       Effect.fn(function* (evt) {
//!         for (const item of evt.provider.list()) {
//!           if (item.provider.api.type !== "aisdk") continue
//!           if (item.provider.api.package !== "@openrouter/ai-sdk-provider") continue
//!           evt.provider.update(item.provider.id, (provider) => {
//!             provider.request.headers["HTTP-Referer"] = "https://opencode.ai/"
//!             provider.request.headers["X-Title"] = "opencode"
//!           })
//!           for (const modelID of [ModelV2.ID.make("gpt-5-chat-latest"), ModelV2.ID.make("openai/gpt-5-chat")]) {
//!             if (!item.models.has(modelID)) continue
//!             evt.model.update(item.provider.id, modelID, (model) => {
//!               model.enabled = false
//!             })
//!           }
//!         }
//!       }),
//!     )
//!     yield* ctx.aisdk.sdk(
//!       Effect.fn(function* (evt) {
//!         if (evt.package !== "@openrouter/ai-sdk-provider") return
//!         const mod = yield* Effect.promise(() => import("@openrouter/ai-sdk-provider"))
//!         evt.sdk = mod.createOpenRouter(evt.options)
//!       }),
//!     )
//!   }),
//! })
//! ```
//!
//! Thirty-six lines, two registered hooks, no schema of its own. Unlike most
//! plugins in `plugin/provider`, OpenRouter does more than build an SDK:
//!
//! 1. A `catalog.transform` hook that walks every catalogue entry, keeps only
//!    providers whose `api.type` is exactly `"aisdk"` **and** whose
//!    `api.package` is exactly `"@openrouter/ai-sdk-provider"`, writes two
//!    HTTP headers into `provider.request.headers`, and then disables two model
//!    aliases on that same provider.
//! 2. An `aisdk.sdk` hook that replaces `evt.sdk` with
//!    `createOpenRouter(evt.options)` when the event names the same package.
//!
//! Both `yield*` registrations are discarded by the source, so the
//! registration/loading machinery is not ported; the two hook bodies are.
//!
//! ## Types imported instead of redeclared
//!
//! This file declares **no** copy of the shared payload type. Two neighbours
//! already carry it, and a twelfth copy would drift silently because Rust
//! allows the same type name in two modules:
//!
//! - [`SdkEvent`], plus its [`Options`] and [`Sdk`] aliases, is imported from
//!   `super::provider_groq`. That copy is the most complete of the batch: all
//!   four fields, every `#[serde(rename = "...")]` stated explicitly instead of
//!   inherited from a lowercase field name, and both type aliases. The test
//!   `the_sdk_event_is_the_one_declared_by_provider_groq` pins the identity of
//!   the imported type so a later refactor cannot silently swap it.
//! - [`ProviderV2Info`] and the `CatalogProviderDraft` event are imported from
//!   `super::provider_cerebras`, which carries the most complete catalogue side
//!   of the batch: the full `ProviderV2.Info` shape with its `type`-tagged
//!   `ProviderApi` union, and an `update` signature that takes the provider id.
//!
//! What *is* declared here is what no neighbour has: `OpenRouterSdkFactory`
//! (the injected npm module), `CatalogModelDraft` (the `evt.model.update`
//! channel, which no sibling ports because no sibling touches models) and
//! `CatalogModel`.
//!
//! ## Porting decisions
//!
//! - **The dynamic `import()` is injected.** `import("@openrouter/ai-sdk-provider")`
//!   loads an npm package and has no Rust equivalent, so it becomes the
//!   [`OpenRouterSdkFactory`] trait, whose single method stands in for
//!   `createOpenRouter`. No module loader is invented anywhere in this file.
//!   `Effect.promise(...)` made that one line async in the source; nothing else
//!   in the hook waits, so the port is synchronous.
//! - **`evt.package !== "@openrouter/ai-sdk-provider"` is a strict
//!   inequality.** It is not a truthiness test and it is not a "non-empty"
//!   test: the empty package matches nothing, the comparison is byte for byte,
//!   and it is case sensitive. Three dedicated tests pin that.
//! - **Header writes are plain assignments, not `??=`.** `headers["X"] = v`
//!   overwrites whatever was there, including an existing empty string. The
//!   neighbouring `provider_nvidia.rs` writes the same two headers but uses
//!   `??=` for its third one. The two semantics are exposed as two separately
//!   named functions, [`OpenRouterPlugin::apply_headers`] (assignment, used by
//!   the plugin) and [`set_header_if_absent`] (the `??=` nullity semantics,
//!   deliberately not used), and a test runs them side by side.
//! - **`item.models.has(modelID)` tests key presence, not truthiness.** A map
//!   entry whose value is `null` still exists, so it still matches. Both
//!   readings are exported under names that show the difference at the call
//!   side: [`has_model`] is what the source means, [`has_model_if_truthy`] is
//!   what it does *not* mean, and a test asserts that the plugin follows the
//!   presence reading for a `null`-valued entry.
//! - **`model.enabled = false` is unconditional.** It is not a `??=` and not a
//!   conditional toggle: a model that was already disabled stays disabled, and
//!   no `enabled` flag of any other value survives the pass for an alias.
//!
//! ## Field names
//!
//! The trap on this file is spelling. TypeScript field names keep their exact
//! casing, and Rust's snake_case habit silently renames them, which is
//! invisible until the value is exchanged with the TypeScript side.
//!
//! | TypeScript          | Rust                | Note                          |
//! |---------------------|---------------------|-------------------------------|
//! | `apiKey`            | `api_key`           | `#[serde(rename = "apiKey")]` |
//! | `baseURL`           | `base_url`          | `#[serde(rename = "baseURL")]`|
//! | `model`             | `model`             | already lowercase, one word   |
//! | `package`           | `package`           | already lowercase, one word   |
//! | `options`           | `options`           | already lowercase, one word   |
//! | `sdk?`              | `Option<Sdk>`       | absent until a hook writes it |
//!
//! `apiKey` and `baseURL` are read from the same options record that
//! `provider-options.ts` splits for this package: line 157 registers
//! `"@openrouter/ai-sdk-provider"` in the `openaiCompatible` family, whose
//! `provider(options)` returns `url: string(options.baseURL)`, and whose
//! sibling families (`openai`, `anthropic`, `google`, `azure`) all read
//! `options.apiKey`. [`OpenRouterOptions`] is a *typed view* over that record,
//! not a replacement for it: every other key is preserved in `extra`, so a
//! round trip through it loses nothing.
//!
//! ## What is not ported
//!
//! - `ModelV2.ID.make(...)` is a branded-string constructor with no validation
//!   in the plugin, so the two aliases are plain string constants.
//! - `ModelV2Info` is reduced to [`CatalogModel`], which models only `enabled`,
//!   the single field this plugin writes. Everything else is kept in `extra`.
//! - Effect's `Scope`, which unregisters the hooks at shutdown, does not exist
//!   in Rust and is not simulated: the source never uses what it registers.
//! - The npm package is not installed on this machine, so the real TypeScript
//!   signature of `createOpenRouter` is unread. Only the call is ported.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::provider_cerebras::{CatalogProviderDraft, ProviderV2Info};
use super::provider_groq::{Options, Sdk, SdkEvent};

/// HTTP header table: `Record<string, string>`.
///
/// An alias, not a copy of anything: it resolves to the same `BTreeMap<String,
/// String>` the neighbouring `ProviderRequest::headers` field already uses.
pub type Headers = BTreeMap<String, String>;

/// A model entry of the catalogue, reduced to the field this plugin writes.
///
/// `enabled` is optional in `ModelV2.Info` (`enabled?: boolean`). Every other
/// field of the real model is preserved verbatim in `extra`, so nothing is lost
/// on a round trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogModel {
    /// `enabled?: boolean`. The only field `openrouter.ts` reads or writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Every other field of the real `ModelV2Info`, untouched.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl CatalogModel {
    /// A model with no `enabled` flag yet, which is the state a fresh entry is
    /// in: the key is absent, so `enabled` reads as `None`.
    pub fn new() -> Self {
        Self { enabled: None, extra: BTreeMap::new() }
    }
}

impl Default for CatalogModel {
    fn default() -> Self {
        Self::new()
    }
}

/// The `evt.model` channel: `model.update(providerID, modelID, fn)`.
///
/// No sibling provider ports this trait, because no sibling provider disables a
/// model. `openrouter.ts` is the only file of the batch that calls it.
pub trait CatalogModelDraft {
    /// Applies `update` to the model `model_id` of provider `provider_id`.
    fn update(&mut self, provider_id: &str, model_id: &str, update: &dyn Fn(&mut CatalogModel));
}

/// A typed view of the options record handed to `createOpenRouter`.
///
/// The event's `options` field stays the raw `Options` map, which is the real
/// type (`Record<string, any>`). This view only pins down the two keys whose
/// exact spelling matters, and keeps every other key in `extra` so that
/// converting to and from it is lossless.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OpenRouterOptions {
    /// `apiKey`, not `api_key`. The authentication token of the provider.
    #[serde(rename = "apiKey", default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// `baseURL`, not `base_url`. The capital `URL` is the TypeScript spelling
    /// and it is preserved here; `provider-options.ts` reads `options.baseURL`.
    #[serde(rename = "baseURL", default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Every other option key, kept verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl OpenRouterOptions {
    /// A view with neither key set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads `apiKey` out of an options record, and nothing else.
    ///
    /// The lookup is by exact key: a record that spells it `api_key` has no
    /// `apiKey`, and this returns `None`.
    pub fn api_key_of(options: &Options) -> Option<&str> {
        options.get("apiKey").and_then(Value::as_str)
    }

    /// Reads `baseURL` out of an options record, and nothing else.
    pub fn base_url_of(options: &Options) -> Option<&str> {
        options.get("baseURL").and_then(Value::as_str)
    }

    /// Builds the typed view from a raw options record.
    ///
    /// Keys that are not strings land in `extra` rather than being lost.
    pub fn from_options(options: &Options) -> Self {
        Self {
            api_key: Self::api_key_of(options).map(str::to_string),
            base_url: Self::base_url_of(options).map(str::to_string),
            extra: options.clone(),
        }
        .without_known_keys()
    }

    /// Expands the typed view back into a raw options record.
    pub fn to_options(&self) -> Options {
        let mut options = self.extra.clone();
        if let Some(key) = &self.api_key {
            options.insert("apiKey".to_string(), Value::String(key.clone()));
        }
        if let Some(url) = &self.base_url {
            options.insert("baseURL".to_string(), Value::String(url.clone()));
        }
        options
    }

    /// Drops the two known keys from `extra`, so a round trip through the view
    /// does not serialise them twice.
    fn without_known_keys(mut self) -> Self {
        self.extra.remove("apiKey");
        self.extra.remove("baseURL");
        self
    }
}

/// The npm module `@openrouter/ai-sdk-provider`, injected.
///
/// The source does `import("@openrouter/ai-sdk-provider")` then
/// `mod.createOpenRouter(evt.options)`. That loading has no Rust equivalent, so
/// the factory is a parameter supplied by the host. One method, the one the
/// source calls.
pub trait OpenRouterSdkFactory {
    /// `createOpenRouter(options)` of the npm package.
    ///
    /// Returns `None` for a factory that yields `undefined`, which leaves
    /// `evt.sdk` absent exactly as the assignment would.
    fn create_openrouter(&self, options: &Options) -> Option<Sdk>;
}

/// Overwrites `key` unconditionally: the semantics of `headers[key] = value`.
///
/// This is what `openrouter.ts` does, so an existing value is lost, whatever
/// it was. In particular an existing empty string is replaced: the operator
/// tests nothing at all.
pub fn set_header(headers: &mut Headers, key: &str, value: &str) {
    headers.insert(key.to_string(), value.to_string());
}

/// Writes `value` under `key` only when the key is absent: `headers[key] ??=
/// value`.
///
/// The nullity semantics, deliberately **not** used by this plugin. A value
/// that is already present survives even when it is the empty string, because
/// in JavaScript `""` is neither `null` nor `undefined`. Kept here because the
/// neighbouring `provider_nvidia.rs` needs exactly this and the difference is
/// invisible at the call site once both are spelled `insert`.
pub fn set_header_if_absent(headers: &mut Headers, key: &str, value: &str) {
    headers.entry(key.to_string()).or_insert_with(|| value.to_string());
}

/// `Map.has`: tests the presence of the key, not the truthiness of its value.
///
/// This is what `item.models.has(modelID)` means. An entry holding `null`,
/// `false`, `0`, `""`, `[]` or `{}` is still an entry, so it still matches.
pub fn has_model(models: &BTreeMap<String, Value>, model_id: &str) -> bool {
    models.contains_key(model_id)
}

/// The truthiness reading of [`has_model`], which the source does **not** use.
///
/// Exported so the distinction is visible where it matters: a `null`-valued or
/// empty-valued entry is found by [`has_model`] and missed by this. Comparing
/// the two in the tests is what keeps the port honest.
pub fn has_model_if_truthy(models: &BTreeMap<String, Value>, model_id: &str) -> bool {
    models.get(model_id).is_some_and(is_truthy)
}

/// JavaScript truthiness of a JSON value.
///
/// `null`, `false`, `0`, `NaN`, `""`, `[]` and `{}` are falsy; everything else
/// is truthy. Note that this is a *different* question from "is the key
/// present", which is what [`has_model`] asks.
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

/// Port of `export const OpenRouterPlugin = define({ id, effect })`.
pub struct OpenRouterPlugin;

impl OpenRouterPlugin {
    /// The `id` field of the `define` argument.
    pub const ID: &'static str = "openrouter";

    /// The npm package this plugin recognises, written literally in the two
    /// strict comparisons of the source.
    pub const PACKAGE: &'static str = "@openrouter/ai-sdk-provider";

    /// The tag compared to `provider.api.type`.
    pub const AISDK_TYPE: &'static str = "aisdk";

    /// The export called on the imported module.
    pub const FACTORY: &'static str = "createOpenRouter";

    /// First header name. Its capitalisation is imposed by the service and is
    /// not normalised to `Http-Referer`.
    pub const HEADER_REFERER: &'static str = "HTTP-Referer";

    /// Value written under [`Self::HEADER_REFERER`].
    pub const REFERER_VALUE: &'static str = "https://opencode.ai/";

    /// Second header name.
    pub const HEADER_TITLE: &'static str = "X-Title";

    /// Value written under [`Self::HEADER_TITLE`].
    pub const TITLE_VALUE: &'static str = "opencode";

    /// The two OpenAI chat aliases the source disables, in source order.
    ///
    /// `ModelV2.ID.make(...)` only brands the string; it validates nothing.
    pub const DISABLED_MODEL_ALIASES: [&'static str; 2] =
        ["gpt-5-chat-latest", "openai/gpt-5-chat"];

    /// Is this provider one of ours?
    ///
    /// Both source conditions, in source order: the `type` tag first, then the
    /// package. A `native` provider is discarded by the first one even if it
    /// somehow carries the right package.
    pub fn targets(provider: &ProviderV2Info) -> bool {
        provider.api.kind() == Self::AISDK_TYPE
            && provider.api.package() == Some(Self::PACKAGE)
    }

    /// Writes the two headers, overwriting whatever was there.
    ///
    /// Plain assignments in the source, so [`set_header_if_absent`] would be
    /// the wrong operator here.
    pub fn apply_headers(headers: &mut Headers) {
        set_header(headers, Self::HEADER_REFERER, Self::REFERER_VALUE);
        set_header(headers, Self::HEADER_TITLE, Self::TITLE_VALUE);
    }

    /// Sets `model.enabled = false`, unconditionally.
    ///
    /// Not a conditional toggle and not a `??=`: there is no reading of the
    /// previous value in the source.
    pub fn disable_model(model: &mut CatalogModel) {
        model.enabled = Some(false);
    }

    /// Body of the callback registered on `ctx.catalog.transform`.
    ///
    /// Walks the draft once. For every OpenRouter provider it writes the two
    /// headers, then disables each of the two aliases the provider actually
    /// carries. Providers that lack an alias are left with that model alone,
    /// and providers of any other vendor are never touched, which is what the
    /// source comment about custom providers with matching ids means.
    pub fn transform<D, M>(draft: &mut D, models: &mut M)
    where
        D: CatalogProviderDraft + ?Sized,
        M: CatalogModelDraft + ?Sized,
    {
        for item in draft.list() {
            if !Self::targets(&item.provider) {
                continue;
            }
            let provider_id = item.provider.id.clone();
            draft.update(&provider_id, &|provider: &mut ProviderV2Info| {
                Self::apply_headers(&mut provider.request.headers);
            });
            for alias in Self::DISABLED_MODEL_ALIASES {
                if !has_model(&item.models, alias) {
                    continue;
                }
                models.update(&provider_id, alias, &|model: &mut CatalogModel| {
                    Self::disable_model(model);
                });
            }
        }
    }

    /// Body of the callback registered on `ctx.aisdk.sdk`.
    ///
    /// Returns `true` when the SDK was built and `false` when the source would
    /// have taken its early `return`, which happens when the strict comparison
    /// fails. Options reach the factory untouched.
    pub fn register_sdk<E>(evt: &mut SdkEvent, factory: &E) -> bool
    where
        E: OpenRouterSdkFactory + ?Sized,
    {
        if evt.package != Self::PACKAGE {
            return false;
        }
        evt.sdk = factory.create_openrouter(&evt.options);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::provider_cerebras::{
        CatalogProviderRecord, ProviderApi, ProviderRequest,
    };
    use crate::swarm::provider_groq::SdkEvent as SdkEventFromGroq;
    use serde_json::json;
    use std::cell::{Cell, RefCell};

    // ------------------------------------------------------------- fixtures

    /// A catalogue draft backed by a plain vector, like the real one.
    #[derive(Default)]
    struct Draft(Vec<CatalogProviderRecord>);

    impl CatalogProviderDraft for Draft {
        fn list(&self) -> Vec<CatalogProviderRecord> {
            self.0.clone()
        }

        fn update(&mut self, provider_id: &str, update: &dyn Fn(&mut ProviderV2Info)) {
            for record in self.0.iter_mut() {
                if record.provider.id == provider_id {
                    update(&mut record.provider);
                    return;
                }
            }
        }
    }

    /// A model draft keyed by provider then by model id.
    #[derive(Default)]
    struct Models(BTreeMap<String, BTreeMap<String, CatalogModel>>);

    impl Models {
        fn with(provider_id: &str, entries: &[(&str, CatalogModel)]) -> Self {
            let mut inner = BTreeMap::new();
            for (id, model) in entries {
                inner.insert((*id).to_string(), model.clone());
            }
            let mut map = BTreeMap::new();
            map.insert(provider_id.to_string(), inner);
            Self(map)
        }

        fn get(&self, provider_id: &str, model_id: &str) -> Option<&CatalogModel> {
            self.0.get(provider_id).and_then(|m| m.get(model_id))
        }
    }

    impl CatalogModelDraft for Models {
        fn update(
            &mut self,
            provider_id: &str,
            model_id: &str,
            update: &dyn Fn(&mut CatalogModel),
        ) {
            let entry = self
                .0
                .entry(provider_id.to_string())
                .or_default()
                .entry(model_id.to_string())
                .or_insert_with(CatalogModel::new);
            update(entry);
        }
    }

    fn aisdk(id: &str, package: &str, headers: &[(&str, &str)]) -> CatalogProviderRecord {
        CatalogProviderRecord {
            provider: ProviderV2Info {
                id: id.to_string(),
                integration_id: None,
                name: id.to_string(),
                disabled: None,
                api: ProviderApi::Aisdk {
                    package: package.to_string(),
                    url: None,
                    settings: None,
                },
                request: ProviderRequest {
                    headers: headers
                        .iter()
                        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                        .collect(),
                    body: BTreeMap::new(),
                },
            },
            models: BTreeMap::new(),
        }
    }

    fn openrouter(id: &str, models: &[(&str, Value)]) -> CatalogProviderRecord {
        let mut record = aisdk(id, OpenRouterPlugin::PACKAGE, &[]);
        record.models = models
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect();
        record
    }

    fn headers_of(record: &CatalogProviderRecord) -> &Headers {
        &record.provider.request.headers
    }

    /// A factory that records the options it received and returns a fixed SDK.
    struct Factory {
        calls: Cell<usize>,
        last_options: RefCell<Option<Options>>,
    }

    impl Factory {
        fn new() -> Self {
            Self { calls: Cell::new(0), last_options: RefCell::new(None) }
        }
    }

    impl OpenRouterSdkFactory for Factory {
        fn create_openrouter(&self, options: &Options) -> Option<Sdk> {
            self.calls.set(self.calls.get() + 1);
            *self.last_options.borrow_mut() = Some(options.clone());
            Some(json!({ "provider": "openrouter" }))
        }
    }

    /// A factory that produces `undefined`, like a module returning nothing.
    struct VoidFactory;

    impl OpenRouterSdkFactory for VoidFactory {
        fn create_openrouter(&self, _options: &Options) -> Option<Sdk> {
            None
        }
    }

    fn sdk_event(package: &str) -> SdkEvent {
        SdkEvent {
            model: json!({ "id": "openai/gpt-5" }),
            package: package.to_string(),
            options: Options::new(),
            sdk: None,
        }
    }

    // -------------------------------------------------------------- identity

    #[test]
    fn the_plugin_is_registered_under_the_openrouter_id() {
        assert_eq!(OpenRouterPlugin::ID, "openrouter");
    }

    #[test]
    fn the_recognised_package_is_the_openrouter_one_not_an_ai_sdk_one() {
        assert_eq!(OpenRouterPlugin::PACKAGE, "@openrouter/ai-sdk-provider");
        assert_ne!(OpenRouterPlugin::PACKAGE, "@ai-sdk/openrouter");
        assert_eq!(OpenRouterPlugin::AISDK_TYPE, "aisdk");
        assert_eq!(OpenRouterPlugin::FACTORY, "createOpenRouter");
    }

    #[test]
    fn the_two_disabled_aliases_are_exactly_the_source_pair_in_order() {
        assert_eq!(
            OpenRouterPlugin::DISABLED_MODEL_ALIASES,
            ["gpt-5-chat-latest", "openai/gpt-5-chat"]
        );
    }

    // ------------------------------------------------------- the shared type

    #[test]
    fn the_sdk_event_is_the_one_declared_by_provider_groq() {
        // The annotation is the assertion: if this file ever declared a second
        // `SdkEvent` of its own, the call below would stop type checking.
        let mut evt: SdkEventFromGroq = sdk_event(OpenRouterPlugin::PACKAGE);
        let factory = Factory::new();
        let same: &mut SdkEvent = &mut evt;

        assert!(OpenRouterPlugin::register_sdk(same, &factory));
    }

    // ---------------------------------------------------------------- headers

    #[test]
    fn an_openrouter_provider_receives_both_headers_with_their_exact_spelling() {
        let mut draft = Draft(vec![openrouter("openrouter", &[])]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        let headers = headers_of(&draft.0[0]);
        assert_eq!(
            headers.get("HTTP-Referer").map(String::as_str),
            Some("https://opencode.ai/")
        );
        assert_eq!(headers.get("X-Title").map(String::as_str), Some("opencode"));
        assert_eq!(headers.len(), 2, "no header beyond the two of the source");
    }

    #[test]
    fn header_names_are_not_normalised_to_another_casing() {
        let mut draft = Draft(vec![openrouter("openrouter", &[])]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        let headers = headers_of(&draft.0[0]);
        assert!(headers.get("Http-Referer").is_none());
        assert!(headers.get("http-referer").is_none());
        assert!(headers.get("x-title").is_none());
        assert!(headers.get("X-Title ").is_none());
    }

    #[test]
    fn existing_header_values_are_overwritten_because_the_operator_is_assignment() {
        let mut draft = Draft(vec![aisdk(
            "openrouter",
            OpenRouterPlugin::PACKAGE,
            &[
                ("HTTP-Referer", "https://example.test/"),
                ("X-Title", "Someone Else"),
            ],
        )]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        let headers = headers_of(&draft.0[0]);
        assert_eq!(
            headers.get("HTTP-Referer").map(String::as_str),
            Some("https://opencode.ai/"),
            "the source assigns, it does not default"
        );
        assert_eq!(headers.get("X-Title").map(String::as_str), Some("opencode"));
    }

    #[test]
    fn an_existing_empty_header_value_is_still_overwritten() {
        // `=` tests nothing, so even an empty previous value is replaced. A
        // truthiness guard would have kept it.
        let mut draft = Draft(vec![aisdk(
            "openrouter",
            OpenRouterPlugin::PACKAGE,
            &[("X-Title", "")],
        )]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        assert_eq!(
            headers_of(&draft.0[0]).get("X-Title").map(String::as_str),
            Some("opencode")
        );
    }

    #[test]
    fn the_nullity_operator_keeps_a_present_value_even_when_it_is_empty() {
        // The contrast with the test above, on the operator the source does not
        // use: `??=` sees presence, not truthiness, so `""` survives it.
        let mut headers = Headers::new();
        headers.insert("X-Title".to_string(), String::new());

        set_header_if_absent(&mut headers, "X-Title", "opencode");

        assert_eq!(headers.get("X-Title").map(String::as_str), Some(""));
    }

    #[test]
    fn the_nullity_operator_fills_an_absent_key() {
        let mut headers = Headers::new();

        set_header_if_absent(&mut headers, "X-Title", "opencode");

        assert_eq!(headers.get("X-Title").map(String::as_str), Some("opencode"));
    }

    #[test]
    fn headers_outside_the_plugin_survive_the_pass() {
        let mut draft = Draft(vec![aisdk(
            "openrouter",
            OpenRouterPlugin::PACKAGE,
            &[("Authorization", "Bearer token")],
        )]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        let headers = headers_of(&draft.0[0]);
        assert_eq!(headers.get("Authorization").map(String::as_str), Some("Bearer token"));
        assert_eq!(headers.len(), 3);
    }

    // ------------------------------------------------- provider strict filter

    #[test]
    fn a_provider_of_another_package_is_left_untouched() {
        let mut draft = Draft(vec![openrouter("openrouter", &[]), aisdk("groq", "@ai-sdk/groq", &[])]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        assert!(headers_of(&draft.0[0]).contains_key("X-Title"));
        assert!(headers_of(&draft.0[1]).is_empty());
    }

    #[test]
    fn a_native_provider_is_skipped_even_with_the_right_package_name() {
        let mut record = aisdk("native", OpenRouterPlugin::PACKAGE, &[]);
        record.provider.api = ProviderApi::Native {
            url: None,
            settings: BTreeMap::new(),
        };
        let mut draft = Draft(vec![record]);

        OpenRouterPlugin::transform(&mut draft, &mut Models::default());

        assert!(headers_of(&draft.0[0]).is_empty());
    }

    #[test]
    fn an_empty_package_name_matches_nothing() {
        // The comparison is `!==`, not a truthiness test: `""` is a value, and
        // it is not the package we want.
        let record = aisdk("vide", "", &[]);
        assert!(!OpenRouterPlugin::targets(&record.provider));

        let mut draft = Draft(vec![record]);
        OpenRouterPlugin::transform(&mut draft, &mut Models::default());
        assert!(headers_of(&draft.0[0]).is_empty());
    }

    #[test]
    fn the_package_comparison_is_case_sensitive() {
        let record = aisdk("majuscule", "@OpenRouter/AI-SDK-Provider", &[]);
        assert!(!OpenRouterPlugin::targets(&record.provider));
    }

    #[test]
    fn a_trailing_space_in_the_package_breaks_the_match() {
        let record = aisdk("espace", " @openrouter/ai-sdk-provider", &[]);
        assert!(!OpenRouterPlugin::targets(&record.provider));
    }

    #[test]
    fn an_empty_catalogue_changes_nothing() {
        let mut draft = Draft::default();
        let mut models = Models::default();

        OpenRouterPlugin::transform(&mut draft, &mut models);

        assert!(draft.0.is_empty());
        assert!(models.0.is_empty());
    }

    // --------------------------------------------------------- disabled models

    #[test]
    fn both_aliases_are_disabled_on_an_openrouter_provider_that_carries_them() {
        let mut draft = Draft(vec![openrouter(
            "openrouter",
            &[
                ("gpt-5-chat-latest", json!({ "id": "gpt-5-chat-latest" })),
                ("openai/gpt-5-chat", json!({ "id": "openai/gpt-5-chat" })),
            ],
        )]);
        let mut models = Models::default();

        OpenRouterPlugin::transform(&mut draft, &mut models);

        for alias in OpenRouterPlugin::DISABLED_MODEL_ALIASES {
            assert_eq!(
                models.get("openrouter", alias).and_then(|m| m.enabled),
                Some(false),
                "{alias} should be disabled"
            );
        }
    }

    #[test]
    fn a_provider_without_those_aliases_keeps_its_models_untouched() {
        let mut draft = Draft(vec![openrouter("openrouter", &[("gpt-5", json!({}))])]);
        let mut models = Models::with("openrouter", &[("gpt-5", CatalogModel::new())]);

        OpenRouterPlugin::transform(&mut draft, &mut models);

        assert_eq!(models.get("openrouter", "gpt-5").and_then(|m| m.enabled), None);
    }

    #[test]
    fn a_custom_provider_with_a_matching_model_id_is_not_disabled() {
        // The source comment says exactly this: custom providers that happen to
        // expose the same model ids must stay untouched.
        let mut custom = Draft(vec![openrouter("maison", &[("gpt-5-chat-latest", json!({}))])]);
        custom.0[0].provider.api = ProviderApi::Aisdk {
            package: "@ai-sdk/openai-compatible".to_string(),
            url: None,
            settings: None,
        };
        let mut models = Models::with(
            "maison",
            &[("gpt-5-chat-latest", CatalogModel::new())],
        );

        OpenRouterPlugin::transform(&mut custom, &mut models);

        assert!(
            headers_of(&custom.0[0]).is_empty(),
            "a provider that is not OpenRouter gets no header"
        );
        assert_eq!(
            models.get("maison", "gpt-5-chat-latest").and_then(|m| m.enabled),
            None,
            "a provider that is not OpenRouter keeps its own alias enabled"
        );
    }

    #[test]
    fn only_the_openrouter_provider_is_modified_among_several() {
        let mut draft = Draft(vec![
            aisdk("natif-ne-pas", "@ai-sdk/anthropic", &[]),
            openrouter("premier", &[("gpt-5-chat-latest", json!({}))]),
            aisdk("autre", "@ai-sdk/groq", &[]),
            openrouter("second", &[("openai/gpt-5-chat", json!({}))]),
        ]);
        let mut models = Models::default();

        OpenRouterPlugin::transform(&mut draft, &mut models);

        assert!(headers_of(&draft.0[0]).is_empty());
        assert!(headers_of(&draft.0[2]).is_empty());
        assert!(headers_of(&draft.0[1]).contains_key("HTTP-Referer"));
        assert!(headers_of(&draft.0[3]).contains_key("HTTP-Referer"));
        assert_eq!(
            models.get("premier", "gpt-5-chat-latest").and_then(|m| m.enabled),
            Some(false)
        );
        assert_eq!(
            models.get("second", "openai/gpt-5-chat").and_then(|m| m.enabled),
            Some(false)
        );
    }

    // ------------------------------------------------ presence vs truthiness

    #[test]
    fn a_model_key_present_with_a_falsy_value_still_counts_as_present() {
        let models = BTreeMap::from([("gpt-5-chat-latest".to_string(), Value::Null)]);

        assert!(has_model(&models, "gpt-5-chat-latest"));
        assert!(
            !has_model_if_truthy(&models, "gpt-5-chat-latest"),
            "the truthiness reading disagrees, which is exactly why both exist"
        );
    }

    #[test]
    fn the_plugin_follows_the_presence_reading_and_disables_a_null_valued_alias() {
        let mut draft = Draft(vec![openrouter(
            "openrouter",
            &[("gpt-5-chat-latest", Value::Null)],
        )]);
        let mut models = Models::default();

        OpenRouterPlugin::transform(&mut draft, &mut models);

        assert_eq!(
            models.get("openrouter", "gpt-5-chat-latest").and_then(|m| m.enabled),
            Some(false),
            "Map.has ignores the value, so the alias is disabled"
        );
    }

    #[test]
    fn the_truthy_helper_matches_javascript_on_the_usual_falsy_values() {
        assert!(!is_truthy(&Value::Null));
        assert!(!is_truthy(&json!(false)));
        assert!(!is_truthy(&json!(0)));
        assert!(!is_truthy(&json!("")));
        assert!(!is_truthy(&json!([])));
        assert!(!is_truthy(&json!({})));
        assert!(is_truthy(&json!(true)));
        assert!(is_truthy(&json!(1)));
        assert!(is_truthy(&json!("opencode")));
        assert!(is_truthy(&json!([1])));
        assert!(is_truthy(&json!({ "a": 1 })));
    }

    #[test]
    fn an_absent_model_matches_neither_reading() {
        let models: BTreeMap<String, Value> = BTreeMap::new();
        assert!(!has_model(&models, "openai/gpt-5-chat"));
        assert!(!has_model_if_truthy(&models, "openai/gpt-5-chat"));
    }

    #[test]
    fn disabling_is_unconditional_even_for_a_model_already_disabled() {
        let already = CatalogModel {
            enabled: Some(false),
            extra: BTreeMap::new(),
        };
        let mut draft = Draft(vec![openrouter("openrouter", &[("gpt-5-chat-latest", json!({}))])]);
        let mut models = Models::with("openrouter", &[("gpt-5-chat-latest", already)]);

        OpenRouterPlugin::transform(&mut draft, &mut models);

        assert_eq!(
            models.get("openrouter", "gpt-5-chat-latest").and_then(|m| m.enabled),
            Some(false)
        );
    }

    #[test]
    fn disabling_keeps_the_other_fields_of_the_model() {
        let stored = CatalogModel {
            enabled: Some(true),
            extra: BTreeMap::from([("id".to_string(), json!("gpt-5-chat-latest"))]),
        };
        let mut draft = Draft(vec![openrouter("openrouter", &[("gpt-5-chat-latest", json!({}))])]);
        let mut models = Models::with("openrouter", &[("gpt-5-chat-latest", stored)]);

        OpenRouterPlugin::transform(&mut draft, &mut models);

        let model = models.get("openrouter", "gpt-5-chat-latest").unwrap();
        assert_eq!(model.enabled, Some(false));
        assert_eq!(model.extra.get("id"), Some(&json!("gpt-5-chat-latest")));
    }

    // --------------------------------------------------------------- the hook

    #[test]
    fn the_sdk_hook_builds_the_sdk_for_the_openrouter_package() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);
        let factory = Factory::new();

        let built = OpenRouterPlugin::register_sdk(&mut evt, &factory);

        assert!(built);
        assert_eq!(evt.sdk, Some(json!({ "provider": "openrouter" })));
        assert_eq!(factory.calls.get(), 1);
    }

    #[test]
    fn the_sdk_hook_leaves_another_package_alone() {
        let mut evt = sdk_event("@ai-sdk/openai");
        let factory = Factory::new();

        let built = OpenRouterPlugin::register_sdk(&mut evt, &factory);

        assert!(!built);
        assert_eq!(evt.sdk, None);
        assert_eq!(factory.calls.get(), 0, "the factory is never reached");
    }

    #[test]
    fn the_sdk_hook_ignores_an_empty_package_name() {
        // Same strict inequality as the catalogue side: `""` matches nothing.
        let mut evt = sdk_event("");
        let factory = Factory::new();

        assert!(!OpenRouterPlugin::register_sdk(&mut evt, &factory));
        assert_eq!(evt.sdk, None);
        assert_eq!(factory.calls.get(), 0);
    }

    #[test]
    fn a_sdk_already_present_is_replaced_for_our_package() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);
        evt.sdk = Some(json!("ancien"));
        let factory = Factory::new();

        OpenRouterPlugin::register_sdk(&mut evt, &factory);

        assert_eq!(evt.sdk, Some(json!({ "provider": "openrouter" })));
    }

    #[test]
    fn a_factory_returning_nothing_leaves_the_sdk_absent() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);

        assert!(OpenRouterPlugin::register_sdk(&mut evt, &VoidFactory));
        assert_eq!(evt.sdk, None);
    }

    #[test]
    fn the_options_reach_the_factory_unchanged() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);
        evt.options.insert("apiKey".to_string(), json!("sk-or-v1-test"));
        evt.options.insert("baseURL".to_string(), json!("https://openrouter.ai/api/v1"));
        let factory = Factory::new();

        OpenRouterPlugin::register_sdk(&mut evt, &factory);

        let seen = factory.last_options.borrow().clone().expect("options seen");
        assert_eq!(seen, evt.options);
        assert_eq!(seen.get("apiKey"), Some(&json!("sk-or-v1-test")));
    }

    #[test]
    fn empty_options_reach_the_factory_as_an_empty_record() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);
        let factory = Factory::new();

        OpenRouterPlugin::register_sdk(&mut evt, &factory);

        let seen = factory.last_options.borrow().clone().expect("options seen");
        assert!(seen.is_empty());
    }

    // ------------------------------------------------------- option key names

    #[test]
    fn options_serialise_with_the_camel_case_keys_of_the_typescript() {
        let options = OpenRouterOptions {
            api_key: Some("sk-or-v1-test".to_string()),
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            extra: BTreeMap::new(),
        };

        let text = serde_json::to_string(&options).expect("serialisation");

        assert!(text.contains("\"apiKey\""), "got {text}");
        assert!(text.contains("\"baseURL\""), "got {text}");
        assert!(!text.contains("api_key"), "got {text}");
        assert!(!text.contains("base_url"), "got {text}");
    }

    #[test]
    fn options_absent_from_the_json_omit_their_key() {
        let value = serde_json::to_value(OpenRouterOptions::new()).expect("serialisation");

        assert!(value.get("apiKey").is_none());
        assert!(value.get("baseURL").is_none());
    }

    #[test]
    fn options_are_read_back_from_the_camel_case_keys() {
        let raw = r#"{"apiKey":"sk-or-v1-test","baseURL":"https://openrouter.ai/api/v1"}"#;

        let parsed: OpenRouterOptions = serde_json::from_str(raw).expect("deserialisation");

        assert_eq!(parsed.api_key.as_deref(), Some("sk-or-v1-test"));
        assert_eq!(parsed.base_url.as_deref(), Some("https://openrouter.ai/api/v1"));
    }

    #[test]
    fn the_snake_case_spelling_is_not_accepted_for_either_option() {
        let raw = r#"{"api_key":"sk-or-v1-test","base_url":"https://openrouter.ai/api/v1"}"#;

        let parsed: OpenRouterOptions = serde_json::from_str(raw).expect("deserialisation");

        assert_eq!(parsed.api_key, None, "api_key is not apiKey");
        assert_eq!(parsed.base_url, None, "base_url is not baseURL");
        // The keys survive as unknown options instead of being honoured.
        assert_eq!(parsed.extra.get("api_key"), Some(&json!("sk-or-v1-test")));
        assert_eq!(parsed.extra.get("base_url"), Some(&json!("https://openrouter.ai/api/v1")));
    }

    #[test]
    fn the_raw_lookup_also_ignores_the_snake_case_spelling() {
        let mut options = Options::new();
        options.insert("api_key".to_string(), json!("sk-or-v1-test"));
        options.insert("baseURL".to_string(), json!("https://openrouter.ai/api/v1"));

        assert_eq!(OpenRouterOptions::api_key_of(&options), None);
        assert_eq!(
            OpenRouterOptions::base_url_of(&options),
            Some("https://openrouter.ai/api/v1")
        );
    }

    #[test]
    fn the_typed_view_keeps_the_options_it_does_not_know_about() {
        let mut options = Options::new();
        options.insert("apiKey".to_string(), json!("sk-or-v1-test"));
        options.insert("baseURL".to_string(), json!("https://openrouter.ai/api/v1"));
        options.insert("reasoningEffort".to_string(), json!("high"));
        options.insert("models".to_string(), json!(["a", "b"]));

        let view = OpenRouterOptions::from_options(&options);

        assert_eq!(view.extra.len(), 2);
        let back = view.to_options();
        assert_eq!(back, options);
        assert_eq!(
            serde_json::to_string(&view).expect("serialisation").matches("apiKey").count(),
            1,
            "the known key must not be serialised twice"
        );
    }

    // ------------------------------------------------------- payload spelling

    #[test]
    fn the_event_serialises_with_the_field_names_of_the_typescript() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);
        evt.sdk = Some(json!({ "provider": "openrouter" }));

        let value = serde_json::to_value(&evt).expect("serialisation");

        assert_eq!(
            value,
            json!({
                "model": { "id": "openai/gpt-5" },
                "package": "@openrouter/ai-sdk-provider",
                "options": {},
                "sdk": { "provider": "openrouter" },
            })
        );
        assert_eq!(value.as_object().unwrap().len(), 4);
    }

    #[test]
    fn an_event_without_a_sdk_omits_the_sdk_key() {
        let evt = sdk_event(OpenRouterPlugin::PACKAGE);

        let value = serde_json::to_value(&evt).expect("serialisation");

        assert!(value.get("sdk").is_none());
        assert_eq!(value.as_object().unwrap().len(), 3);
    }

    #[test]
    fn the_package_key_is_read_with_that_exact_spelling() {
        let good = json!({ "model": {}, "package": "@openrouter/ai-sdk-provider", "options": {} });
        assert!(serde_json::from_value::<SdkEvent>(good).is_ok());

        // Every spelling that is not exactly `package` leaves the required
        // field missing, so the read fails instead of defaulting silently.
        for wrong in ["packagE", "package_", "Package", "packag"] {
            let mut bad = serde_json::Map::new();
            bad.insert("model".to_string(), json!({}));
            bad.insert(wrong.to_string(), json!("@openrouter/ai-sdk-provider"));
            bad.insert("options".to_string(), json!({}));

            assert!(
                serde_json::from_value::<SdkEvent>(Value::Object(bad)).is_err(),
                "{wrong} must not be read as package"
            );
        }
    }

    #[test]
    fn an_event_round_trips_through_json_without_loss() {
        let mut evt = sdk_event(OpenRouterPlugin::PACKAGE);
        evt.options.insert("apiKey".to_string(), json!("sk-or-v1-test"));
        evt.sdk = Some(json!({ "provider": "openrouter" }));

        let text = serde_json::to_string(&evt).expect("serialisation");
        let back: SdkEvent = serde_json::from_str(&text).expect("deserialisation");

        assert_eq!(back, evt);
    }
}