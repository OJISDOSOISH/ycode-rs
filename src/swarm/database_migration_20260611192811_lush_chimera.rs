//! Portage Rust de `opencode/packages/core/src/database/migration/20260611192811_lush_chimera.ts`.
//!
//! La source tient en vingt-cinq lignes : une migration
//! `DatabaseMigration.Migration` dont `up` execute trois ordres via `tx.run`
//! dans un `Effect.gen` : suppression de l'index partiel, suppression de la
//! table `credential`, puis recreation de la table sous sa forme finale a
//! neuf colonnes. Il n'y a ni lecture, ni branche, ni test TypeScript dedie.
//!
//! `Cargo.toml` ne declare aucun pilote SQL : ce fichier n'execute rien. Il
//! porte le contrat que la couche d'execution viendra consommer :
//! l'identifiant, les trois ordres dans l'ordre d'execution, et les predicats
//! purs qui decrivent l'ecart avec la migration precedente
//! (`20260611035744_credential`).

/// Identifiant de la migration, tel que la source l'exporte dans `id`.
pub const MIGRATION_ID: &str = "20260611192811_lush_chimera";

/// Nom de la table supprimee puis recreee.
pub const TABLE_CREDENTIAL: &str = "credential";

/// Nom de l'index supprime par le premier ordre.
pub const DROPPED_INDEX: &str = "credential_connector_active_idx";

/// Premier ordre : suppression de l'index partiel pose par la migration precedente.
pub const STATEMENT_DROP_INDEX: &str = r#"DROP INDEX IF EXISTS `credential_connector_active_idx`;"#;

/// Deuxieme ordre : suppression de la table intermediaire.
pub const STATEMENT_DROP_CREDENTIAL: &str = r#"DROP TABLE `credential`;"#;

/// Troisieme ordre : recreation de la table sous sa forme finale.
pub const STATEMENT_CREATE_CREDENTIAL: &str = r#"CREATE TABLE `credential` (
          `id` text PRIMARY KEY,
          `integration_id` text,
          `label` text NOT NULL,
          `value` text NOT NULL,
          `connector_id` text,
          `method_id` text,
          `active` integer,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL
        );"#;

/// Les ordres de la migration, dans l'ordre d'execution de `up`.
pub const STATEMENTS: &[&str] = &[
    STATEMENT_DROP_INDEX,
    STATEMENT_DROP_CREDENTIAL,
    STATEMENT_CREATE_CREDENTIAL,
];

/// Les neuf colonnes de la table recreee, dans l'ordre du `CREATE TABLE`.
pub const COLUMNS: [&str; 9] = [
    "id",
    "integration_id",
    "label",
    "value",
    "connector_id",
    "method_id",
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
        assert_eq!(id(), "20260611192811_lush_chimera");
        assert_eq!(MIGRATION_ID, id());
    }

    #[test]
    fn la_migration_contient_exactement_trois_ordres() {
        assert_eq!(statement_count(), 3);
        assert_eq!(statements().len(), 3);
    }

    #[test]
    fn le_premier_ordre_supprime_l_index_partiel() {
        let ordre = statements()[0];
        assert!(ordre.contains("DROP INDEX"));
        assert!(ordre.contains(DROPPED_INDEX));
        assert!(ordre.contains("IF EXISTS"));
    }

    #[test]
    fn le_deuxieme_ordre_supprime_la_table() {
        let ordre = statements()[1];
        assert!(ordre.contains("DROP TABLE"));
        assert!(ordre.contains("`credential`"));
    }

    #[test]
    fn le_troisieme_ordre_recree_la_table_finale() {
        let ordre = statements()[2];
        assert!(ordre.contains("CREATE TABLE"));
        assert!(ordre.contains("`credential`"));
        assert!(ordre.contains("integration_id"));
    }

    #[test]
    fn la_table_finale_porte_ses_neuf_colonnes_dans_l_ordre() {
        assert_eq!(
            COLUMNS,
            [
                "id",
                "integration_id",
                "label",
                "value",
                "connector_id",
                "method_id",
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
    fn les_colonnes_heritieres_ne_sont_plus_not_null() {
        for colonne in ["connector_id", "method_id", "active"] {
            assert!(
                !STATEMENT_CREATE_CREDENTIAL.contains(&format!("`{colonne}` text NOT NULL")),
                "{colonne} a perdu son NOT NULL"
            );
            assert!(
                !STATEMENT_CREATE_CREDENTIAL.contains(&format!("`{colonne}` integer NOT NULL")),
                "{colonne} a perdu son NOT NULL"
            );
        }
    }

    #[test]
    fn l_ordre_de_recreation_ne_recree_pas_l_index() {
        assert!(!STATEMENT_CREATE_CREDENTIAL.contains("CREATE INDEX"));
        assert!(!STATEMENT_CREATE_CREDENTIAL.contains("credential_connector_active_idx"));
    }
}
