//! Portage de `core/src/plugin/internal.ts`.
//!
//! AUCUNE donnée sérialisable : le fichier TypeScript original est du
//! câblage Effect-TS pur (composition de `Layer`, injection des services
//! `Catalog`, `CommandV2`, `Config`, `Location`, `ModelsDev`, `Npm`,
//! `EventV2`, `FSUtil`, `FileSystem`, `Global`, `HttpClient`, `SkillV2`,
//! `Reference`, puis enregistrement ordonné des plugins internes dans
//! `PluginV2.Service` via `State.batch` forké, span `PluginInternal.boot`).
//!
//! En Rust, ce rôle est tenu par la construction du runtime / du graphe de
//! services (voir `effect_runtime.rs` et `effect_app_node.rs`). Il n'existe
//! ici aucun contrat JSON : toute struct serde inventée serait fausse.
//!
//! Points de repère du TS, pour un futur portage effectif :
//! - `Plugin<R>` : { id: string, effect: (ctx) => Effect<void, never, R | Scope> }
//! - `define()` : fonction identité (aucun comportement).
//! - Ordre de boot des plugins internes : ConfigReference, Agent, Command,
//!   Skill, ModelsDev, ConfigAgent, ConfigCommand, ConfigSkill,
//!   ProviderPlugins (itération), ConfigExternal, ConfigProvider, Variant.
