//! Port of the portable part of
//! `opencode/packages/core/src/config/plugin/command.ts`.
//!
//! Two pure pieces of the TS plugin, both ported here:
//!
//! 1. `command_name_from_path` - the command name a markdown file gets inside
//!    a directory: relative path, backslashes turned into slashes, the
//!    `command/` or `commands/` prefix removed, the `.md` suffix removed.
//! 2. `apply_command_info` - the draft update the plugin performs, with one
//!    subtlety worth naming: a `variant` is only applied when the item already
//!    has a model, because the variant lives on the model.
//!
//! Not ported: the Effect wiring, the filesystem glob, markdown parsing and
//! the config service. The caller supplies the decoded command infos.
//!
//! `CommandInfo` mirrors `config/command.ts`. The same contract is also ported
//! in `swarm::config_command`; consolidating the two is still open, and this
//! copy is the one the plugin layer depends on.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Model reference as it appears inside a command info.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CommandModel {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// `ConfigCommand.Info`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CommandInfo {
    pub template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<CommandModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask: Option<bool>,
}

/// The mutable command entry the plugin updates.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CommandEntry {
    pub name: String,
    pub template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<CommandModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask: Option<bool>,
}

/// Name derived from a markdown path, as the TS `decode` does.
///
/// `directory` is the base the file was found under; `filepath` the full path.
pub fn command_name_from_path(directory: &str, filepath: &str) -> String {
    super::config_plugin_path::plugin_name(
        directory,
        filepath,
        &super::config_plugin_path::COMMAND_PREFIXES,
    )
}

/// Parsed form of `provider/model`, as `ModelV2.parse` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModelRef {
    pub model_id: String,
    pub provider_id: String,
}

/// `ModelV2.parse`: `fournisseur/modele`, provider first.
pub fn parse_model_ref(raw: &str) -> Option<ParsedModelRef> {
    let (provider, model) = raw.split_once('/')?;
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some(ParsedModelRef { model_id: model.to_string(), provider_id: provider.to_string() })
}

/// The draft update from the TS: template always, the rest when defined.
pub fn apply_command_info(item: &mut CommandEntry, info: &CommandInfo) {
    item.template = info.template.clone();
    if let Some(d) = &info.description {
        item.description = Some(d.clone());
    }
    if let Some(a) = &info.agent {
        item.agent = Some(a.clone());
    }
    if let Some(model) = &info.model {
        if let Some(parsed) = parse_model_ref(model.id.as_str()) {
            let variant = item.model.as_ref().and_then(|m| m.variant.clone());
            item.model = Some(CommandModel {
                id: parsed.model_id,
                provider_id: parsed.provider_id,
                variant,
            });
        }
    }
    // A variant only lands when the item already has a model to carry it.
    if let Some(variant) = &info.variant {
        if let Some(m) = item.model.as_mut() {
            m.variant = Some(variant.clone());
        }
    }
    if let Some(s) = info.subtask {
        item.subtask = Some(s);
    }
}

/// The map the plugin feeds to the draft: name -> info.
pub fn commands_by_name(infos: Vec<(&str, CommandInfo)>) -> BTreeMap<String, CommandInfo> {
    infos.into_iter().map(|(n, i)| (n.to_string(), i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_drops_the_prefix_and_the_suffix() {
        assert_eq!(command_name_from_path("/base", "/base/command/plan.md"), "plan");
        assert_eq!(command_name_from_path("/base", "/base/commands/deep/plan.md"), "deep/plan");
        assert_eq!(command_name_from_path("/base", "/base/plan.md"), "plan");
    }

    #[test]
    fn backslashes_become_slashes() {
        // A backslash is not a separator on a POSIX host, so this pair shares
        // none and Node's `relative` hands the target back whole; the folds then
        // leave `command/` no longer at the start, so nothing is stripped. On
        // Windows the win32 branch of `path.relative` gives "plan" instead -
        // documented divergence, not a bug in the folding itself.
        assert_eq!(
            command_name_from_path("C:\\base", "C:\\base\\command\\plan.md"),
            "C:/base/command/plan.md"
        );
        assert_eq!(command_name_from_path("/base", "/base/commands/a\\b.md"), "a/b");
    }

    #[test]
    fn a_path_outside_the_directory_still_yields_a_name() {
        // `path.relative` really does produce the `..` form, and the TS keeps
        // it: the name of a file found outside the plugin directory is its whole
        // relative path, prefix and all. Inventing a bare suffix here would hide
        // the very thing the caller needs to see - that the file is not where
        // it was expected to be.
        assert_eq!(command_name_from_path("/other", "/base/command/plan.md"), "../base/command/plan.md");
    }

    #[test]
    fn the_model_reference_puts_the_provider_first() {
        let p = parse_model_ref("anthropic/claude-sonnet").unwrap();
        assert_eq!(p.provider_id, "anthropic");
        assert_eq!(p.model_id, "claude-sonnet");
        assert!(parse_model_ref("anthropic").is_none());
        assert!(parse_model_ref("/claude").is_none());
    }

    #[test]
    fn applying_an_info_overwrites_the_template_and_keeps_absent_fields() {
        let mut item = CommandEntry {
            name: "plan".into(),
            template: "old".into(),
            description: Some("kept".into()),
            ..CommandEntry::default()
        };
        let info = CommandInfo { template: "new".into(), ..CommandInfo::default() };
        apply_command_info(&mut item, &info);
        assert_eq!(item.template, "new");
        assert_eq!(item.description, Some("kept".into()), "an absent field does not clear");
    }

    #[test]
    fn a_variant_needs_a_model_to_land_on() {
        let mut item = CommandEntry { name: "p".into(), template: "t".into(), ..CommandEntry::default() };
        let info = CommandInfo {
            template: "t".into(),
            variant: Some("fast".into()),
            ..CommandInfo::default()
        };
        apply_command_info(&mut item, &info);
        assert!(item.model.is_none(), "no model, so no variant either");

        let info = CommandInfo {
            template: "t".into(),
            model: Some(CommandModel {
                id: "anthropic/claude".into(),
                provider_id: String::new(),
                variant: None,
            }),
            variant: Some("fast".into()),
            ..CommandInfo::default()
        };
        apply_command_info(&mut item, &info);
        let m = item.model.expect("model parsed from provider/model");
        assert_eq!(m.id, "claude");
        assert_eq!(m.provider_id, "anthropic");
        assert_eq!(m.variant, Some("fast".to_string()));
    }

    #[test]
    fn an_existing_variant_survives_a_model_refresh() {
        let mut item = CommandEntry {
            name: "p".into(),
            template: "t".into(),
            model: Some(CommandModel {
                id: "old".into(),
                provider_id: "old".into(),
                variant: Some("keep".into()),
            }),
            ..CommandEntry::default()
        };
        let info = CommandInfo {
            template: "t".into(),
            model: Some(CommandModel {
                id: "anthropic/claude".into(),
                provider_id: String::new(),
                variant: None,
            }),
            ..CommandInfo::default()
        };
        apply_command_info(&mut item, &info);
        assert_eq!(item.model.unwrap().variant, Some("keep".to_string()));
    }
}