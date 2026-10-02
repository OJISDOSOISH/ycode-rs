//! Portage Rust de `packages/core/src/database/sqlite.bun.ts`.
//!
//! ## Ce que contient la source
//!
//! 183 lignes : le client SQLite pour le runtime Bun, construit au-dessus de
//! `bun:sqlite` et expose sous les deux services declares dans `sqlite.ts`
//! (`Sqlite.Native`, `Sqlite.Drizzle`, portes dans `database_sqlite`) :
//!
//! ```ts
//! const nativeLayer = (config: Config) => Layer.effect(Sqlite.Native, Effect.gen(function* () {
//!   const native = new Database(config.filename, {
//!     readonly: config.readonly,
//!     readwrite: config.readwrite ?? true,
//!     create: config.create ?? true,
//!   })
//!   yield* Effect.addFinalizer(() => Effect.sync(() => native.close()))
//!   if (config.disableWAL !== true) native.run("PRAGMA journal_mode = WAL;")
//!   return native
//! }))
//! export const layer = (config: Config) => {
//!   const native = nativeLayer(config)
//!   return Layer.merge(native, Layer.merge(sqliteLayer(config), drizzleLayer).pipe(Layer.provide(native))).pipe(
//!     Layer.provide(Reactivity.layer),
//!   )
//! }
//! ```
//!
//! ## Ce qui n'a pas de traduction ici, et pourquoi
//!
//! - `bun:sqlite`, `drizzle-orm/bun-sqlite`, `effect/unstable/sql/*` : le
//!   pilote et le compilateur de requetes. `Cargo.toml` ne declare aucun crate
//!   SQL : les fonctions `run`, `runValues`, `execute`, `executeStream` ne sont
//!   pas portees comme execution, seulement leurs contrats d'erreur.
//! - `Semaphore.make(1)`, `Layer`, `Scope`, `Fiber`, `Reactivity.layer` :
//!   l'appareil Effect. Porte comme donnees (capacite, ordre des layers).
//! - `transformResultNames`, `transformQueryNames` : des fonctions de
//!   transformation des noms. Sans type fonctionnel cible dans ce crate, seule
//!   leur presence est portee (`avec_transformations`).
//!
//! ## La difference avec le jumeau Node, qui est reelle
//!
//! Ce client Bun applique `PRAGMA journal_mode = WAL` des que `disableWAL`
//! n'est pas `true`, meme en lecture seule, la ou le client Node (`sqlite.node.ts`,
//! porte dans `database_sqlite_node`) s'abstient en lecture seule. [`doit_activer_wal`]
//! fige cet ecart. Bun expose en outre `export` (via `native.serialize()`),
//! que Node n'a pas.

/// Identifiant de type du client Bun, tel que la source l'ecrit.
pub const TYPE_ID: &str = "~@opencode-ai/core/database/SqliteBun";

/// Nom de l'attribut de span qui signe le systeme de base.
pub const ATTR_DB_SYSTEM_NAME: &str = "db.system.name";

/// Valeur de l'attribut de span : le moteur est SQLite des deux cotes.
pub const DB_SYSTEM_NAME: &str = "sqlite";

/// Pragma applique a l'ouverture sauf desactivation explicite.
pub const PRAGMA_WAL: &str = "PRAGMA journal_mode = WAL;";

/// Erreur rendue par `executeStream`, qui n'est pas implemente des deux cotes.
pub const ERREUR_FLUX_NON_IMPLEMENTE: &str = "executeStream not implemented";

/// Nombre de permis du semaphore qui serialize les acces.
pub const SEMAPHORE_PERMIS: usize = 1;

/// Configuration du layer, transcription de `interface Config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Fichier de la base. Seul champ obligatoire.
    pub filename: String,
    /// Passe a `new Database` tel quel (`None` vaut absence d'option).
    pub readonly: Option<bool>,
    /// `None` vaut `true` (`config.create ?? true`).
    pub create: Option<bool>,
    /// `None` vaut `true` (`config.readwrite ?? true`).
    pub readwrite: Option<bool>,
    /// Seul `Some(true)` desactive le WAL.
    pub disable_wal: Option<bool>,
    /// Presence d'attributs de span supplementaires.
    pub avec_attributs_span: bool,
    /// Presence de transformations de noms (requete ou resultat).
    pub avec_transformations: bool,
}

impl Config {
    /// Construit la configuration minimale, comme `layer({ filename })`.
    pub fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
            readonly: None,
            create: None,
            readwrite: None,
            disable_wal: None,
            avec_attributs_span: false,
            avec_transformations: false,
        }
    }

    /// Valeur effective de `create` (`?? true`).
    pub fn create_effectif(&self) -> bool {
        self.create.unwrap_or(true)
    }

    /// Valeur effective de `readwrite` (`?? true`).
    pub fn readwrite_effectif(&self) -> bool {
        self.readwrite.unwrap_or(true)
    }
}

/// Dit si le layer natif applique le pragma WAL a l'ouverture.
///
/// Transcription de `if (config.disableWAL !== true)` : seule la valeur `true`
/// desactive. Contrairement au client Node, la lecture seule ne change rien
/// ici.
pub fn doit_activer_wal(config: &Config) -> bool {
    config.disable_wal != Some(true)
}

/// Les trois couches fusionnees par `layer`, dans l'ordre de la source :
/// natif, client SQL, Drizzle. Toutes recoivent le natif ; l'ensemble recoit
/// en plus la reactivite.
pub const COUCHES: [&str; 3] = ["native", "sqlite", "drizzle"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_type_signe_le_client_bun() {
        assert_eq!(TYPE_ID, "~@opencode-ai/core/database/SqliteBun");
    }

    #[test]
    fn l_attribut_de_span_signe_sqlite() {
        assert_eq!(ATTR_DB_SYSTEM_NAME, "db.system.name");
        assert_eq!(DB_SYSTEM_NAME, "sqlite");
    }

    #[test]
    fn les_options_create_et_readwrite_valent_vrai_par_defaut() {
        let config = Config::new("base.db");
        assert!(config.create_effectif());
        assert!(config.readwrite_effectif());
        assert_eq!(config.readonly, None);
    }

    #[test]
    fn les_options_explicites_traversent_telles_quelles() {
        let mut config = Config::new("base.db");
        config.create = Some(false);
        config.readwrite = Some(false);
        config.readonly = Some(true);
        assert!(!config.create_effectif());
        assert!(!config.readwrite_effectif());
    }

    #[test]
    fn seul_true_desactive_le_wal_meme_en_lecture_seule() {
        let mut config = Config::new("base.db");
        assert!(doit_activer_wal(&config));
        config.disable_wal = Some(false);
        assert!(doit_activer_wal(&config));
        config.disable_wal = Some(true);
        assert!(!doit_activer_wal(&config));
        // Ecart avec le client Node : la lecture seule ne desactive pas ici.
        config.disable_wal = None;
        config.readonly = Some(true);
        assert!(doit_activer_wal(&config));
    }

    #[test]
    fn le_pragma_wal_est_celui_de_la_source() {
        assert_eq!(PRAGMA_WAL, "PRAGMA journal_mode = WAL;");
    }

    #[test]
    fn le_flux_n_est_pas_implemente_comme_dans_la_source() {
        assert_eq!(
            ERREUR_FLUX_NON_IMPLEMENTE,
            "executeStream not implemented"
        );
    }

    #[test]
    fn le_layer_fusionne_trois_couches_sous_un_seul_permis() {
        assert_eq!(COUCHES, ["native", "sqlite", "drizzle"]);
        assert_eq!(SEMAPHORE_PERMIS, 1);
    }
}
