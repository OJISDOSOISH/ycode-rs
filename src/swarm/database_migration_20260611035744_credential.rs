//! Portage Rust de `opencode/packages/core/src/database/migration/20260611035744_credential.ts`.
//!
//! La source tient en vingt-cinq lignes : une migration
//! `DatabaseMigration.Migration` dont `up` execute deux ordres via `tx.run`
//! dans un `Effect.gen` : un `CREATE TABLE credential`, puis un
//! `CREATE UNIQUE INDEX` partiel. Il n'y a ni lecture, ni branche, ni test
//! TypeScript dedie.
//!
//! `Cargo.toml` ne declare aucun pilote SQL : ce fichier n'execute rien. Il
//! porte le contrat que la couche d'execution viendra consommer :
//! l'identifiant, les deux ordres SQL dans l'ordre d'execution, et les
//! predicats purs qui les decrivent (table, colonnes, index partiel).
//!
//! La table creee ici n'est pas la table finale : la migration suivante
//! (`20260611192811_lush_chimera`) la supprime et la recree sous une autre
//! forme. Ce fichier porte l'etat intermediaire, sans l'anticiper.

/// Identifiant de la migration, tel que la source l'exporte dans `id`.
pub const MIGRATION_ID: &str = "20260611035744_credential";

/// Nom de la table creee par la migration.
pub const TABLE_CREDENTIAL: &str = "credential";

/// Nom de l'index unique partiel cree par la migration.
pub const INDEX_CONNECTOR_ACTIVE: &str = "credential_connector_active_idx";

/// Premier ordre : creation de la table `credential` a huit colonnes.
pub const STATEMENT_CREATE_CREDENTIAL: &str = r#"CREATE TABLE `credential` (
          `id` text PRIMARY KEY,
          `connector_id` text NOT NULL,
          `method_id` text NOT NULL,
          `label` text NOT NULL,
          `value` text NOT NULL,
          `active` integer DEFAULT false NOT NULL,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL
        );"#;

/// Second ordre : index unique partiel sur `connector_id` pour les lignes actives.
pub const STATEMENT_CREATE_CONNECTOR_ACTIVE_INDEX: &str = r#"CREATE UNIQUE INDEX `credential_connector_active_idx` ON `credential` (`connector_id`) WHERE "credential"."active" = 1;"#;

/// Les ordres de la migration, dans l'ordre d'execution de `up`.
pub const STATEMENTS: &[&str] = &[
    STATEMENT_CREATE_CREDENTIAL,
    STATEMENT_CREATE_CONNECTOR_ACTIVE_INDEX,
];

/// Les huit colonnes de la table initiale, dans l'ordre du `CREATE TABLE`.
pub const COLUMNS: [&str; 8] = [
    "id",
    "connector_id",
    "method_id",
    "label",
    "value",
    "active",
    "time_created",
    "time_updated",
];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(id(), "20260611035744_credential");
        assert_eq!(MIGRATION_ID, id());
    }

    #[test]
    fn la_migration_contient_exactement_deux_ordres() {
        assert_eq!(statement_count(), 2);
        assert_eq!(statements().len(), 2);
    }

    #[test]
    fn le_premier_ordre_cree_la_table_credential() {
        let ordre = statements()[0];
        assert!(ordre.contains("CREATE TABLE"));
        assert!(ordre.contains("`credential`"));
        assert_eq!(TABLE_CREDENTIAL, "credential");
    }

    #[test]
    fn la_table_initiale_porte_ses_huit_colonnes_dans_l_ordre() {
        assert_eq!(
            COLUMNS,
            [
                "id",
                "connector_id",
                "method_id",
                "label",
                "value",
                "active",
                "time_created",
                "time_updated"
            ]
        );
        for colonne in COLUMNS {
            assert!(
                STATEMENT_CREATE_CREDENTIAL.contains(&format!("`{colonne}`")),
                "la colonne {colonne} doit figurer dans l ordre"
            );
        }
    }

    #[test]
    fn la_table_initiale_n_a_pas_d_integration_id() {
        assert!(!STATEMENT_CREATE_CREDENTIAL.contains("integration_id"));
    }

    #[test]
    fn le_second_ordre_cree_l_index_unique_partiel() {
        let ordre = statements()[1];
        assert!(ordre.contains("CREATE UNIQUE INDEX"));
        assert!(ordre.contains(INDEX_CONNECTOR_ACTIVE));
        assert!(ordre.contains("`connector_id`"));
        assert!(ordre.contains(r#""credential"."active" = 1"#));
    }

    #[test]
    fn l_index_ne_porte_que_sur_les_lignes_actives() {
        assert!(STATEMENT_CREATE_CONNECTOR_ACTIVE_INDEX.contains("WHERE"));
        assert!(!STATEMENT_CREATE_CREDENTIAL.contains("WHERE"));
    }

    #[test]
    fn les_ordres_sont_dans_l_ordre_table_puis_index() {
        assert!(statements()[0].contains("CREATE TABLE"));
        assert!(statements()[1].contains("CREATE UNIQUE INDEX"));
    }
}
