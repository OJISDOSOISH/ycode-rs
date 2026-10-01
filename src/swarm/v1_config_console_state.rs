//! Rust port of `opencode/packages/core/src/v1/config/console-state.ts`.
//!
//! The source is sixteen lines and contains **no logic at all**: one schema class
//! and one frozen value built from it.
//!
//! ```ts
//! export * as ConfigConsoleStateV1 from "./console-state"
//!
//! import { Schema } from "effect"
//! import { NonNegativeInt } from "../../schema"
//!
//! export class ConsoleState extends Schema.Class<ConsoleState>("ConsoleState"){
//!   consoleManagedProviders: Schema.mutable(Schema.Array(Schema.String)),
//!   activeOrgName: Schema.optional(Schema.String),
//!   switchableOrgCount: NonNegativeInt,
//! }) {}
//!
//! export const emptyConsoleState: ConsoleState = ConsoleState.make({
//!   consoleManagedProviders: [],
//!   activeOrgName: undefined,
//!   switchableOrgCount: 0,
//! })
//! ```
//!
//! ## The line 1 re-export
//!
//! `export * as ConfigConsoleStateV1 from "./console-state"` is a namespace
//! re-export that points at the file itself, so it is dead code. The Rust module
//! `v1_config_console_state` already *is* that namespace. Nothing to retype, as
//! in `v1_config_server.rs` and `v1_config_layout.rs`.
//!
//! ## `Schema.Class`, not `Schema.Struct`
//!
//! The source uses `Schema.Class`, which adds a constructor and `make`. The
//! difference from a plain struct is invisible on the wire: an instance still
//! serialises as an object with its own fields, and decoding is still a struct
//! decode. Rust has no prototypes, so there is nothing to carry over but the
//! three fields. `emptyConsoleState` is built with `ConsoleState.make`, which
//! *validates* its argument; that is why the constant below exists at all rather
//! than a bare literal, and it is also why the constant is checked in the tests
//! against a JSON round trip.
//!
//! ## Trap 1: the field names, verified one by one
//!
//! All three keys are camelCase, and two of them carry a capital letter in the
//! middle. They are transcribed literally, in source order:
//!
//! | line 7 | `consoleManagedProviders` | Rust `console_managed_providers` |
//! | line 8 | `activeOrgName`           | Rust `active_org_name`           |
//! | line 9 | `switchableOrgCount`      | Rust `switchable_org_count`      |
//!
//! Three keys. No fourth. Nothing is added to make the structure look more
//! complete, and nothing is renamed into a shape the source does not use.
//!
//! Each `#[serde(rename = "...")]` is **written out in full**, never derived from
//! a `rename_all`. That is deliberate: a `rename_all` would be computed from the
//! Rust field names and nobody could read the three names off the file at a
//! glance. This is the same reasoning as `v1_config_server.rs`, where
//! `mdnsDomain` carries its rename explicitly.
//!
//! `#[serde(rename)]` **replaces** the field name, it does not add an alias. So
//! `active_org_name` is not an alternative spelling of `activeOrgName`: it is an
//! unknown property, and unknown properties are ignored. That is why
//! `snake_case_keys_are_refused_on_read` can assert that a snake_case payload
//! leaves the field at `None` instead of filling it.
//!
//! ## `Schema.mutable(Schema.Array(Schema.String))` is a `Vec`, not a set
//!
//! `mutable` is explicit in the source: this is not a `readonly`, so it is not a
//! `BTreeSet` and not a `BTreeMap`. It is a `Vec<String>` and **the order read is
//! the order kept**, and **duplicates are kept**. Neither the type nor the schema
//! deduplicates.
//!
//! That matters because the producer does deduplicate:
//! `packages/opencode/src/config/config.ts:334` fills a `Set<string>` and line 605
//! calls `Array.from(...)` on it. So in practice the array has no duplicates. The
//! schema itself promises nothing of the sort, and porting the `Set` semantics
//! here would silently change the contract for any other caller. Order and
//! duplicates are pinned by `console_managed_providers_keeps_order_and_duplicates`.
//!
//! ## `NonNegativeInt` is reused, not redeclared
//!
//! `NonNegativeInt` comes from `packages/schema/src/schema.ts` line 4 and is
//! `Schema.Int.check(Schema.isGreaterThanOrEqualTo(0))`. In this batch the type
//! was already ported by `config_compaction.rs`, which declares
//! `pub type NonNegativeInt = u64;`.
//!
//! It is **re-exported** here rather than written a second time:
//! `v1_config_server.rs` already established that precedent with
//! `pub use crate::swarm::config_tool_output::{est_positif, PositiveInt};`, for
//! the same reason. A type alias is not a distinct contract, so redefining it
//! here would create two spellings of one thing that can drift apart without
//! producing a single compile error, because Rust allows the same name in two
//! modules. The re-export keeps one owner.
//!
//! The unsigned type does the whole job of the `check`: it refuses negatives and
//! it accepts `0`, exactly as the schema does. Two gaps remain, both inherited
//! from `u64` and documented rather than hidden:
//!
//! 1. `Schema.Int` is a **JavaScript** integer, capped by
//!    `Number.MAX_SAFE_INTEGER` (2^53 - 1). `u64` accepts values far above that.
//! 2. In JavaScript `Number.isInteger(0.0)` is `true`, so `0.0` would be an `Int`.
//!    `serde_json` refuses a JSON float where an integer is expected. The two
//!    together are pinned by `switchable_org_count_refuses_what_the_schema_refuses`.
//!
//! ## `Schema.optional` and the only filter this module applies
//!
//! `Schema.optional(Schema.String)` on line 8 makes the key optional: the decoded
//! type is `string | undefined`. The Rust translation is `Option<String>` with
//! `skip_serializing_if = "Option::is_none"`, which is exactly what
//! `JSON.stringify` does with `undefined` - the key disappears instead of being
//! written `null`. That is why `emptyConsoleState` may spell
//! `activeOrgName: undefined` and still produce `{"consoleManagedProviders":[],"switchableOrgCount":0}`
//! on the wire, which is the exact byte string the TypeScript test fixtures use
//! (`packages/opencode/test/fixture/tui-sdk.ts:65` and
//! `packages/opencode/test/server/httpapi-experimental.test.ts:160`).
//!
//! The two value judgements of the source language, for the record:
//!
//! - the **ternary** `x ? a : b` tests **truthiness**. In JavaScript `""` is
//!   falsy, so `state.activeOrgName ? state.activeOrgName : undefined` swallows
//!   the empty string.
//! - the **coalescent** `x ?? y` tests **nullity**. Only `null` and `undefined`
//!   trigger `y`, so `x ?? y` keeps `""`.
//!
//! **This module does neither.** It is a schema declaration, not an expression:
//! an empty string is a perfectly valid `activeOrgName` and it must survive both
//! the decode and the re-serialisation. The truthiness family does exist, but in
//! the *consumer* - see the next section - and it is reproduced in the test module
//! as a separate function, never merged with the nullity family.
//!
//! ## There is no merge, and none is invented
//!
//! This is persisted-looking state, so the obvious place to look for a merge or a
//! default is a merge. There is none, and that is a finding, not an omission:
//!
//! - **Nothing merges it.** `packages/opencode/src/config/config.ts:604` builds
//!   the whole object as a **fresh literal** on every load:
//!   `consoleState: { consoleManagedProviders: Array.from(...), activeOrgName,
//!   switchableOrgCount: 0 }`. No previous value is read, so there is no merge to
//!   port.
//! - **Nothing defaults it either.** No `Schema.withDecodingDefault`, no
//!   `withDefaults`, no default branch. A payload missing one of the two required
//!   keys is a **decode failure**, not a zero-filled object. That is why
//!   `console_managed_providers` and `switchable_org_count` carry **no**
//!   `#[serde(default)]`: adding one would be a real behavioural change, and it
//!   would be the wrong one, because it would accept payloads the TypeScript
//!   schema rejects. `a_missing_required_key_is_a_decode_failure` pins this.
//! - **`activeOrgName` is written once and never assigned** in
//!   `config.ts:335` (`let activeOrgName: string | undefined`) before being read
//!   at line 606, so in that file the value is always `undefined`. The schema
//!   merely transports it; the real value comes from elsewhere. Adding a default
//!   for it here would be inventing a value the source never supplies.
//!
//! So the only default-shaped thing in the source is `emptyConsoleState` itself,
//! which is a **named constant**, not a decoding rule. It is ported as
//! `EMPTY_CONSOLE_STATE`, and `Default` is implemented *by returning that
//! constant* so the two can never disagree.
//!
//! `switchableOrgCount: 0` in that constant is not a default either: it is the
//! value the producer writes, and the HTTP handler overwrites it with a real
//! count (`groups` reduced by `orgs.length`) before it reaches the wire.
//!
//! ## What the consumer does to `activeOrgName`
//!
//! `packages/opencode/src/server/routes/instance/httpapi/handlers/experimental.ts:55`
//! builds the response with a spread:
//!
//! ```ts
//! ...(state.activeOrgName ? { activeOrgName: state.activeOrgName } : {})
//! ```
//!
//! That is the **truthiness** family: `""` disappears, even though the schema
//! accepted it. The result is a wire value that this module never produces, which
//! is fine and normal - the two live in two different files. The test module
//! reproduces the filter as `filtre_de_veracite_du_consommateur`, clearly marked
//! as **not** part of this module's API, so the two judgements can be compared
//! side by side on `Some("")`.
//!
//! Note also that the HTTP response type is a **different declaration**:
//! `ConsoleStateResponse` in
//! `packages/opencode/src/server/routes/instance/httpapi/groups/experimental.ts:22`
//! uses `Schema.optionalKey(Schema.String)` rather than `Schema.optional`, and its
//! generated OpenAPI carries `additionalProperties: false` against the core
//! class's leniency. It lives in another package and belongs to another port; it
//! is **not** redeclared here, or two definitions of one contract would drift
//! apart with no compile error to announce it.
//!
//! ## Unknown keys are ignored, missing required keys are not
//!
//! A `Schema.Struct` ignores properties it does not know, so no
//! `deny_unknown_fields` here. Two consequences worth stating, because they are
//! the forward- and backward-compatibility rules of this type:
//!
//! - a payload written by a **newer** version, carrying a field this port does
//!   not know, still decodes;
//! - a payload written by an **older** version, missing `consoleManagedProviders`
//!   or `switchableOrgCount`, is **rejected**, because those two keys are
//!   required in `openapi.json` (`"required": ["consoleManagedProviders",
//!   "switchableOrgCount"]`).
//!
//! There is no file read here to be strict about: `loadInstanceState` computes
//! this state, it does not load it from disk in the core package.
//!
//! ## Explicit `null` is read as absence, and is then lost
//!
//! A serde `Option` reads an explicit JSON `null` as `None`, silently. So
//! `{"activeOrgName": null}` decodes here, and re-serialising it **drops the
//! key**. The TypeScript type is `string | undefined`, and `null` is not
//! `undefined`, so the effect schema is expected to *reject* it. That divergence
//! is real, invisible from inside Rust, and the only thing that catches it is a
//! test: `an_explicit_null_is_read_as_absence_and_is_then_lost`.
//!
//! No `#[serde(deserialize_with)]` and no `deny` of `null` is added here to hide
//! it, because doing so would break the far more common TypeScript-produced
//! payload, where the key is simply absent.
//!
//! ## Byte boundaries
//!
//! Nothing in this module indexes a `&str`. `console_managed_providers` is
//! compared with whole-string equality, and no slicing, truncation or
//! byte-offset arithmetic exists anywhere. That is a deliberate choice: a
//! `&s[..n]` that lands inside a multi-byte character panics at run time with no
//! compile error, and a provider id or an org name is arbitrary user input.
//! `multi_byte_content_never_panics_and_survives_the_round_trip` uses a five
//! dash string, five characters and fifteen bytes, and shows the only safe way to
//! cut one: `char_indices()`, which always lands on a character boundary.
//!
//! ## Names that must not be declared here
//!
//! - `ConsoleStateResponse` and the rest of the HTTP group: another package, and
//!   another port.
//! - `isConsoleManagedProvider` in `packages/tui/src/util/provider-origin.ts`: it
//!   accepts `string[] | ReadonlySet<string>` and belongs to the TUI.
//! - the `emptyConsoleState` literal written a second time at
//!   `packages/tui/src/context/sync.tsx:36`: the same contract written twice in
//!   the source repository. It is not a reason to declare a second Rust type, and
//!   it is the reason the tests compare against the exact fixture bytes instead.
//!
//! `manages` below is the one addition, and it adds no contract: it is
//! `consoleManagedProviders.includes(providerID)` in readable form, with the same
//! strictness as `Array.prototype.includes` - exact, same-value-zero, no
//! trimming and no case folding.
//!
//! ## Limit of verification
//!
//! The `effect` library is not installed on this machine, so the runtime
//! behaviour of `Schema.Class`, `Schema.optional` and `Schema.Int` could not be
//! observed. Two points rest on the source text plus the generated
//! `packages/sdk/openapi.json` (lines 21881-21899) and
//! `packages/sdk/js/src/v2/gen/types.gen.ts` (lines 2136-2140), which agree with
//! the reading above:
//!
//! - `activeOrgName` is optional and `null` is outside the type;
//! - `consoleManagedProviders` and `switchableOrgCount` are required.
//!
//! Everything this module actually asserts about itself is tested in Rust, so the
//! assertions do not depend on that reading being confirmed.

