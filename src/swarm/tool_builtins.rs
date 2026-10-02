//! Port of `packages/core/src/tool/builtins.ts`.
//!
//! The source is forty-eight lines, of which thirteen are code. It holds **no
//! implementation**, no schema, no state and no logic: it is a single object
//! literal fed to `makeLocationNode`, plus the import list that supplies the
//! twelve values it references.
//!
//! ```ts
//! export const node = makeLocationNode({
//!   name: "built-in-tools",
//!   layer: Layer.empty,
//!   deps: [
//!     ApplyPatchTool.node,
//!     BashTool.node,
//!     EditTool.node,
//!     GlobTool.node,
//!     GrepTool.node,
//!     QuestionTool.node,
//!     ReadTool.node,
//!     SkillTool.node,
//!     TodoWriteTool.node,
//!     WebFetchTool.node,
//!     WebSearchTool.node,
//!     WriteTool.node,
//!   ],
//! })
//! ```
//!
//! Everything interesting in this file is therefore **data**, and the four ways
//! a data port goes wrong are the four ways this one could go wrong. All four
//! are pinned:
//!
//! 1. **Order.** The `deps` array is a literal, so its order is fixed and
//!    observable: it is the order of the import block above it, which is sorted
//!    by module path. [`CATALOGUE`] is that array, and
//!    [`catalogue_is_sorted_by_module_path`] proves the sort is a real
//!    invariant rather than a coincidence, so a future reordering is caught
//!    even if the expectation list is updated carelessly.
//! 2. **The enable flag.** Membership in `deps` *is* the enable flag: a tool is
//!    composed into the built-in set exactly when it appears in that array.
//!    [`is_composed`] reads it, and the test
//!    [`a_tool_absent_from_the_deps_array_is_not_composed`] walks the seven
//!    leaves the source explicitly defers in its `TODO` and checks that none of
//!    them is switched on. This is the direction the error takes in practice:
//!    adding `task` or `lsp` to a catalogue that the source deliberately keeps
//!    short would compile silently and change what the model can call.
//! 3. **A name off by one character.** Two spellings per tool travel side by
//!    side and differ by two characters: the node name is `tool/apply-patch`
//!    (slash and hyphen) and the model-facing tool name is `apply_patch`
//!    (underscore). `todowrite` is one word, not `todo_write`. Nothing in the
//!    type system relates the two, and
//!    [`a_tool_name_off_by_one_character_is_not_composed`] pins the
//!    neighbourhood of each.
//! 4. **Duplicates.** [`catalogue_has_no_duplicate_tool_name`] and its node-name
//!    twin. A duplicate is invisible in a positional array and catastrophic in
//!    the registry the tools feed, where the key is the tool name.
//!
//! # Trap 1, the uppercase field names: it does not arise here, and saying so
//! is part of the port
//!
//! The classic trap of this migration is that TypeScript field names carry
//! internal capitals — `projectID`, not `projectId`, `sessionID`, not
//! `sessionId` — and that a snake_case rename is invisible at compile time and
//! only breaks at the JSON boundary. `builtins.ts` has **exactly one** object
//! literal, and its three keys are `name`, `layer` and `deps`, all lowercase,
//! all exactly like that. There is no `ID` anywhere in this contract, so there
//! is nothing to rename and no `#[serde(rename)]` to add.
//!
//! Inventing a capitalised key here would be fabrication. What *is* real, and
//! is pinned in both directions, is the neighbouring hazard: the **values**.
//! [`NodeSpec`] is the one wire shape in this file and it refuses to guess:
//! [`the_serialised_node_spec_pins_key_names_and_dependency_order`] writes the
//! exact keys, and [`a_misspelled_or_snake_cased_field_name_is_rejected_on_read`]
//! proves that `dep`, `deps_`, `Dep` and `DEPS` all fail to read. The strict
//! read is the point: a tolerant read is what turns a typo into a silent
//! no-op.
//!
//! # Trap 2, truthiness against nullity: also absent, also pinned
//!
//! The source contains **no** `?` ternary and **no** `??` coalescent. There is
//! nothing to choose between, and no pair of functions is invented for it.
//!
//! What is nevertheless real is the shape a careless port would give the
//! lookups. A truthiness port filters the catalogue by `Boolean` and answers an
//! empty query with the first entry, because `"" || CATALOGUE[0]` is
//! `CATALOGUE[0]`. A nullity port does neither: `""` is a name nobody has, and
//! the array keeps all twelve entries. This module is the second, and
//! [`an_empty_name_never_falls_back_to_the_first_entry`] fixes the difference so
//! a later edit cannot swap it for the first without a test failing.
//!
//! The same distinction appears one level up, in `makeLocationNode` itself, whose
//! name resolution is `input.service !== undefined ? input.service.key :
//! input.name` — `undefined`, not truthiness, so an empty service key yields an
//! empty node name. That code lives in
//! [`crate::swarm::effect_app_node`] and is not repeated here; this file only
//! supplies a **named** identity, which has no branch at all.
//!
//! # What is not translated, and why
//!
//! 1. **`Layer.empty`.** An Effect `Layer` is a constructed, wired value and has
//!    no Rust equivalent; this is the same choice as
//!    [`crate::swarm::effect_app_node_builder::LayerRef`], and it is the owner's
//!    type, imported and not redeclared. The source's `Layer.empty` becomes the
//!    label [`EMPTY_LAYER`], and [`node`] accepts any layer so the caller can
//!    wire a real one.
//! 2. **The twelve dependency subtrees.** Each `XxxTool.node` is itself a
//!    location layer node carrying its own implementation and its own
//!    dependencies, all declared in a different file that is not part of this
//!    batch. [`dependencies`] therefore builds **unbound** placeholders that
//!    carry the right name and the right tag and nothing else. This is a
//!    deliberate stand-in and it is narrower than the source — a source
//!    dependency is `kind: "layer"`, a placeholder is `kind: "unbound"` — and
//!    [`a_dependency_placeholder_declares_its_own_shallowness`] pins the
//!    stand-in so nobody reads it as a full node.
//! 3. **The `TODO` list.** The source's comment enumerates seven leaves still
//!    to be ported. That is prose, not code, but it is the only statement in the
//!    file about what is deliberately **not** composed, so it is kept as
//!    [`DEFERRED_LEAVES`] with the wording of the source, and a test checks the
//!    two sets are disjoint.
//! 4. **`export * as BuiltInTools from "./builtins"`.** A re-export of the
//!    module's own namespace onto itself. In Rust the module already plays that
//!    role, so nothing is written.
//! 5. **Dynamic MCP and plugin tools.** The comment is explicit that they use
//!    separate scoped registrations and are not in this static list. Nothing is
//!    added for them.
//!
//! # Purity
//!
//! No thread, no lock, no channel, no sleep, no file, no socket, no clock, no
//! loop that can fail to end. Every function is a scan of a twelve-element
//! static array or a construction of an in-memory node, and every test is
//! instantaneous.
//!
//! # Integration note
//!
//! This file expects `pub mod tool_builtins;` in `src/swarm/mod.rs`, which the
//! batch rules forbid me from adding. Until that line exists the module is not
//! compiled and its tests do not run. Reported to the main agent.

