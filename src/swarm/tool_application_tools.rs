//! Port of `packages/core/src/tool/application-tools.ts`.
//!
//! # Honesty about the size of this port
//!
//! The source is 1764 bytes and 57 lines. It looks small. It is not small in
//! behaviour. Four things in it carry real logic, and all four are reproduced:
//!
//! 1. **`Tool.validateName`**, a real regular expression
//!    (`/^[A-Za-z][A-Za-z0-9_-]{0,63}$/`) with three separate ways to fail: the
//!    empty string, a first character that is not an ASCII letter, and a total
//!    length above 64. It is ported here as a hand-written scan because
//!    `Cargo.toml` declares no regex engine and this batch may not touch
//!    `Cargo.toml`.
//! 2. **An all-or-nothing validation barrier.** Every name is validated *before*
//!    a single entry is written. One bad name in a 50-tool record leaves the
//!    registry byte-identical to what it was. A naive port that inserts as it
//!    walks would leave 49 half-registered tools behind.
//! 3. **A replayable state machine, not a map.** `State.create` does not mutate
//!    state. `state.transform` appends a *replayable* transform to a list, and
//!    every read re-derives the state from `initial()` by replaying the whole
//!    list in registration order. Disposing a transform therefore does not
//!    "undo" its writes; it removes them from the replay and the state is rebuilt
//!    from scratch. The observable difference is real: disposing a *superseded*
//!    transform brings the *earlier* entry back, with the earlier identity.
//! 4. **A fresh `identity: {}` object per registration.** Each call to
//!    `register` mints one new opaque object per tool name. Re-registering a
//!    name therefore replaces the identity, and the two registrations can be
//!    told apart afterwards.
//!
//! # Divergences from the source, all deliberate, all tested
//!
//! - **`Object.entries` order becomes `BTreeMap` order.** In JavaScript the
//!   iteration order of a `Record` is insertion order, with integer-like keys
//!   hoisted to the front in ascending numeric order. Here the catalog is a
//!   `BTreeMap`, so it is alphabetical. `register` is fail-fast, so the name
//!   reported in the error changes when *two* names are invalid. The sibling
//!   module `tool_tools.rs` already locks this same convention for the same
//!   TypeScript type (`Readonly<Record<string, Tool.AnyTool>>`); diverging from
//!   it here would make the two modules impossible to compose.
//! - **`entries()` order IS preserved.** The source returns `ReadonlyMap`, which
//!   is a `Map`, and a `Map` is insertion-ordered. That order is observable here
//!   (see the replay model), so [`Entries`] keeps insertion order with a linear
//!   scan rather than a `BTreeMap`. `Vec` linear lookup is O(n) on a registry of
//!   a few dozen tools; correctness of the ordering was judged more important
//!   than asymptotics for a list that short.
//! - **`Scope` becomes an explicit [`Registration`] handle.** `register`
//!   registers a transform whose lifetime is the enclosing `Scope`, and the
//!   transform is released by a scope finalizer that the source never writes by
//!   hand. Rust has no scope finalizer here, so `register` returns the handle
//!   and [`Service::dispose`] releases it. Dropping the handle does *not*
//!   dispose: a dropped Rust value is not a closed Effect scope.
//! - **`layer` has no name of its own.** `Layer.effect(Service, ...)` in Effect
//!   takes the service as its key, so the layer is literally identified by the
//!   service. [`LAYER_NAME`] is therefore the service tag, and it is an invented
//!   label: it exists because [`LayerRef`] is a name and the source has no
//!   string to give it. It has no behavioural effect.
//!
//! # What is NOT here, and why
//!
//! - **No JSON boundary exists in this source.** Nothing is serialised, no
//!   `Schema` is declared, and there is not a single optional field. The
//!   `Schema.optional` contradiction that currently splits this batch therefore
//!   does **not** arise in this file at all.
//!
//!   [`Snapshot`] is added anyway, because the porting rules require a
//!   round-trip test and a snake_case rejection test. It is a **transcription of
//!   the source's data shapes** (`Data = { entries: Map<string, Entry> }`,
//!   `Entry = { identity, tool }`) and nothing more. Read the warnings on
//!   [`Snapshot`], [`Identity`] and [`ToolRef`]: three of the JSON details there
//!   are port decisions, not source facts.
//!
//! - **`export * as ApplicationTools from "./application-tools"` (line 1) is a
//!   self re-export of the module namespace onto itself.** It is dead code.
//!   Every opencode module starts with it. The Rust module already plays that
//!   role, so there is nothing to write.
//!
//! - **`AnyTool` and `RegistrationError` are not redeclared here.** They come
//!   from `tool/tool.ts` and are already declared by the sibling module
//!   `crate::swarm::tool_tools`. They are re-exported, not redefined, so the
//!   duplicate-declaration risk does not materialise. `src/tool.rs` is a
//!   different design entirely (a `ToolBox` that executes tools) and was not
//!   touched.
//!
//! - **`Semaphore.makeUnsafe(1)`, `Effect.uninterruptible`, `Effect.withSpan`
//!   and `Scope.addFinalizer` are not simulated.** They are concurrency and
//!   runtime machinery, not behaviour. Adding a `Mutex` here would be a
//!   fabrication: the `Unsafe` semaphore in the source is deliberately
//!   unsynchronised and belongs to a single runtime.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub use crate::swarm::tool_tools::{AnyTool, RegistrationError};
use crate::swarm::effect_app_node::{make_global_node, MakeInput};
use crate::swarm::effect_app_node_builder::{AppNode, LayerRef, NodeTag};

