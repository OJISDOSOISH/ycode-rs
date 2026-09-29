//! Une session, son historique, et la fenetre de contexte visible.
//!
//! C'est ici que se joue la limite de contexte : `history` determine quels
//! messages le modele voit reellement, en combinant compaction et epoch.

pub mod compaction;
pub mod context_epoch;
pub mod history;
pub mod max_steps;
pub mod revert;
pub mod todo;
pub mod turn;
