//! Une session, son historique, et la fenetre de contexte visible.
//!
//! C'est ici que se joue la limite de contexte : `history` determine quels
//! messages le modele voit reellement, en combinant compaction et epoch.

pub mod compaction;
pub mod context_epoch;
pub mod execution;
pub mod history;
pub mod info;
pub mod max_steps;
pub mod model_route;
pub mod revert;
pub mod run_coordinator;
pub mod schema;
pub mod todo;
pub mod to_llm_message;
pub mod turn;
