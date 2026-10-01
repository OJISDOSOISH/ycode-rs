//! Port of the portable part of `opencode/packages/core/src/tool/registry.ts`.
//!
//! The Effect service, the output store and the application-tools bridge are
//! not ported. What is ported is the registry's two decisions, and both of
//! them are silent when they are wrong:
//!
//! - A name can be registered several times, and only the LAST registration
//!   counts. The `local` map holds a stack per name precisely so that a
//!   re-registered tool shadows the previous one instead of colliding with it.
//!   Application-provided tools form the base layer, and local registrations
//!   override them.
//!
//! - A tool disappears from the advertised definitions only when the LAST
//!   matching permission rule both denies and targets `"*"`. `findLast` is the
//!   whole point: a broad `deny *` earlier in the list is overridden by a later
//!   `allow`, so the tool stays. Reading the FIRST match instead would hide a
//!   tool the user has explicitly re-enabled.
//!
//! Two paths resolve a call, and the difference between them is the difference
//! between the two failure strings. The advertised path looks the name up in
//! the frozen `Materialization`, so a tool that was disabled after the model
//! read the definitions reads as `Unknown tool: <name>`, not as stale. The
//! internal path looks it up in the live registry, where a re-registration
//! since the call was emitted reads as `Stale tool call: <name>`. A tool that
//! vanished mid-conversation and a name that never existed are different
//! events, and both texts are model-visible.
//!
//! `Wildcard.match` comes from Kilo's port in `crate::swarm::util_wildcard`;
//! it is not re-implemented here. That path is a layering wrinkle - the TS
//! module belongs to core but the port landed in swarm - and it is noted rather
//! than duplicated.

use std::collections::BTreeMap;

use crate::permission::{Effect as RuleEffect, Rule, Ruleset};

/// A tool as the registry stores it. The TS keeps an opaque `identity` beside
/// it and a scope token; only the identity survives here, because comparing it
/// is what makes a call stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// Opaque identity, compared to detect a stale call.
    pub identity: u64,
    /// The action name this registration is filed under.
    pub action: String,
}

/// The registry: an application layer, and a stack of local registrations per
/// name.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    applications: BTreeMap<String, Registration>,
    local: BTreeMap<String, Vec<Registration>>,
}

impl Registry {
    /// An empty registry with no application tools.
    pub fn new() -> Self {
        Self::default()
    }

    /// `register`: pushes onto the name's stack, newest last.
    ///
    /// Registering an empty set does nothing at all, which is why the TS
    /// returns before validating any name.
    pub fn register(&mut self, tools: &[(String, Registration)]) {
        for (name, registration) in tools {
            self.local.entry(name.clone()).or_default().push(registration.clone());
        }
    }

    /// The last registration for a name, application layer included.
    pub fn resolve(&self, name: &str) -> Option<&Registration> {
        self.local
            .get(name)
            .and_then(|stack| stack.last())
            .or_else(|| self.applications.get(name))
    }

    /// The effective set: applications, then local registrations on top.
    pub fn effective(&self) -> BTreeMap<String, Registration> {
        let mut out = self.applications.clone();
        for (name, stack) in &self.local {
            if let Some(last) = stack.last() {
                out.insert(name.clone(), last.clone());
            }
        }
        out
    }

    /// `materialize`: the effective set minus the wholly disabled tools.
    pub fn materialize(&self, permissions: &Ruleset) -> Materialization {
        let entries: Vec<(String, Registration)> = self
            .effective()
            .into_iter()
            .filter(|(name, registration)| !wholly_disabled(&registration.action, permissions))
            .collect();
        Materialization { entries }
    }

    /// `settleWith`'s identity check, against the LIVE registry.
    ///
    /// This is the internal path; the advertised path is
    /// `Materialization::settle_error`.
    pub fn check_settlement(&self, name: &str, advertised: Option<u64>) -> Option<String> {
        match self.resolve(name) {
            None => Some(unknown_tool(name)),
            Some(registration) => match advertised {
                Some(identity) if identity != registration.identity => Some(stale_tool_call(name)),
                _ => None,
            },
        }
    }
}

/// The advertised set, frozen at one point in time.
///
/// This is the map the TS closes over when building `materialize`, and the
/// `settle` it hands back resolves names THERE - not in the live registry.
/// That is the mechanism behind the `Unknown` text: a name missing from this
/// snapshot is unknown even when the registry still holds it, because the model
/// was never told about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Materialization {
    pub entries: Vec<(String, Registration)>,
}

impl Materialization {
    /// The names advertised, in order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// The advertised path's two checks: was the name advertised, and is the
    /// registration the one the model was shown?
    pub fn settle_error(&self, name: &str, advertised: Option<u64>) -> Option<String> {
        match self.entries.iter().find(|(advertised_name, _)| advertised_name == name) {
            None => Some(unknown_tool(name)),
            Some((_, registration)) => match advertised {
                Some(identity) if identity != registration.identity => Some(stale_tool_call(name)),
                _ => None,
            },
        }
    }
}

/// `Unknown tool: <name>`, verbatim.
pub fn unknown_tool(name: &str) -> String {
    format!("Unknown tool: {}", name)
}

/// `Stale tool call: <name>`, verbatim.
pub fn stale_tool_call(name: &str) -> String {
    format!("Stale tool call: {}", name)
}

