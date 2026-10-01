//! Port of the portable part of `opencode/packages/core/src/tool/builtins.ts`.
//!
//! That file is a list: twelve Location-scoped tool nodes composed into one
//! node. The graph itself is Effect and is not ported, but the LIST is the
//! contract - it is what decides which tools exist before any provider or model
//! filtering, and the count and the spelling are both load-bearing. A name here
//! is the string a model sends back, so a typo does not fail a build: it fails
//! at runtime, as an unknown tool.
//!
//! Two deliberate omissions are carried over from the source rather than fixed:
//! dynamic MCP and plugin tools register separately and are NOT in this list,
//! and provider/model filtering belongs to a later materialization phase rather
//! than here. A test asserts the list is exactly twelve and that every name
//! passes the registration gate from `tool_tool`, so the two cannot drift apart
//! silently.

/// The twelve built-in tools, in the order `builtins.ts` lists them.
pub const BUILT_INS: [&str; 12] = [
    "apply_patch", "bash", "edit", "glob", "grep", "question", "read", "skill", "todowrite", "webfetch",
    "websearch", "write",
];

/// `BuiltInTools.node.name`, the name the composed node is registered under.
pub const NODE_NAME: &str = "built-in-tools";

/// Whether `name` is one of the built-ins.
pub fn is_built_in(name: &str) -> bool {
    BUILT_INS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tool_tool;

    #[test]
    fn the_list_holds_twelve_tools() {
        assert_eq!(BUILT_INS.len(), 12);
    }

    #[test]
    fn every_name_passes_the_registration_gate() {
        // The two files can only agree if the names are valid; a mismatch here
        // means a tool cannot register at runtime.
        for name in BUILT_INS {
            assert!(tool_tool::is_valid_name(name), "{} would be refused at registration", name);
        }
    }

    #[test]
    fn no_name_is_listed_twice() {
        let mut seen: Vec<&str> = Vec::new();
        for name in BUILT_INS {
            assert!(!seen.contains(&name), "{} appears twice", name);
            seen.push(name);
        }
    }

    #[test]
    fn the_shipped_tools_are_all_present() {
        for name in [
            "apply_patch", "bash", "edit", "glob", "grep", "question", "read", "skill", "todowrite",
            "webfetch", "websearch", "write",
        ] {
            assert!(is_built_in(name), "{} is missing", name);
        }
    }

    #[test]
    fn apply_patch_keeps_its_underscore() {
        // The node is `apply-patch.ts`, the tool name is `apply_patch`. The file
        // name and the wire name differ, and only the latter is a tool name.
        assert!(is_built_in("apply_patch"));
        assert!(!is_built_in("apply-patch"), "the file name is not the tool name");
    }

    #[test]
    fn mcp_and_plugin_tools_are_not_in_the_list() {
        // They register through their own scoped path, by design.
        assert!(!is_built_in("mcp__anything"));
        assert!(!is_built_in("plugin_anything"));
    }

    #[test]
    fn an_unknown_name_is_not_a_built_in() {
        assert!(!is_built_in("task"));
        assert!(!is_built_in("lsp"));
        assert!(!is_built_in(""));
    }

    #[test]
    fn the_composed_node_keeps_its_name() {
        assert_eq!(NODE_NAME, "built-in-tools");
    }
}