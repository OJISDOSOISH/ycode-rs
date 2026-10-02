//! Portage Rust de `packages/core/src/database/migration.ts`.
//!
//! ## Ce que contient la source
//!
//! Cent huit lignes, deux fonctions et un verrou :
//!
//! ```ts
//! export type Migration = { id: string; up: (tx: Transaction) => Effect.Effect<void, unknown> }
//! export function apply(db: Database) { ... }
//! export function applyOnly(db: Database, input: Migration[]) { ... }
//! ```
//!
//! `apply` trie les bases en trois tas : base neuve (aucune table) qui recoit
//! le schema genere puis le journal, base existante (table `session` presente)
//! qui recoit `applyOnly`, base non vide sans `session` qui meurt avec
//! `"Database is not empty and has no session table"`. `applyOnly` cree le
//! journal s'il manque, reimporte une fois le journal historique de Drizzle
//! (`__drizzle_migrations`, avec ou sans colonne `name`), puis applique chaque
//! migration manquante dans sa propre transaction.
//!
//! ## Ce qui n'a pas de traduction ici, et pourquoi
//!
//! - `Semaphore.makeUnsafe(1)` et `lock.withPermit` : le verrou global
//!   d'application. Porte comme capacite (`VERROU_PERMIS = 1`), pas comme
//!   semaphore : aucun runtime d'effets n'existe dans ce crate.
//! - `db.all`, `db.get`, `db.run`, `db.transaction`, `Effect.gen`,
//!   `Effect.forEach`, `Effect.die` : l'execution contre SQLite. `Cargo.toml`
//!   ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce fichier ne touche
//!   aucune base. Il porte la decision (triage, journal, rattrapage) sous
//!   forme de fonctions pures sur des tranches.
//! - `schema.up` et `migrations` (voir `migration.gen.ts`) : l'operation de
//!   creation initiale et la liste des migrations, portees dans leurs propres
//!   modules (`database_schema_gen`, lanes 12-19). Ce fichier ne les duplique
//!   pas.
//! - `Date.now()` : l'horodatage du journal est fourni par l'appelant
//!   (`sceau_journal`), pas lu dans l'horloge, pour rester testable.
//!
//! ## Le point delicat : les deux formes du journal historique
//!
//! Quand le journal `migration` est vide et que `__drizzle_migrations` existe,
//! la source distingue deux schemas historiques : avec colonne `name` (copie
//! directe `SELECT name ... WHERE name IS NOT NULL`), sans colonne `name`
//! (chaque `created_at` est converti en prefixe `strftime('%Y%m%d%H%M%S', ...)`
//! puis apparie a `input.find(item => item.id.startsWith(prefix + "_"))`, et
//! un horodatage sans appariement fait mourir l'effet avec
//! `` `Legacy migration timestamp ${created_at} does not match any known
//! migration` ``). Les deux branches sont portees : [`import_historique_nomme`]
//! et [`trouver_migration_pour_prefixe`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Nom de la table `session`, dont la presence signe une base existante.
pub const TABLE_SESSION: &str = "session";

/// Nom du journal des migrations TypeScript.
pub const TABLE_MIGRATION: &str = "migration";

/// Nom du journal historique tenu par Drizzle.
pub const TABLE_DRIZZLE_HISTORIQUE: &str = "__drizzle_migrations";

/// Colonnes du journal, dans l'ordre du `CREATE TABLE`.
pub const MIGRATION_COLONNES: [&str; 2] = ["id", "time_completed"];

/// Erreur fatale quand une base non vide n'a pas de table `session`.
pub const ERREUR_BASE_NON_VIDE: &str = "Database is not empty and has no session table";

/// Nombre de permis du verrou global (`Semaphore.makeUnsafe(1)`).
pub const VERROU_PERMIS: usize = 1;

/// Message quand un horodatage historique ne correspond a aucune migration.
pub fn erreur_legacy_sans_migration(horodatage: i64) -> String {
    format!("Legacy migration timestamp {horodatage} does not match any known migration")
}

