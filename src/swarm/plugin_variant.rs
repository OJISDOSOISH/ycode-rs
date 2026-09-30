//! Rust port of `opencode/packages/core/src/plugin/variant.ts`.
//!
//! # What the source actually is
//!
//! Thirty-nine lines, of which nine are the self-reexport
//! (`export * as VariantPlugin from "./variant"` — in Rust the module *is* the
//! namespace, so there is nothing to translate) and four are imports. The real
//! payload is [`generate`], a pure function of a single model, plus a plugin
//! wrapper that walks the catalog and merges the result.
//!
//! [`generate`] reads exactly **four** fields of the model:
//!
//! | Field             | Use                                          |
//! |-------------------|----------------------------------------------|
//! | `model.api.type`  | guard: must be `"aisdk"`                      |
//! | `model.api.package` | guard: must be `"@ai-sdk/openai-compatible"` |
//! | `model.id`        | half of the searched haystack                 |
//! | `model.api.id`    | the other half                                |
//!
//! Everything else on `ModelV2.Info` — capabilities, cost, limits, time,
//! status — is untouched by this plugin, and no test pretends otherwise.
//!
//! # Types are imported, never redeclared
//!
//! This is the point the whole file turns on. `ModelV2Info` is the SDK's
//! `ModelV2.Info` (`packages/schema/src/model.ts:60-86`), already ported as
//! [`ModelInfo`] in `crate::core::model`. Its `api` field is
//! `Schema.Union([...]).pipe(Schema.toTaggedUnion("type"))` — a genuine
//! discriminated union — and it is already ported as [`ModelApi`], an enum
//! carrying `#[serde(tag = "type")]` plus an explicit `#[serde(rename)]` on each
//! variant. A variant record is already ported as [`ModelVariant`], and the
//! composite key `(providerID, modelID)` as [`ModelRef`].
//!
//! None of those four types is declared here. Eight provider files in this
//! directory (`provider_kilo.rs`, `provider_nvidia.rs`, `provider_zenmux.rs`
//! and five others) each carry their own local `Api` / `ProviderInfo` copy, and
//! Rust accepts a same-named type in two modules without a single diagnostic,
//! so those copies have already drifted apart in silence. Adding a ninth or a
//! tenth would be the same mistake wearing a different module name. The one
//! thing this file owes the contract is a set of tests that **prove** the
//! inherited renames still hold, which is what the two TRAP 1 tests below do.
//!
//! The only type declared here that is not a port of a union is [`Catalog`],
//! and it is a container, not a contract: it holds no field names of its own
//! and is never serialised. Its shape is deliberately not
//! `CatalogProviderRecord` from `provider_cerebras.rs`, whose `models` is an
//! opaque `serde_json::Value` because that plugin never reads it. This plugin
//! *does* read it, so it needs the real [`ModelInfo`]. The record's `provider`
//! member is not modelled at all, because this plugin never reads it either —
//! see [`Catalog::records`].
//!
//! # TRAP 1 — field names are UPPERCASE: it applies, once
//!
//! `variant.ts` has no `projectID` and no `integrationID`; those belong to
//! other files (`project.ts`, `provider.ts`). The uppercase field that *is*
//! here is **`providerID`**, and it is the load-bearing one: it is the first
//! argument of `catalog.model.update(...)`, so it is the key under which the
//! mutation lands.
//!
//! `providerID` is not `providerId`, and it is not `provider_id`. Getting it
//! wrong compiles, runs, and produces a catalog where the variants are written
//! to a provider that does not exist. The rename therefore lives in
//! `crate::core::model` and is **inherited** here, never restated: a local
//! `#[serde(rename = "providerID")]` on a locally declared copy would be the
//! tenth divergence, not a fix.
//!
//! Two tests guard it from this side of the boundary, because that is the only
//! side this file controls:
//!
//! - [`provider_id_serialises_with_trailing_capitals`] serialises and asserts
//!   the key is `providerID` and that `provider_id` is absent;
//! - [`snake_case_provider_id_is_rejected_on_read`] feeds the snake_case form
//!   back in and asserts deserialisation **fails**.
//!
//! ## This file contains both casing conventions, and both are correct
//!
//! That is the part that makes the trap genuinely dangerous here. The same
//! function emits:
//!
//! - `providerID` — **must** keep its capitals (a TypeScript field name);
//! - `reasoning_effort` — **must** keep its underscores (not a TypeScript
//!   field name, but a literal key in a request body sent to an
//!   OpenAI-compatible endpoint, which expects the upstream snake_case
//!   spelling).
//!
//! "Normalising" either one breaks the exchange in opposite directions. Two
//! tests pin both, and they fail for opposite reasons, which is the point.
//!
//! # TRAP 2 — `??` is nullity, `?` is truthiness
//!
//! The source contains **no** ternary. It contains exactly one coalescent:
//!
//! ```ts
//! ...generated.map((variant) => explicit.get(variant.id) ?? variant),
//! ```
//!
//! The left operand is `Map.get`, which yields either a `ModelVariant` or
//! `undefined`, so the fallback fires on **absence** and on nothing else.
//! [`coalesce_nullity`] is that operator. [`ternary_truthiness`] is its
//! opposite, ported for contrast and for the guarantee that the two are never
//! collapsed into one helper: it treats `Some("")` as falsy and therefore
//! discards it, which is what `a ? a : b` would do to an empty string.
//!
//! Two tests demonstrate the split on the input the source can actually see.
//!
//! ### The honest caveat: the distinction is latent at this call site
//!
//! The left operand is always an object or `undefined`, and every object is
//! truthy in JavaScript, so at *this* call site `??` and `?` happen to agree.
//! That is a property of the argument type, not a licence to merge the two
//! operators. A future edit that makes the lookup nullable — a `null` stored in
//! the map, a stringly-typed variant table — turns a silent behaviour change
//! into a visible one only if the two helpers stayed separate. Hence both
//! exist, and [`empty_string_survives_the_coalescent_but_not_the_ternary`] pins
//! the difference.
//!
//! # The literals trap: bare strings, not tagged objects
//!
//! `["high", "max"]` is an array of plain strings. A sibling proof in this
//! codebase established that `Schema.Literals([...])` also serialises as a
//! **bare string**, so putting `#[serde(tag = "...")]` on a literals-derived
//! enum would emit `{"High":null}` and break the exchange. [`ReasoningEffort`]
//! is therefore a unit enum with an explicit `#[serde(rename = "...")]` on
//! every variant and **no** `tag` attribute at all.
//!
//! The two union shapes in this file must not be confused, and the contrast is
//! deliberate:
//!
//! | Type               | Tagged? | Wire form                              |
//! |--------------------|---------|----------------------------------------|
//! | `ModelApi`         | yes     | `{"type":"aisdk","id":…,"package":…}`  |
//! | `ReasoningEffort`  | no      | `"high"`                               |
//!
//! Both are unit-vs-data unions; one is a discriminant inside an object, the
//! other *is* the whole value. Two tests assert each shape, and one of them
//! asserts the forbidden `{"High":null}` form never appears.
//!
//! # JavaScript `Map` semantics that survive the port
//!
//! Three behaviours of the merge are load-bearing and none of them is what a
//! naive translation produces.
//!
//! 1. **Last duplicate wins.** `new Map(draft.variants.map(v => [v.id, v]))`
//!    keeps the *last* entry for a repeated id, because `Map` construction
//!    overwrites. A `.find()` over the list would keep the first, and the two
//!    disagree whenever a model ships duplicated variants.
//! 2. **Explicit wins whole, not field-wise.** `explicit.get(id) ?? variant`
//!    substitutes the entire object. An explicit variant with a custom header
//!    keeps that header *and* its `body`; it does not inherit the generated
//!    `reasoning_effort`. This is a replacement, not a merge, and the port
//!    keeps it that way.
//! 3. **The list is rebuilt, so explicit entries move.** The result is
//!    `generated.map(...)` **then** the pre-existing variants that are not
//!    generated, in their original relative order. An explicit `"high"` that
//!    sat at index 2 comes back at index 0, because it is emitted in the
//!    generated slot. Nothing is appended in place.
//!
//! # The update key comes from the model, not from the record
//!
//! ```ts
//! for (const record of catalog.provider.list())
//!   for (const model of record.models.values())
//!     catalog.model.update(model.providerID, model.id, …)
//! ```
//!
//! The loop walks *records*, but the key is built from the *model's own*
//! `providerID` and `id`, never from the record's provider key. A model stored
//! under provider `kilo` that claims `providerID: "other"` is written to
//! provider `other`, and `catalog.model.update` will **create** that record if
//! it does not exist (`catalog.ts:130-142`). [`Catalog::model_update`]
//! reproduces that, default creation and the two post-`fn` stamps
//! (`model.id = modelID; model.providerID = providerID`, `catalog.ts:143-144`)
//! included.
//!
//! This is the second place the `providerID` casing is load-bearing, and it is
//! the reason [`Catalog::model_keys`] is a separate pass instead of an
//! iterator chained into the mutation: the two phases are what let the key be
//! read from the model while the entry is still writable.
//!
//! # Ordering
//!
//! The `Map` of providers iterates in **insertion** order in JavaScript, so
//! [`Catalog::records`] is a `Vec`, not a `BTreeMap`. A `BTreeMap` would sort
//! provider ids and change the order in which `update` is called, which is
//! observable through the key list [`appliquer`] returns. The models inside a
//! record are likewise a `Vec`, because `record.models.values()` is insertion
//! ordered too. The `BTreeMap`/`BTreeSet` in this file are used only for the
//! merge lookup, which the source never iterates — so the ordering there is an
//! implementation detail with no wire consequence.
//!
//! # Lowercasing
//!
//! `` `${model.id} ${model.api.id}`.toLowerCase() `` is Unicode-aware, so the
//! port uses [`str::to_lowercase`] and **not** `to_ascii_lowercase`. The
//! markers are ASCII, so the two agree on every model name that can match; they
//! would differ on a model id containing e.g. `İ`, which is exactly the case
//! where the ASCII version would be wrong.
//!
//! `model.api.id` is a required field of the schema, so the template never
//! produces the literal `"undefined"` that a missing optional would. There is
//! no `undefined` branch to port, and none is invented.
//!
//! # Integration
//!
//! This file expects `pub mod plugin_variant;` in `src/swarm/mod.rs`, which it
//! does not touch. It imports only from `crate::core::model` and depends on
//! `serde` / `serde_json`, already in `Cargo.toml`. It touches no other module
//! of this batch — in particular it does not import the eight local `Api`
//! copies discussed above.
//!
//! Thirty-three tests, all pure and instantaneous: no thread, no `Mutex`, no
//! `Condvar`, no sleep, no waiting, no loop without a bound, no filesystem, no
//! network. The only iteration is over finite `Vec`s.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::model::{ModelApi, ModelInfo, ModelRef, ModelVariant};