use serde::{Deserialize, Serialize};

// `NonNegativeInt` is already ported by `config_compaction.rs` in the same batch.
// It is re-exported instead of redeclared: two definitions of one contract would
// drift apart silently, because Rust allows the same name in two modules and
// never complains. See `v1_config_server.rs` for the same move with `PositiveInt`.
pub use crate::swarm::config_compaction::NonNegativeInt;

/// Schema identifier in the `effect/Schema` registry.
///
/// From `Schema.Class<ConsoleState>("ConsoleState")`, line 6. The first argument
/// is also the TypeScript class name, so the two coincide here.
///
/// Not to be confused with `ModuleName` in the file name, and not to be confused
/// with the HTTP response schema, which reuses the same identifier string in a
/// different registry and is not ported here.
pub const SCHEMA_IDENTIFIER: &str = "ConsoleState";

/// The three keys of the object, in source order.
///
/// Witnesses against drift: this list and [`ConsoleState`] must describe the same
/// three names. The test `field_names_match_the_typescript_key_for_key` checks it
/// in both directions.
pub const FIELD_NAMES: [&str; 3] = [
    "consoleManagedProviders",
    "activeOrgName",
    "switchableOrgCount",
];

/// The keys that are **not** optional, that is the ones a payload must carry.
///
/// `openapi.json` line 21898 states it for the HTTP mirror of this type:
/// `"required": ["consoleManagedProviders", "switchableOrgCount"]`. Only
/// `activeOrgName` is `Schema.optional`.
///
/// This list exists so the decoder and the documentation cannot quietly disagree
/// about what is required.
pub const REQUIRED_FIELD_NAMES: [&str; 2] = ["consoleManagedProviders", "switchableOrgCount"];

