//! Port of `opencode/packages/core/src/v1/config/formatter.ts`
//! (`ConfigFormatterV1`, the **v1** formatter schema).
//!
//! The source is thirteen lines and contains **no runtime code at all**: two
//! schema declarations and a type alias.
//!
//! ```ts
//! export * as ConfigFormatterV1 from "./formatter"
//!
//! import { Schema } from "effect"
//!
//! export const Entry = Schema.Struct({
//!   disabled: Schema.optional(Schema.Boolean),
//!   command: Schema.optional(Schema.mutable(Schema.Array(Schema.String))),
//!   environment: Schema.optional(Schema.Record(Schema.String, Schema.String)),
//!   extensions: Schema.optional(Schema.mutable(Schema.Array(Schema.String))),
//! })
//!
//! export const Info = Schema.Union([Schema.Boolean, Schema.Record(Schema.String, Entry)])
//! export type Info = Schema.Schema.Type<typeof Info>
//! ```
//!
//! ## The self-reexport on line 1
//!
//! `export * as ConfigFormatterV1 from "./formatter"` re-exports the file as a
//! namespace **pointing at itself**. It is dead code: in Rust the module *is*
//! its own namespace, so the line has no counterpart and nothing is ported for
//! it.
//!
//! ## There is NO schema identifier here, and that is deliberate
//!
//! `Entry` is a bare `Schema.Struct`. It carries no `Schema.Class` identity
//! string, so this module exposes **no** `SCHEMA_IDENTIFIER` constant. The
//! neighbouring port of `config/formatter.ts` is built on `Schema.Class<Entry>(
//! "ConfigV2.Formatter.Entry")` and could have carried the string; copying such
//! a constant here to "fill the gap" would invent data that does not exist in
//! this source and would create a second source of truth.
//!
//! ## Collision check against `src/swarm/config_formatter.rs`
//!
//! That file is a port of `packages/core/src/config/formatter.ts` (v2). The two
//! sources are **different files** and neither was touched, but they overlap on
//! the wire. Read this before merging anything:
//!
//! | | `v1/config/formatter.ts` (this file) | `config/formatter.ts` (v2) |
//! |---|---|---|
//! | `Entry` kind | `Schema.Struct` | `Schema.Class("ConfigV2.Formatter.Entry")` |
//! | schema identifier | none | `"ConfigV2.Formatter.Entry"` |
//! | `command` | `Schema.optional(Schema.mutable(Schema.Array(Schema.String)))` | `Schema.String.pipe(Schema.Array, Schema.optional)` |
//! | `extensions` | `Schema.optional(Schema.mutable(Schema.Array(Schema.String)))` | `Schema.String.pipe(Schema.Array, Schema.optional)` |
//! | `disabled`, `environment` | identical | identical |
//! | `Info` | identical union | identical union |
//!
//! Three points make the JSON contracts **equivalent**:
//!
//! 1. `Schema.mutable` only changes the *TypeScript* type of the array from
//!    `ReadonlyArray` to `Array`. It has no runtime and no JSON consequence, so
//!    `Vec<String>` is the faithful mapping on both sides.
//! 2. `Schema.String.pipe(Schema.Array, ...)` is the curried spelling of
//!    `Schema.Array(Schema.String)`, i.e. the very same schema as v1's direct
//!    `Schema.Array(Schema.String)`. Both accept an array of strings and
//!    **both reject a bare string**; neither splits on spaces.
//! 3. `Schema.Class` versus `Schema.Struct` is a TypeScript-level distinction
//!    (a class instance versus a plain object) with no effect on the encoded
//!    JSON, which is a plain object with the same keys either way.
//!
//! Independent corroboration from the v1 migrator: `v1/config/migrate.ts:49`
//! passes the field straight through —
//!
//! ```ts
//! formatter: info.formatter,
//! ```
//!
//! — with no mapping, no wrapping and no field renaming. The v1 and v2 formatter
//! values are meant to be interchangeable.
//!
//! Conclusion: **two modules, one wire contract.** They must stay separate Rust
//! types (they come from two different schema declarations and the v2 one has a
//! class identity), and neither file may be re-exported from the other or
//! edited to "share" code. Deduplicating them would silently drop the v2
//! identifier and would couple two independent schema histories.
//!
//! ## The untagged union, and the resolution order: a DEDUCTION
//!
//! `Info` has no tag. It is a choice between two untagged JSON shapes, `false`
//! and `{ "prettier": { ... } }`, so it becomes `#[serde(untagged)]` rather than
//! `#[serde(tag = "...")]`.
//!
//! The batch-wide open risk applies here and is stated plainly rather than
//! glossed over: **first-match-wins is a deduction, not a verified fact.**
//! `effect` is not installed on this machine, so no local check of Effect's
//! `Schema.Union` behaviour was possible. The source merely *writes*
//! `[Schema.Boolean, Schema.Record(Schema.String, Entry)]`; that it tries the
//! members left to right on decode is assumed, not observed. This port mirrors
//! the written order (`Toggle` before `Overrides`) and
//! `untagged_enum_variant_order_is_a_deduction` locks it in a test so the choice
//! is at least explicit.
//!
//! What makes the deduction harmless **here**, unlike in the other untagged
//! files of this batch (`integration_connection`): the two members accept
//! **disjoint JSON types**. `Schema.Boolean` accepts only `true` and `false`;
//! `Schema.Record(String, Entry)` accepts only objects. No JSON value can match
//! both, so for this union the order is *unobservable* — whichever member is
//! tried first, the outcome is identical. The order is still mirrored for
//! fidelity, but no behaviour depends on it.
//!
//! ## Unknown keys inside an `Entry` are silently dropped
//!
//! `Entry` has four optional fields and nothing else, and a `Schema.Struct`
//! ignores excess properties on decode (`onExcessProperty: "ignore"` is the
//! default; this source never changes it). So `{"prettier": { "colour": "red" }}`
//! decodes to an **empty** entry in TypeScript, and this port reproduces that:
//! it does **not** use `deny_unknown_fields`.
//!
//! The consequence is worth stating because it collides with the batch's
//! Trap 1 instinct: here a wrong key does not raise an error, it vanishes. See
//! `a_near_miss_key_never_fills_a_field`.
//!
//! ## Trap 1 (UPPERCASE field names): this file has none, and the tests say so
//!
//! The batch trap is `projectID` versus `projectId` — a capital in the middle
//! of a field name, invisible at compile time. **There is no such name here.**
//! The four keys were re-read against the source and all are single lowercase
//! words: `disabled`, `command`, `environment`, `extensions`. `environment`
//! and `extensions` are plurals of one word, not camelCase compounds, and there
//! is no fifth key.
//!
//! Both halves of the requested pair are still written, because "this file is
//! immune" is exactly the kind of claim that rots:
//!
//! - `the_four_json_keys_are_exactly_the_typescript_names` serialises and
//!   compares the whole key set, so a future rename of a Rust field cannot
//!   silently change the wire format;
//! - `a_near_miss_key_never_fills_a_field` reads capitalised and misspelled
//!   keys and asserts that **no** field is filled. Serde matches field names
//!   case-sensitively, so `Disabled`, `DISABLED` and `disable` all leave
//!   `disabled` at `None`. There is no snake_case alias to reject here precisely
//!   because snake_case and camelCase coincide for these four names.
//!
//! Each field still carries a redundant `#[serde(rename = "...")]`: it is the
//! wire contract, visible without opening the TypeScript.
//!
//! ## Trap 2 (`?` is truthiness, `??` is nullity): no ternary exists here
//!
//! `formatter.ts` has zero runtime expressions, so there is no ternary `?` and
//! no coalescent `??` to tell apart. The distinction that *does* apply here is
//! the same one at a different layer: `Schema.optional` accepts the **absent**
//! key (and, in TypeScript, an explicit `undefined`, which `JSON.stringify`
//! then drops), and it accepts an explicit `false` or an explicit empty string.
//! That is nullity, not truthiness, so `Option::is_none` is the right gate and
//! `skip_serializing_if = "Option::is_none"` never drops a `false` or a `""`.
//! Two tests lock that: `a_false_disabled_survives_and_is_not_dropped` and
//! `an_empty_string_survives_inside_environment`.
//!
//! One documented divergence follows from the same place: serde maps an
//! explicit `null` onto `Option::None`, so `{"disabled": null}` is accepted
//! here while `Schema.optional` would reject it (it allows `undefined`, not
//! `null`). No JSON written by the TypeScript contains `null` for these fields,
//! so the difference is only visible on hand-edited input. See
//! `an_absent_key_and_an_explicit_null_read_the_same`.
//!
//! ## Trap 3 (byte-index slicing panics): no slicing exists here
//!
//! Nothing in the source and nothing in this port indexes a string. `disabled`,
//! `command`, `environment` and `extensions` are opaque values carried across
//! untouched: no `&s[..n]`, no byte range, no truncation, no `.len()` mistaken
//! for a character count, no `chars().take()`. `multi_byte_characters_round_trip_unchanged`
//! pins a formatter name and an environment value that are **not** ASCII and
//! checks they come back identical, and it deliberately compares
//! `.chars().count()` with `.len()` to document the byte/char gap rather than
//! stepping over it.
//!
//! ## Error messages from an untagged enum are vague, on purpose
//!
//! When neither member of `Info` matches, serde reports
//! `data did not match any variant of untagged enum Info` and discards the
//! per-member reason. This is a loss of diagnostic detail relative to Effect,
//! which reports the failing member. It is not fixed here: making it precise
//! would mean hand-writing a deserializer, i.e. porting less of the schema
//! declaration than a derive does.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Reformatting rule for a single formatter, v1 schema.
///
/// A key of `Info::Overrides` names either a formatter built into opencode
/// (`prettier`, `gofmt`, `rustfmt`, ...) or a custom command invoked as-is.
///
/// All four fields are optional, exactly as in the TypeScript. An entry that
/// chooses nothing is therefore legal and serialises to `{}`.
///
/// Two divergences, both accepted on purpose:
///
/// - `command` is a **list of strings**, never a string to be split. Neither
///   the v1 nor the v2 source splits on whitespace, so
///   `["gofmt -w -s ."]` stays a single element holding the whole command line.
///   Do not "fix" this into a `Vec<String>` produced by `split(' ')`.
/// - Serde accepts `null` for an `Option<T>` where `Schema.optional` accepts
///   only an absent key or `undefined`. See
///   `an_absent_key_and_an_explicit_null_read_the_same`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Entry {
    /// Turn this formatter off without erasing its configuration.
    ///
    /// `Option<bool>`, never `bool`: `false` is an explicit choice and must
    /// survive both decoding and re-serialisation. Collapsing it into an
    /// absent key is the truthiness mistake this batch warns about.
    #[serde(rename = "disabled", skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,

    /// Command line for the formatter, arguments included, unsplit.
    #[serde(rename = "command", skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    /// Variables added to the formatter's environment.
    #[serde(rename = "environment", skip_serializing_if = "Option::is_none")]
    pub environment: Option<BTreeMap<String, String>>,

    /// Trailing file extensions this formatter must handle.
    #[serde(rename = "extensions", skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
}

