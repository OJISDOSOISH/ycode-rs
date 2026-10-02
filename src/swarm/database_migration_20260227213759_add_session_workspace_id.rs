//! Portage Rust de `packages/core/src/database/migration/20260227213759_add_session_workspace_id.ts`.
//!
//! La source ajoute la colonne `workspace_id` (texte, nullable) a la table
//! `session`, puis cree l index `session_workspace_idx` dessus. Deux ordres
//! `tx.run`, dans cet ordre. Ce module fige l `id` et ces deux ordres comme
//! des donnees pures, sans pilote SQL.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce fichier
//! ne peut ni ouvrir de base ni executer quoi que ce soit.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260227213759_add_session_workspace_id";

/// Premier ordre : ajout de `workspace_id` a `session`.
pub const STATEMENT_ADD_WORKSPACE_ID: &str = "ALTER TABLE `session` ADD `workspace_id` text";

/// Second ordre : index sur la nouvelle colonne.
pub const STATEMENT_CREATE_INDEX: &str =
    "CREATE INDEX `session_workspace_idx` ON `session` (`workspace_id`)";

/// Les ordres SQL de la migration, dans leur ordre d execution.
pub const STATEMENTS: [&str; 2] = [STATEMENT_ADD_WORKSPACE_ID, STATEMENT_CREATE_INDEX];

/// Nombre d ordres executes par la migration.
pub fn statement_count() -> usize {
    STATEMENTS.len()
}

/// Nom de la table visee par la migration.
pub fn target_table() -> &'static str {
    "session"
}

/// Nom de la colonne ajoutee par la migration.
pub fn added_column() -> &'static str {
    "workspace_id"
}

/// Nom de l index cree par la migration.
pub fn created_index() -> &'static str {
    "session_workspace_idx"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(MIGRATION_ID, "20260227213759_add_session_workspace_id");
    }

    #[test]
    fn la_migration_execute_deux_ordres() {
        assert_eq!(statement_count(), 2);
        assert_eq!(STATEMENTS.len(), 2);
    }

    #[test]
    fn le_premier_ordre_ajoute_workspace_id_a_session() {
        assert!(STATEMENT_ADD_WORKSPACE_ID.contains("ALTER TABLE"));
        assert!(STATEMENT_ADD_WORKSPACE_ID.contains("`session`"));
        assert!(STATEMENT_ADD_WORKSPACE_ID.contains("`workspace_id`"));
        assert_eq!(STATEMENTS[0], STATEMENT_ADD_WORKSPACE_ID);
    }

    #[test]
    fn le_second_ordre_indexe_la_nouvelle_colonne() {
        assert!(STATEMENT_CREATE_INDEX.contains("CREATE INDEX"));
        assert!(STATEMENT_CREATE_INDEX.contains("`session_workspace_idx`"));
        assert!(STATEMENT_CREATE_INDEX.contains("`workspace_id`"));
        assert_eq!(STATEMENTS[1], STATEMENT_CREATE_INDEX);
    }

    #[test]
    fn les_accesseurs_decrivent_la_migration() {
        assert_eq!(target_table(), "session");
        assert_eq!(added_column(), "workspace_id");
        assert_eq!(created_index(), "session_workspace_idx");
    }

    #[test]
    fn aucun_ordre_n_est_vide() {
        for stmt in STATEMENTS {
            assert!(!stmt.trim().is_empty());
        }
    }
}
