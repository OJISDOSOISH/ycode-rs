//! Lot de portage swarm : fichiers TypeScript traduits en parallele, par vagues.
//!
//! Ce module est un CONTENEUR, pas une hierarchie de conception. Chaque fichier
//! correspond a un module TypeScript independant et ne parle a aucun autre. Le
//! nom de fichier reprend le nom de module, prefixe par son domaine, avec TOUT
//! caractere qui n est pas [a-z0-9] remplace par un souligne : un nom de module
//! Rust ne peut contenir ni tiret ni majuscule, et l'oubli s paye cher.
//!
//! La declaration est ecrite par l agent principal, pas par les sous-agents :
//! chacun ne touche qu a son propre fichier.
//!
//! REGLE D ORGANISATION : le swarm travaille par equipes de 5. Une equipe porte
//! 5 fichiers, un agent chacun. L equipe suivante ne demarre que lorsque la
//! precedente a livre, ce qui garantit que mod.rs est toujours a jour et que la
//! CI voit toujours un etat coherent.
//!
//! Seule verification : GitHub Actions. La compilation est interdite sur le poste
//! de travail. Les rapports sont dans reponses/swarm-*.md.
pub mod config_command;
pub mod config_compaction;
pub mod config_experimental;
pub mod config_formatter;
pub mod config_lsp;
pub mod config_plugin;
pub mod config_reference;
pub mod config_tool_output;
pub mod config_watcher;
pub mod copilot_finish_reason;
pub mod core_file;
pub mod core_workspace;
pub mod effect_app_node_builder;
pub mod effect_app_node_platform;
pub mod effect_memo_map;
pub mod event_sql;
pub mod github_copilot_chat_openai_compatible_chat_options;
pub mod github_copilot_openai_compatible_error;
pub mod installation_version;
pub mod integration_connection;
pub mod location;
pub mod location_service_map;
pub mod observability;
pub mod observability_shared;
pub mod permission_sql;
pub mod plugin_command;
pub mod plugin_provider_llmgateway;
pub mod project_schema;
pub mod provider_alibaba;
pub mod provider_anthropic;
pub mod provider_cerebras;
pub mod provider_cohere;
pub mod provider_deepinfra;
pub mod provider_gateway;
pub mod provider_google;
pub mod provider_groq;
pub mod provider_kilo;
pub mod provider_mistral;
pub mod provider_nvidia;
pub mod provider_openai_compatible;
pub mod provider_perplexity;
pub mod provider_togetherai;
pub mod provider_venice;
pub mod provider_vercel;
pub mod provider_xai;
pub mod provider_zenmux;
pub mod pty_pty;
pub mod pty_schema;
pub mod public_event_manifest;
pub mod session_runner_index;
pub mod tool_tools;
pub mod util_array;
pub mod util_binary;
pub mod util_glob;
pub mod util_hash;
pub mod util_identifier;
pub mod util_iife;
pub mod util_lazy;
pub mod util_slug;
pub mod util_token;
pub mod util_wildcard;
pub mod v1_config_error;
pub mod v1_config_server;
