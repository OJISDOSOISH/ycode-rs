//! Port of the portable part of
//! `opencode/packages/core/src/config/plugin/agent.ts`.
//!
//! This plugin builds agents out of markdown files found in config
//! directories. The filesystem walk, the markdown parse and the Effect wiring
//! are not ported; the decisions they feed are, and they are the parts worth
//! locking down:
//!
//! - `expand_home` has five cases, and `expand_permissions` applies it only to
//!   the three actions whose resources are filesystem paths. A bash resource
//!   is raw shell text: rewriting `$HOME/private/**` there would miss
//!   `$HOME/private/key`, and doing it safely needs a shell parser. That
//!   asymmetry is a deliberate rule in the TS, not an oversight.
//! - the frontmatter key set decides whether a document is read as a current
//!   agent or as a legacy one: ANY key outside the set means legacy.
//! - the name of an agent file drops the `agent/`, `agents/`, `mode/` or
//!   `modes/` prefix and the `.md` suffix, after relative path and slash
//!   normalisation.
//! - an agent's model is `{id, providerID}` parsed from `provider/model`, the
//!   variant only lands when a model is already there, and request maps merge
//!   key by key.

use crate::core::config_agent::{Color, Mode};
use crate::core::config_provider::Request;
use crate::permission::{Effect, Rule, Ruleset};

/// Actions whose resource is a filesystem path, and may therefore be expanded.
pub const PATH_ACTIONS: [&str; 3] = ["external_directory", "read", "edit"];

/// Frontmatter keys the current agent schema recognises.
pub const AGENT_KEYS: [&str; 11] = [
    "model", "variant", "request", "system", "description", "mode", "hidden", "color", "steps",
    "disabled", "permissions",
];

/// `isPathAction`.
pub fn is_path_action(action: &str) -> bool {
    PATH_ACTIONS.contains(&action)
}

/// `expandHome`: `~/x`, `~`, `$HOME`, `$HOME/x`, `$HOME\x` become the home
/// directory; everything else is returned untouched.
/// `expandHome` from the TS, case for case.
///
/// The slice offsets are the source's and they matter: `~/x` is `home` plus
/// `resource.slice(1)`, which KEEPS the slash, so the result is `/h/a` and not
/// `/ha`. Getting that wrong is invisible in a test that only checks a bare
/// `~`, and obvious everywhere else.
///
/// The `$HOME\\` case keeps the backslash for the same reason - `slice(5)` of
/// `$HOME\\a` is `\\a` - so the result is `/h\\a` and not `/h/a`. The source
/// does not normalise the separator there, and neither does this.
pub fn expand_home(resource: &str, home: &str) -> String {
    if resource.starts_with("~/") {
        return format!("{}{}", home, &resource[1..]);
    }
    if resource == "~" {
        return home.to_string();
    }
    if resource == "$HOME" {
        return home.to_string();
    }
    if resource.starts_with("$HOME/") || resource.starts_with("$HOME\\") {
        return format!("{}{}", home, &resource[5..]);
    }
    resource.to_string()
}

/// `expandPermissions`: expansion applies to path actions only.
pub fn expand_permissions(rules: &Ruleset, home: &str) -> Ruleset {
    rules
        .iter()
        .map(|rule| {
            if is_path_action(&rule.action) {
                Rule {
                    action: rule.action.clone(),
                    resource: expand_home(&rule.resource, home),
                    effect: match rule.effect {
                        Effect::Allow => Effect::Allow,
                        Effect::Ask => Effect::Ask,
                        Effect::Deny => Effect::Deny,
                    },
                }
            } else {
                rule.clone()
            }
        })
        .collect()
}

/// Name of an agent file, as the TS `decode` computes it.
/// The agent name a discovered file yields.
///
/// `path.relative`, slashes, then one of the four prefixes and the `.md`
/// suffix - see `super::config_plugin_path` for why `relative` cannot be a
/// `strip_prefix` here.
pub fn agent_name_from_path(directory: &str, filepath: &str) -> String {
    super::config_plugin_path::plugin_name(directory, filepath, &super::config_plugin_path::AGENT_PREFIXES)
}

