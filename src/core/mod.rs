//! Cœur d'OpenCode : session, historique, outillage.
//!
//! Le portage suit les dependances du TypeScript d'origine : les schemas
//! d'abord, parce que tout le reste les manipule.





















pub mod command;
pub mod config_agent;
pub mod config_mcp;
pub mod config_plugin_agent;
pub mod config_plugin_command;
pub mod config_plugin_external;
pub mod config_plugin_path;
pub mod config_plugin_provider;
pub mod config_plugin_skill;
pub mod config_provider;
pub mod control_plane_move_session;
pub mod copilot_chat;
pub mod copilot_responses;
pub mod credential;
pub mod fs_util;
pub mod git;
pub mod global;
pub mod image;
pub mod integration;
pub mod model;
pub mod observability_logging;
pub mod observability_otlp;
pub mod open;
pub mod permission_saved;
pub mod process;
pub mod provider;
pub mod repository;
pub mod ripgrep;
pub mod run;
pub mod session;
pub mod session_event;
pub mod session_v1;
pub mod skill_discovery;
pub mod skill_guidance;
pub mod snapshot;
pub mod state;
pub mod system_context;
pub mod system_context_builtins;
pub mod system_context_registry;
pub mod util_error;
pub mod websearch;