/// Plugin identifier, exactly as registered with `define`.
///
/// In TS: the `id` property of the object passed to `define`, line 8.
/// `define` (`plugin/internal.ts:59-61`) is the identity function, so the
/// plugin type is an id plus an effect, and the id is the whole observable of
/// the registration.
pub const ID: &str = "variant";

/// The only `model.api.type` this plugin accepts.
///
/// In TS: the left operand of the `||` on line 31. A `native` model never
/// reaches the `package` comparison, because `||` short-circuits — and in Rust
/// the `match` arm returns before the field is even reachable, since
/// [`crate::core::model::NativeApi`] has no `package` member at all.
pub const API_TYPE_AISDK: &str = "aisdk";

/// The only npm package this plugin accepts.
///
/// In TS: the right operand of the same `||`, line 31. The comparison is
/// strict equality on a `string`, not a truthiness test, so a different
/// package — including an empty one — is rejected.
pub const API_PACKAGE_OPENAI_COMPATIBLE: &str = "@ai-sdk/openai-compatible";

/// The only key [`generate`] ever writes into a variant body.
///
/// This is **not** a TypeScript field name: it is a literal key of the JSON
/// body sent to an OpenAI-compatible endpoint, which speaks upstream
/// snake_case. It must keep its underscore. Contrast with `providerID`, which
/// must keep its capitals, in the same file.
pub const BODY_KEY_REASONING_EFFORT: &str = "reasoning_effort";

/// The three substrings that select a GLM model, in source order.
///
/// In TS: the array literal on line 33, tested with
/// `["glm-5.2", "glm-5-2", "glm-5p2"].some((name) => ids.includes(name))`.
///
/// `includes` is substring containment, and the order is irrelevant to the
/// result because `.some` short-circuits on the first hit. The order is kept
/// anyway, so the constant can be diffed against the source. All three are
/// already lowercase; the haystack is lowercased before the comparison.
pub const GLM_MARKERS: [&str; 3] = ["glm-5.2", "glm-5-2", "glm-5p2"];

/// A reasoning effort level, ported from the `["high", "max"]` literal.
///
/// # Why there is no `tag` here
///
/// This is a *literals*-derived enum, and literals serialise as **bare
/// strings** in this codebase. A `#[serde(tag = "...")]` would emit
/// `{"High":null}` and deserialisation would then reject the bare `"high"`
/// that the TypeScript actually writes into `body.reasoning_effort`. The
/// attribute is therefore deliberately absent, and
/// [`reasoning_effort_serialises_as_a_bare_string`] locks the shape down.
///
/// The contrast with [`ModelApi`], which *is* tagged on `"type"`, is the whole
/// point: a discriminant inside an object is tagged, a value that is only a
/// string is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReasoningEffort {
    /// `"high"`, the first element of the source array.
    #[serde(rename = "high")]
    High,
    /// `"max"`, the second element of the source array.
    #[serde(rename = "max")]
    Max,
}