/// A document is legacy as soon as ONE frontmatter key is unknown.
pub fn is_legacy_document(keys: &[&str]) -> bool {
    keys.iter().any(|k| !AGENT_KEYS.contains(k))
}

/// Model reference carried by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentModelRef {
    pub id: String,
    pub provider_id: String,
    pub variant: Option<String>,
}

/// The mutable agent the plugin updates.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentDraft {
    pub model: Option<AgentModelRef>,
    pub request: Request,
    pub system: Option<String>,
    pub description: Option<String>,
    pub mode: Option<Mode>,
    pub hidden: Option<bool>,
    pub color: Option<Color>,
    pub steps: Option<i64>,
    pub permissions: Ruleset,
}

/// The config side of an agent, from `config/agent.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentConfig {
    pub model: Option<String>,
    pub variant: Option<String>,
    pub request: Option<Request>,
    pub system: Option<String>,
    pub description: Option<String>,
    pub mode: Option<Mode>,
    pub hidden: Option<bool>,
    pub color: Option<Color>,
    pub steps: Option<i64>,
    pub disabled: Option<bool>,
    pub permissions: Option<Ruleset>,
}

/// One plugin update: the TS `draft.update` body, without the draft.
pub fn apply_agent_config(agent: &mut AgentDraft, item: &AgentConfig, home: &str) {
    if let Some(raw) = &item.model {
        if let Some((provider, model)) = raw.split_once('/') {
            if !provider.is_empty() && !model.is_empty() {
                let variant = agent.model.as_ref().and_then(|m| m.variant.clone());
                agent.model = Some(AgentModelRef {
                    id: model.to_string(),
                    provider_id: provider.to_string(),
                    variant,
                });
            }
        }
    }
    if let Some(variant) = &item.variant {
        if let Some(m) = agent.model.as_mut() {
            m.variant = Some(variant.clone());
        }
    }
    if let Some(patch) = &item.request {
        if let Some(h) = &patch.headers {
            agent.request.headers.get_or_insert_with(Default::default).extend(h.clone());
        }
        if let Some(b) = &patch.body {
            agent.request.body.get_or_insert_with(Default::default).extend(b.clone());
        }
    }
    if let Some(v) = &item.system {
        agent.system = Some(v.clone());
    }
    if let Some(v) = &item.description {
        agent.description = Some(v.clone());
    }
    if let Some(v) = item.mode {
        agent.mode = Some(v);
    }
    if let Some(v) = item.hidden {
        agent.hidden = Some(v);
    }
    if let Some(v) = &item.color {
        agent.color = Some(v.clone());
    }
    if let Some(v) = item.steps {
        agent.steps = Some(v);
    }
    if let Some(rules) = &item.permissions {
        agent.permissions.extend(expand_permissions(rules, home));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn expand_home_covers_the_five_cases_of_the_ts() {
        assert_eq!(expand_home("~/a", "/h"), "/h/a");
        assert_eq!(expand_home("~", "/h"), "/h");
        assert_eq!(expand_home("$HOME", "/h"), "/h");
        assert_eq!(expand_home("$HOME/a", "/h"), "/h/a");
        assert_eq!(expand_home("$HOME\\a", "/h"), "/h\\a", "the backslash is kept, as in the source");
        assert_eq!(expand_home("~other/a", "/h"), "~other/a", "only ~/ and ~ are special");
        assert_eq!(expand_home("/abs", "/h"), "/abs");
        assert_eq!(expand_home("src/**", "/h"), "src/**");
        assert_eq!(expand_home("$OTHER/a", "/h"), "$OTHER/a");
    }

    #[test]
    fn only_path_actions_get_their_resources_expanded() {
        let rules: Ruleset = vec![
            Rule::new("read", "~/a", Effect::Allow),
            Rule::new("edit", "$HOME/b", Effect::Allow),
            Rule::new("external_directory", "~", Effect::Ask),
            Rule::new("bash", "$HOME/private/key", Effect::Deny),
        ];
        let out = expand_permissions(&rules, "/h");
        assert_eq!(out[0].resource, "/h/a");
        assert_eq!(out[1].resource, "/h/b");
        assert_eq!(out[2].resource, "/h");
        assert_eq!(out[3].resource, "$HOME/private/key", "a bash resource is raw shell text");
        assert_eq!(out[3].effect, Effect::Deny, "the effect is carried over");
    }

    #[test]
    fn the_path_action_list_is_exactly_three() {
        assert_eq!(PATH_ACTIONS.len(), 3);
        assert!(is_path_action("read"));
        assert!(!is_path_action("bash"));
        assert!(!is_path_action("webfetch"));
    }

    #[test]
    fn an_agent_name_drops_any_of_the_four_prefixes() {
        assert_eq!(agent_name_from_path("/base", "/base/agent/plan.md"), "plan");
        assert_eq!(agent_name_from_path("/base", "/base/agents/plan.md"), "plan");
        assert_eq!(agent_name_from_path("/base", "/base/mode/build.md"), "build");
        assert_eq!(agent_name_from_path("/base", "/base/modes/build.md"), "build");
        assert_eq!(agent_name_from_path("/base", "/base/agents/deep/plan.md"), "deep/plan");
    }

    #[test]
    fn a_single_unknown_key_makes_the_document_legacy() {
        assert!(!is_legacy_document(&["model", "system", "steps"]));
        assert!(is_legacy_document(&["model", "temperature"]), "temperature is v1");
        assert!(is_legacy_document(&["prompt"]), "prompt is a v1 key");
        assert_eq!(AGENT_KEYS.len(), 11);
    }

    #[test]
    fn a_variant_only_lands_when_a_model_is_already_there() {
        let mut agent = AgentDraft::default();
        apply_agent_config(
            &mut agent,
            &AgentConfig { variant: Some("fast".into()), ..AgentConfig::default() },
            "/h",
        );
        assert!(agent.model.is_none());

        apply_agent_config(
            &mut agent,
            &AgentConfig {
                model: Some("anthropic/claude".into()),
                variant: Some("fast".into()),
                ..AgentConfig::default()
            },
            "/h",
        );
        let m = agent.model.unwrap();
        assert_eq!(m.provider_id, "anthropic");
        assert_eq!(m.id, "claude");
        assert_eq!(m.variant, Some("fast".to_string()));
    }

    #[test]
    fn a_request_patch_keeps_the_headers_it_does_not_mention() {
        let mut agent = AgentDraft {
            request: Request {
                headers: Some(BTreeMap::from([("a".into(), "1".into())])),
                body: None,
            },
            ..AgentDraft::default()
        };
        apply_agent_config(
            &mut agent,
            &AgentConfig {
                request: Some(Request {
                    headers: Some(BTreeMap::from([("b".into(), "2".into())])),
                    body: None,
                }),
                ..AgentConfig::default()
            },
            "/h",
        );
        let h = agent.request.headers.unwrap();
        assert_eq!(h["a"], "1");
        assert_eq!(h["b"], "2");
    }

    #[test]
    fn a_malformed_model_reference_is_ignored_rather_than_half_applied() {
        let mut agent = AgentDraft::default();
        for raw in ["nope", "/claude", "anthropic/"] {
            apply_agent_config(
                &mut agent,
                &AgentConfig { model: Some(raw.into()), ..AgentConfig::default() },
                "/h",
            );
            assert!(agent.model.is_none(), "model {:?} should not have been set", raw);
        }
    }

    #[test]
    fn permissions_from_the_entry_are_expanded_and_appended() {
        let mut agent = AgentDraft { permissions: vec![Rule::new("bash", "*", Effect::Ask)], ..AgentDraft::default() };
        apply_agent_config(
            &mut agent,
            &AgentConfig {
                permissions: Some(vec![Rule::new("read", "~/src", Effect::Allow)]),
                ..AgentConfig::default()
            },
            "/h",
        );
        assert_eq!(agent.permissions.len(), 2, "the existing rule is kept");
        assert_eq!(agent.permissions[1].resource, "/h/src");
    }
}