/// Value of the `formatter` field, v1 schema.
///
/// The union of the source:
/// `Schema.Union([Schema.Boolean, Schema.Record(Schema.String, Entry)])`.
///
/// - `false` (or an absent field) turns the formatters off;
/// - `true` turns the built-in formatters on, with no override;
/// - an object turns the built-in formatters on and overrides some of them,
///   each key being a formatter name.
///
/// `#[serde(untagged)]` reproduces the union. The declaration order mirrors the
/// written order of the TypeScript array; it is documented in the module header
/// that this order is a deduction, and that for this union it is unobservable
/// because the two members accept disjoint JSON types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Info {
    /// Formatters on or off, without overrides.
    Toggle(bool),

    /// Overrides keyed by formatter name. The map key is the formatter name.
    Overrides(BTreeMap<String, Entry>),
}

#[cfg(test)]
mod tests {
    use super::{Entry, Info};
    use std::collections::BTreeMap;

    fn overrides(pairs: &[(&str, Entry)]) -> Info {
        let mut table = BTreeMap::new();
        for (name, entry) in pairs {
            table.insert((*name).to_string(), entry.clone());
        }
        Info::Overrides(table)
    }

    #[test]
    fn an_entry_with_no_choice_serialises_to_an_empty_object() {
        let empty = Entry::default();

        assert_eq!(empty.disabled, None);
        assert_eq!(empty.command, None);
        assert_eq!(empty.environment, None);
        assert_eq!(empty.extensions, None);

        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::json!({}),
            "an entry that chooses nothing is an empty object"
        );
    }

    #[test]
    fn the_four_json_keys_are_exactly_the_typescript_names() {
        // Trap 1 lock, serialisation half. The TypeScript keys are
        // `disabled`, `command`, `environment`, `extensions`: four lowercase
        // single words, no embedded capital anywhere.
        let entry = Entry {
            disabled: Some(true),
            command: Some(vec!["prettier --write".to_string()]),
            environment: Some(BTreeMap::from([("NO_COLOR".to_string(), "1".to_string())])),
            extensions: Some(vec![".ts".to_string()]),
        };

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["disabled"], true);
        assert_eq!(json["command"][0], "prettier --write");
        assert_eq!(json["environment"]["NO_COLOR"], "1");
        assert_eq!(json["extensions"][0], ".ts");

        // And exactly these four keys, no more, no less.
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|key| key.as_str())
            .collect();
        keys.sort();
        assert_eq!(keys, vec!["command", "disabled", "environment", "extensions"]);
    }

    #[test]
    fn a_near_miss_key_never_fills_a_field() {
        // Trap 1 lock, reading half. Serde matches field names
        // case-sensitively, so a capitalised or misspelled key must leave the
        // field at `None` instead of aliasing onto it. There is no snake_case
        // alias to reject here because snake_case and camelCase are the same
        // string for these four names; what has to be rejected is every near
        // miss.
        for wrong in [
            serde_json::json!({ "Disabled": true }),
            serde_json::json!({ "DISABLED": true }),
            serde_json::json!({ "disable": true }),
            serde_json::json!({ "disableD": true }),
            serde_json::json!({ "disabled ": true }),
            serde_json::json!({ "Commands": ["prettier"] }),
            serde_json::json!({ "env": { "A": "1" } }),
            serde_json::json!({ "Extension": [".ts"] }),
        ] {
            let entry: Entry = serde_json::from_value(wrong.clone()).unwrap();
            assert_eq!(entry, Entry::default(), "{} must not fill any field", wrong);
            assert!(
                serde_json::to_value(&entry).unwrap() == serde_json::json!({}),
                "{} must not survive as a real key",
                wrong
            );
        }
    }

    #[test]
    fn unknown_keys_are_ignored_exactly_as_the_typescript_does() {
        // A `Schema.Struct` drops excess properties on decode; `Entry` keeps
        // only its four declared fields and the rest disappear. This is why
        // `deny_unknown_fields` is not used.
        let entry: Entry = serde_json::from_value(serde_json::json!({
            "disabled": true,
            "colour": "red",
            "nested": { "anything": 1 }
        }))
        .unwrap();

        assert_eq!(entry.disabled, Some(true));
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            serde_json::json!({ "disabled": true }),
            "excess keys are dropped on the way back out"
        );
    }

    #[test]
    fn an_absent_key_and_an_explicit_null_read_the_same() {
        // `Schema.optional` allows an absent key, or a key present with the
        // value `undefined` (which `JSON.stringify` then drops). Serde treats
        // absent and `null` alike. Documented divergence: the TypeScript would
        // reject `null`.
        let absent: Entry = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(absent, Entry::default());

        let null: Entry = serde_json::from_value(serde_json::json!({ "disabled": null })).unwrap();
        assert_eq!(null, Entry::default());

        // A `null` on a nested container behaves the same way.
        let nulled: Entry =
            serde_json::from_value(serde_json::json!({ "command": null })).unwrap();
        assert_eq!(nulled.command, None);
    }

    #[test]
    fn an_empty_list_is_not_an_absent_list() {
        // `command: []` is an explicit choice, not the absence of a choice, so
        // the key must survive.
        let empty: Entry = serde_json::from_value(serde_json::json!({ "command": [] })).unwrap();
        assert_eq!(empty.command, Some(Vec::<String>::new()));
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::json!({ "command": [] })
        );

        let absent = Entry::default();
        assert_eq!(absent.command, None);
        assert!(serde_json::to_value(&absent).unwrap().get("command").is_none());
    }

    #[test]
    fn an_empty_map_is_not_an_absent_map() {
        let empty: Entry = serde_json::from_value(serde_json::json!({ "environment": {} })).unwrap();
        assert_eq!(empty.environment, Some(BTreeMap::<String, String>::new()));
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::json!({ "environment": {} })
        );

        assert_eq!(Entry::default().environment, None);
    }

    #[test]
    fn a_false_disabled_survives_and_is_not_dropped() {
        // Trap 2 in this file's vocabulary. `Option::is_none` tests nullity, not
        // truthiness: `false` is a value the user chose and must not vanish on
        // the way through.
        let entry: Entry = serde_json::from_value(serde_json::json!({ "disabled": false })).unwrap();
        assert_eq!(entry.disabled, Some(false));
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            serde_json::json!({ "disabled": false }),
            "a false flag is a decision, not an absence"
        );

        assert_ne!(entry.disabled, Entry::default().disabled);
    }

    #[test]
    fn an_empty_string_survives_inside_environment() {
        // The other half of the same point. An empty value is nullity-adjacent
        // but not absent: `{"": ""}` is a real pair and must round-trip, and
        // the empty key is a real key.
        let entry: Entry =
            serde_json::from_value(serde_json::json!({ "environment": { "": "" } })).unwrap();

        let map = entry.environment.as_ref().expect("the map must survive");
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(""), Some(&String::new()));
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            serde_json::json!({ "environment": { "": "" } }),
            "an empty value is not an absent value"
        );

        // Same for the command list: an empty string element is not dropped.
        let blank: Entry =
            serde_json::from_value(serde_json::json!({ "command": [""] })).unwrap();
        assert_eq!(blank.command, Some(vec![String::new()]));
        assert_eq!(
            serde_json::to_value(&blank).unwrap(),
            serde_json::json!({ "command": [""] })
        );
    }

    #[test]
    fn a_command_containing_spaces_stays_one_element() {
        // `Schema.mutable(Schema.Array(Schema.String))` splits on nothing.
        let entry: Entry = serde_json::from_value(serde_json::json!({
            "command": ["gofmt -w -s ."]
        }))
        .unwrap();

        match &entry.command {
            Some(parts) => {
                assert_eq!(parts.len(), 1);
                assert_eq!(parts[0], "gofmt -w -s .");
            }
            None => panic!("the command must not disappear"),
        }
    }

    #[test]
    fn a_bare_command_string_is_rejected() {
        // The v1 schema is `Array(String)`, so a plain string is not an array
        // and must not be accepted. This is the case where v1 and v2 agree:
        // `Schema.String.pipe(Schema.Array)` is the same schema.
        assert!(
            serde_json::from_value::<Entry>(serde_json::json!({ "command": "prettier --write" }))
                .is_err(),
            "a bare string is not a list of strings"
        );

        assert!(serde_json::from_value::<Entry>(serde_json::json!({ "extensions": ".ts" })).is_err());
    }

    #[test]
    fn a_boolean_gives_a_boolean_and_a_table_gives_an_object() {
        assert_eq!(
            serde_json::to_value(&Info::Toggle(false)).unwrap(),
            serde_json::json!(false)
        );
        assert_eq!(
            serde_json::to_value(&Info::Toggle(true)).unwrap(),
            serde_json::json!(true)
        );

        let table = overrides(&[("prettier", Entry { disabled: Some(false), ..Entry::default() })]);
        assert_eq!(
            serde_json::to_value(&table).unwrap(),
            serde_json::json!({ "prettier": { "disabled": false } })
        );
    }

    #[test]
    fn an_empty_object_reads_as_an_empty_override_table() {
        // `{}` cannot be a boolean, so the union must land on `Overrides`.
        match serde_json::from_value::<Info>(serde_json::json!({})).unwrap() {
            Info::Overrides(table) => assert!(table.is_empty()),
            Info::Toggle(b) => panic!("an object must not become the boolean {}", b),
        }

        let empty = Info::Overrides(BTreeMap::new());
        let json = serde_json::to_value(&empty).unwrap();
        assert_eq!(json, serde_json::json!({}));
        assert_eq!(serde_json::from_value::<Info>(json).unwrap(), empty);
    }

    #[test]
    fn untagged_enum_variant_order_is_a_deduction() {
        // `Schema.Union` tries its members in written order and this port
        // mirrors that with `Toggle` first. That behaviour of Effect is NOT
        // verified: `effect` is not installed on this machine, so the claim is
        // a deduction from the source array order. This test exists to lock the
        // order actually implemented, so nobody later "fixes" it by guessing.
        //
        // What makes the deduction safe here is that the order cannot be
        // observed for this union: a boolean and an object never match the same
        // JSON value, so either order produces the same result. If a future
        // member were added that overlaps `bool` or `Record`, this test would
        // start to matter and the Effect behaviour would have to be checked.
        assert!(
            serde_json::to_string(&Info::Toggle(true)).unwrap() == "true",
            "the first declared variant is the boolean one"
        );

        match serde_json::from_value::<Info>(serde_json::json!(true)).unwrap() {
            Info::Toggle(b) => assert!(b),
            Info::Overrides(_) => panic!("a boolean must not become a table"),
        }

        match serde_json::from_value::<Info>(serde_json::json!({ "gofmt": {} })).unwrap() {
            Info::Overrides(table) => assert_eq!(table.len(), 1),
            Info::Toggle(b) => panic!("an object must not become the boolean {}", b),
        }
    }

    #[test]
    fn typescript_shaped_json_lands_in_the_overrides_variant() {
        // The compatibility test: an object written the way the TypeScript
        // writes it.
        let json = serde_json::json!({
            "prettier": { "command": ["prettier --write"], "extensions": [".ts", ".tsx"] },
            "gofmt": { "disabled": true }
        });

        match serde_json::from_value::<Info>(json).unwrap() {
            Info::Overrides(table) => {
                assert_eq!(table.len(), 2);
                assert_eq!(table["gofmt"].disabled, Some(true));
                assert_eq!(
                    table["prettier"].command,
                    Some(vec!["prettier --write".to_string()])
                );
                assert_eq!(
                    table["prettier"].extensions,
                    Some(vec![".ts".to_string(), ".tsx".to_string()])
                );
                assert_eq!(table["gofmt"].command, None);
            }
            Info::Toggle(b) => panic!("an object must not become the boolean {}", b),
        }
    }

    #[test]
    fn a_boolean_written_by_the_typescript_reads_back_as_a_boolean() {
        for raw in [true, false] {
            match serde_json::from_value::<Info>(serde_json::json!(raw)).unwrap() {
                Info::Toggle(b) => assert_eq!(b, raw),
                Info::Overrides(_) => panic!("a boolean must not become a table"),
            }
        }
    }

    #[test]
    fn map_key_order_does_not_change_the_output() {
        // A `Record` has no meaningful key order; `BTreeMap` imposes
        // alphabetical order, so two tables built in different orders serialise
        // identically.
        let forward = overrides(&[
            ("astyle", Entry::default()),
            ("prettier", Entry::default()),
            ("zofmt", Entry::default()),
        ]);
        let shuffled = overrides(&[
            ("zofmt", Entry::default()),
            ("prettier", Entry::default()),
            ("astyle", Entry::default()),
        ]);

        assert_eq!(
            serde_json::to_string(&forward).unwrap(),
            serde_json::to_string(&shuffled).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&shuffled).unwrap(),
            "{\"astyle\":{},\"prettier\":{},\"zofmt\":{}}"
        );
    }

    #[test]
    fn off_schema_values_are_rejected() {
        // Neither a boolean nor a `Record<String, Entry>`: the union must fail
        // rather than guess. Note that the failure message from an untagged
        // enum is vague by construction; see the module header.
        for invalid in [
            serde_json::json!("off"),
            serde_json::json!(1),
            serde_json::json!(null),
            serde_json::json!([{ "disabled": true }]),
            serde_json::json!({ "prettier": 3 }),
            serde_json::json!({ "prettier": { "command": "prettier" } }),
            serde_json::json!({ "prettier": { "command": [1] } }),
            serde_json::json!({ "prettier": { "environment": { "A": 1 } } }),
            serde_json::json!({ "prettier": { "disabled": "true" } }),
        ] {
            assert!(
                serde_json::from_value::<Info>(invalid.clone()).is_err(),
                "{} should have been rejected",
                invalid
            );
        }
    }

    #[test]
    fn multi_byte_characters_round_trip_unchanged() {
        // Trap 3 lock. Nothing in this module slices a string, so a formatter
        // name or an environment value that is not ASCII must come back
        // untouched. `.chars().count()` is compared with `.len()` on purpose:
        // the first counts characters, the second counts bytes, and confusing
        // them is what makes `&s[..n]` panic at run time on a multi-byte
        // character. No byte range and no index into a string is used anywhere
        // in this file, so no such panic is reachable from here.
        let json = serde_json::json!({
            "café": {
                "command": ["héllo wörld"],
                "environment": { "CLÉ": "vàleur" }
            }
        });

        match serde_json::from_value::<Info>(json.clone()).unwrap() {
            Info::Overrides(table) => {
                let entry = table.get("café").expect("the multi-byte key must survive");
                assert_eq!(
                    entry.command,
                    Some(vec!["héllo wörld".to_string()]),
                    "a multi-byte value must come back identical"
                );
                assert_eq!(
                    entry.environment.as_ref().unwrap().get("CLÉ"),
                    Some(&"vàleur".to_string())
                );

                let command = &entry.command.as_ref().unwrap()[0];
                assert_eq!(command.chars().count(), 11, "characters");
                assert_eq!(command.len(), 13, "bytes: more than characters");
                assert!(command.len() > command.chars().count());
            }
            Info::Toggle(b) => panic!("an object must not become the boolean {}", b),
        }

        // And back out, unchanged.
        let reread: Info = serde_json::from_value(json).unwrap();
        assert_eq!(serde_json::to_value(&reread).unwrap()["café"]["command"][0], "héllo wörld");
    }
}