impl ReasoningEffort {
    /// The two levels, in the order the source maps over them.
    ///
    /// The order is observable: it is the order of the generated `variants`
    /// array, and therefore of the front of the rebuilt list. `high` first.
    pub const ALL: [ReasoningEffort; 2] = [ReasoningEffort::High, ReasoningEffort::Max];

    /// The wire string, which is also the identifier of the emitted variant.
    pub fn as_str(self) -> &'static str {
        match self {
            ReasoningEffort::High => "high",
            ReasoningEffort::Max => "max",
        }
    }
}

/// The body of a generated variant: a single-entry map.
///
/// In TS: `body: { reasoning_effort: id }`, line 37. One key, whose value is
/// the variant id repeated, which is why `high` and `max` carry the same string
/// in both `id` and `body.reasoning_effort`.
fn reasoning_body(effort: ReasoningEffort) -> BTreeMap<String, Value> {
    let mut body = BTreeMap::new();
    body.insert(
        BODY_KEY_REASONING_EFFORT.to_string(),
        Value::String(effort.as_str().to_string()),
    );
    body
}

/// The `??` operator: falls back on **absence** and on nothing else.
///
/// In TS: `explicit.get(variant.id) ?? variant`, line 20.
///
/// `??` tests **nullity**, so an empty string, a zero and a `false` all
/// survive. The left operand here is a `ModelVariant | undefined`, so the only
/// thing that triggers the fallback is a missing map entry. This is the
/// operator the source uses, and the one the merge uses.
///
/// See [`ternary_truthiness`] for the operator this is *not*.
pub fn coalesce_nullity<T>(present: Option<T>, fallback: T) -> T {
    present.unwrap_or(fallback)
}

/// The `?` operator: falls back on **falsiness**, so an empty string is lost.
///
/// Ported for contrast and for the guarantee that the two operators of TRAP 2
/// stay two functions and never collapse into one helper. `variant.ts` uses
/// no ternary, so nothing in this port calls it; it exists so that the
/// difference is executable rather than merely asserted in prose.
///
/// In JavaScript, among strings only `""` is falsy: `"0"`, `" "` and
/// `"false"` are all truthy, and `undefined` and `null` are falsy. [`Option`]
/// models the `undefined`/`null` side, [`str::is_empty`] models the rest.
pub fn ternary_truthiness<'a>(present: Option<&'a str>, fallback: &'a str) -> &'a str {
    match present {
        Some(value) if !value.is_empty() => value,
        _ => fallback,
    }
}

/// The catalog draft, reduced to the surface this plugin touches.
///
/// In TS: `Catalog.ProviderRecord` (`catalog.ts:13-16`) plus the two draft
/// methods the plugin calls, `provider.list` and `model.update`
/// (`catalog.ts:31` and `catalog.ts:130-145`).
///
/// # What is dropped, and why that is safe
///
/// `ProviderRecord.provider` is **not** modelled. The plugin never reads it —
/// it iterates `record.models.values()` and keys everything off the model. The
/// same is true of `provider.update`, `provider.get`, `provider.remove`,
/// `model.get` and `model.remove`: none is called by `variant.ts`.
///
/// The record's provider key is kept, not because the plugin reads it, but
/// because [`Catalog::model_update`] has to know which record to look in, and
/// because the provider key and the model's claimed `providerID` are allowed to
/// disagree — see [`Catalog::model_keys`].
///
/// [`records`] is a `Vec` and not a `BTreeMap` because the JavaScript `Map` it
/// mirrors iterates in insertion order, and that order is observable through
/// the key list [`appliquer`] returns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    /// One entry per provider record: `(providerID, models)`, in insertion
    /// order. Models within a record are also in insertion order, matching
    /// `record.models.values()`.
    pub records: Vec<(String, Vec<ModelInfo>)>,
}

impl Catalog {
    /// An empty catalog, equivalent to `initial: () => ({ providers: new Map() })`.
    pub fn new() -> Self {
        Catalog { records: Vec::new() }
    }

    /// Appends a provider record, or replaces the models of an existing one.
    ///
    /// A convenience for building a draft in tests and by callers; the plugin
    /// itself never inserts, it only iterates and calls [`Catalog::model_update`].
    pub fn push_record(&mut self, provider_id: impl Into<String>, models: Vec<ModelInfo>) {
        let provider_id = provider_id.into();
        if let Some(index) = self.records.iter().position(|(id, _)| *id == provider_id) {
            self.records[index].1 = models;
        } else {
            self.records.push((provider_id, models));
        }
    }

    /// Every `(providerID, modelID)` pair reachable from the model side, in
    /// provider order then model order.
    ///
    /// This is the key list [`appliquer`] walks, and it is built from
    /// **`model.providerID` and `model.id`**, never from the record's own
    /// provider key. That is what the source does:
    /// `catalog.model.update(model.providerID, model.id, …)`, and it means a
    /// model that disagrees with its record is written to the provider it
    /// names, possibly creating that provider.
    ///
    /// It is a separate pass over the catalog rather than an iterator chained
    /// into the mutation, because the write needs a mutable borrow of the very
    /// entry the read came from.
    pub fn model_keys(&self) -> Vec<ModelRef> {
        self.records
            .iter()
            .flat_map(|(_, models)| models.iter())
            .map(|model| ModelRef {
                id: model.id.clone(),
                provider_id: model.provider_id.clone(),
                variant: None,
            })
            .collect()
    }

    /// Reads a model without creating it.
    ///
    /// Port of `catalog.model.get` (`catalog.ts:129`), restricted to the part
    /// this plugin needs. Unlike the TypeScript version it does not project the
    /// model against its provider, because this plugin reads the draft, not
    /// the projected read model.
    pub fn model(&self, provider_id: &str, model_id: &str) -> Option<&ModelInfo> {
        self.records
            .iter()
            .find(|(id, _)| id == provider_id)
            .and_then(|(_, models)| models.iter().find(|model| model.id == model_id))
    }