/// The single optional key.
pub const OPTIONAL_FIELD_NAMES: [&str; 1] = ["activeOrgName"];

/// Console-side provider and organisation state.
///
/// Source: `ConfigConsoleStateV1.ConsoleState`, line 6.
///
/// This is a carrier, not an accumulator. The producer builds a fresh object on
/// every configuration load (`packages/opencode/src/config/config.ts:604`) and no
/// code merges an older value into it, so there is no merge method here and none
/// is invented.
///
/// The object is not read from disk by the core package; the decode rules below
/// therefore describe a payload exchanged with the other language and with the
/// HTTP layer, not a configuration file. The same rules apply to both.
///
/// Unknown properties are tolerated, as in TypeScript, where a `Schema.Struct`
/// ignores the fields it does not declare: no `deny_unknown_fields`.
///
/// Two of the three keys are required and carry **no** `#[serde(default)]` on
/// purpose. A missing required key is a decode failure here, exactly as it is in
/// the TypeScript schema; a `default` would silently turn the failure into a zero
/// value that the other language would have rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleState {
    /// Provider ids managed by the console, in order, with duplicates kept.
    ///
    /// From line 7: `Schema.mutable(Schema.Array(Schema.String))`. `mutable`
    /// means not `readonly`, so this is a `Vec<String>`, not a set: the order is
    /// preserved and nothing is deduplicated. The producer happens to fill it
    /// from a `Set`, which removes duplicates *there*, but the schema promises
    /// nothing and this type does not add anything.
    ///
    /// Required: a payload without this key is refused.
    #[serde(rename = "consoleManagedProviders")]
    pub console_managed_providers: Vec<String>,

    /// Name of the active organisation, when there is one.
    ///
    /// From line 8: `Schema.optional(Schema.String)`, so the decoded type is
    /// `string | undefined`.
    ///
    /// `skip_serializing_if` is the translation of what `JSON.stringify` does with
    /// `undefined`: the key is omitted rather than written `null`. That is also
    /// why `emptyConsoleState` can spell `activeOrgName: undefined` and still
    /// produce `{"consoleManagedProviders":[],"switchableOrgCount":0}`.
    ///
    /// The **empty string is a valid value** and survives decoding and
    /// re-serialisation: the schema performs no truthiness test. The consumer
    /// that drops it is another file
    /// (`handlers/experimental.ts:55`, `...(state.activeOrgName ? {...} : {})`).
    #[serde(default, rename = "activeOrgName", skip_serializing_if = "Option::is_none")]
    pub active_org_name: Option<String>,

    /// How many organisations can be switched to.
    ///
    /// From line 9: `NonNegativeInt`, so an integer greater than or equal to
    /// zero. `u64` refuses negatives and accepts `0`, which is the whole check.
    ///
    /// Required: a payload without this key is refused. Note that the producer
    /// always writes `0` here
    /// (`packages/opencode/src/config/config.ts:607`) and the HTTP handler
    /// recomputes a real count before answering; the schema itself has no
    /// default.
    #[serde(rename = "switchableOrgCount")]
    pub switchable_org_count: NonNegativeInt,
}