use crate::swarm::effect_app_node::{make_location_node, MakeInput};
use crate::swarm::effect_app_node_builder::{AppNode, LayerRef, NodeTag};
use serde::{Deserialize, Serialize};

/// Name of the composed node, verbatim from the source.
///
/// The source writes `name: "built-in-tools"`. It is a free name, not a service
/// key, so it never takes the `input.service.key` branch of `LayerNode.make`.
pub const NODE_NAME: &str = "built-in-tools";

/// Label standing for the source's `Layer.empty`.
///
/// The source passes a real, if empty, Effect layer. Rust has no such value, so
/// the layer is a name supplied by the caller, exactly as
/// `effect_app_node_builder::LayerRef` documents. The literal spelling
/// `Layer.empty` is kept so the provenance of the label is readable.
pub const EMPTY_LAYER: &str = "Layer.empty";

/// Number of built-in tools the source composes.
///
/// Twelve. Not a magic number invented here: it is the length of the `deps`
/// array, and it is pinned against the literal in the tests below.
pub const CATALOGUE_LENGTH: usize = 12;

/// One entry of the built-in catalogue.
///
/// Four strings per tool, and every one of them is a spelling the source
/// fixes. They are related only by the convention of the project, never by the
/// type system, which is why all four are carried together and pinned
/// together.
///
/// - `namespace` is the export namespace each tool file opens with
///   (`export * as BashTool from "./bash"`). It is the binding used in `deps`.
/// - `module` is the import specifier of the same file, `"."` and a slash
///   included.
/// - `name` is the model-facing tool name, the `export const name` of that file.
///   It is the key the tool registers itself under in the tools service, so two
///   entries sharing one would collide at registration time.
/// - `node_name` is the name of the graph node, the `name:` of that file's
///   `makeLocationNode` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuiltInTool {
    /// The binding the `deps` array uses, for example `BashTool`.
    pub namespace: &'static str,
    /// The module the binding comes from, for example `"./bash"`.
    pub module: &'static str,
    /// The name the model calls, for example `"bash"`.
    pub name: &'static str,
    /// The name of the graph node, for example `"tool/bash"`.
    pub node_name: &'static str,
}