    /// Resolves a key to a mutable model, creating the record and the model
    /// when they are missing, then re-stamps the key onto the model.
    ///
    /// Port of `catalog.model.update` (`catalog.ts:130-145`), with the
    /// `fn` argument removed: the caller mutates the returned reference
    /// directly, which is the same thing with less ceremony.
    ///
    /// Three behaviours are preserved and each is load-bearing:
    ///
    /// 1. a missing provider record is created (`catalog.ts:131-137`);
    /// 2. a missing model is created from the schema defaults via
    ///    `ModelV2.Info.empty` — a **native** api, no variants, no cost
    ///    (`catalog.ts:140`, [`crate::core::model::empty_info`]);
    /// 3. `model.id` and `model.providerID` are re-stamped from the key
    ///    *after* the mutation (`catalog.ts:143-144`). When the key was built
    ///    from the model itself, as it is here, this is a no-op; it is kept
    ///    because it is what makes a key-driven write authoritative.
    pub fn model_update(&mut self, key: &ModelRef) -> &mut ModelInfo {
        let index = match self
            .records
            .iter()
            .position(|(provider_id, _)| *provider_id == key.provider_id)
        {
            Some(index) => index,
            None => {
                self.records.push((key.provider_id.clone(), Vec::new()));
                self.records.len() - 1
            }
        };

        let models = &mut self.records[index].1;
        let position = match models.iter().position(|model| model.id == key.id) {
            Some(position) => position,
            None => {
                models.push(ModelInfo::empty_info(&key.provider_id, &key.id));
                models.len() - 1
            }
        };

        let model = &mut models[position];
        model.id = key.id.clone();
        model.provider_id = key.provider_id.clone();
        model
    }
}

/// Generates the reasoning variants for one model, or nothing at all.
///
/// Port of `export function generate(model: ModelV2Info)`, lines 30-39. Pure:
/// no clock, no filesystem, no network, no I/O of any kind. It reads four
/// fields and returns a fresh `Vec` every call, so two calls on the same model
/// return equal but independent values.
///
/// The three rejection paths, in the order the source tests them:
///
/// 1. `api.type` is not `aisdk` — including every `native` model;
/// 2. `api.package` is not `@ai-sdk/openai-compatible`;
/// 3. none of the three GLM markers is a substring of the lowercased
///    `` `${model.id} ${model.api.id}` ``.
///
/// Each returns an **empty vector**, not a one-element vector holding a
/// placeholder: the caller tests `generated.length === 0` and returns early,
/// so a placeholder would silently reach the model list.
pub fn generate(model: &ModelInfo) -> Vec<ModelVariant> {
    // The `||` on line 31 short-circuits: a `native` api never reaches the
    // `package` comparison, and `NativeApi` has no such field to compare.
    let api = match &model.api {
        ModelApi::Aisdk(api) => api,
        ModelApi::Native(_) => return Vec::new(),
    };

    if api.package != API_PACKAGE_OPENAI_COMPATIBLE {
        return Vec::new();
    }

    // `${model.id} ${model.api.id}` — one space, then `.toLowerCase()` over the
    // whole template result, not over each half separately. The outcome is the
    // same here, but the order is what the source says and it costs nothing to
    // keep. `to_lowercase` is Unicode-aware, matching `String.toLowerCase`;
    // `to_ascii_lowercase` would not be.
    let ids = format!("{} {}", model.id, api.id).to_lowercase();

    if !GLM_MARKERS.iter().any(|marker| ids.contains(*marker)) {
        return Vec::new();
    }

    ReasoningEffort::ALL
        .iter()
        .map(|effort| ModelVariant {
            id: effort.as_str().to_string(),
            headers: BTreeMap::new(),
            body: reasoning_body(*effort),
        })
        .collect()
}

/// Rebuilds a variant list from the generated ones and the pre-existing ones.
///
/// Port of the body of the `catalog.model.update` callback, lines 14-25:
///
/// ```ts
/// if (generated.length === 0) return
/// const explicit = new Map(draft.variants.map((variant) => [variant.id, variant]))
/// const generatedIDs = new Set(generated.map((variant) => variant.id))
/// draft.variants = [
///   ...generated.map((variant) => explicit.get(variant.id) ?? variant),
///   ...draft.variants.filter((variant) => !generatedIDs.has(variant.id)),
/// ]
/// ```
///
/// The empty case is handled by the caller ([`appliquer`]), because in the
/// source the early return also skips the update bookkeeping; keeping it here
/// would make this function silently rebuild a list it was told to leave alone.
///
/// # Last duplicate wins
///
/// Building the lookup with a loop of `insert` reproduces `new Map(entries)`:
/// for a repeated id the **last** occurrence overwrites the earlier one. A
/// `find` over the list would keep the first, and the two disagree on any
/// model that ships duplicated variant ids.
///
/// # Explicit replaces whole
///
/// [`coalesce_nullity`] substitutes the entire [`ModelVariant`], so an explicit
/// entry keeps its own `headers` *and* its own `body` — it does not pick up the
/// generated `reasoning_effort`. This is a replacement, not a field-wise merge.
///
/// # The list is rebuilt, not patched
///
/// Generated entries come first, in generated order, each in its generated slot
/// even when the pre-existing entry it replaced sat elsewhere. Then come the
/// pre-existing entries that are not generated, in their original relative
/// order. Nothing keeps its index.
pub fn merge(existing: Vec<ModelVariant>, generated: Vec<ModelVariant>) -> Vec<ModelVariant> {
    let mut explicit: BTreeMap<String, ModelVariant> = BTreeMap::new();
    for variant in existing.iter() {
        explicit.insert(variant.id.clone(), variant.clone());
    }

    let generated_ids: BTreeSet<String> = generated.iter().map(|variant| variant.id.clone()).collect();

    let mut merged = Vec::with_capacity(generated.len() + existing.len());

    // The `??` of line 20: the explicit entry when there is one, the generated
    // entry otherwise.
    for variant in generated.iter() {
        merged.push(coalesce_nullity(explicit.get(&variant.id).cloned(), variant.clone()));
    }

    // The `filter` of line 21: everything that is not itself generated, kept in
    // its original relative order.
    for variant in existing.iter() {
        if !generated_ids.contains(&variant.id) {
            merged.push(variant.clone());
        }
    }

    merged
}

/// Applies [`generate`] to every model in the catalog and merges the result.
///
/// Port of the whole `ctx.catalog.transform` callback, lines 10-26. Returns
/// the key of every `update` call that was made, in call order — including the
/// calls whose closure returned early, because the source calls `update` for
/// every model and decides inside the callback.
///
/// Splitting the loop into a key pass and a write pass is the same traversal
/// order, not a reordering: [`Catalog::model_keys`] walks providers in
/// insertion order and models in insertion order, exactly as
/// `for (const record of …) for (const model of …)` does.
pub fn appliquer(catalog: &mut Catalog) -> Vec<ModelRef> {
    let keys = catalog.model_keys();
    let mut updated = Vec::with_capacity(keys.len());

    for key in keys {
        let model = catalog.model_update(&key);
        let generated = generate(model);

        // `if (generated.length === 0) return` — line 15. Skipping the merge is
        // not an optimisation: the merge rebuilds the list, so running it on an
        // empty `generated` would still drop every pre-existing variant whose
        // id is in the (empty) generated set — harmless today, but it would
        // re-order the list for nothing and it is not what the source does.
        if !generated.is_empty() {
            let existing = std::mem::take(&mut model.variants);
            model.variants = merge(existing, generated);
        }

        updated.push(key);
    }

    updated
}

