//! Portage Rust de `opencode/packages/core/src/database/migration/20260612174303_project_dir_strategy.ts`.
//!
//! La source tient en vingt-neuf lignes : une migration
//! `DatabaseMigration.Migration` dont `up` execute sept ordres via `tx.run`
//! dans un `Effect.gen`. Le motif est la recreation d'une table SQLite qui ne
//! sait pas alterer une contrainte : ajout d'une colonne `strategy`, copie
//! vers une table neuve `__new_project_directory`, suppression de l'ancienne,
//! renommage de la neuve, avec les foreign keys coupees pendant l'operation.
//! Il n'y a ni lecture, ni branche, ni test TypeScript dedie.
//!
//! `Cargo.toml` ne declare aucun pilote SQL : ce fichier n'execute rien. Il
//! porte le contrat que la couche d'execution viendra consommer :
//! l'identifiant, les sept ordres dans l'ordre d'execution, et les predicats
//! purs qui decrivent la copie (colonnes copiees, table intermediaire).

/// Identifiant de la migration, tel que la source l'exporte dans `id`.
pub const MIGRATION_ID: &str = "20260612174303_project_dir_strategy";

/// Nom de la table visee par la migration.
pub const TABLE_PROJECT_DIRECTORY: &str = "project_directory";

/// Nom de la table intermediaire utilisee pendant la copie.
pub const TABLE_NEW_PROJECT_DIRECTORY: &str = "__new_project_directory";

/// Nom de la colonne ajoutee par la migration.
pub const COLUMN_STRATEGY: &str = "strategy";

/// Ordre 1 : ajout de la colonne (elle sera videe par la copie qui suit).
pub const STATEMENT_ADD_STRATEGY: &str =
    r#"ALTER TABLE `project_directory` ADD `strategy` text;"#;

/// Ordre 2 : coupure des cles etrangeres pendant la recreation.
pub const STATEMENT_PRAGMA_OFF: &str = r#"PRAGMA foreign_keys=OFF;"#;

/// Ordre 3 : creation de la table neuve avec sa contrainte composite.
pub const STATEMENT_CREATE_NEW: &str = r#"CREATE TABLE `__new_project_directory` (
          `project_id` text NOT NULL,
          `directory` text NOT NULL,
          `type` text,
          `strategy` text,
          `time_created` integer NOT NULL,
          CONSTRAINT `project_directory_pk` PRIMARY KEY(`project_id`, `directory`),
          CONSTRAINT `fk_project_directory_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
        );"#;

/// Ordre 4 : copie des lignes, sans la nouvelle colonne.
pub const STATEMENT_COPY_ROWS: &str = r#"INSERT INTO `__new_project_directory`(`project_id`, `directory`, `type`, `time_created`) SELECT `project_id`, `directory`, `type`, `time_created` FROM `project_directory`;"#;

/// Ordre 5 : suppression de l'ancienne table.
pub const STATEMENT_DROP_OLD: &str = r#"DROP TABLE `project_directory`;"#;

/// Ordre 6 : la table neuve prend le nom de l'ancienne.
pub const STATEMENT_RENAME_NEW: &str =
    r#"ALTER TABLE `__new_project_directory` RENAME TO `project_directory`;"#;

/// Ordre 7 : retablissement des cles etrangeres.
pub const STATEMENT_PRAGMA_ON: &str = r#"PRAGMA foreign_keys=ON;"#;

/// Les ordres de la migration, dans l'ordre d'execution de `up`.
pub const STATEMENTS: &[&str] = &[
    STATEMENT_ADD_STRATEGY,
    STATEMENT_PRAGMA_OFF,
    STATEMENT_CREATE_NEW,
    STATEMENT_COPY_ROWS,
    STATEMENT_DROP_OLD,
    STATEMENT_RENAME_NEW,
    STATEMENT_PRAGMA_ON,
];

/// Les quatre colonnes copiees d'une table a l'autre, dans l'ordre du `INSERT`.
pub const COPIED_COLUMNS: [&str; 4] = ["project_id", "directory", "type", "time_created"];

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
        assert_eq!(id(), "20260612174303_project_dir_strategy");
        assert_eq!(MIGRATION_ID, id());
    }

    #[test]
    fn la_migration_contient_exactement_sept_ordres() {
        assert_eq!(statement_count(), 7);
        assert_eq!(statements().len(), 7);
    }

    #[test]
    fn le_premier_ordre_ajoute_la_colonne_strategy() {
        let ordre = statements()[0];
        assert!(ordre.contains("ALTER TABLE"));
        assert!(ordre.contains("`project_directory`"));
        assert!(ordre.contains("`strategy`"));
    }

    #[test]
    fn les_cles_etrangeres_sont_coupees_puis_retablies() {
        assert_eq!(statements()[1], STATEMENT_PRAGMA_OFF);
        assert_eq!(statements()[6], STATEMENT_PRAGMA_ON);
        assert!(STATEMENT_PRAGMA_OFF.contains("OFF"));
        assert!(STATEMENT_PRAGMA_ON.contains("ON"));
    }

    #[test]
    fn la_table_neuve_porte_la_contrainte_composite() {
        assert!(STATEMENT_CREATE_NEW.contains(TABLE_NEW_PROJECT_DIRECTORY));
        assert!(STATEMENT_CREATE_NEW.contains("PRIMARY KEY(`project_id`, `directory`)"));
        assert!(STATEMENT_CREATE_NEW.contains("REFERENCES `project`(`id`)"));
        assert!(STATEMENT_CREATE_NEW.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn la_copie_transfere_quatre_colonnes_sans_strategy() {
        assert_eq!(COPIED_COLUMNS, ["project_id", "directory", "type", "time_created"]);
        assert!(!STATEMENT_COPY_ROWS.contains("strategy"));
        for colonne in COPIED_COLUMNS {
            assert!(
                STATEMENT_COPY_ROWS.contains(colonne),
                "la colonne {colonne} doit etre copiee"
            );
        }
    }

    #[test]
    fn l_ancienne_table_est_supprimee_puis_la_neuve_renommee() {
        assert!(statements()[4].contains("DROP TABLE"));
        assert!(statements()[4].contains("`project_directory`"));
        assert!(statements()[5].contains("RENAME TO"));
        assert!(statements()[5].contains("`project_directory`"));
    }

    #[test]
    fn la_colonne_strategy_est_nullable_dans_la_table_neuve() {
        assert!(STATEMENT_CREATE_NEW.contains("`strategy` text,"));
        assert!(!STATEMENT_CREATE_NEW.contains("`strategy` text NOT NULL"));
    }
}
