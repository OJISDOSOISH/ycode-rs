//! Portage Rust de `packages/core/src/database/sqlite.node.ts`.
//!
//! ## Ce que contient la source
//!
//! 178 lignes : le client SQLite pour le runtime Node, jumeau de
//! `sqlite.bun.ts` (porte dans `database_sqlite_bun`) au-dessus de
//! `node:sqlite` au lieu de `bun:sqlite` :
//!
//! ```ts
//! const nativeLayer = (config: Config) => Layer.effect(Sqlite.Native, Effect.gen(function* () {
//!   const native = new DatabaseSync(config.filename, {
//!     readOnly: config.readonly,
//!     timeout: config.timeout,
//!     allowExtension: config.allowExtension,
//!     enableForeignKeyConstraints: true,
//!     open: true,
//!   })
//!   yield* Effect.addFinalizer(() => Effect.sync(() => native.close()))
//!   if (config.disableWAL !== true && config.readonly !== true) native.exec("PRAGMA journal_mode = WAL;")
//!   return native
//! }))
//! ```
//!
//! ## Les trois ecarts avec Bun, qui sont reels
//!
//! - Le WAL est desactive en lecture seule (`&& config.readonly !== true`),
//!   la ou Bun l'applique toujours. [`doit_activer_wal`] fige l'ecart.
//! - Pas d'`export` : `node:sqlite` ne sait pas serialiser la base, donc le
//!   client n'expose que `loadExtension`.
//! - Deux options supplementaires, `timeout` et `allowExtension`, et deux
//!   options forcees a `true` (`enableForeignKeyConstraints`, `open`).
//!
//! ## Ce qui n'a pas de traduction ici, et pourquoi
//!
//! Meme perimetre que le jumeau : pas de pilote ni d'ORM dans `Cargo.toml`,
//! donc la requete (`prepare`, `setReadBigInts`, `setReturnArrays(true)` pour
//! les valeurs) est portee comme contrat, pas comme execution. Voir
//! `database_sqlite_bun` pour le raisonnement commun.

/// Identifiant de type du client Node, tel que la source l'ecrit.
pub const TYPE_ID: &str = "~@opencode-ai/core/database/SqliteNode";

/// Pragma applique a l'ouverture, identique des deux cotes.
pub const PRAGMA_WAL: &str = "PRAGMA journal_mode = WAL;";

/// Erreur rendue par `executeStream`, qui n'est pas implemente des deux cotes.
pub const ERREUR_FLUX_NON_IMPLEMENTE: &str = "executeStream not implemented";

/// Nombre de permis du semaphore qui serialize les acces.
pub const SEMAPHORE_PERMIS: usize = 1;

/// Contraintes de cles etrangeres activees a l'ouverture de `DatabaseSync`.
pub const FOREIGN_KEYS_ACTIVEES: bool = true;

/// Ouverture immediate de la base (`open: true`).
pub const OUVERTURE_IMMEDIATE: bool = true;

/// Configuration du layer, transcription de `interface Config`.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Fichier de la base. Seul champ obligatoire.
    pub filename: String,
    /// Passe en `readOnly` tel quel ; `true` inhibe aussi le WAL.
    pub readonly: Option<bool>,
    /// Transmise telle quelle au pilote (sans defaut dans la source).
    pub create: Option<bool>,
    /// Transmise telle quelle au pilote (sans defaut dans la source).
    pub readwrite: Option<bool>,
    /// Seul `Some(true)` desactive le WAL, comme cote Bun.
    pub disable_wal: Option<bool>,
    /// Delai d'attente propre a `node:sqlite`, sans equivalent Bun.
    pub timeout: Option<u64>,
    /// Chargement d'extensions, sans equivalent Bun.
    pub allow_extension: Option<bool>,
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
            timeout: None,
            allow_extension: None,
            avec_attributs_span: false,
            avec_transformations: false,
        }
    }
}

/// Dit si le layer natif applique le pragma WAL a l'ouverture.
///
/// Transcription de `if (config.disableWAL !== true && config.readonly !==
/// true)` : le WAL saute des que l'une des deux options vaut `true`.
/// C'est l'ecart avec Bun, qui n'ecoute que `disableWAL`.
pub fn doit_activer_wal(config: &Config) -> bool {
    config.disable_wal != Some(true) && config.readonly != Some(true)
}

/// Les trois couches fusionnees par `layer`, comme cote Bun.
pub const COUCHES: [&str; 3] = ["native", "sqlite", "drizzle"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_type_signe_le_client_node() {
        assert_eq!(TYPE_ID, "~@opencode-ai/core/database/SqliteNode");
    }

    #[test]
    fn les_options_forcees_sont_vraies_sans_lecture() {
        assert!(FOREIGN_KEYS_ACTIVEES);
        assert!(OUVERTURE_IMMEDIATE);
    }

    #[test]
    fn la_lecture_seule_coupe_le_wal_contrairement_a_bun() {
        let mut config = Config::new("base.db");
        assert!(doit_activer_wal(&config));
        config.readonly = Some(true);
        assert!(
            !doit_activer_wal(&config),
            "Node s'abstient en lecture seule, Bun non"
        );
        config.readonly = Some(false);
        assert!(doit_activer_wal(&config));
    }

    #[test]
    fn la_desactivation_explicite_coupe_le_wal_comme_cote_bun() {
        let mut config = Config::new("base.db");
        config.disable_wal = Some(true);
        assert!(!doit_activer_wal(&config));
        config.disable_wal = Some(false);
        assert!(doit_activer_wal(&config));
    }

    #[test]
    fn les_options_node_sont_absentes_par_defaut() {
        let config = Config::new("base.db");
        assert_eq!(config.timeout, None);
        assert_eq!(config.allow_extension, None);
        assert_eq!(config.create, None);
        assert_eq!(config.readwrite, None);
    }

    #[test]
    fn le_pragma_et_le_flux_sont_communs_aux_deux_pilotes() {
        assert_eq!(PRAGMA_WAL, "PRAGMA journal_mode = WAL;");
        assert_eq!(
            ERREUR_FLUX_NON_IMPLEMENTE,
            "executeStream not implemented"
        );
        assert_eq!(COUCHES, ["native", "sqlite", "drizzle"]);
        assert_eq!(SEMAPHORE_PERMIS, 1);
    }
}