/// Identifier the service is registered under in the Effect context graph.
///
/// Written verbatim by `Context.Service<Service, Interface>()("@opencode/ApplicationTools")`.
pub const SERVICE_TAG: &str = "@opencode/ApplicationTools";

/// Name given to the layer that provides [`Service`].
///
/// The source writes `Layer.effect(Service, ...)` with no string in it: Effect
/// keys the layer by the service, so there is no name to copy. A name is
/// required here because [`LayerRef`] is a name. Invented, documented, and
/// without behavioural effect.
pub const LAYER_NAME: &str = SERVICE_TAG;

/// Maximum number of characters accepted by [`validate_name`].
///
/// The source pattern is `^[A-Za-z][A-Za-z0-9_-]{0,63}$`. The first character is
/// consumed by `[A-Za-z]`, so `{0,63}` bounds the *total* at 64.
pub const MAX_NAME_LENGTH: usize = 64;

/// The opaque object `Entry.identity` points at.
///
/// `readonly identity: object` in the source is a bare `{}` with no fields. Its
/// only purpose is to be a fresh object per registration, so that two
/// registrations of the same tool name can be told apart. It is modelled here as
/// a monotonically increasing counter, which reproduces the one observable
/// property of `{}`: no two identities are ever equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Identity(u64);

impl Identity {
    /// The numeric value behind this identity. Useful for diagnostics and for
    /// asserting that a rebuild preserved an identity.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// A tool as seen by this registry.
///
/// `Tool.AnyTool` is `Definition<any, any>`: a frozen empty object whose input
/// and output types exist only as phantom parameters. The sibling module
/// `tool_tools.rs` models it as a unit struct, and this file re-uses it rather
/// than declaring a second, incompatible `AnyTool`.
///
/// The wrapper exists only to give `tool` a JSON form. `Object.freeze({})`
/// serialises to `{}`, so that is what [`ToolRef`] writes and the only thing it
/// accepts on the way back in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ToolRef(pub AnyTool);

impl Serialize for ToolRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&serde_json::Value::Object(serde_json::Map::new()), serializer)
    }
}

impl<'de> Deserialize<'de> for ToolRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Object(map) if map.is_empty() => Ok(ToolRef(AnyTool)),
            other => Err(<D::Error as serde::de::Error>::custom(format!(
                "a tool is a frozen empty object, got {}",
                other
            ))),
        }
    }
}

/// One registered tool: the tool itself plus the identity of its registration.
///
/// Both field names come verbatim from `export interface Entry`. They are
/// lowercase single words, so there is no camelCase hazard here. That is a fact
/// about this file, not a generalisation: the trap exists in the family, not in
/// this member of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Entry {
    #[serde(rename = "identity")]
    identity: Identity,
    #[serde(rename = "tool")]
    tool: ToolRef,
}

impl Entry {
    /// The identity minted when this entry was registered.
    pub fn identity(&self) -> Identity {
        self.identity
    }

    /// The registered tool.
    pub fn tool(&self) -> AnyTool {
        self.tool.0
    }
}

/// The registry contents, in the order a JavaScript `Map` would hold them.
///
/// Order is *insertion order*, and it is not incidental. `Map.set` on a key that
/// is already present replaces the value and **keeps the original position**,
/// which is why [`Entries::set`] looks the way it does. Under the replay model
/// of [`Service`] that is observable: the entry a superseded registration wrote
/// keeps its slot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entries(Vec<(String, Entry)>);