/// The catalogue, in the exact order of the `deps` array.
///
/// The order is the one thing about this array that is not recoverable from the
/// set of its members, and it is fixed by the import block above the literal,
/// which is sorted by module path. The sort happens to agree with the sort by
/// tool name on all twelve entries, which is exactly why it is worth pinning:
/// an order that looks natural is the one a reader edits without noticing, and
/// the two sorts would only part company the day a fourteenth tool arrived.
/// [`catalogue_is_sorted_by_module_path`] pins the invariant that really holds,
/// so a reordering cannot pass by accident even if the flat expectation in the
/// tests is edited to match it.
///
/// A `static` and not a `const`, because [`find`] hands out references into it:
/// a `const` is re-materialised at every mention, and a shared table that lends
/// its entries out wants one address, not twelve copies.
pub static CATALOGUE: [BuiltInTool; CATALOGUE_LENGTH] = [
    BuiltInTool {
        namespace: "ApplyPatchTool",
        module: "./apply-patch",
        name: "apply_patch",
        node_name: "tool/apply-patch",
    },
    BuiltInTool {
        namespace: "BashTool",
        module: "./bash",
        name: "bash",
        node_name: "tool/bash",
    },
    BuiltInTool {
        namespace: "EditTool",
        module: "./edit",
        name: "edit",
        node_name: "tool/edit",
    },
    BuiltInTool {
        namespace: "GlobTool",
        module: "./glob",
        name: "glob",
        node_name: "tool/glob",
    },
    BuiltInTool {
        namespace: "GrepTool",
        module: "./grep",
        name: "grep",
        node_name: "tool/grep",
    },
    BuiltInTool {
        namespace: "QuestionTool",
        module: "./question",
        name: "question",
        node_name: "tool/question",
    },
    BuiltInTool {
        namespace: "ReadTool",
        module: "./read",
        name: "read",
        node_name: "tool/read",
    },
    BuiltInTool {
        namespace: "SkillTool",
        module: "./skill",
        name: "skill",
        node_name: "tool/skill",
    },
    BuiltInTool {
        namespace: "TodoWriteTool",
        module: "./todowrite",
        name: "todowrite",
        node_name: "tool/todowrite",
    },
    BuiltInTool {
        namespace: "WebFetchTool",
        module: "./webfetch",
        name: "webfetch",
        node_name: "tool/webfetch",
    },
    BuiltInTool {
        namespace: "WebSearchTool",
        module: "./websearch",
        name: "websearch",
        node_name: "tool/websearch",
    },
    BuiltInTool {
        namespace: "WriteTool",
        module: "./write",
        name: "write",
        node_name: "tool/write",
    },
];

/// The leaves the source explicitly refuses to compose yet.
///
/// Kept with the source's own wording, because these are prose, not
/// identifiers: the comment reads
///
/// ```text
/// TODO: Port the remaining launch-follow-up leaves deliberately: edit fuzzy
/// parity, task, LSP,
/// repo_clone, repo_overview, plan_exit, and Rune/code mode.
/// ```
///
/// The list is here for one reason: a catalogue is where scope creep is
/// invisible. `task` and `LSP` are plausible-looking tools that are
/// deliberately absent, and the test
/// [`a_tool_absent_from_the_deps_array_is_not_composed`] is what stops the next
/// port from adding them "because the list already mentions them".
pub const DEFERRED_LEAVES: [&str; 7] = [
    "edit fuzzy parity",
    "task",
    "LSP",
    "repo_clone",
    "repo_overview",
    "plan_exit",
    "Rune/code mode",
];

/// The `deps` array, as node names, in source order.
///
/// This is the catalogue reduced to the only thing the source actually puts in
/// the array.
pub fn dependency_names() -> [&'static str; CATALOGUE_LENGTH] {
    let mut names = [""; CATALOGUE_LENGTH];
    for (slot, tool) in names.iter_mut().zip(CATALOGUE.iter()) {
        *slot = tool.node_name;
    }
    names
}

/// The namespaces the `deps` array references, in source order.
///
/// The binding names, which is what a reader of the source sees first.
pub fn namespaces() -> [&'static str; CATALOGUE_LENGTH] {
    let mut names = [""; CATALOGUE_LENGTH];
    for (slot, tool) in names.iter_mut().zip(CATALOGUE.iter()) {
        *slot = tool.namespace;
    }
    names
}

/// The model-facing tool names, in source order.
pub fn tool_names() -> [&'static str; CATALOGUE_LENGTH] {
    let mut names = [""; CATALOGUE_LENGTH];
    for (slot, tool) in names.iter_mut().zip(CATALOGUE.iter()) {
        *slot = tool.name;
    }
    names
}