/// Ordre SQL de creation du journal, forme fraiche (`apply`, sans `IF NOT EXISTS`).
pub const DDL_JOURNAL: &str =
    "CREATE TABLE \"migration\" (id TEXT PRIMARY KEY, time_completed INTEGER NOT NULL)";

/// Ordre SQL de creation du journal, forme rattrapage (`applyOnly`, avec `IF NOT EXISTS`).
pub const DDL_JOURNAL_SI_ABSENT: &str =
    "CREATE TABLE IF NOT EXISTS \"migration\" (id TEXT PRIMARY KEY, time_completed INTEGER NOT NULL)";

/// Ce que `apply` decide a la lecture des tables existantes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionInitiale {
    /// Aucune table : creer le schema genere puis sceller le journal.
    InitialiserSchema,
    /// Table `session` presente : appliquer seulement les manquantes.
    AppliquerSeulement,
    /// Tables sans `session` : mourir avec [`ERREUR_BASE_NON_VIDE`].
    ErreurBaseNonVide,
}

/// Trie une base d'apres les noms de ses tables, comme `apply`.
pub fn trier_base_existante(noms_tables: &[&str]) -> DecisionInitiale {
    if noms_tables.contains(&TABLE_SESSION) {
        return DecisionInitiale::AppliquerSeulement;
    }
    if noms_tables.is_empty() {
        return DecisionInitiale::InitialiserSchema;
    }
    DecisionInitiale::ErreurBaseNonVide
}

/// Une ligne du journal `migration`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LigneJournal {
    /// Identifiant de la migration, cle primaire.
    pub id: String,
    /// Millisecondes depuis l'epoch, posees par l'appelant.
    pub time_completed: i64,
}

/// Construit le sceau pose dans le journal pour une migration donnee.
pub fn sceau_journal(id_migration: &str, maintenant: i64) -> LigneJournal {
    LigneJournal {
        id: id_migration.to_string(),
        time_completed: maintenant,
    }
}

/// Migrations d'entree encore absentes du journal, dans l'ordre d'entree.
///
/// C'est la boucle finale d'`applyOnly` : `if (completed.has(migration.id))
/// continue`. L'ordre est celui de la liste d'entree, jamais celui du journal.
pub fn migrations_en_attente<'a>(entree: &[&'a str], terminees: &[&str]) -> Vec<&'a str> {
    let connues: BTreeSet<&&str> = terminees.iter().collect();
    entree
        .iter()
        .filter(|id| !connues.contains(id))
        .copied()
        .collect()
}

/// Dit si le journal doit reimporter l'historique Drizzle.
///
/// La source ne le fait que quand le journal est vide **et** que la table
/// `__drizzle_migrations` existe.
pub fn doit_importer_historique(journal_vide: bool, table_historique_presente: bool) -> bool {
    journal_vide && table_historique_presente
}

/// Filtre les noms non nuls du journal historique nomme.
///
/// Transcription de `SELECT name ... WHERE name IS NOT NULL` : les noms nuls
/// ne sont jamais inseres dans le journal.
pub fn import_historique_nomme<'a>(noms: &[Option<&'a str>]) -> Vec<&'a str> {
    noms.iter().filter_map(|nom| *nom).collect()
}

