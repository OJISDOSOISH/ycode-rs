//! Portage Rust de `opencode/packages/core/src/database/migration/20260605003541_add_session_context_snapshot.ts`.
//!
//! La source tient en vingt et une lignes et ne declare qu'une migration
//! `DatabaseMigration.Migration` : un identifiant et une fonction `up` qui
//! execute un unique ordre `CREATE TABLE` via `tx.run` dans un `Effect.gen`.
//! Il n'y a ni lecture, ni branche, ni test TypeScript dedie.
//!
//! `Cargo.toml` ne declare aucun pilote SQL (`sqlx`, `rusqlite`, `diesel`) :
//! ce fichier n'execute donc rien et n'ouvre aucune base. Il porte le contrat
//! que la couche d'execution viendra consommer : l'identifiant de la migration,
//! l'ordre SQL tel que la source l'envoie, et les quelques predicats purs qui
//! decrivent cet ordre (table visee, colonnes, cle etrangere).

/// Identifiant de la migration, tel que la source l'exporte dans `id`.
pub const MIGRATION_ID: &str = "20260605003541_add_session_context_snapshot";

/// Nom de la table creee par la migration.
pub const TABLE_SESSION_CONTEXT_EPOCH: &str = "session_context_epoch";

/// Nom de la contrainte de cle etrangere posee sur `session_id`.
pub const FK_SESSION_CONTEXT_EPOCH_SESSION: &str =
    "fk_session_context_epoch_session_id_session_id_fk";

/// L'unique ordre SQL de la migration, tel que la source le passe a `tx.run`.
///
/// Les blancs et les retours a la ligne sont ceux de la source : SQLite les
/// ignore, et les tests comparent sur le contenu normalise, pas sur les
/// octets exacts.
pub const STATEMENT_CREATE_SESSION_CONTEXT_EPOCH: &str = r#"CREATE TABLE `session_context_epoch` (
          `session_id` text PRIMARY KEY,
          `baseline` text NOT NULL,
          `snapshot` text NOT NULL,
          `baseline_seq` integer NOT NULL,
          `replacement_seq` integer,
          `revision` integer DEFAULT 0 NOT NULL,
          CONSTRAINT `fk_session_context_epoch_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
        );"#;

/// Les ordres de la migration, dans l'ordre d'execution de `up`.
pub const STATEMENTS: &[&str] = &[STATEMENT_CREATE_SESSION_CONTEXT_EPOCH];

/// Les six colonnes de la table, dans l'ordre du `CREATE TABLE`.
pub const COLUMNS: [&str; 6] = [
    "session_id",
    "baseline",
    "snapshot",
    "baseline_seq",
    "replacement_seq",
    "revision",
];

/// Les colonnes declarees `NOT NULL` dans l'ordre.
pub const COLUMNS_NOT_NULL: [&str; 4] = ["session_id", "baseline", "snapshot", "baseline_seq"];

/// L'identifiant de la migration.
pub fn id() -> &'static str {
    MIGRATION_ID
}

/// Les ordres SQL de la migration, dans l'ordre d'execution.
pub fn statements() -> &'static [&'static str] {
    STATEMENTS
}

/// Le nombre d'ordres de la migration.
pub fn statement_count() -> usize {
    STATEMENTS.len()
}

/// Vrai si l'ordre vise la table `table`.
pub fn statement_targets_table(statement: &str, table: &str) -> bool {
    statement.contains(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(id(), "20260605003541_add_session_context_snapshot");
        assert_eq!(MIGRATION_ID, id());
    }

    #[test]
    fn la_migration_ne_contient_qu_un_seul_ordre() {
        assert_eq!(statement_count(), 1);
        assert_eq!(statements().len(), 1);
    }

    #[test]
    fn l_ordre_cree_la_table_session_context_epoch() {
        let ordre = statements()[0];
        assert!(ordre.contains("CREATE TABLE"));
        assert!(ordre.contains("`session_context_epoch`"));
        assert!(statement_targets_table(ordre, TABLE_SESSION_CONTEXT_EPOCH));
    }

    #[test]
    fn la_table_porte_ses_six_colonnes_dans_l_ordre() {
        assert_eq!(
            COLUMNS,
            [
                "session_id",
                "baseline",
                "snapshot",
                "baseline_seq",
                "replacement_seq",
                "revision"
            ]
        );
        for colonne in COLUMNS {
            assert!(
                STATEMENT_CREATE_SESSION_CONTEXT_EPOCH.contains(&format!("`{colonne}`")),
                "la colonne {colonne} doit figurer dans l ordre"
            );
        }
    }

    #[test]
    fn les_colonnes_not_null_sont_celles_de_la_source() {
        assert_eq!(
            COLUMNS_NOT_NULL,
            ["session_id", "baseline", "snapshot", "baseline_seq"]
        );
        assert!(
            !COLUMNS_NOT_NULL.contains(&"replacement_seq"),
            "replacement_seq est nullable dans la source"
        );
    }

    #[test]
    fn la_cle_primaire_est_session_id() {
        assert!(STATEMENT_CREATE_SESSION_CONTEXT_EPOCH.contains("`session_id` text PRIMARY KEY"));
    }

    #[test]
    fn la_revision_vaut_zero_par_defaut() {
        assert!(STATEMENT_CREATE_SESSION_CONTEXT_EPOCH.contains("`revision` integer DEFAULT 0 NOT NULL"));
    }

    #[test]
    fn la_cle_etrangere_pointe_vers_session_avec_suppression_en_cascade() {
        assert!(STATEMENT_CREATE_SESSION_CONTEXT_EPOCH.contains(FK_SESSION_CONTEXT_EPOCH_SESSION));
        assert!(STATEMENT_CREATE_SESSION_CONTEXT_EPOCH.contains("REFERENCES `session`(`id`)"));
        assert!(STATEMENT_CREATE_SESSION_CONTEXT_EPOCH.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn une_autre_table_n_est_pas_visee() {
        assert!(!statement_targets_table(statements()[0], "credential"));
        assert!(!statement_targets_table(statements()[0], "project_directory"));
    }
}