/// `whollyDisabled`: the LAST matching rule must be a blanket deny.
///
/// A tool is hidden only when the final rule matching its action both denies
/// and names `"*"` as the resource. Any later `allow` or `ask`, or a later deny
/// scoped to particular resources, keeps the tool visible.
pub fn wholly_disabled(action: &str, rules: &Ruleset) -> bool {
    let rule = rules
        .iter()
        .rev()
        .find(|rule| crate::swarm::util_wildcard::r#match(action, &rule.action));
    match rule {
        Some(rule) => rule.resource == "*" && rule.effect == RuleEffect::Deny,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(identity: u64, action: &str) -> Registration {
        Registration { identity, action: action.to_string() }
    }

    fn rule(action: &str, resource: &str, effect: RuleEffect) -> Rule {
        Rule::new(action, resource, effect)
    }

    #[test]
    fn the_last_registration_of_a_name_wins() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(1, "read"))]);
        registry.register(&[("read".into(), reg(2, "read"))]);
        assert_eq!(registry.resolve("read").unwrap().identity, 2);
    }

    #[test]
    fn a_name_that_was_never_registered_does_not_resolve() {
        assert!(Registry::new().resolve("read").is_none());
    }

    #[test]
    fn registering_nothing_leaves_the_registry_untouched() {
        let mut registry = Registry::new();
        registry.register(&[]);
        assert!(registry.effective().is_empty());
    }

    #[test]
    fn a_blanket_deny_hides_the_tool() {
        let rules: Ruleset = vec![rule("*", "*", RuleEffect::Deny)];
        assert!(wholly_disabled("read", &rules));
    }

    #[test]
    fn a_later_allow_brings_the_tool_back() {
        // The rule that matters is the LAST match, not the first.
        let rules: Ruleset = vec![rule("*", "*", RuleEffect::Deny), rule("read", "*", RuleEffect::Allow)];
        assert!(!wholly_disabled("read", &rules), "findLast, not find");
        assert!(wholly_disabled("write", &rules), "write still matches the deny");
    }

    #[test]
    fn a_deny_scoped_to_resources_does_not_hide_the_tool() {
        let rules: Ruleset = vec![rule("read", "/etc/*", RuleEffect::Deny)];
        assert!(!wholly_disabled("read", &rules), "the resource is not *");
    }

    #[test]
    fn no_matching_rule_never_hides_the_tool() {
        let none: Ruleset = Vec::new();
        assert!(!wholly_disabled("read", &none));
        let other: Ruleset = vec![rule("write", "*", RuleEffect::Deny)];
        assert!(!wholly_disabled("read", &other));
    }

    #[test]
    fn an_ask_does_not_hide_the_tool() {
        let rules: Ruleset = vec![rule("*", "*", RuleEffect::Ask)];
        assert!(!wholly_disabled("read", &rules));
    }

    #[test]
    fn a_disabled_tool_is_dropped_from_the_advertised_set() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(1, "read")), ("write".into(), reg(2, "write"))]);
        let listed = registry.materialize(&[rule("write", "*", RuleEffect::Deny)]);
        assert_eq!(listed.names(), vec!["read"]);
    }

    #[test]
    fn materialize_keeps_every_tool_when_nothing_is_disabled() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(1, "read")), ("write".into(), reg(2, "write"))]);
        assert_eq!(registry.materialize(&[]).names(), vec!["read", "write"]);
    }

    #[test]
    fn an_unknown_name_is_reported_as_unknown() {
        let registry = Registry::new();
        assert_eq!(registry.check_settlement("read", None), Some(unknown_tool("read")));
        assert_eq!(unknown_tool("read"), "Unknown tool: read");
    }

    #[test]
    fn a_replaced_registration_is_reported_as_stale() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(1, "read"))]);
        registry.register(&[("read".into(), reg(2, "read"))]);
        // The model was shown identity 1; the registry now holds 2.
        assert_eq!(registry.check_settlement("read", Some(1)), Some(stale_tool_call("read")));
        assert_eq!(stale_tool_call("read"), "Stale tool call: read");
    }

    #[test]
    fn a_call_matching_the_advertised_registration_proceeds() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(7, "read"))]);
        assert_eq!(registry.check_settlement("read", Some(7)), None);
    }

    #[test]
    fn the_advertised_path_reads_from_the_frozen_set() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(1, "read"))]);
        let frozen = registry.materialize(&[]);
        // Registered again after the definitions went out.
        registry.register(&[("read".into(), reg(2, "read"))]);
        assert_eq!(frozen.settle_error("read", Some(1)), None, "the snapshot still holds identity 1");
        assert_eq!(
            registry.materialize(&[]).settle_error("read", Some(1)),
            Some(stale_tool_call("read")),
            "the fresh snapshot holds identity 2"
        );
    }

    #[test]
    fn a_tool_disabled_after_the_definitions_reads_as_unknown() {
        let mut registry = Registry::new();
        registry.register(&[("read".into(), reg(1, "read"))]);
        let frozen = registry.materialize(&[]);
        let disabled = registry.materialize(&[rule("read", "*", RuleEffect::Deny)]);
        assert!(frozen.settle_error("read", Some(1)).is_none());
        assert_eq!(
            disabled.settle_error("read", Some(1)),
            Some(unknown_tool("read")),
            "it is absent from the advertised set, not stale"
        );
    }

    #[test]
    fn a_name_never_registered_is_unknown_on_both_paths() {
        let registry = Registry::new();
        let frozen = registry.materialize(&[]);
        assert_eq!(frozen.settle_error("read", Some(1)), Some(unknown_tool("read")));
        assert_eq!(registry.check_settlement("read", Some(1)), Some(unknown_tool("read")));
    }
}