/// Retrouve la migration dont l'identifiant commence par `prefixe + "_"`.
///
/// Transcription de `input.find((item) => item.id.startsWith(`${prefixe}_`))`.
/// Un horodatage sans appariement fait mourir l'effet dans la source ; ici
/// elle rend `None` et c'est l'appelant qui porte l'erreur via
/// [`erreur_legacy_sans_migration`].
pub fn trouver_migration_pour_prefixe(prefixe: &str, ids_entree: &[&str]) -> Option<String> {
    let debut = format!("{prefixe}_");
    ids_entree
        .iter()
        .find(|id| id.starts_with(&debut))
        .map(|id| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_base_avec_session_mene_au_rattrapage_seul() {
        assert_eq!(
            trier_base_existante(&["session", "message"]),
            DecisionInitiale::AppliquerSeulement
        );
        // La session suffit, meme seule.
        assert_eq!(
            trier_base_existante(&["session"]),
            DecisionInitiale::AppliquerSeulement
        );
    }

    #[test]
    fn une_base_vide_mene_a_l_initialisation() {
        assert_eq!(
            trier_base_existante(&[]),
            DecisionInitiale::InitialiserSchema
        );
    }

    #[test]
    fn des_tables_sans_session_menent_a_l_erreur_fatale() {
        assert_eq!(
            trier_base_existante(&["message"]),
            DecisionInitiale::ErreurBaseNonVide
        );
        assert_eq!(
            trier_base_existante(&["__drizzle_migrations"]),
            DecisionInitiale::ErreurBaseNonVide
        );
        assert_eq!(
            ERREUR_BASE_NON_VIDE,
            "Database is not empty and has no session table"
        );
    }

    #[test]
    fn le_verrou_global_ne_laisse_passer_qu_un_permis() {
        assert_eq!(VERROU_PERMIS, 1);
    }

    #[test]
    fn le_journal_porte_ses_deux_colonnes_dans_l_ordre() {
        assert_eq!(MIGRATION_COLONNES, ["id", "time_completed"]);
        assert!(DDL_JOURNAL.contains("\"migration\""));
        assert!(DDL_JOURNAL.contains("PRIMARY KEY"));
        assert!(DDL_JOURNAL_SI_ABSENT.contains("IF NOT EXISTS"));
        assert!(!DDL_JOURNAL.contains("IF NOT EXISTS"));
    }

    #[test]
    fn l_attente_ignore_les_terminees_et_garde_l_ordre_d_entree() {
        let entree = ["a_1", "b_2", "c_3"];
        let terminees = ["b_2"];
        assert_eq!(migrations_en_attente(&entree, &terminees), vec!["a_1", "c_3"]);
    }

    #[test]
    fn l_attente_vide_ne_retient_rien_dans_les_deux_sens() {
        let vide: [&str; 0] = [];
        assert!(migrations_en_attente(&vide, &["a_1"]).is_empty());
        assert!(migrations_en_attente(&["a_1"], &[]).contains(&"a_1"));
        assert!(migrations_en_attente(&["a_1"], &["a_1"]).is_empty());
    }

    #[test]
    fn l_import_n_a_lieu_que_sur_journal_vide_avec_table_historique() {
        assert!(doit_importer_historique(true, true));
        assert!(!doit_importer_historique(false, true));
        assert!(!doit_importer_historique(true, false));
        assert!(!doit_importer_historique(false, false));
    }

    #[test]
    fn l_import_nomme_ecarte_les_noms_nuls() {
        let noms = [Some("20260127_x"), None, Some("20260211_y")];
        assert_eq!(
            import_historique_nomme(&noms),
            vec!["20260127_x", "20260211_y"]
        );
    }

    #[test]
    fn le_prefixe_legacy_retrouve_la_migration_avec_souligne() {
        let entree = ["20260127222353_familiar_lady_ursula", "20260211171708_add_project_commands"];
        assert_eq!(
            trouver_migration_pour_prefixe("20260127222353", &entree),
            Some("20260127222353_familiar_lady_ursula".to_string())
        );
        // Sans le souligne separateur, pas d'appariement.
        assert_eq!(trouver_migration_pour_prefixe("20260127", &entree), None);
        assert_eq!(trouver_migration_pour_prefixe("99999999999999", &entree), None);
    }

    #[test]
    fn un_horodatage_sans_migration_produit_le_message_fatal_de_la_source() {
        assert_eq!(
            erreur_legacy_sans_migration(1_700_000_000_000),
            "Legacy migration timestamp 1700000000000 does not match any known migration"
        );
    }

    #[test]
    fn le_sceau_pose_l_instant_fourni_par_l_appelant() {
        let sceau = sceau_journal("20260127_x", 42);
        assert_eq!(sceau.id, "20260127_x");
        assert_eq!(sceau.time_completed, 42);
        let json = serde_json::to_value(&sceau).unwrap();
        assert_eq!(json["id"], "20260127_x");
        assert_eq!(json["time_completed"], 42);
    }
}
