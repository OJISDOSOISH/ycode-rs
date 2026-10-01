//! Cœur d'OpenCode : session, historique, outillage.
//!
//! Le portage suit les dependances du TypeScript d'origine : les schemas
//! d'abord, parce que tout le reste les manipule.

pub mod run;
pub mod session;

pub mod command;
pub mod config_agent;
pub mod config_mcp;
pub mod config_plugin_agent;
pub mod config_plugin_command;
pub mod config_plugin_external;
pub mod config_plugin_provider;
pub mod config_plugin_skill;
pub mod config_provider;
pub mod copilot_chat;
pub mod copilot_responses;
pub mod credential;
pub mod git;
pub mod global;
pub mod image;
pub mod integration;
pub mod model;
pub mod open;
pub mod provider;
pub mod ripgrep;
pub mod session_event;
pub mod session_v1;
pub mod snapshot;
pub mod websearch;

pub mod process;

pub mod util_error;
