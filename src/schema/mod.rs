//! Wire contracts of the crate: event envelope, locations, and the small
//! schemas that travel on the wire.

pub mod event;
pub mod file_diff;
pub mod filesystem_watcher;
pub mod ide_event;
pub mod location;
pub mod lsp_event;
pub mod project_directories;
pub mod pty_ticket;
pub mod server_event;
pub mod session_compaction_event;
pub mod session_delivery;
pub mod session_message;
pub mod vcs_event;