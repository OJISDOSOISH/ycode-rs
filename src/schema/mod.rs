//! Wire contracts of the crate: event envelope, locations, and the small
//! schemas that travel on the wire.

pub mod event;
pub mod file_diff;
pub mod filesystem_watcher;
pub mod ide_event;
pub mod installation_event;
pub mod integration_id;
pub mod llm;
pub mod location;
pub mod lsp_event;
pub mod mcp_event;
pub mod project_copy;
pub mod project_directories;
pub mod prompt_input;
pub mod pty_ticket;
pub mod reference;
pub mod server_event;
pub mod session_compaction_event;
pub mod session_delivery;
pub mod session_input;
pub mod session_message;
pub mod session_status_event;
pub mod tui_event;
pub mod v1_legacy_event;
pub mod v1_permission;
pub mod v1_question;
pub mod vcs_event;
pub mod workspace_event;
pub mod worktree_event;