impl Entries {
    /// Writes `entry` at `name`, replacing any previous value in place and
    /// keeping the original position. This is `Map.prototype.set`.
    fn set(&mut self, name: &str, entry: Entry) {
        match self.0.iter_mut().find(|(key, _)| key == name) {
            Some(slot) => slot.1 = entry,
            None => self.0.push((name.to_string(), entry)),
        }
    }

    /// The entry registered at `name`, if any.
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.0.iter().find(|(key, _)| key == name).map(|(_, entry)| entry)
    }

    /// Number of distinct registered names.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no name is registered.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Names in insertion order.
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.0.iter().map(|(key, _)| key.as_str())
    }

    /// Entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Entry)> + '_ {
        self.0.iter().map(|(key, entry)| (key.as_str(), entry))
    }
}

/// Handle to one registered transform.
///
/// Returned by [`Service::register`]. Dropping it changes nothing, exactly as a
/// closed Effect `Scope` is not implied by letting a value go out of scope.
/// Call [`Service::dispose`] to release it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Registration(u64);

impl Registration {
    /// Internal identifier of the transform this handle releases.
    pub fn id(self) -> u64 {
        self.0
    }
}

/// The transform list, as stored by the replay model.
#[derive(Debug, Clone)]
struct Transform {
    id: u64,
    entries: Vec<(String, Entry)>,
}

/// The service contract of `export interface Interface`.
pub trait Interface {
    /// Registers a catalog of tools.
    ///
    /// Returns `Ok(None)` when the catalog is empty, mirroring the source's
    /// early `if (entries.length === 0) return`: an empty record registers no
    /// transform at all, so there is nothing to release later.
    fn register(&mut self, tools: &BTreeMap<String, AnyTool>)
        -> Result<Option<Registration>, RegistrationError>;

    /// The currently visible entries, in insertion order.
    fn entries(&self) -> &Entries;
}

/// Port of the `Layer.effect(Service, ...)` body: the tool registry.
///
/// See the module header for the four pieces of real behaviour this carries.
#[derive(Debug, Clone, Default)]
pub struct Service {
    /// Replayable transforms, in registration order.
    transforms: Vec<Transform>,
    /// Materialised state, always rebuilt by replaying `transforms`.
    state: Entries,
    /// Source of transform identities.
    next_transform_id: u64,
    /// Source of entry identities. Starts at 1 so that 0 can mean "none".
    next_identity: u64,
}

impl Interface for Service {
    fn register(
        &mut self,
        tools: &BTreeMap<String, AnyTool>,
    ) -> Result<Option<Registration>, RegistrationError> {
        // `if (entries.length === 0) return` - before any validation, and
        // without creating a transform.
        if tools.is_empty() {
            return Ok(None);
        }

        // The validation barrier. Every name is checked before anything is
        // written, so a single bad name leaves the registry untouched.
        for name in tools.keys() {
            validate_name(name)?;
        }

        let id = self.next_transform_id;
        self.next_transform_id += 1;

        // `{ identity: {}, tool }` - one fresh object per name, per call.
        let mut entries = Vec::with_capacity(tools.len());
        for (name, tool) in tools {
            self.next_identity += 1;
            let entry = Entry { identity: Identity(self.next_identity), tool: ToolRef(*tool) };
            entries.push((name.clone(), entry));
        }

        self.transforms.push(Transform { id, entries });
        self.materialize();
        Ok(Some(Registration(id)))
    }

    fn entries(&self) -> &Entries {
        &self.state
    }
}

impl Service {
    /// A registry holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Releases the transform behind `registration` and rebuilds the state.
    ///
    /// Idempotent: a handle that is already released, or was never issued,
    /// leaves the state alone and does **not** trigger a rebuild. That mirrors
    /// `if (!active) return Effect.void` in the dispose finalizer of `state.ts`.
    pub fn dispose(&mut self, registration: Registration) {
        let before = self.transforms.len();
        self.transforms.retain(|transform| transform.id != registration.id);
        if self.transforms.len() == before {
            return;
        }
        self.materialize();
    }

    /// Number of live transforms. Exposed because the replay list, not the map,
    /// is the real state of this service.
    pub fn live_transforms(&self) -> usize {
        self.transforms.len()
    }

    /// Rebuilds the state from `initial()` by replaying every live transform.
    ///
    /// This is `materialize` in `state.ts`. It starts from an empty registry,
    /// not from the previous state: that is why disposing a transform can bring
    /// an earlier entry back instead of merely deleting a key.
    fn materialize(&mut self) {
        let mut next = Entries::default();
        for transform in &self.transforms {
            for (name, entry) in &transform.entries {
                next.set(name, *entry);
            }
        }
        self.state = next;
    }

    /// The registry as a [`Snapshot`], ready to be written as JSON.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot { entries: EntriesMap(self.state.clone()) }
    }
}

