//! Ycode : reecriture en Rust d'OpenCode.
//!
//! Le portage suit l'ordre de dependance du projet TypeScript d'origine :
//! d'abord les schemas, parce que le session, les outils et le serveur les
//! manipulent tous.

pub mod schema;

pub mod llm;
pub mod tool;