/// The frozen empty state, port of `emptyConsoleState` (lines 12 to 16).
///
/// `consoleManagedProviders: []`, `activeOrgName: undefined`,
/// `switchableOrgCount: 0`.
///
/// It is a **named constant**, not a decoding rule: a payload that omits a
/// required key still fails, and nothing here is ever substituted for a missing
/// value. `Vec::new()` is a `const fn`, so this allocates nothing and can be used
/// wherever a `const` is expected.
///
/// This is exactly the byte string the TypeScript fixtures expect:
/// `{"consoleManagedProviders":[],"switchableOrgCount":0}`.
pub const EMPTY_CONSOLE_STATE: ConsoleState = ConsoleState {
    console_managed_providers: Vec::new(),
    active_org_name: None,
    switchable_org_count: 0,
};

impl ConsoleState {
    /// The empty state, as a function for callers that prefer a call over a
    /// constant.
    pub const fn empty() -> Self {
        EMPTY_CONSOLE_STATE
    }

    /// Builds a state from a list of console-managed provider ids.
    ///
    /// `ConsoleState.make` in the source validates a literal; this constructor
    /// performs no check either, because there is nothing to check on two
    /// integers and a list of strings.
    pub fn new(
        console_managed_providers: Vec<String>,
        active_org_name: Option<String>,
        switchable_org_count: NonNegativeInt,
    ) -> Self {
        Self {
            console_managed_providers,
            active_org_name,
            switchable_org_count,
        }
    }

    /// Whether `provider_id` is managed by the console.
    ///
    /// Translates `state.consoleManagedProviders.includes(providerID)`, as called
    /// from `packages/tui/src/component/dialog-provider.tsx:135` through
    /// `isConsoleManagedProvider`.
    ///
    /// The comparison is as strict as `Array.prototype.includes`: exact string
    /// equality, no trimming, no case folding, no numeric coercion. The
    /// `string[] | ReadonlySet<string>` overload of the TUI helper is **not**
    /// ported here; that is another module, and it would drag a second container
    /// contract into this file.
    pub fn manages(&self, provider_id: &str) -> bool {
        self.console_managed_providers
            .iter()
            .any(|provider| provider == provider_id)
    }
}

/// Implemented by returning the constant, so the two can never disagree.
impl Default for ConsoleState {
    fn default() -> Self {
        EMPTY_CONSOLE_STATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // The two value judgements, written TWICE and never merged.
    //
    // Neither is ported from `console-state.ts`, which contains no ternary and
    // no coalescent: it is a schema declaration. They live here so the
    // distinction stays checkable. On `Some("")` they disagree, and the module
    // itself belongs to neither: it accepts the empty string.
    // ---------------------------------------------------------------------

    /// Truthiness family: the ternary `x ? a : b`.
    ///
    /// In JavaScript an empty string is falsy, so the value disappears and the
    /// result is `None`. This is what
    /// `...(state.activeOrgName ? { activeOrgName } : {})` does, in
    /// `handlers/experimental.ts:55`.
    fn disparait_si_falsy(valeur: Option<String>) -> Option<String> {
        valeur.filter(|v| !v.is_empty())
    }

    /// Nullity family: the coalescent `x ?? y`.
    ///
    /// Only `null` and `undefined` trigger the right-hand side, so the empty
    /// string **survives**. This is what the schema does.
    fn survit_si_null(valeur: Option<String>) -> Option<String> {
        // `x ?? y` equals `x` unless `x` is `null`, in which case it equals `y`.
        // Here `y` gives `x` back: the only case that could change is absence,
        // which stays absence, and a present value stays present.
        valeur.or(None)
    }

    /// Truthiness filter of the HTTP consumer. NOT part of this module's API.
    ///
    /// Reproduces `handlers/experimental.ts:55` so the two families can be
    /// compared on the same input. Its result is a wire value this module never
    /// produces, which is why it lives in the test module and not next to
    /// `ConsoleState`.
    fn filtre_de_veracite_du_consommateur(valeur: Option<&String>) -> Option<String> {
        match valeur {
            Some(v) if !v.is_empty() => Some(v.clone()),
            _ => None,
        }
    }

    /// Char-boundary-safe cut, the shape the module is allowed to use.
    ///
    /// `char_indices()` yields the byte offset of each character, so
    /// `.nth(n)` lands on a boundary. A raw `&s[..n]` does not, and panics as
    /// soon as `n` falls inside a multi-byte character.
    fn truncate_sur_frontieres(valeur: &str, max_chars: usize) -> &str {
        match valeur.char_indices().nth(max_chars) {
            Some((index, _)) => &valeur[..index],
            None => valeur,
        }
    }

    /// Witness that the three names in the source are the three names on the
    /// wire, in both directions.
    #[test]
    fn field_names_match_the_typescript_key_for_key() {
        assert_eq!(
            FIELD_NAMES,
            [
                "consoleManagedProviders",
                "activeOrgName",
                "switchableOrgCount"
            ]
        );
        assert_eq!(REQUIRED_FIELD_NAMES, ["consoleManagedProviders", "switchableOrgCount"]);
        assert_eq!(OPTIONAL_FIELD_NAMES, ["activeOrgName"]);
        assert_eq!(SCHEMA_IDENTIFIER, "ConsoleState");

        // The three names partition the optional and the required, with nothing
        // left over and nothing counted twice.
        assert_eq!(
            REQUIRED_FIELD_NAMES.len() + OPTIONAL_FIELD_NAMES.len(),
            FIELD_NAMES.len()
        );
        for optionnel in OPTIONAL_FIELD_NAMES {
            assert!(
                FIELD_NAMES.contains(&optionnel),
                "optional key absent from FIELD_NAMES: {optionnel}"
            );
            assert!(
                !REQUIRED_FIELD_NAMES.contains(&optionnel),
                "key declared optional and required: {optionnel}"
            );
        }
        for requis in REQUIRED_FIELD_NAMES {
            assert!(
                FIELD_NAMES.contains(&requis),
                "required key absent from FIELD_NAMES: {requis}"
            );
        }

        // The TypeScript shape, as written in `packages/sdk/js/src/v2/gen/types.gen.ts`
        // lines 2136 to 2140, and as generated in `openapi.json` lines 21881 to
        // 21899. Neither has a fourth key.
        assert_eq!(FIELD_NAMES.len(), 3);

        // No capital letter survives on the wire. This is the whole point of the
        // three `#[serde(rename)]`, and it is invisible from inside the module.
        let etat = ConsoleState::new(vec!["anthropic".to_string()], Some("acme".to_string()), 3);
        let objet = serde_json::to_value(&etat).unwrap();
        for nom in FIELD_NAMES {
            assert!(
                objet.get(nom).is_some(),
                "declared key absent from the JSON: {nom}"
            );
        }
        assert_eq!(objet.as_object().unwrap().len(), FIELD_NAMES.len());

        for snake in [
            "console_managed_providers",
            "active_org_name",
            "switchable_org_count",
            "ConsoleManagedProviders",
            "console_managed_provider",
        ] {
            assert!(
                objet.get(snake).is_none(),
                "form not in the source leaked onto the wire: {snake}"
            );
        }
    }

    /// Serialisation test: the exact bytes the TypeScript fixtures expect.
    #[test]
    fn serialisation_uses_the_typescript_names() {
        let etat = ConsoleState::new(
            vec!["anthropic".to_string(), "openai".to_string()],
            Some("acme".to_string()),
            2,
        );
        assert_eq!(
            serde_json::to_string(&etat).unwrap(),
            r#"{"consoleManagedProviders":["anthropic","openai"],"activeOrgName":"acme","switchableOrgCount":2}"#
        );

        // Declaration order, which serde preserves: the JSON follows the source
        // order of lines 7, 8 and 9.
        let brut = serde_json::to_string(&etat).unwrap();
        let providers = brut.find("consoleManagedProviders").unwrap();
        let org = brut.find("activeOrgName").unwrap();
        let count = brut.find("switchableOrgCount").unwrap();
        assert!(providers < org && org < count, "keys left source order");

        // Full round trip through both languages' shape.
        let relu: ConsoleState = serde_json::from_str(&brut).unwrap();
        assert_eq!(relu, etat);

        // The one key the wire never carries when it is absent: `skip_serializing_if`
        // writes nothing, `JSON.stringify` drops `undefined`.
        let sans_org: ConsoleState =
            serde_json::from_str(r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&sans_org).unwrap(),
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#
        );
        assert!(!serde_json::to_string(&sans_org).unwrap().contains("activeOrgName"));
    }