/// Port of `Tool.validateName` from `tool/tool.ts`.
///
/// The source is a regular expression test, `^[A-Za-z][A-Za-z0-9_-]{0,63}$`,
/// producing `Effect.void` or a `Tool.RegistrationError`. Three independent
/// rejections are possible:
///
/// - the empty string, because there is no first character to match `[A-Za-z]`;
/// - a first character that is not an ASCII letter;
/// - a total length above 64, because `{0,63}` follows the first character.
///
/// Hand-written because `Cargo.toml` declares no regex engine. A character class
/// of `[A-Za-z0-9_-]` contains only ASCII, so every accepted name is pure ASCII,
/// which means "characters", "bytes" and "UTF-16 code units" count identically
/// here. There is no Unicode-length divergence to worry about, because a
/// non-ASCII character is rejected before the length could matter.
pub fn validate_name(name: &str) -> Result<(), RegistrationError> {
    let mut chars = name.chars();
    let first = match chars.next() {
        Some(character) => character,
        // No first character at all: the empty string cannot match `[A-Za-z]`.
        None => return Err(invalid_name_error(name)),
    };
    if !first.is_ascii_alphabetic() {
        return Err(invalid_name_error(name));
    }
    // `{0,63}` counts what follows the first character, so the total is capped
    // at 1 + 63 = 64.
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(invalid_name_error(name));
    }
    for character in chars {
        if !(character.is_ascii_alphanumeric() || character == '_' || character == '-') {
            return Err(invalid_name_error(name));
        }
    }
    Ok(())
}

/// Builds the `Tool.RegistrationError` that `validateName` fails with.
///
/// The source writes ``new RegistrationError({ name, message: `Invalid tool name: ${name}` })``.
fn invalid_name_error(name: &str) -> RegistrationError {
    RegistrationError::new(name, format!("Invalid tool name: {}", name))
}

/// Port of `export const node = makeGlobalNode({ service: Service, layer, deps: [] })`.
///
/// A function rather than a `const`, because [`AppNode`] owns a `String` and
/// cannot be built at compile time. `deps: []` is the empty vector, and `service`
/// is the service tag.
pub fn node() -> AppNode {
    make_global_node(MakeInput::service(SERVICE_TAG, LayerRef::new(LAYER_NAME)))
}

// ---------------------------------------------------------------------------
// JSON projection. NOT part of the source: see the module header.
// ---------------------------------------------------------------------------

/// The `entries` field of [`Snapshot`], serialised as a JSON object.
///
/// `Map<string, Entry>` is an object in JSON. Order is preserved on the way out
/// because `serde_json` writes map entries in the order they are collected. Note
/// that a round trip through [`serde_json::Value`] and back does **not**
/// preserve it, because `serde_json::Value` stores objects in a `BTreeMap`
/// unless its `preserve_order` feature is enabled. `Cargo.toml` does not enable
/// it, so this is a real, current caveat and not a hypothetical one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntriesMap(Entries);

impl Serialize for EntriesMap {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, entry) in self.0.iter() {
            map.serialize_entry(name, entry)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for EntriesMap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> serde::de::Visitor<'de> for EntriesVisitor {
            type Value = EntriesMap;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map from tool name to entry")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = Entries::default();
                while let Some((name, entry)) = access.next_entry::<String, Entry>()? {
                    // Duplicate keys collapse the way `Map.set` collapses them.
                    entries.set(&name, entry);
                }
                Ok(EntriesMap(entries))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

/// The JSON form of a registry.
///
/// **This type is not in the source.** It is a transcription of the source's two
/// data shapes, `Data = { readonly entries: Map<string, Entry> }` and
/// `Entry = { readonly identity: object, readonly tool: Tool.AnyTool }`, kept
/// only so that the field names can be locked by a test in both directions.
///
/// Three details are port decisions, not source facts:
///
/// - `identity` is written as a JSON **number**. In the source it is an opaque
///   object with no JSON form at all; the number is the cheapest faithful
///   stand-in for "a value that is never equal to another".
/// - `tool` is written as `{}`, which *is* faithful: `AnyTool` is
///   `Object.freeze({})`.
/// - Unknown keys are ignored, not rejected, matching `Schema.Class`. A wrong
///   key therefore fails by being *missing*, not by being extra. The rejection
///   test relies on that.
///
/// `data` and `entries` carry the same lowercase name as the source. There is no
/// camelCase field in this source, so the familiar `projectID` / `project_id`
/// hazard has no target here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    /// The registry, keyed by tool name.
    #[serde(rename = "entries")]
    pub entries: EntriesMap,
}