/// The catalogue entry registered under `name`, if there is one.
///
/// The lookup is an **exact match**, which is the nullity reading: a name that
/// the catalogue does not carry is absent, and it is absent in a way that
/// carries no information about any other name. In particular the empty string
/// is not a wildcard, and the match is not case-folded and not separator-folded:
/// `"apply-patch"`, `"ApplyPatch"` and `"apply_patch "` all miss, and
/// [`a_tool_name_off_by_one_character_is_not_composed`] is where that is fixed.
pub fn find(name: &str) -> Option<&'static BuiltInTool> {
    CATALOGUE.iter().find(|tool| tool.name == name)
}

/// The catalogue entry whose graph node is named `node_name`, if there is one.
///
/// Same exact-match rule as [`find`], on the other of the two spellings. A tool
/// name is never accepted here and a node name is never accepted by [`find`]:
/// the two namespaces are disjoint in the source and must stay disjoint here.
pub fn find_node(node_name: &str) -> Option<&'static BuiltInTool> {
    CATALOGUE.iter().find(|tool| tool.node_name == node_name)
}

/// The enable flag: is the tool named `name` composed into the built-in set?
///
/// Composition is membership of the `deps` array, so this is exactly
/// `find(name).is_some()`. There is no second source of truth, no separate
/// boolean to forget to update, and no way to be enabled in one place and
/// absent in the other.
pub fn is_composed(name: &str) -> bool {
    find(name).is_some()
}

/// The twelve dependencies, in source order.
///
/// Each is an **unbound** placeholder carrying the node's name and its
/// `location` tag. The source's real dependency is a location layer node with
/// its own implementation and its own dependencies, all of which live in the
/// tool file and outside this batch. A placeholder is therefore strictly less
/// than the source, and it says so: nothing is invented to fill the gap. See
/// the module documentation, note 2.
pub fn dependencies() -> Vec<AppNode> {
    CATALOGUE
        .iter()
        .map(|tool| AppNode::unbound(tool.node_name, NodeTag::Location))
        .collect()
}

/// The composed node, wired to the caller's layer.
///
/// Equivalent of the whole export: `makeLocationNode({ name, layer, deps })`.
///
/// The identity is a free name, so `LayerNode.make` takes its `input.name`
/// branch and never touches `input.service`. The tag is `location` because the
/// constructor is `makeLocationNode`, and the twelve dependencies arrive in the
/// order of the literal.
pub fn node(implementation: LayerRef) -> AppNode {
    make_location_node(
        MakeInput::named(NODE_NAME, implementation).with_dependencies(dependencies()),
    )
}

/// The composed node with the source's own `Layer.empty`.
///
/// [`node`] is the general form; this is the literal of the source, so a
/// reader comparing the two has nothing left to translate. A `Layer` has no
/// Rust equivalent, so [`EMPTY_LAYER`] is a label, not a value.
pub fn default_node() -> AppNode {
    node(LayerRef::new(EMPTY_LAYER))
}

/// The object literal of the source, as it crosses a wire.
///
/// This is the **only** object literal in `builtins.ts`, and therefore the only
/// set of field names in the file, which is why it is the only serialisable
/// type here. The three keys are `name`, `layer` and `deps`, all lowercase,
/// all exactly that. `BuiltInTool` is deliberately not serialisable: no object
/// literal in the source describes a tool, so there is no key for it and any
/// rename would be invented.
///
/// `deny_unknown_fields` is not a nicety. A tolerant read is precisely what
/// turns `deps` into `dep` without a word: the value comes back as an empty
/// list, the graph loses its twelve edges, and nothing anywhere fails. Unknown
/// fields are therefore an error, and a missing field is an error too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    /// The `name` key: `built-in-tools`.
    pub name: String,
    /// The `layer` key: the caller's layer name, `Layer.empty` by default.
    pub layer: String,
    /// The `deps` key, in source order.
    pub deps: Vec<String>,
}

impl NodeSpec {
    /// The spec of the composed node, with the given layer name.
    ///
    /// The dependency list is [`dependency_names`], in order, and the name is
    /// [`NODE_NAME`].
    pub fn new(layer: impl Into<String>) -> Self {
        NodeSpec {
            name: NODE_NAME.to_string(),
            layer: layer.into(),
            deps: dependency_names().iter().map(|name| name.to_string()).collect(),
        }
    }
}

