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
pub mod account_sql;
pub mod config_command;
pub mod config_compaction;
pub mod config_experimental;
pub mod config_formatter;
pub mod config_lsp;
pub mod config_markdown;
pub mod config_plugin;
pub mod config_reference;
pub mod config_tool_output;
pub mod config_watcher;
pub mod control_plane_workspace_sql;
pub mod copilot_finish_reason;
pub mod copilot_openai_config;
pub mod copilot_response_metadata;
pub mod copilot_responses_settings;
pub mod core_file;
pub mod core_workspace;
pub mod cpk_metadata_extractor;
pub mod credential_sql;
pub mod effect_app_node;
pub mod effect_app_node_builder;
pub mod effect_app_node_platform;
pub mod effect_keyed_mutex;
pub mod effect_memo_map;
pub mod effect_runtime;
pub mod event_sql;
pub mod filesystem_ignore;
pub mod filesystem_protected;
pub mod github_copilot_chat_openai_compatible_chat_options;
pub mod github_copilot_openai_compatible_error;
pub mod id_id;
pub mod installation_version;
pub mod integration_connection;
pub mod location;
pub mod location_service_map;
pub mod markdown_types;
pub mod npm_config;
pub mod observability;
pub mod observability_shared;
pub mod permission_sql;
pub mod plugin_command;
pub mod plugin_agent;
pub mod plugin_provider_llmgateway;
pub mod plugin_skill;
pub mod plugin_variant;
pub mod project_schema;
pub mod project_sql;
pub mod provider_alibaba;
pub mod provider_anthropic;
pub mod provider_cerebras;
pub mod provider_cohere;
pub mod provider_deepinfra;
pub mod provider_dynamic;
pub mod provider_gateway;
pub mod provider_google;
pub mod provider_groq;
pub mod provider_kilo;
pub mod provider_mistral;
pub mod provider_nvidia;
pub mod provider_openai_compatible;
pub mod provider_openrouter;
pub mod provider_perplexity;
pub mod provider_togetherai;
pub mod provider_venice;
pub mod provider_vercel;
pub mod provider_xai;
pub mod provider_zenmux;
pub mod pty_bun;
pub mod pty_node;
pub mod pty_protocol;
pub mod pty_pty;
pub mod pty_schema;
pub mod public_event_manifest;
pub mod session_event;
pub mod session_execution_local;
pub mod session_message_v1;
pub mod session_prompt;
pub mod session_runner_index;
pub mod session_runner_max_steps;
pub mod share_sql;
pub mod tool_application_tools;
pub mod tool_builtins;
pub mod tool_http_body;
pub mod tool_tools;
pub mod util_array;
pub mod util_binary;
pub mod util_encode;
pub mod util_glob;
pub mod util_hash;
pub mod util_identifier;
pub mod util_iife;
pub mod util_lazy;
pub mod util_module;
pub mod util_path;
pub mod util_retry;
pub mod util_slug;
pub mod util_token;
pub mod util_which;
pub mod util_wildcard;
pub mod v1_config_attachment;
pub mod v1_config_console_state;
pub mod v1_config_error;
pub mod v1_config_formatter;
pub mod v1_config_layout;
pub mod v1_config_server;
pub mod v1_config_skills;
pub mod v2_schema;
pub mod models_dev;
pub mod plugin_promise;
pub mod plugin_host;
pub mod plugin_internal;
pub mod provider_google_vertex;
pub mod provider_azure;
pub mod provider_openai;
pub mod provider_github_copilot;
pub mod provider_gitlab;
pub mod provider_amazon_bedrock;
pub mod provider_cloudflare_ai_gateway;
pub mod provider_cloudflare_workers_ai;
pub mod provider_opencode;
pub mod provider_sap_ai_core;
pub mod provider_snowflake_cortex;
pub mod util_effect_flock;
pub mod util_flock;