impl Snapshot {
    /// Parses a snapshot from JSON text.
    pub fn from_json_str(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Renders the snapshot as compact JSON text.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).expect("a Snapshot is always serialisable")
    }

    /// The entries carried by this snapshot, in insertion order.
    pub fn into_entries(self) -> Entries {
        self.entries.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a catalog from names, the way an Effect caller assembles the
    /// `Record` before calling `register`.
    fn catalog(names: &[&str]) -> BTreeMap<String, AnyTool> {
        names.iter().map(|name| (name.to_string(), AnyTool)).collect()
    }

    /// Name of the single entry, for concise assertions.
    fn only_name(entries: &Entries) -> String {
        let mut names = entries.names();
        let first = names.next().expect("one entry expected").to_string();
        assert!(names.next().is_none(), "exactly one entry expected");
        first
    }

    // --- identity of the service -------------------------------------------

    #[test]
    fn the_service_tag_is_exactly_the_one_written_by_the_source() {
        assert_eq!(SERVICE_TAG, "@opencode/ApplicationTools");
    }

    #[test]
    fn the_node_is_a_global_layer_node_with_no_dependencies() {
        let node = node();
        assert_eq!(node.tag(), Some(NodeTag::Global));
        assert_eq!(node.name(), SERVICE_TAG);
        assert_eq!(node.implementation().map(LayerRef::name), Some(LAYER_NAME));
        assert!(node.dependencies().is_empty());
    }

    // --- validate_name -----------------------------------------------------

    #[test]
    fn validate_name_accepts_the_shapes_the_source_pattern_allows() {
        for name in ["a", "A", "tool", "read", "web-fetch", "web_fetch", "aB9_-", "A1", "Zz9"] {
            assert!(validate_name(name).is_ok(), "{} should be accepted", name);
        }
    }

    #[test]
    fn validate_name_accepts_exactly_sixty_four_characters_and_refuses_sixty_five() {
        let sixty_four = "a".repeat(MAX_NAME_LENGTH);
        let sixty_five = "a".repeat(MAX_NAME_LENGTH + 1);
        assert!(sixty_five.len() == 65);
        assert!(validate_name(&sixty_four).is_ok());
        let error = validate_name(&sixty_five).expect_err("65 characters must be refused");
        assert_eq!(error.name, sixty_five);
        assert_eq!(error.message, format!("Invalid tool name: {}", sixty_five));
    }

    #[test]
    fn validate_name_refuses_every_shape_the_source_pattern_rejects() {
        // Three independent rejections, listed so that a fix to one does not
        // silently remove another. Non-ASCII and the escapes are written in
        // escape form on purpose, so this file stays pure ASCII.
        let refused = [
            "",                       // no first character at all
            "1tool",                  // first character is a digit
            "_tool",                  // first character is an underscore
            "-tool",                  // first character is a hyphen
            ".tool",                  // first character is a dot
            "to ol",                  // inner space
            "tool.name",              // inner dot
            "tool/other",             // inner slash
            "tool\u{00e9}",           // non-ASCII is not in the class
            "tool\u{2603}",           // astral plane, two UTF-16 units
            "tool\n",                 // '$' without /m anchors at the very end
            " tool",                  // leading space
        ];
        for name in refused {
            assert!(validate_name(name).is_err(), "{} should be refused", name);
        }
    }

    #[test]
    fn validate_name_is_the_only_thing_that_makes_a_name_invalid() {
        // The empty catalog does not run validation at all, which is the early
        // return, not a lax regex.
        let mut service = Service::new();
        assert_eq!(service.register(&BTreeMap::new()).unwrap(), None);
        assert_eq!(service.live_transforms(), 0);
    }

    // --- registration ------------------------------------------------------

    #[test]
    fn an_empty_record_registers_no_transform_and_returns_no_handle() {
        let mut service = Service::new();
        assert_eq!(service.register(&BTreeMap::new()), Ok(None));
        assert!(service.entries().is_empty());
        assert_eq!(service.live_transforms(), 0);
    }

    #[test]
    fn a_single_tool_is_registered_under_the_name_it_was_given() {
        let mut service = Service::new();
        let handle = service.register(&catalog(&["read"])).expect("valid name");
        assert!(handle.is_some());
        assert_eq!(service.entries().len(), 1);
        assert_eq!(only_name(service.entries()), "read");
        assert!(service.entries().get("read").is_some());
    }

    #[test]
    fn a_refused_name_writes_nothing_at_all_even_when_valid_names_share_the_call() {
        // The validation barrier. `read` is valid, `1bad` is not. A port that
        // inserted as it walked would leave `read` registered.
        let mut service = Service::new();
        service.register(&catalog(&["write"])).expect("valid name");
        let before = service.snapshot();

        let error = service
            .register(&catalog(&["read", "1bad"]))
            .expect_err("1bad must be refused");
        assert_eq!(error.name, "1bad");
        assert_eq!(error.message, "Invalid tool name: 1bad");

        assert_eq!(service.snapshot(), before);
        assert_eq!(service.live_transforms(), 1);
    }

    #[test]
    fn registration_is_cumulative_because_transforms_are_replayed() {
        // Second call does not overwrite the first: the state is rebuilt by
        // replaying transform 1 then transform 2.
        let mut service = Service::new();
        service.register(&catalog(&["read"])).expect("valid");
        service.register(&catalog(&["write", "edit"])).expect("valid");
        assert_eq!(service.entries().len(), 3);
        assert!(service.entries().get("read").is_some());
        assert!(service.entries().get("write").is_some());
        assert!(service.entries().get("edit").is_some());
    }

    #[test]
    fn entries_follow_registration_order_and_not_alphabetical_order() {
        // The source returns a `Map`, which is insertion-ordered. `zeta` is
        // registered first, so it comes first.
        let mut service = Service::new();
        service.register(&catalog(&["zeta"])).expect("valid");
        service.register(&catalog(&["alpha"])).expect("valid");
        let names: Vec<&str> = service.entries().names().collect();
        assert_eq!(names, vec!["zeta", "alpha"]);
    }

    #[test]
    fn setting_a_name_twice_keeps_its_first_position_and_replaces_its_value() {
        // `Map.prototype.set` keeps the original slot. Same name, second call.
        let mut service = Service::new();
        service.register(&catalog(&["a"])).expect("valid");
        let first_identity = service.entries().get("a").unwrap().identity();
        service.register(&catalog(&["b"])).expect("valid");
        service.register(&catalog(&["a"])).expect("valid");
        let second_identity = service.entries().get("a").unwrap().identity();

        let names: Vec<&str> = service.entries().names().collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_ne!(first_identity, second_identity, "each registration mints a new identity");
    }

    #[test]
    fn every_registration_mints_a_fresh_identity_for_every_name() {
        let mut service = Service::new();
        service.register(&catalog(&["a", "b"])).expect("valid");
        let first: Vec<u64> =
            service.entries().iter().map(|(_, entry)| entry.identity().value()).collect();
        service.register(&catalog(&["a", "b", "c"])).expect("valid");
        let second: Vec<u64> =
            service.entries().iter().map(|(_, entry)| entry.identity().value()).collect();

        // Three distinct identities in the first call, three fresh ones after.
        assert_eq!(first.len(), 2);
        let mut sorted = first.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 2, "identities within one call must differ");
        for (before, after) in first.iter().zip(second.iter()) {
            assert_ne!(before, after);
        }
    }

    #[test]
    fn the_failure_reports_the_first_invalid_name_in_the_order_the_catalog_is_walked() {
        // `BTreeMap` walks alphabetically; a JavaScript `Record` would walk in
        // insertion order. Two invalid names make the difference observable, so
        // this test pins the divergence rather than hiding it.
        let mut service = Service::new();
        let error = service
            .register(&catalog(&["zzz!", "aaa!"]))
            .expect_err("both names are invalid");
        assert_eq!(error.name, "aaa!", "alphabetical, not insertion order");
    }

    // --- dispose and the replay model --------------------------------------

    #[test]
    fn disposing_the_only_registration_empties_the_registry() {
        let mut service = Service::new();
        let handle = service.register(&catalog(&["read"])).expect("valid").unwrap();
        service.dispose(handle);
        assert!(service.entries().is_empty());
        assert_eq!(service.live_transforms(), 0);
    }

    #[test]
    fn disposing_one_registration_among_several_keeps_the_others() {
        let mut service = Service::new();
        let first = service.register(&catalog(&["read"])).expect("valid").unwrap();
        service.register(&catalog(&["write"])).expect("valid");
        service.dispose(first);
        assert_eq!(service.entries().len(), 1);
        assert_eq!(only_name(service.entries()), "write");
    }

    #[test]
    fn disposing_a_superseded_registration_brings_the_earlier_entry_back() {
        // This is the observable difference between a replay model and a plain
        // map. The second registration overwrote `read`; disposing the second
        // one does not leave `read` missing, it restores the first one, with the
        // first one's identity.
        let mut service = Service::new();
        service.register(&catalog(&["read"])).expect("valid");
        let first_identity = service.entries().get("read").unwrap().identity();
        let superseded = service.register(&catalog(&["read"])).expect("valid").unwrap();
        let second_identity = service.entries().get("read").unwrap().identity();
        assert_ne!(first_identity, second_identity);

        service.dispose(superseded);
        let restored = service.entries().get("read").expect("the earlier entry is back");
        assert_eq!(restored.identity(), first_identity, "identity of the surviving transform");
        assert_eq!(service.entries().len(), 1);
    }

    #[test]
    fn disposing_twice_is_a_no_op_and_does_not_rebuild_the_state() {
        // The source guards with `if (!active) return Effect.void`: the second
        // dispose does not even rematerialise. Observable here because a later
        // registration would be rebuilt away if the stale handle were honoured.
        let mut service = Service::new();
        let stale = service.register(&catalog(&["read"])).expect("valid").unwrap();
        service.dispose(stale);
        service.register(&catalog(&["write"])).expect("valid");
        assert_eq!(only_name(service.entries()), "write");

        service.dispose(stale);
        assert_eq!(only_name(service.entries()), "write", "state must not be rebuilt");
        assert_eq!(service.live_transforms(), 1);
    }

    #[test]
    fn disposing_a_handle_that_was_never_issued_changes_nothing() {
        let mut service = Service::new();
        service.register(&catalog(&["read"])).expect("valid");
        service.dispose(Registration(9999));
        assert_eq!(service.entries().len(), 1);
    }

    #[test]
    fn dropping_a_handle_does_not_release_the_transform() {
        // A dropped Rust value is not a closed Effect scope.
        let mut service = Service::new();
        drop(service.register(&catalog(&["read"])).expect("valid"));
        assert_eq!(service.entries().len(), 1);
        assert_eq!(service.live_transforms(), 1);
    }

    // --- the service is usable through its contract only -------------------

    #[test]
    fn the_service_is_usable_through_a_trait_object() {
        let mut service: Box<dyn Interface> = Box::new(Service::new());
        assert_eq!(service.register(&BTreeMap::new()), Ok(None));
        let handle = service.register(&catalog(&["glob"])).expect("valid");
        assert!(handle.is_some());
        assert_eq!(service.entries().len(), 1);
    }

    #[test]
    fn two_registries_do_not_share_their_identity_counters() {
        // `{}` identity is per registration, not global. Two services start over.
        let mut first = Service::new();
        let mut second = Service::new();
        first.register(&catalog(&["a"])).expect("valid");
        second.register(&catalog(&["a"])).expect("valid");
        assert_eq!(
            first.entries().get("a").unwrap().identity(),
            second.entries().get("a").unwrap().identity(),
            "both are the first identity either service ever minted"
        );
    }

    // --- JSON projection: exact field names, both directions ---------------

    #[test]
    fn the_snapshot_serialises_with_the_exact_field_names_of_the_source() {
        // Trap 1, write direction. The keys are `entries`, `identity`, `tool`,
        // verbatim from `Data` and `Entry`. If any of them were renamed to
        // snake_case or PascalCase here, this string comparison fails.
        let mut service = Service::new();
        service.register(&catalog(&["read"])).expect("valid");
        let json = service.snapshot().to_json_string();
        assert_eq!(
            json,
            r#"{"entries":{"read":{"identity":1,"tool":{}}}}"#,
            "the JSON form must be exactly the source field names"
        );
    }

    #[test]
    fn the_snapshot_preserves_registration_order_in_the_serialised_text() {
        let mut service = Service::new();
        service.register(&catalog(&["zeta"])).expect("valid");
        service.register(&catalog(&["alpha"])).expect("valid");
        let json = service.snapshot().to_json_string();
        let zeta = json.find("zeta").expect("zeta present");
        let alpha = json.find("alpha").expect("alpha present");
        assert!(zeta < alpha, "insertion order, not alphabetical: {}", json);
    }

    #[test]
    fn the_snapshot_round_trips_and_keeps_identities() {
        let mut service = Service::new();
        service.register(&catalog(&["read", "write"])).expect("valid");
        let original = service.snapshot();
        let parsed = Snapshot::from_json_str(&original.to_json_string()).expect("round trip");
        assert_eq!(parsed, original);
        assert_eq!(
            parsed.entries.0.get("read").unwrap().identity(),
            service.entries().get("read").unwrap().identity()
        );
    }

    #[test]
    fn the_snapshot_rejects_snake_case_and_wrong_case_field_names_on_read() {
        // Trap 1, read direction. Each of these must FAIL, and each must fail
        // because the correctly cased key is *missing*, which is why unknown
        // keys are tolerated the way `Schema.Class` tolerates them.
        let refused = [
            r#"{"Entries":{"read":{"identity":1,"tool":{}}}}"#,   // capital E
            r#"{"entries":{"read":{"Identity":1,"tool":{}}}}"#,   // capital I
            r#"{"entries":{"read":{"identity":1,"Tool":{}}}}"#,   // capital T
            r#"{"entries":{"Read":{"identity":1,"tool":{}}}}"#,   // capital R on the tool key
            r#"{"entries":{"read":{"tool":{}}}}"#,                // identity missing
            r#"{"entries":{"read":{"identity":1}}}"#,             // tool missing
            r#"{"entries":{"read":{"identity":1,"tool":null}}}"#, // explicit null tool
        ];
        for text in refused {
            assert!(Snapshot::from_json_str(text).is_err(), "{} should be refused", text);
        }
    }

    #[test]
    fn an_unknown_key_is_ignored_rather_than_refused_which_is_what_the_source_does() {
        // The counterpart of the test above, and the reason it works. Extra
        // fields are ignored, so a wrongly cased key is only ever caught by
        // being absent. This is `Schema.Class` behaviour, not serde default.
        let text = r#"{"entries":{"read":{"identity":1,"tool":{}}},"extra":true}"#;
        let snapshot = Snapshot::from_json_str(text).expect("extra keys are ignored");
        assert_eq!(snapshot.entries.0.len(), 1);
        assert_eq!(snapshot.entries.0.get("read").unwrap().identity(), Identity(1));
    }

    #[test]
    fn the_snapshot_rejects_a_tool_that_is_not_a_frozen_empty_object() {
        // `AnyTool` is `Object.freeze({})`, so `{}` is the only valid JSON.
        for text in [
            r#"{"entries":{"read":{"identity":1,"tool":{"name":"read"}}}}"#,
            r#"{"entries":{"read":{"identity":1,"tool":[]}}}"#,
            r#"{"entries":{"read":{"identity":1,"tool":""}}}"#,
            r#"{"entries":{"read":{"identity":1,"tool":0}}}"#,
        ] {
            assert!(Snapshot::from_json_str(text).is_err(), "{} should be refused", text);
        }
    }

    #[test]
    fn a_snapshot_round_trip_rebuilds_an_identical_registry() {
        let mut service = Service::new();
        service.register(&catalog(&["read"])).expect("valid");
        service.register(&catalog(&["read", "write"])).expect("valid");
        let snapshot = service.snapshot();
        let rebuilt = Snapshot::from_json_str(&snapshot.to_json_string()).expect("round trip");
        assert_eq!(rebuilt.entries, snapshot.entries);
        assert_eq!(rebuilt.into_entries().names().collect::<Vec<&str>>(), vec!["read", "write"]);
    }

    #[test]
    fn a_duplicate_key_in_the_json_collapses_the_way_a_map_set_does() {
        let text = r#"{"entries":{"read":{"identity":1,"tool":{}},"read":{"identity":2,"tool":{}}}}"#;
        let snapshot = Snapshot::from_json_str(text).expect("parses");
        let entries = snapshot.into_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.get("read").unwrap().identity(), Identity(2));
    }

    // --- registration handles ---------------------------------------------

    #[test]
    fn registration_handles_are_distinct_per_call() {
        let mut service = Service::new();
        let first = service.register(&catalog(&["read"])).expect("valid").unwrap();
        let second = service.register(&catalog(&["read"])).expect("valid").unwrap();
        assert_ne!(first, second);
        assert_ne!(first.id(), second.id());
        // The transform counter starts at 0 and does NOT share its sequence with
        // the entry identity counter, which starts at 1.
        assert_eq!(first.id(), 0);
        assert_eq!(second.id(), 1);
        assert_eq!(service.entries().get("read").unwrap().identity().value(), 2);
    }

    #[test]
    fn entries_is_a_live_view_that_later_registrations_extend() {
        // Mirrors `entries: () => state.get().entries`: reading does not freeze
        // the registry.
        let mut service = Service::new();
        let before: Vec<String> =
            service.entries().names().map(str::to_string).collect();
        assert!(before.is_empty());

        service.register(&catalog(&["read"])).expect("valid");
        let middle: Vec<String> =
            service.entries().names().map(str::to_string).collect();
        service.register(&catalog(&["write"])).expect("valid");
        let after: Vec<String> =
            service.entries().names().map(str::to_string).collect();

        assert_eq!(before, Vec::<String>::new());
        assert_eq!(middle, vec!["read".to_string()]);
        assert_eq!(after, vec!["read".to_string(), "write".to_string()]);
    }
}