/// The spec of the source's literal, using the source's own empty layer.
pub fn node_spec() -> NodeSpec {
    NodeSpec::new(EMPTY_LAYER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::effect_app_node_builder::NodeKind;

    /// The catalogue as it must read, entry by entry, in order.
    ///
    /// Written out flat and in full on purpose: a test that iterates the
    /// implementation and compares it to itself proves nothing, and the whole
    /// value of pinning a catalogue is that the expectation is an independent
    /// copy of the source.
    static ATTENDU: [(&str, &str, &str, &str); CATALOGUE_LENGTH] = [
        ("ApplyPatchTool", "./apply-patch", "apply_patch", "tool/apply-patch"),
        ("BashTool", "./bash", "bash", "tool/bash"),
        ("EditTool", "./edit", "edit", "tool/edit"),
        ("GlobTool", "./glob", "glob", "tool/glob"),
        ("GrepTool", "./grep", "grep", "tool/grep"),
        ("QuestionTool", "./question", "question", "tool/question"),
        ("ReadTool", "./read", "read", "tool/read"),
        ("SkillTool", "./skill", "skill", "tool/skill"),
        ("TodoWriteTool", "./todowrite", "todowrite", "tool/todowrite"),
        ("WebFetchTool", "./webfetch", "webfetch", "tool/webfetch"),
        ("WebSearchTool", "./websearch", "websearch", "tool/websearch"),
        ("WriteTool", "./write", "write", "tool/write"),
    ];

    /// Every quartet of a [`BuiltInTool`], in one comparable form.
    fn quarte(entry: &BuiltInTool) -> (&'static str, &'static str, &'static str, &'static str) {
        (entry.namespace, entry.module, entry.name, entry.node_name)
    }

    #[test]
    fn the_catalogue_pins_the_twelve_tools_in_source_order() {
        // The `deps` array, one line at a time. Order is part of the contract.
        assert_eq!(CATALOGUE.len(), 12);
        for (index, attendu) in ATTENDU.iter().enumerate() {
            assert_eq!(
                quarte(&CATALOGUE[index]),
                *attendu,
                "entry {index} of the deps array does not match the source"
            );
        }
    }

    #[test]
    fn catalogue_is_sorted_by_module_path() {
        // The import block above the literal is sorted by module path, and
        // `deps` repeats that order. Proving the sort is a real invariant
        // means a reordering cannot pass by accident, even if the flat
        // expectation above is edited to match it.
        let mut triee: Vec<&str> = CATALOGUE.iter().map(|tool| tool.module).collect();
        triee.sort_unstable();
        let reelle: Vec<&str> = CATALOGUE.iter().map(|tool| tool.module).collect();
        assert_eq!(reelle, triee);
    }

    #[test]
    fn the_namespaces_are_the_bindings_of_the_import_block_in_order() {
        assert_eq!(
            namespaces(),
            [
                "ApplyPatchTool",
                "BashTool",
                "EditTool",
                "GlobTool",
                "GrepTool",
                "QuestionTool",
                "ReadTool",
                "SkillTool",
                "TodoWriteTool",
                "WebFetchTool",
                "WebSearchTool",
                "WriteTool",
            ]
        );
    }

    #[test]
    fn the_dependency_names_are_the_deps_array_in_order() {
        assert_eq!(
            dependency_names(),
            [
                "tool/apply-patch",
                "tool/bash",
                "tool/edit",
                "tool/glob",
                "tool/grep",
                "tool/question",
                "tool/read",
                "tool/skill",
                "tool/todowrite",
                "tool/webfetch",
                "tool/websearch",
                "tool/write",
            ]
        );
    }

    #[test]
    fn the_tool_names_are_the_model_facing_names_in_order() {
        assert_eq!(
            tool_names(),
            [
                "apply_patch", "bash", "edit", "glob", "grep", "question", "read", "skill",
                "todowrite", "webfetch", "websearch", "write",
            ]
        );
    }

    #[test]
    fn catalogue_has_no_duplicate_tool_name() {
        // A duplicate is invisible in a positional array and fatal in the tools
        // service, whose key is the tool name.
        for (index, tool) in CATALOGUE.iter().enumerate() {
            let precedents: Vec<&str> = CATALOGUE[..index]
                .iter()
                .filter(|other| other.name == tool.name)
                .map(|other| other.name)
                .collect();
            assert!(
                precedents.is_empty(),
                "tool name {} is declared twice: {precedents:?} and index {index}",
                tool.name
            );
        }
        // And the count says the same thing: twelve distinct names out of twelve.
        let mut noms = tool_names().to_vec();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(noms.len(), CATALOGUE_LENGTH);
    }

    #[test]
    fn catalogue_has_no_duplicate_node_name() {
        // Same check on the other spelling. Two entries sharing a node name
        // would make the graph ambiguous, and `LayerNode.compile` keys on it.
        for (index, tool) in CATALOGUE.iter().enumerate() {
            let doublon = CATALOGUE[..index]
                .iter()
                .any(|other| other.node_name == tool.node_name);
            assert!(
                !doublon,
                "node name {} is declared twice, at or before index {index}",
                tool.node_name
            );
        }
        let mut noms = dependency_names().to_vec();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(noms.len(), CATALOGUE_LENGTH);
    }

    #[test]
    fn catalogue_has_no_duplicate_namespace_or_module() {
        let mut modules: Vec<&str> = CATALOGUE.iter().map(|tool| tool.module).collect();
        modules.sort_unstable();
        modules.dedup();
        assert_eq!(modules.len(), CATALOGUE_LENGTH, "one module per tool");

        let mut espaces: Vec<&str> = CATALOGUE.iter().map(|tool| tool.namespace).collect();
        espaces.sort_unstable();
        espaces.dedup();
        assert_eq!(espaces.len(), CATALOGUE_LENGTH, "one binding per tool");
    }

    #[test]
    fn a_tool_name_off_by_one_character_is_not_composed() {
        // The two spellings sit one underscore and one hyphen apart, and nothing
        // in the types relates them. Each neighbour below is a plausible typo
        // that a rename or a hand edit would produce, and every one of them
        // must miss.
        let voisins = [
            "apply-patch",  // node name without its prefix
            "applypatch",   // separator dropped
            "apply_patch_", // trailing separator
            "_apply_patch", // leading separator
            "Apply_Patch",  // capitalised
            "todo_write",   // the one that looks like a word boundary
            "todowrite ",
            "web_fetch",
            "webfetchh",
            "read ",
            "writ",
            "grep ",
        ];
        for faux in voisins {
            assert!(find(faux).is_none(), "{faux:?} must not resolve to a tool");
            assert!(!is_composed(faux), "{faux:?} must not be composed");
            assert!(
                find_node(faux).is_none(),
                "{faux:?} must not resolve to a node either"
            );
        }
        // The two that do resolve, with their exact spellings.
        assert_eq!(find("apply_patch").map(|t| t.node_name), Some("tool/apply-patch"));
        assert_eq!(find("todowrite").map(|t| t.node_name), Some("tool/todowrite"));
    }

    #[test]
    fn a_tool_name_is_never_a_node_name_and_the_reverse() {
        // The two namespaces are disjoint in the source and must stay disjoint.
        for tool in CATALOGUE.iter() {
            assert!(find(tool.node_name).is_none(), "{} is not a tool name", tool.node_name);
            assert_eq!(find_node(tool.node_name), Some(tool));
            assert_eq!(find(tool.name), Some(tool));
            assert!(find_node(tool.name).is_none(), "{} is not a node name", tool.name);
        }
    }

    #[test]
    fn the_enable_flag_has_no_second_source_of_truth() {
        for tool in CATALOGUE.iter() {
            assert!(is_composed(tool.name), "{} must be composed", tool.name);
            assert_eq!(find(tool.name).map(|found| found.namespace), Some(tool.namespace));
        }
        // The flag is a projection of the array, so the two cannot disagree.
        let composes: Vec<&str> = tool_names()
            .iter()
            .copied()
            .filter(|name| is_composed(name))
            .collect();
        assert_eq!(composes.len(), CATALOGUE_LENGTH);
    }

    #[test]
    fn a_tool_absent_from_the_deps_array_is_not_composed() {
        // The source's TODO names seven leaves it deliberately does not
        // compose. None of them may appear in the catalogue, in any casing or
        // separator spelling, and this is the test that says so.
        for leaf in DEFERRED_LEAVES {
            assert!(!is_composed(leaf), "{leaf:?} is deferred and must not be composed");
            assert!(find(leaf).is_none(), "{leaf:?} is deferred and must not resolve");
            assert!(find_node(leaf).is_none(), "{leaf:?} must not be a node either");
        }
        // The two leaves that carry an identifier-shaped spelling, checked in
        // the form a hand edit would actually take.
        for faux in ["task", "Task", "lsp", "LSP", "repo_clone", "repoOverview", "planExit"] {
            assert!(!is_composed(faux), "{faux:?} is not a built-in tool");
        }
        assert_eq!(DEFERRED_LEAVES.len(), 7);
    }

    #[test]
    fn an_empty_name_never_falls_back_to_the_first_entry() {
        // Truthiness against nullity, the second trap. A port that filtered or
        // fell back on `Boolean` would answer `""` with `apply_patch`, because
        // `"" || CATALOGUE[0]` is `CATALOGUE[0]`. This one reads absence as
        // absence.
        assert!(find("").is_none(), "the empty name is a name nobody has");
        assert!(!is_composed(""), "the empty name composes nothing");
        assert!(find_node("").is_none());
        // No entry is dropped from the catalogue either: an empty *value* would
        // be filtered by truthiness, and there is none, and there could not be.
        assert_eq!(tool_names().len(), CATALOGUE_LENGTH);
        assert_eq!(namespaces().len(), CATALOGUE_LENGTH);
        assert_eq!(dependency_names().len(), CATALOGUE_LENGTH);
        assert!(CATALOGUE.iter().all(|tool| !tool.name.is_empty()));
        assert!(CATALOGUE.iter().all(|tool| !tool.namespace.is_empty()));
        assert!(CATALOGUE.iter().all(|tool| !tool.module.is_empty()));
        assert!(CATALOGUE.iter().all(|tool| !tool.node_name.is_empty()));
        // A lookup for a name nobody has is None, not a fallback and not a panic.
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn the_composed_node_is_a_location_layer_named_built_in_tools() {
        let node = default_node();

        assert_eq!(node.name(), "built-in-tools");
        assert_eq!(node.name(), NODE_NAME);
        assert_eq!(node.tag(), Some(NodeTag::Location));
        assert_eq!(node.kind(), NodeKind::Layer);
        assert_ne!(node.kind(), NodeKind::Group);
        // `Layer.empty`, carried as a label because an Effect layer has no
        // Rust value.
        assert_eq!(node.implementation().map(LayerRef::name), Some(EMPTY_LAYER));
        assert_eq!(EMPTY_LAYER, "Layer.empty");
    }

    #[test]
    fn the_composed_node_carries_the_twelve_dependencies_in_order() {
        let node = default_node();
        let deps = node.dependencies();

        assert_eq!(deps.len(), CATALOGUE_LENGTH);
        let vus: Vec<&str> = deps.iter().map(AppNode::name).collect();
        assert_eq!(vus, dependency_names().to_vec());
        // And against the source, spelled out.
        assert_eq!(
            vus,
            vec![
                "tool/apply-patch",
                "tool/bash",
                "tool/edit",
                "tool/glob",
                "tool/grep",
                "tool/question",
                "tool/read",
                "tool/skill",
                "tool/todowrite",
                "tool/webfetch",
                "tool/websearch",
                "tool/write",
            ]
        );
        // Every dependency is location-scoped, which is the tag
        // `makeLocationNode` imposes on the node it builds.
        for dep in deps {
            assert_eq!(dep.tag(), Some(NodeTag::Location));
        }
    }

    #[test]
    fn a_dependency_placeholder_declares_its_own_shallowness() {
        // A placeholder is narrower than the source, which builds a real
        // location layer node per tool. Pinned so nobody reads the stand-in as
        // the real subtree: it is unbound, it has no dependencies and it has no
        // implementation.
        for dep in dependencies() {
            assert_eq!(
                dep.kind(),
                NodeKind::Unbound,
                "a placeholder is not the location layer node of the source"
            );
            assert!(dep.dependencies().is_empty());
            assert!(dep.implementation().is_none());
        }
    }

    #[test]
    fn the_layer_is_injected_and_never_hardcoded() {
        // `Layer.empty` is the source's value, not a requirement of the node.
        let fourni = node(LayerRef::new("Layer.provide(couches)"));
        let vide = default_node();

        assert_eq!(fourni.name(), vide.name());
        assert_eq!(fourni.tag(), vide.tag());
        assert_eq!(fourni.dependencies(), vide.dependencies());
        assert_eq!(
            fourni.implementation().map(LayerRef::name),
            Some("Layer.provide(couches)")
        );
        assert_ne!(fourni, vide, "the implementation is the only difference");
        assert_eq!(default_node(), node(LayerRef::new(EMPTY_LAYER)));
    }

    #[test]
    fn the_composed_node_is_deterministic() {
        // The source is a literal, so two constructions are the same value.
        // A port that carried a counter, a cache or a per-call identity would
        // break this, and would break the graph compile above it.
        assert_eq!(default_node(), default_node());
        assert_eq!(dependencies(), dependencies());
        assert_eq!(node_spec(), node_spec());
    }

    #[test]
    fn the_serialised_node_spec_pins_key_names_and_dependency_order() {
        // The write direction of the field-name trap. The three keys are
        // `name`, `layer` and `deps`: all lowercase, no capitals, no
        // underscores, no hyphens, and exactly those.
        let json = serde_json::to_value(node_spec()).expect("serialisation cannot fail");
        let objet = json.as_object().expect("the spec is a JSON object");

        let mut cles: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        cles.sort_unstable();
        assert_eq!(cles, vec!["deps", "layer", "name"], "the key set is exact");

        assert_eq!(objet.get("name"), Some(&serde_json::json!("built-in-tools")));
        assert_eq!(objet.get("layer"), Some(&serde_json::json!("Layer.empty")));
        assert_eq!(
            objet.get("deps"),
            Some(&serde_json::json!([
                "tool/apply-patch",
                "tool/bash",
                "tool/edit",
                "tool/glob",
                "tool/grep",
                "tool/question",
                "tool/read",
                "tool/skill",
                "tool/todowrite",
                "tool/webfetch",
                "tool/websearch",
                "tool/write",
            ])),
            "the array order is part of the value"
        );

        // Nothing anywhere carries a capital, an underscore or a hyphen in a
        // key: the source has no `projectID` to rename, and this is what
        // proves the port did not invent one.
        for cle in cles {
            assert!(!cle.chars().any(|c| c.is_ascii_uppercase()), "unexpected capital in {cle:?}");
            assert!(!cle.contains('_'), "unexpected underscore in {cle:?}");
            assert!(!cle.contains('-'), "unexpected hyphen in {cle:?}");
        }
    }

    #[test]
    fn a_misspelled_or_snake_cased_field_name_is_rejected_on_read() {
        // The read direction. A tolerant read is what turns a typo into a
        // silent no-op: `deps` mistyped as `dep` comes back as an empty graph
        // and nothing anywhere fails. Every one of these must be refused, and
        // the failure must happen while parsing, not in a later use.
        let errones = [
            r#"{"name":"built-in-tools","layer":"Layer.empty","dep":[]}"#,
            r#"{"name":"built-in-tools","layer":"Layer.empty","deps":[],"dep":[]}"#,
            r#"{"name":"built-in-tools","layer":"Layer.empty","deps_":[]}"#,
            r#"{"name":"built-in-tools","layer":"Layer.empty","Dep":[]}"#,
            r#"{"name":"built-in-tools","layer":"Layer.empty","DEPS":[]}"#,
            r#"{"name":"built-in-tools","layer":"Layer.empty","Dependencies":[]}"#,
            r#"{"name":"built-in-tools","layer":"Layer.empty"}"#,
            r#"{"name":"built-in-tools","deps":[]}"#,
            r#"{"layer":"Layer.empty","deps":[]}"#,
            r#"{"Name":"built-in-tools","layer":"Layer.empty","deps":[]}"#,
            r#"{"name":"built-in-tools","Layer":"Layer.empty","deps":[]}"#,
        ];
        for errone in errones {
            let resultat = serde_json::from_str::<NodeSpec>(errone);
            assert!(
                resultat.is_err(),
                "this shape must not read back: {errone}"
            );
        }
    }

    #[test]
    fn the_serialised_spec_round_trips_through_the_strict_reader() {
        // The two directions, joined: what the write test pins is exactly what
        // the read test accepts, and the round trip is the identity.
        let original = node_spec();
        let texte = serde_json::to_string(&original).expect("serialisation cannot fail");
        let relu: NodeSpec = serde_json::from_str(&texte).expect("the round trip must succeed");

        assert_eq!(relu, original);
        assert_eq!(relu.deps.len(), CATALOGUE_LENGTH);
        assert_eq!(relu.deps.first().map(String::as_str), Some("tool/apply-patch"));
        assert_eq!(relu.deps.last().map(String::as_str), Some("tool/write"));
    }

    #[test]
    fn a_misspelled_dependency_value_survives_serialisation_unchanged() {
        // Values are not renamed either. `apply_patch` keeps its underscore on
        // the way out, and `tool/apply-patch` keeps its hyphen, because those
        // are the two spellings the source uses and they are load-bearing.
        let json = serde_json::to_value(node_spec()).expect("serialisation cannot fail");
        let deps = json["deps"].as_array().expect("deps is an array");

        assert_eq!(deps[0], serde_json::json!("tool/apply-patch"));
        assert_eq!(deps[8], serde_json::json!("tool/todowrite"));
        for valeur in deps {
            let texte = valeur.as_str().expect("every dependency is a string");
            assert!(texte.starts_with("tool/"), "unexpected dependency {texte:?}");
        }
        // The underscore belongs to the tool name, the hyphen to the node name,
        // and neither one leaks into the other.
        for tool in CATALOGUE.iter() {
            assert_ne!(tool.name, tool.node_name);
            assert!(!tool.name.contains('/'), "{} has no slash", tool.name);
            assert!(tool.node_name.starts_with("tool/"), "{}", tool.node_name);
        }
    }
}
