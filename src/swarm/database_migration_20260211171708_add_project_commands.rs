//! Portage Rust de `packages/core/src/database/migration/20260211171708_add_project_commands.ts`.
//!
//! La source ajoute la colonne `commands` (texte, nullable) a la table
//! `project`, en un seul ordre `ALTER TABLE`. Ce module fige l `id` et cet
//! ordre unique comme des donnees pures, sans pilote SQL.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce fichier
//! ne peut ni ouvrir de base ni executer quoi que ce soit.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260211171708_add_project_commands";

/// Unique ordre de la migration : ajout de `commands` a `project`.
pub const STATEMENT_ADD_COMMANDS: &str = "ALTER TABLE `project` ADD `commands` text";

/// Les ordres SQL de la migration, dans leur ordre d execution.
pub const STATEMENTS: [&str; 1] = [STATEMENT_ADD_COMMANDS];

/// Nombre d ordres executes par la migration.
pub fn statement_count() -> usize {
    STATEMENTS.len()
}

/// Nom de la table visee par la migration.
pub fn target_table() -> &'static str {
    "project"
}

/// Nom de la colonne ajoutee par la migration.
pub fn added_column() -> &'static str {
    "commands"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(MIGRATION_ID, "20260211171708_add_project_commands");
    }

    #[test]
    fn la_migration_execute_un_seul_ordre() {
        assert_eq!(statement_count(), 1);
        assert_eq!(STATEMENTS.len(), 1);
    }

    #[test]
    fn l_ordre_ajoute_commands_a_project() {
        assert!(STATEMENT_ADD_COMMANDS.contains("ALTER TABLE"));
        assert!(STATEMENT_ADD_COMMANDS.contains("`project`"));
        assert!(STATEMENT_ADD_COMMANDS.contains("`commands`"));
        assert!(STATEMENT_ADD_COMMANDS.contains("text"));
    }

    #[test]
    fn la_table_visee_est_project() {
        assert_eq!(target_table(), "project");
        assert!(STATEMENTS[0].contains("`project`"));
    }

    #[test]
    fn la_colonne_ajoutee_est_commands() {
        assert_eq!(added_column(), "commands");
        assert!(STATEMENTS[0].contains("`commands`"));
    }

    #[test]
    fn aucun_ordre_n_est_vide() {
        for stmt in STATEMENTS {
            assert!(!stmt.trim().is_empty());
        }
    }
}