    /// The trap of this port, in its second half: a missing `rename` is invisible
    /// until the other language is involved.
    ///
    /// `#[serde(rename)]` **replaces** the field name, it does not add an alias.
    /// So `active_org_name` in a payload is an unknown property, and unknown
    /// properties are ignored: the field stays `None`.
    ///
    /// If the `#[serde(rename = "activeOrgName")]` were deleted tomorrow, the
    /// first assertion of this test would fail. Without it, the exchange with the
    /// TypeScript would break, silently, with no compile error anywhere.
    #[test]
    fn snake_case_keys_are_refused_on_read() {
        // The optional key, in every tempting spelling. None of them fills the
        // field, because none of them is the declared name.
        for faux in [
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0,"active_org_name":"acme"}"#,
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0,"ActiveOrgName":"acme"}"#,
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0,"activeorgname":"acme"}"#,
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0,"activeOrgname":"acme"}"#,
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0,"active_Org_Name":"acme"}"#,
        ] {
            let etat: ConsoleState = serde_json::from_str(faux).unwrap();
            assert_eq!(
                etat.active_org_name, None,
                "forbidden spelling filled the field: {faux}"
            );
            assert!(!serde_json::to_string(&etat).unwrap().contains("acme"));
        }

        // The two required keys are not silently defaulted when they arrive under
        // a forbidden name: the camelCase key is missing, so the decode fails.
        for faux in [
            r#"{"console_managed_providers":[],"switchable_org_count":0}"#,
            r#"{"consoleManagedProviders":[],"switchable_org_count":0}"#,
            r#"{"console_managed_providers":[],"switchableOrgCount":0}"#,
            r#"{"ConsoleManagedProviders":[],"SwitchableOrgCount":0}"#,
        ] {
            assert!(
                serde_json::from_str::<ConsoleState>(faux).is_err(),
                "forbidden spelling of a required key was accepted: {faux}"
            );
        }

        // Witness, mandatory: without it, a `rename` pointing at a name that does
        // not exist would pass for a correct refusal.
        let bon: ConsoleState =
            serde_json::from_str(r#"{"consoleManagedProviders":["a"],"activeOrgName":"acme","switchableOrgCount":1}"#).unwrap();
        assert_eq!(bon.active_org_name.as_deref(), Some("acme"));
        assert_eq!(bon.console_managed_providers, vec!["a".to_string()]);
        assert_eq!(bon.switchable_org_count, 1);
    }

    /// A missing optional key is an absence, and stays one.
    ///
    /// `Option` + `skip_serializing_if` is the `Schema.optional` behaviour, and it
    /// belongs to the nullity family: an absent key is not a value.
    #[test]
    fn a_missing_optional_key_stays_absent_and_serialises_away() {
        let etat: ConsoleState =
            serde_json::from_str(r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#).unwrap();
        assert_eq!(etat.active_org_name, None);
        assert_eq!(etat, EMPTY_CONSOLE_STATE);

        // The key is absent from the JSON, not present as `null`. That is what
        // `JSON.stringify` does with `undefined`, and the two are different byte
        // strings.
        let brut = serde_json::to_string(&etat).unwrap();
        assert_eq!(brut, r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#);
        assert!(!brut.contains("activeOrgName"));
        assert!(!brut.contains("null"));

        // Round trip: the absence survives.
        let relu: ConsoleState = serde_json::from_str(&brut).unwrap();
        assert_eq!(relu.active_org_name, None);
    }

    /// Explicit `null` is read as `None`, silently, and is then LOST.
    ///
    /// A serde `Option` accepts a JSON `null` where the TypeScript type
    /// `string | undefined` does not include `null`. Nothing fails, nothing warns:
    /// the null becomes an absence, and re-serialising writes no key at all. The
    /// original `null` is gone. Only a test can see this.
    #[test]
    fn an_explicit_null_is_read_as_absence_and_is_then_lost() {
        let etat: ConsoleState = serde_json::from_str(
            r#"{"consoleManagedProviders":[],"activeOrgName":null,"switchableOrgCount":0}"#,
        )
        .unwrap();
        assert_eq!(etat.active_org_name, None, "a null was not read as an absence");
        assert_eq!(etat, EMPTY_CONSOLE_STATE, "the null changed nothing observable");

        // And the loss: the key that was explicitly present on input is absent on
        // output. The two JSON documents are not the same document.
        let brut = serde_json::to_string(&etat).unwrap();
        assert!(!brut.contains("activeOrgName"));
        assert!(!brut.contains("null"));

        // A `null` on a required key is NOT accepted, because `u64` is not an
        // `Option`: the same lenient reading does not extend there.
        assert!(serde_json::from_str::<ConsoleState>(
            r#"{"consoleManagedProviders":[],"switchableOrgCount":null}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ConsoleState>(
            r#"{"consoleManagedProviders":null,"switchableOrgCount":0}"#
        )
        .is_err());

        // And the truthiness family cannot be blamed for this: it also drops the
        // empty string, which the nullity family keeps. Two separate defects,
        // never merged into one.
        assert_eq!(survit_si_null(Some(String::new())), Some(String::new()));
        assert_eq!(disparait_si_falsy(Some(String::new())), None);
    }

    /// The "older version wrote the file" case: a missing required key is a
    /// failure, not a zero.
    ///
    /// This is where defaults hide. The source declares no decoding default
    /// anywhere, so a payload that predates one of the two required keys cannot be
    /// read. Adding `#[serde(default)]` to either field would turn this failure
    /// into a silent `[]` or `0`, and the TypeScript would still refuse the same
    /// payload.
    #[test]
    fn a_missing_required_key_is_a_decode_failure() {
        for incomplet in [
            r#"{"switchableOrgCount":0}"#,
            r#"{"consoleManagedProviders":[]}"#,
            r#"{}"#,
            r#"{"activeOrgName":"acme"}"#,
        ] {
            assert!(
                serde_json::from_str::<ConsoleState>(incomplet).is_err(),
                "incomplete payload accepted, a default was invented: {incomplet}"
            );
        }

        // The empty state is **not** the fallback for a broken payload. It is a
        // constant the caller chooses to use; the decoder never substitutes it.
        assert!(serde_json::from_str::<ConsoleState>(r#"{}"#).is_err());
        assert_eq!(EMPTY_CONSOLE_STATE.switchable_org_count, 0);
        assert!(EMPTY_CONSOLE_STATE.console_managed_providers.is_empty());

        // Same for a payload where the required key is present but empty: an empty
        // list is a value, not an absence, and must be kept as such.
        let vide: ConsoleState =
            serde_json::from_str(r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#).unwrap();
        assert!(vide.console_managed_providers.is_empty());
        assert_eq!(vide.switchable_org_count, 0);
    }

    /// The "newer version wrote the file" case: an unknown key is ignored, as in
    /// TypeScript, where a `Schema.Struct` ignores properties it does not declare.
    ///
    /// The tolerance is **only** for extra keys. The two required keys keep
    /// failing when absent, as the previous test shows: a type cannot be lenient
    /// in one direction and strict in the other without meaning it.
    #[test]
    fn an_unknown_key_is_ignored_like_a_typescript_struct() {
        let etat: ConsoleState = serde_json::from_str(
            r#"{"consoleManagedProviders":["a"],"switchableOrgCount":1,"futureField":{"nested":[1,2]},"consoleStateVersion":3}"#,
        )
        .unwrap();
        assert_eq!(etat.console_managed_providers, vec!["a".to_string()]);
        assert_eq!(etat.switchable_org_count, 1);
        assert_eq!(etat.active_org_name, None);

        // The unknown keys are not re-emitted, exactly as `JSON.stringify` drops
        // them when it meets a class instance with only three own fields.
        let brut = serde_json::to_string(&etat).unwrap();
        assert_eq!(
            brut,
            r#"{"consoleManagedProviders":["a"],"switchableOrgCount":1}"#
        );
        assert!(!brut.contains("futureField"));
        assert!(!brut.contains("consoleStateVersion"));

        // Still strict about what is required: tolerance for extras does not
        // extend to the required keys.
        assert!(serde_json::from_str::<ConsoleState>(r#"{"futureField":1}"#).is_err());
    }

    /// The two judgement families, side by side, on the same inputs.
    ///
    /// This module implements neither: it is a schema declaration. It accepts the
    /// empty string, so it belongs to the nullity family, and the test below says
    /// so explicitly rather than leaving it to be inferred.
    #[test]
    fn the_two_judgement_families_disagree() {
        // The line that makes the trap visible: on `Some("")`, the ternary
        // erases and the coalescent keeps.
        assert_eq!(disparait_si_falsy(Some(String::new())), None);
        assert_eq!(survit_si_null(Some(String::new())), Some(String::new()));

        // On a full value the two agree, which is the only thing separating them,
        // and that is what makes the comparison honest.
        assert_eq!(
            disparait_si_falsy(Some("acme".to_string())),
            Some("acme".to_string())
        );
        assert_eq!(
            survit_si_null(Some("acme".to_string())),
            Some("acme".to_string())
        );

        // And on absence both are `None`: `None` is neither falsy nor null, but it
        // disappears in both families. The parameter type is fixed at
        // `Option<String>`, so the bare `None` infers from it.
        assert_eq!(disparait_si_falsy(None), None);
        assert_eq!(survit_si_null(None), None);

        // The consumer's filter is the truthiness family, and it drops what the
        // schema legitimately accepted.
        let etat: ConsoleState = serde_json::from_str(
            r#"{"consoleManagedProviders":[],"activeOrgName":"","switchableOrgCount":0}"#,
        )
        .unwrap();
        assert_eq!(etat.active_org_name, Some(String::new()));
        assert_eq!(
            filtre_de_veracite_du_consommateur(etat.active_org_name.as_ref()),
            None,
            "the consumer filter stopped being a truthiness test"
        );
        assert_eq!(
            filtre_de_veracite_du_consommateur(Some(&"acme".to_string())),
            Some("acme".to_string())
        );
        assert_eq!(filtre_de_veracite_du_consommateur(None), None);
    }

    /// The empty string: valid for the schema, dropped by the consumer.
    ///
    /// Both halves are pinned here, because they belong to two different files and
    /// a port that keeps only one of them looks correct and behaves wrongly.
    #[test]
    fn an_empty_string_survives_decoding_and_disappears_on_the_wire() {
        let etat: ConsoleState = serde_json::from_str(
            r#"{"consoleManagedProviders":[],"activeOrgName":"","switchableOrgCount":0}"#,
        )
        .unwrap();

        // The module accepts it, because `Schema.optional(Schema.String)` accepts
        // every string, empty included. No `is_empty` filter is introduced here.
        assert_eq!(etat.active_org_name, survit_si_null(Some(String::new())));
        assert_eq!(disparait_si_falsy(etat.active_org_name.clone()), None);

        // And it re-serialises as an empty string, not as a missing key: the
        // difference between `""` and absent is preserved by this type.
        assert_eq!(
            serde_json::to_string(&etat).unwrap(),
            r#"{"consoleManagedProviders":[],"activeOrgName":"","switchableOrgCount":0}"#
        );

        // Only the consumer's truthiness filter removes it. Same input, different
        // family, different output: the two functions never share a body.
        assert_eq!(
            filtre_de_veracite_du_consommateur(etat.active_org_name.as_ref()),
            None
        );

        // And `emptyConsoleState` uses `undefined`, which never becomes `""`.
        assert_eq!(EMPTY_CONSOLE_STATE.active_org_name, None);
    }

    /// The port of `emptyConsoleState`, byte for byte.
    ///
    /// The same two keys and the same two values as the TypeScript fixtures
    /// `packages/opencode/test/fixture/tui-sdk.ts:65` and
    /// `packages/opencode/test/server/httpapi-experimental.test.ts:160`, and the
    /// same required set as `openapi.json` line 21898.
    #[test]
    fn the_empty_state_matches_the_source_constant() {
        assert_eq!(ConsoleState::empty(), EMPTY_CONSOLE_STATE);
        assert_eq!(ConsoleState::default(), EMPTY_CONSOLE_STATE);
        assert_eq!(ConsoleState::new(Vec::new(), None, 0), EMPTY_CONSOLE_STATE);

        assert!(EMPTY_CONSOLE_STATE.console_managed_providers.is_empty());
        assert_eq!(EMPTY_CONSOLE_STATE.active_org_name, None);
        assert_eq!(EMPTY_CONSOLE_STATE.switchable_org_count, 0);

        // The exact fixture bytes.
        assert_eq!(
            serde_json::to_string(&EMPTY_CONSOLE_STATE).unwrap(),
            r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#
        );

        // And the read direction: the same bytes give the same constant.
        let relu: ConsoleState = serde_json::from_str(&serde_json::to_string(&EMPTY_CONSOLE_STATE).unwrap()).unwrap();
        assert_eq!(relu, EMPTY_CONSOLE_STATE);

        // A frozen empty list cannot be mutated through the constant, which is
        // what `Vec::new()` in a `const` guarantees: no allocation happened.
        let copie = EMPTY_CONSOLE_STATE;
        assert_eq!(copie.console_managed_providers.len(), 0);
    }

    /// What `NonNegativeInt` refuses, and the two places where `u64` is laxer or
    /// stricter than the JavaScript check.
    #[test]
    fn switchable_org_count_refuses_what_the_schema_refuses() {
        let modele = r#""consoleManagedProviders":[],"switchableOrgCount":"#;

        // Refused on both sides: the signed range, the non-integers, the strings,
        // the booleans.
        for valeur in ["-1", "1.5", r#""0""#, "true", "null", "-0.0"] {
            let brut = format!("{{{modele}{valeur}}}");
            assert!(
                serde_json::from_str::<ConsoleState>(&brut).is_err(),
                "value accepted where NonNegativeInt should refuse it: {valeur}"
            );
        }

        // Accepted on both sides: zero is the lower bound, inclusive.
        for valeur in ["0", "1", "17"] {
            let brut = format!("{{{modele}{valeur}}}");
            let etat: ConsoleState = serde_json::from_str(&brut).unwrap();
            assert_eq!(etat.switchable_org_count, valeur.parse::<u64>().unwrap());
        }

        // Documented divergence, stricter side: in JavaScript
        // `Number.isInteger(0.0)` is true, so `0.0` is an `Int` and the schema
        // would accept it. `serde_json` refuses a JSON float where an integer is
        // expected. Kept, because a float count has no meaning and no producer
        // emits one.
        assert!(serde_json::from_str::<ConsoleState>(&format!("{{{modele}0.0}}")).is_err());

        // Documented divergence, laxer side: `Schema.Int` is a JavaScript integer,
        // capped by `Number.MAX_SAFE_INTEGER`. `u64` has no such cap. Accepted
        // here, and the exact ceiling of the effect schema could not be observed
        // on this machine.
        let grand = format!("{{{modele}9007199254740993}}");
        let etat: ConsoleState = serde_json::from_str(&grand).unwrap();
        assert_eq!(etat.switchable_org_count, 9_007_199_254_740_993);
        assert_eq!(
            serde_json::to_string(&etat).unwrap(),
            r#"{"consoleManagedProviders":[],"switchableOrgCount":9007199254740993}"#
        );
    }

    /// `Schema.mutable(Schema.Array(Schema.String))`: a list, not a set.
    ///
    /// The producer fills it from a `Set`, so duplicates never appear in practice.
    /// The schema does not promise that, and neither does this type: a set here
    /// would be an invented constraint, and it would also destroy the order.
    #[test]
    fn console_managed_providers_keeps_order_and_duplicates() {
        let dans_le_desordre = vec![
            "zeta".to_string(),
            "alpha".to_string(),
            "zeta".to_string(),
        ];
        let etat = ConsoleState::new(dans_le_desordre.clone(), None, 3);
        let brut = serde_json::to_string(&etat).unwrap();
        assert_eq!(
            brut,
            r#"{"consoleManagedProviders":["zeta","alpha","zeta"],"switchableOrgCount":3}"#
        );

        // The order is the source order, and the duplicate is still there.
        let relu: ConsoleState = serde_json::from_str(&brut).unwrap();
        assert_eq!(relu.console_managed_providers, dans_le_desordre);
        assert_eq!(relu.console_managed_providers.len(), 3);
        assert_eq!(relu.console_managed_providers[0], "zeta");
        assert_eq!(relu.console_managed_providers[1], "alpha");
        assert_eq!(relu.console_managed_providers[2], "zeta");

        // An empty list is a value, not an absence: it survives, it is not
        // replaced by anything, and it is not turned into `null`.
        let vide: ConsoleState =
            serde_json::from_str(r#"{"consoleManagedProviders":[],"switchableOrgCount":0}"#).unwrap();
        assert_eq!(vide.console_managed_providers, Vec::<String>::new());

        // The list holds strings, and only strings.
        for faux in [
            r#"{"consoleManagedProviders":[1],"switchableOrgCount":0}"#,
            r#"{"consoleManagedProviders":[null],"switchableOrgCount":0}"#,
            r#"{"consoleManagedProviders":["ok",2],"switchableOrgCount":0}"#,
            r#"{"consoleManagedProviders":[[]],"switchableOrgCount":0}"#,
            r#"{"consoleManagedProviders":{},"switchableOrgCount":0}"#,
            r#"{"consoleManagedProviders":"anthropic","switchableOrgCount":0}"#,
        ] {
            assert!(
                serde_json::from_str::<ConsoleState>(faux).is_err(),
                "non-string payload accepted: {faux}"
            );
        }
    }

    /// `includes`, with the strictness of `includes`.
    #[test]
    fn manages_matches_exactly() {
        let etat = ConsoleState::new(
            vec!["anthropic".to_string(), "OpenAI".to_string(), "".to_string()],
            None,
            2,
        );

        assert!(etat.manages("anthropic"));
        assert!(etat.manages("OpenAI"), "the comparison must be case sensitive");
        assert!(etat.manages(""), "an empty entry matches an empty id");
        assert!(!etat.manages("openai"), "case folding crept in");
        assert!(!etat.manages("anthropic "), "trimming crept in");
        assert!(!etat.manages(" anthropic"), "trimming crept in");
        assert!(!etat.manages("Anthropic"));
        assert!(!etat.manages("1"), "no numeric coercion");

        // `includes` is exact, so the empty entry above matches an empty id and
        // nothing else. The reverse needs its own state: this test previously
        // asserted `manages("")` both TRUE and FALSE on the same value, four
        // lines apart. A list with no empty entry is what proves the other half.
        let sans_vide = ConsoleState::new(vec!["anthropic".to_string()], None, 2);
        assert!(
            !sans_vide.manages(""),
            "an empty id matches no non-empty entry"
        );

        // The empty state manages nothing.
        assert!(!EMPTY_CONSOLE_STATE.manages("anthropic"));
        assert!(!EMPTY_CONSOLE_STATE.manages(""));
    }

    /// Multi-byte content, and the reason this module never slices bytes.
    ///
    /// Five em dashes: five characters, fifteen bytes. A `&s[..1]` would land
    /// inside the first character and panic at run time, with no compile error,
    /// which is the failure this test exists to make impossible to reintroduce
    /// unnoticed.
    #[test]
    fn multi_byte_content_never_panics_and_survives_the_round_trip() {
        let tirets = "\u{2014}".repeat(5);
        assert_eq!(tirets.chars().count(), 5);
        assert_eq!(tirets.len(), 15);

        // Byte index 1 is not a character boundary, so the module may never slice
        // there. `char_indices` is the shape the module is allowed to use, and
        // every offset it yields is a boundary.
        assert!(!tirets.is_char_boundary(1));
        for (index, _) in tirets.char_indices() {
            assert!(
                tirets.is_char_boundary(index),
                "char_indices yielded a non-boundary offset: {index}"
            );
        }

        // The safe cut, at every possible length. No panic, no broken character,
        // and the character count is exact.
        for max in 0..=6 {
            let coupe = truncate_sur_frontieres(&tirets, max);
            assert!(tirets.starts_with(coupe));
            assert_eq!(coupe.chars().count(), max.min(5));
            assert!(coupe.is_char_boundary(0));
        }

        // The values that actually flow through this module are byte-transparent.
        let etat = ConsoleState::new(vec![tirets.clone()], Some(tirets.clone()), 5);
        let brut = serde_json::to_string(&etat).unwrap();
        let relu: ConsoleState = serde_json::from_str(&brut).unwrap();
        assert_eq!(relu.console_managed_providers, vec![tirets.clone()]);
        assert_eq!(relu.active_org_name.as_deref(), Some(tirets.as_str()));
        assert_eq!(relu.active_org_name.as_ref().map(String::len), Some(15));
        assert_eq!(relu.active_org_name.as_ref().map(|v| v.chars().count()), Some(5));
        // A prefix is not a match: the list holds five em-dashes, `manages` is
        // `includes`, so two em-dashes is a different string. The message below
        // always said so; the assertion said the opposite.
        assert!(
            !relu.manages("\u{2014}\u{2014}"),
            "an exact prefix is not a match"
        );
        assert!(relu.manages(&tirets));

        // An emoji is four bytes, a combining sequence two: no special case is
        // needed because nothing here looks at bytes at all.
        let melange = "a\u{0301}\u{1F600}\u{2014}";
        assert!(melange.len() > melange.chars().count());
        let etat = ConsoleState::new(vec![melange.to_string()], Some(melange.to_string()), 0);
        let relu: ConsoleState = serde_json::from_str(&serde_json::to_string(&etat).unwrap()).unwrap();
        assert_eq!(relu.active_org_name.as_deref(), Some(melange));
    }
}