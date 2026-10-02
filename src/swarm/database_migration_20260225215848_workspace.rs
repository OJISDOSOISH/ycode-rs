//! Portage Rust de `packages/core/src/database/migration/20260225215848_workspace.ts`.
//!
//! La source cree la table `workspace` (cle etrangere vers `project` avec
//! suppression en cascade), en un seul ordre `CREATE TABLE`. Ce module fige
//! l `id` et cet ordre unique comme des donnees pures, sans pilote SQL.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce fichier
//! ne peut ni ouvrir de base ni executer quoi que ce soit.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260225215848_workspace";

/// Unique ordre de la migration : creation de `workspace`.
pub const STATEMENT_CREATE_WORKSPACE: &str =
    "CREATE TABLE `workspace` (`id` text PRIMARY KEY, `branch` text, `project_id` text NOT NULL, `config` text NOT NULL, CONSTRAINT `fk_workspace_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE)";

/// Les ordres SQL de la migration, dans leur ordre d execution.
pub const STATEMENTS: [&str; 1] = [STATEMENT_CREATE_WORKSPACE];

/// Nombre d ordres executes par la migration.
pub fn statement_count() -> usize {
    STATEMENTS.len()
}

/// Nom de la table creee par la migration.
pub fn created_table() -> &'static str {
    "workspace"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(MIGRATION_ID, "20260225215848_workspace");
    }

    #[test]
    fn la_migration_execute_un_seul_ordre() {
        assert_eq!(statement_count(), 1);
        assert_eq!(STATEMENTS.len(), 1);
    }

    #[test]
    fn l_ordre_cree_la_table_workspace() {
        assert!(STATEMENT_CREATE_WORKSPACE.contains("CREATE TABLE"));
        assert!(STATEMENT_CREATE_WORKSPACE.contains("`workspace`"));
        assert!(STATEMENT_CREATE_WORKSPACE.contains("`id` text PRIMARY KEY"));
    }

    #[test]
    fn la_cle_vers_project_supprime_en_cascade() {
        assert!(STATEMENT_CREATE_WORKSPACE.contains("`fk_workspace_project_id_project_id_fk`"));
        assert!(STATEMENT_CREATE_WORKSPACE.contains("REFERENCES `project`(`id`)"));
        assert!(STATEMENT_CREATE_WORKSPACE.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn les_colonnes_obligatoires_sont_not_null() {
        assert!(STATEMENT_CREATE_WORKSPACE.contains("`project_id` text NOT NULL"));
        assert!(STATEMENT_CREATE_WORKSPACE.contains("`config` text NOT NULL"));
    }

    #[test]
    fn aucun_ordre_n_est_vide() {
        for stmt in STATEMENTS {
            assert!(!stmt.trim().is_empty());
        }
    }
}