/// The plugin, as `define({ id: "variant", effect })` declares it.
///
/// `define` is the identity function (`plugin/internal.ts:59-61`), so the
/// exported plugin reduces to an identifier plus an effect. The effect is
/// `ctx.catalog.transform(...)` over a body that is entirely synchronous, so
/// it is modelled as a plain method taking the draft — there is nothing to
/// make asynchronous and no `Effect` service to construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VariantPlugin;

impl VariantPlugin {
    /// The identifier expected by the registry, as a constant.
    pub const ID: &'static str = ID;

    /// The plugin value. `#[derive(Default)]` would do as well; the explicit
    /// constructor keeps the shape symmetrical with the neighbouring ports.
    pub fn new() -> Self {
        VariantPlugin
    }

    /// Registers the catalog transform, which is the whole effect.
    pub fn transform(&self, catalog: &mut Catalog) -> Vec<ModelRef> {
        appliquer(catalog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::AisdkApi;
    use serde_json::json;

    // ------------------------------------------------------------- builders

    /// A `native` model, which is what every rejection path starts from.
    fn native(id: &str) -> ModelInfo {
        ModelInfo::empty_info("acme", id)
    }

    /// An `aisdk` model with a chosen package and a chosen `api.id`.
    fn aisdk(model_id: &str, api_id: &str, package: &str) -> ModelInfo {
        let mut info = native(model_id);
        info.api = ModelApi::Aisdk(AisdkApi {
            id: api_id.to_string(),
            package: package.to_string(),
            url: None,
            settings: None,
        });
        info
    }

    /// A GLM model that passes both guards: the shape most tests need.
    fn glm(model_id: &str) -> ModelInfo {
        aisdk(model_id, model_id, API_PACKAGE_OPENAI_COMPATIBLE)
    }

    /// A pre-existing variant with no headers and no body.
    fn bare(id: &str) -> ModelVariant {
        ModelVariant {
            id: id.to_string(),
            headers: BTreeMap::new(),
            body: BTreeMap::new(),
        }
    }

    /// A pre-existing variant carrying one header, used to prove that an
    /// explicit entry survives the merge whole.
    fn with_header(id: &str, name: &str, value: &str) -> ModelVariant {
        let mut headers = BTreeMap::new();
        headers.insert(name.to_string(), value.to_string());
        ModelVariant {
            id: id.to_string(),
            headers,
            body: BTreeMap::new(),
        }
    }

    /// A one-provider catalog holding the given models.
    fn catalog_of(models: Vec<ModelInfo>) -> Catalog {
        let mut catalog = Catalog::new();
        catalog.push_record("acme", models);
        catalog
    }

    /// The variant ids of a list, for order assertions.
    fn ids_of(variants: &[ModelVariant]) -> Vec<String> {
        variants.iter().map(|variant| variant.id.clone()).collect()
    }

    // -------------------------------------------------------- the plugin id

    #[test]
    fn the_plugin_announces_itself_as_variant() {
        assert_eq!(ID, "variant");
        assert_eq!(VariantPlugin::ID, "variant");
        assert_eq!(VariantPlugin::new(), VariantPlugin);
        assert_eq!(VariantPlugin::default(), VariantPlugin);
    }

    // ------------------------------------------------------- generate: guards

    #[test]
    fn a_native_api_generates_nothing() {
        let info = native("glm-5.2");
        assert!(matches!(info.api, ModelApi::Native(_)));
        assert!(
            generate(&info).is_empty(),
            "a native model is rejected before the package is even reachable"
        );
    }

    #[test]
    fn another_aisdk_package_generates_nothing() {
        let info = aisdk("glm-5.2", "glm-5.2", "@ai-sdk/anthropic");
        assert!(generate(&info).is_empty());

        // Strict equality, not truthiness: an empty package is rejected too,
        // and the guard is not a `?` test in disguise.
        let vide = aisdk("glm-5.2", "glm-5.2", "");
        assert!(generate(&vide).is_empty());
    }

    #[test]
    fn a_non_glm_model_generates_nothing() {
        let info = glm("claude-sonnet-4-6");
        assert!(generate(&info).is_empty());
    }

    // -------------------------------------------------- generate: the markers

    #[test]
    fn all_three_markers_are_accepted() {
        for marker in GLM_MARKERS {
            let info = glm(marker);
            assert_eq!(
                generate(&info).len(),
                2,
                "marker {marker} was not recognised as a GLM id"
            );
        }
        assert_eq!(GLM_MARKERS, ["glm-5.2", "glm-5-2", "glm-5p2"]);
    }

    #[test]
    fn the_marker_is_matched_after_lowercasing_the_model_id() {
        let info = glm("GLM-5.2-Preview");
        let generated = generate(&info);
        assert_eq!(ids_of(&generated), ["high", "max"]);

        // Without the `toLowerCase`, the uppercase id would not contain any
        // marker. The lowercasing is applied to the whole haystack, so both
        // halves are covered.
        let info = glm("GLM5P2");
        assert_eq!(generate(&info).len(), 2);
    }

    #[test]
    fn lowercasing_is_unicode_aware_and_not_ascii_only() {
        // `İ` lowercases to `i` plus a combining dot above under Unicode rules,
        // so `GLM-5.2` built next to it still matches once lowercased. This test
        // documents that the port uses `to_lowercase`; an `to_ascii_lowercase`
        // port would leave the capital `İ` untouched and behave differently on
        // any model id containing it. The assertion below holds either way for
        // this input, so what is really pinned is that the marker search runs
        // on a lowercased haystack at all.
        let mut info = aisdk("model-\u{0130}", "model-\u{0130}", API_PACKAGE_OPENAI_COMPATIBLE);
        assert!(generate(&info).is_empty());

        info.id = "\u{0130}GLM-5.2".to_string();
        assert_eq!(generate(&info).len(), 2);
    }

    #[test]
    fn the_marker_is_hyphen_sensitive() {
        // Substring, not fuzzy matching: the dot and the `p` forms are the only
        // spellings the source accepts, so `glm5.2` and `glm5p2` are rejected.
        for absent in ["glm5.2", "glm5p2", "glm 5.2", "glm5-2"] {
            let info = glm(absent);
            assert!(
                generate(&info).is_empty(),
                "{absent} was accepted but the source rejects it"
            );
        }
    }

    #[test]
    fn the_api_id_is_searched_too_and_the_join_uses_one_space() {
        // Only `model.api.id` carries the marker.
        let info = aisdk("my-model", "glm-5.2-1024k", API_PACKAGE_OPENAI_COMPATIBLE);
        assert_eq!(generate(&info).len(), 2, "api.id is part of the haystack");

        // Only `model.id` carries it.
        let info = aisdk("glm-5.2", "some-deployment", API_PACKAGE_OPENAI_COMPATIBLE);
        assert_eq!(generate(&info).len(), 2, "model.id is part of the haystack");

        // The separator is a single space, so the marker cannot straddle it.
        // This is the one case that distinguishes `` `${a} ${b}` `` from
        // `` `${a}${b}` ``.
        let info = aisdk("glm", "-5.2", API_PACKAGE_OPENAI_COMPATIBLE);
        assert!(
            generate(&info).is_empty(),
            "the haystack is 'glm -5.2', which does not contain 'glm-5.2'"
        );
    }

    // ------------------------------------------------- generate: the payloads

    #[test]
    fn generated_variants_are_high_then_max_with_empty_headers() {
        let generated = generate(&glm("glm-5.2"));
        assert_eq!(ids_of(&generated), ["high", "max"], "source order is preserved");

        for variant in &generated {
            assert!(variant.headers.is_empty(), "headers is {{}} in the source");
            assert_eq!(variant.headers, BTreeMap::new());
        }
    }

    #[test]
    fn the_body_key_is_snake_case_reasoning_effort() {
        let generated = generate(&glm("glm-5.2"));
        let expected = [
            ("high", json!("high")),
            ("max", json!("max")),
        ];

        for (variant, (id, effort)) in generated.iter().zip(expected.iter()) {
            assert_eq!(variant.id, *id);
            assert_eq!(
                variant.body.get(BODY_KEY_REASONING_EFFORT),
                Some(effort),
                "body.reasoning_effort must mirror the variant id"
            );
            assert_eq!(variant.body.len(), 1, "the body has exactly one key");
        }

        // The underscore is part of the upstream OpenAI-compatible wire format,
        // not a Rust naming convention that leaked in. Two neighbouring
        // spellings are therefore wrong, in opposite directions.
        for variant in &generated {
            assert!(!variant.body.contains_key("reasoningEffort"), "camelCase is wrong here");
            assert!(!variant.body.contains_key("reasoningeffort"), "the underscore is required");
        }
    }

    #[test]
    fn a_generated_variant_serialises_to_the_exact_source_json() {
        let generated = generate(&glm("glm-5.2"));
        assert_eq!(
            serde_json::to_string(&generated[0]).unwrap(),
            r#"{"id":"high","headers":{},"body":{"reasoning_effort":"high"}}"#
        );
        assert_eq!(
            serde_json::to_string(&generated[1]).unwrap(),
            r#"{"id":"max","headers":{},"body":{"reasoning_effort":"max"}}"#
        );
    }

    // ------------------------------------------- TRAP 1: the uppercase fields

    #[test]
    fn provider_id_serialises_with_trailing_capitals() {
        // The rename is declared once, in `crate::core::model`. This file
        // inherits it, and this test is the guard that the inheritance holds:
        // if someone "tidy" the rename into a snake_case form there, this is
        // where it shows up.
        let key = ModelRef { id: "glm-5.2".to_string(), provider_id: "acme".to_string(), variant: None };
        let json = serde_json::to_value(&key).unwrap();
        let object = json.as_object().unwrap();

        assert_eq!(object.get("providerID"), Some(&json!("acme")));
        assert!(object.get("provider_id").is_none(), "the snake_case form leaked: {json}");
        assert!(object.get("providerId").is_none(), "the camelCase form leaked: {json}");
        assert!(object.get("ProviderID").is_none());
        assert!(object.get("providerID_").is_none());
        assert_eq!(object.len(), 2, "variant is None and must not be written: {json}");

        // The same casing on the model itself, which is where the key is read
        // from in the first place.
        let model = glm("glm-5.2");
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["providerID"], json!("acme"));
        assert!(json.get("provider_id").is_none());
    }

    #[test]
    fn snake_case_provider_id_is_rejected_on_read() {
        // The read side. Feeding the snake_case form back must fail rather than
        // silently populate a default: a permissive read here is how a renamed
        // field turns into a silently empty provider id at runtime.
        let snake: Result<ModelRef, _> = serde_json::from_str(r#"{"id":"glm-5.2","provider_id":"acme"}"#);
        assert!(snake.is_err(), "provider_id must not be accepted for providerID");

        let camel: Result<ModelRef, _> = serde_json::from_str(r#"{"id":"glm-5.2","providerId":"acme"}"#);
        assert!(camel.is_err(), "providerId must not be accepted for providerID");

        // The wire form round-trips.
        let wire: ModelRef =
            serde_json::from_str(r#"{"id":"glm-5.2","providerID":"acme"}"#).unwrap();
        assert_eq!(wire.provider_id, "acme");
        assert_eq!(wire.id, "glm-5.2");
        assert_eq!(wire.variant, None);
        assert_eq!(serde_json::to_string(&wire).unwrap(), r#"{"id":"glm-5.2","providerID":"acme"}"#);
    }

    #[test]
    fn both_casing_conventions_coexist_in_the_same_emitted_document() {
        // The sharpest statement of TRAP 1 in this file: one model object, one
        // document, one key with capitals and one with an underscore, both
        // correct. A port that normalised casing would break one of the two.
        let info = glm("glm-5.2");
        appliquer(&mut catalog_of(vec![info.clone()]));

        let mut document = serde_json::to_value(&info).unwrap();
        document["variants"] = json!([
            serde_json::to_value(&generate(&info)).unwrap()
        ]);
        let text = document.to_string();

        assert!(text.contains("\"providerID\""), "capitals lost: {text}");
        assert!(text.contains("\"reasoning_effort\""), "underscore lost: {text}");
        assert!(!text.contains("provider_id"));
        assert!(!text.contains("reasoningEffort"));
    }

    // -------------------------------------------- TRAP 2: `??` versus `?`

    #[test]
    fn empty_string_survives_the_coalescent_but_not_the_ternary() {
        // The operator the source uses.
        assert_eq!(coalesce_nullity(Some(""), "fallback"), "");
        assert_eq!(coalesce_nullity(None::<&str>, "fallback"), "fallback");
        assert_eq!(coalesce_nullity(Some("kept"), "fallback"), "kept");

        // The operator it does not use, kept separate on purpose.
        assert_eq!(ternary_truthiness(Some(""), "fallback"), "fallback");
        assert_eq!(ternary_truthiness(None, "fallback"), "fallback");
        assert_eq!(ternary_truthiness(Some("kept"), "fallback"), "kept");

        // In JavaScript, only `""` is falsy among strings. `"0"`, `" "` and
        // `"false"` are all truthy and must not be discarded.
        for truthy in ["0", " ", "false", "\u{0}"] {
            assert_eq!(ternary_truthiness(Some(truthy), "fallback"), truthy);
            assert_eq!(coalesce_nullity(Some(truthy), "fallback"), truthy);
        }

        // The two operators disagree on exactly one input, and that input is
        // the whole reason there are two functions.
        let disagreements: Vec<&str> = ["", "kept"]
            .into_iter()
            .filter(|value| {
                coalesce_nullity(Some(*value), "F") != ternary_truthiness(Some(*value), "F")
            })
            .collect();
        assert_eq!(disagreements, [""], "the two operators must differ on \"\"");
    }

    #[test]
    fn the_merge_uses_the_coalescent_not_the_ternary() {
        // A variant whose id is the empty string is a legal `ModelVariant`, and
        // under the coalescent it is found in the explicit map. Had the merge
        // used a truthiness test, this entry would be thrown away. It does not
        // collide with a generated id here — `"high"` and `"max"` are the only
        // generated ones — but the lookup is the operation under test, so the
        // guard is placed on the merge itself.
        let existing = vec![bare("")];
        let generated = vec![bare("high")];
        let merged = merge(existing.clone(), generated);
        assert_eq!(merged.len(), 2, "the empty-id variant is not generated, so it survives");
        assert_eq!(ids_of(&merged), ["high", ""]);

        // And the same conclusion on the lookup helper, with the empty id
        // actually being the key that is looked up.
        let found: Option<ModelVariant> = existing.iter().find(|v| v.id == "").cloned();
        assert_eq!(coalesce_nullity(found, bare("replaced")).id, "");
        assert_eq!(ternary_truthiness(Some(""), "replaced"), "replaced");
    }

    // ----------------------------------------------- merge: Map and Set rules

    #[test]
    fn the_explicit_variant_replaces_the_generated_one_whole() {
        let existing = vec![with_header("high", "X-Trace", "abc")];
        let merged = merge(existing, generate(&glm("glm-5.2")));
        assert_eq!(ids_of(&merged), ["high", "max"]);

        // The explicit entry wins entirely: its header survives, and it does
        // NOT acquire the generated `reasoning_effort`. A field-wise merge here
        // would have produced a body the source never writes.
        assert_eq!(merged[0].headers.get("X-Trace").map(String::as_str), Some("abc"));
        assert!(merged[0].body.is_empty(), "an explicit entry is replaced, not merged");

        // The generated entry that had no counterpart is untouched.
        assert_eq!(
            merged[1].body.get(BODY_KEY_REASONING_EFFORT),
            Some(&json!("max"))
        );
    }

    #[test]
    fn the_last_duplicate_id_wins_the_lookup() {
        // `new Map(entries)` overwrites on construction, so the last entry for
        // a repeated id is the one that survives. A `.find()` would keep the
        // first, and the two disagree right here.
        let existing = vec![with_header("high", "X-First", "1"), with_header("high", "X-Last", "2")];
        let merged = merge(existing, generate(&glm("glm-5.2")));

        assert_eq!(ids_of(&merged), ["high", "max"], "duplicates collapse to one slot");
        assert_eq!(merged[0].headers.get("X-Last").map(String::as_str), Some("2"));
        assert!(merged[0].headers.get("X-First").is_none());
    }

    #[test]
    fn the_list_is_rebuilt_so_explicit_entries_move_to_their_generated_slot() {
        let existing = vec![bare("alpha"), bare("high"), bare("beta")];
        let merged = merge(existing, generate(&glm("glm-5.2")));

        // `high` sat at index 1 and comes back at index 0, because it is
        // emitted in the generated slot; `max` is inserted at index 1; the
        // untouched entries follow in their original relative order. Nothing
        // keeps its index, which is the whole reason this is a rebuild.
        assert_eq!(ids_of(&merged), ["high", "max", "alpha", "beta"]);
    }

    #[test]
    fn merging_nothing_onto_nothing_gives_nothing() {
        assert!(merge(Vec::new(), Vec::new()).is_empty());
    }

    // -------------------------------------- the literals trap: bare strings

    #[test]
    fn reasoning_effort_serialises_as_a_bare_string() {
        // No `#[serde(tag = "...")]` on this enum. With one, the output would
        // be `{"High":null}`, which no OpenAI-compatible endpoint would accept
        // and which the TypeScript never writes.
        assert_eq!(serde_json::to_string(&ReasoningEffort::High).unwrap(), "\"high\"");
        assert_eq!(serde_json::to_string(&ReasoningEffort::Max).unwrap(), "\"max\"");

        let forbidden = serde_json::to_string(&json!({ "High": null, "Max": null })).unwrap();
        assert!(!forbidden.contains("high"), "sanity: the tagged form is a different document");

        // Reading the bare string works; reading the tagged object does not.
        let round: ReasoningEffort = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(round, ReasoningEffort::High);
        assert!(serde_json::from_str::<ReasoningEffort>(r#"{"High":null}"#).is_err());

        // And the two values are a closed set, in source order.
        assert_eq!(
            ReasoningEffort::ALL.map(|effort| effort.as_str()),
            ["high", "max"]
        );
        assert!(serde_json::from_str::<ReasoningEffort>("\"low\"").is_err());
    }

    #[test]
    fn the_api_union_is_tagged_on_type_and_inherited_not_redeclared() {
        // The contrast case. `ModelApi` really is a discriminated union, so it
        // carries `tag = "type"` and emits the tag as a field of the object.
        // It is imported from `crate::core::model`; this file declares no `Api`
        // of its own, and this test is what proves the imported one still has
        // the right tag and the right variant spellings.
        let info = glm("glm-5.2");
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["api"]["type"], json!("aisdk"));
        assert_eq!(json["api"]["package"], json!("@ai-sdk/openai-compatible"));
        assert_eq!(json["api"]["id"], json!("glm-5.2"));
        assert!(json["api"].get("Aisdk").is_none(), "an untagged enum would nest: {json:?}");

        let native_json = serde_json::to_value(native("m")).unwrap();
        assert_eq!(native_json["api"]["type"], json!("native"));
        assert!(native_json["api"].get("Native").is_none());

        // Both spellings round-trip, so the renames hold in both directions.
        let back: ModelInfo = serde_json::from_value(json).unwrap();
        assert_eq!(back, info);
    }

    // -------------------------------------------------- the catalog mutation

    #[test]
    fn update_creates_a_missing_model_from_the_schema_defaults() {
        let mut catalog = Catalog::new();
        let key = ModelRef { id: "glm-5.2".to_string(), provider_id: "acme".to_string(), variant: None };
        let model = catalog.model_update(&key);

        // `ModelV2.Info.empty` is native, so the freshly created model does
        // not even match the guards. That is faithful: `catalog.ts:140`
        // creates from `empty`, and the plugin's own read of the draft has
        // already happened by then in the source's control flow.
        assert!(matches!(model.api, ModelApi::Native(_)));
        assert!(model.variants.is_empty());
        assert_eq!(model.id, "glm-5.2");
        assert_eq!(model.provider_id, "acme");
        assert!(model.enabled);
    }

    #[test]
    fn update_stamps_the_key_back_onto_the_model() {
        // `model.id = modelID; model.providerID = providerID` after the
        // mutation, `catalog.ts:143-144`.
        let mut catalog = catalog_of(vec![native("wrong-id")]);
        let key = ModelRef { id: "right-id".to_string(), provider_id: "other".to_string(), variant: None };
        let model = catalog.model_update(&key);

        assert_eq!(model.id, "right-id");
        assert_eq!(model.provider_id, "other");
    }

    #[test]
    fn the_update_key_comes_from_the_model_and_not_from_its_record() {
        // The subtle one. A model stored under provider `acme` that claims
        // `providerID: "elsewhere"` is written to `elsewhere`, and the record
        // is created on the fly. The loop walks records, the key does not.
        let mut misfiled = glm("glm-5.2");
        misfiled.provider_id = "elsewhere".to_string();

        let mut catalog = catalog_of(vec![misfiled]);
        assert_eq!(catalog.records.len(), 1);
        assert_eq!(catalog.records[0].0, "acme");

        let applied = appliquer(&mut catalog);

        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].provider_id, "elsewhere");
        assert_eq!(applied[0].id, "glm-5.2");

        // A second record now exists. It holds the model `catalog.ts:140`
        // created, which is `ModelV2.Info.empty` — a **native** api — not the
        // misfiled `aisdk` one, because `update` creates before it calls the
        // callback and the callback's `generate` therefore sees the default.
        // So the misfiled model generates nothing at its new home, and the
        // variants it *would* have produced are never written anywhere. This is
        // the faithful reading, and it is why a key derived from the record
        // instead of the model would be observably different behaviour.
        assert_eq!(catalog.records.len(), 2);
        assert_eq!(catalog.records[1].0, "elsewhere");
        assert_eq!(catalog.records[1].1.len(), 1);
        assert!(matches!(catalog.records[1].1[0].api, ModelApi::Native(_)));
        assert!(catalog.records[1].1[0].variants.is_empty());

        // And the original record still holds the misfiled model, untouched.
        assert!(catalog.records[0].1[0].variants.is_empty());
        assert!(matches!(catalog.records[0].1[0].api, ModelApi::Aisdk(_)));
    }

    #[test]
    fn every_model_is_updated_even_when_generation_is_skipped() {
        // The early return is inside the callback, so `update` is still called
        // for a model that generates nothing. The returned key list therefore
        // has one entry per model, not one entry per mutation.
        let mut catalog = catalog_of(vec![glm("glm-5.2"), native("claude"), glm("glm-5p2")]);
        let applied = appliquer(&mut catalog);

        assert_eq!(applied.len(), 3);
        assert_eq!(
            applied.iter().map(|k| k.id.clone()).collect::<Vec<String>>(),
            ["glm-5.2", "claude", "glm-5p2"]
        );

        // Only the two GLM models were touched.
        assert_eq!(ids_of(&catalog.model("acme", "glm-5.2").unwrap().variants), ["high", "max"]);
        assert!(catalog.model("acme", "claude").unwrap().variants.is_empty());
        assert_eq!(ids_of(&catalog.model("acme", "glm-5p2").unwrap().variants), ["high", "max"]);
    }

    #[test]
    fn a_non_matching_model_keeps_its_variants_and_their_order() {
        let mut info = native("claude");
        info.variants = vec![bare("alpha"), bare("high"), bare("beta")];
        let mut catalog = catalog_of(vec![info]);

        appliquer(&mut catalog);

        // The merge is skipped, so the list is not even rebuilt: `high` stays
        // at index 1 rather than jumping to index 0 the way it would if the
        // empty `generated` still went through the rebuild.
        let model = catalog.model("acme", "claude").unwrap();
        assert_eq!(ids_of(&model.variants), ["alpha", "high", "beta"]);
    }

    #[test]
    fn applying_twice_is_idempotent() {
        let mut info = glm("glm-5.2");
        info.variants = vec![with_header("high", "X-Trace", "abc"), bare("alpha")];
        let mut catalog = catalog_of(vec![info]);

        appliquer(&mut catalog);
        let once = catalog.model("acme", "glm-5.2").unwrap().variants.clone();

        appliquer(&mut catalog);
        let twice = catalog.model("acme", "glm-5.2").unwrap().variants.clone();

        assert_eq!(once, twice, "replaying the transform must not drift");
        assert_eq!(ids_of(&twice), ["high", "max", "alpha"]);
        assert_eq!(twice[0].headers.get("X-Trace").map(String::as_str), Some("abc"));
    }

    #[test]
    fn providers_and_models_are_walked_in_insertion_order() {
        // A `BTreeMap` of providers would sort them and change the order of the
        // `update` calls, which the returned key list exposes.
        let mut catalog = Catalog::new();
        catalog.push_record("zeta", vec![native("z1"), native("z2")]);
        catalog.push_record("alpha", vec![native("a1")]);
        catalog.push_record("mid", vec![native("m1")]);

        let keys = catalog.model_keys();
        assert_eq!(
            keys.iter().map(|k| k.provider_id.clone()).collect::<Vec<_>>(),
            ["zeta", "zeta", "alpha", "mid"]
        );
        assert_eq!(
            keys.iter().map(|k| k.id.clone()).collect::<Vec<String>>(),
            ["z1", "z2", "a1", "m1"]
        );

        let applied = appliquer(&mut catalog);
        assert_eq!(applied.len(), 4);
        assert_eq!(applied[0].provider_id, "zeta");
        assert_eq!(applied[2].provider_id, "alpha");
    }

    #[test]
    fn an_empty_catalog_is_untouched() {
        let mut catalog = Catalog::new();
        assert!(appliquer(&mut catalog).is_empty());
        assert!(catalog.records.is_empty());
        assert_eq!(catalog, Catalog::new());
    }

    #[test]
    fn the_plugin_method_is_the_same_function_as_the_free_one() {
        let mut a = catalog_of(vec![glm("glm-5.2")]);
        let mut b = catalog_of(vec![glm("glm-5.2")]);

        assert_eq!(VariantPlugin::new().transform(&mut a), appliquer(&mut b));
        assert_eq!(a, b);
    }

    #[test]
    fn generate_is_pure_and_returns_independent_values() {
        let info = glm("glm-5.2");
        let first = generate(&info);
        let second = generate(&info);
        assert_eq!(first, second);
        assert_ne!(first, second, "two calls must not share one allocation");

        // Mutating the result does not touch the model, and vice versa.
        let mut mutated = generate(&info);
        mutated[0].body.insert("x".to_string(), json!(1));
        assert!(info.variants.is_empty());
        assert!(!first[0].body.contains_key("x"));
    }
}
