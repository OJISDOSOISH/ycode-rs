//! Rust port of `packages/core/src/system-context/builtins.ts`.
//!
//! The source is a `Layer.effectDiscard` that pulls `Location`,
//! `SystemContextRegistry` and `InstructionContext`, renders two context
//! sources (`core/environment` and `core/date`), and registers them. The
//! rendering is pure string formatting over those inputs; the `Layer` assembly
//! and the `registry.register` side effect are out of scope (Rule 7).
//!
//! This port exposes the two render functions so the produced strings are
//! assertable on the wire without an `Effect` runtime, matching the style of
//! `src/core/command.rs` (Rule 5: pure behavior over testable data).
//!
//! # Dependencies not in this batch
//!
//! - `Location` (`../location`) and `SystemContext.Key` are opaque here.

use serde::{Deserialize, Serialize};

/// Opaque alias for the `Location` service inputs used by the environment
/// source. The TS reads `location.directory`, `location.project.directory`
/// and `location.vcs?.type === "git"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentInput {
    pub working_directory: String,
    pub workspace_root_folder: String,
    pub is_directory_a_git_repo: bool,
}

/// `core/environment` source key.
pub const ENVIRONMENT_KEY: &str = "core/environment";

/// Renders the environment context block, mirroring `builtins.ts`.
pub fn render_environment(input: &EnvironmentInput) -> String {
    let git = if input.is_directory_a_git_repo { "yes" } else { "no" };
    [
        "<env>",
        &format!("  Working directory: {}", input.working_directory),
        &format!("  Workspace root folder: {}", input.workspace_root_folder),
        &format!("  Is directory a git repo: {}", git),
        "</env>",
    ]
    .join("\n")
}

/// `core/date` source key.
pub const DATE_KEY: &str = "core/date";

/// Renders the date context baseline, mirroring `builtins.ts`.
pub fn render_date_baseline(date: &str) -> String {
    format!("Here is some useful information about the environment you are running in:\nToday's date: {}", date)
}

/// Renders the date context update, mirroring `builtins.ts`.
pub fn render_date_update(date: &str) -> String {
    format!("The environment you are running in is now:\nToday's date is now: {}", date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_block_matches_source_shape() {
        let input = EnvironmentInput {
            working_directory: "/home/u/.cache".to_string(),
            workspace_root_folder: "/home/u".to_string(),
            is_directory_a_git_repo: true,
        };
        let rendered = render_environment(&input);
        assert!(rendered.contains("Working directory: /home/u/.cache"));
        assert!(rendered.contains("Workspace root folder: /home/u"));
        assert!(rendered.contains("Is directory a git repo: yes"));
        assert!(rendered.starts_with("<env>"));
        assert!(rendered.ends_with("</env>"));
    }

    #[test]
    fn environment_reports_no_when_not_a_git_repo() {
        let input = EnvironmentInput {
            is_directory_a_git_repo: false,
            ..EnvironmentInput {
                working_directory: "/w".to_string(),
                workspace_root_folder: "/w".to_string(),
            }
        };
        assert!(render_environment(&input).contains("Is directory a git repo: no"));
    }

    #[test]
    fn date_blocks_render() {
        assert_eq!(
            render_date_baseline("2025-09-30"),
            "Here is some useful information about the environment you are running in:\nToday's date: 2025-09-30"
        );
        assert_eq!(
            render_date_update("2025-10-01"),
            "The environment you are running in is now:\nToday's date is now: 2025-10-01"
        );
    }
}
