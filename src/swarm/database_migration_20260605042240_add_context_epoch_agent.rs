//! Portage Rust de `opencode/packages/core/src/database/migration/20260605042240_add_context_epoch_agent.ts`.
//!
//! La source tient en onze lignes : une migration `DatabaseMigration.Migration`
//! dont `up` execute un unique `ALTER TABLE ... ADD` via `tx.run` dans un
//! `Effect.gen`. Il n'y a ni lecture, ni branche, ni test TypeScript dedie.
//!
//! `Cargo.toml` ne declare aucun pilote SQL : ce fichier n'execute rien. Il
//! porte le contrat que la couche d'execution viendra consommer :
//! l'identifiant, l'ordre SQL tel que la source l'envoie, et les predicats
//! purs qui le decrivent (table visee, colonne ajoutee, valeur par defaut).

/// Identifiant de la migration, tel que la source l'exporte dans `id`.
pub const MIGRATION_ID: &str = "20260605042240_add_context_epoch_agent";

/// Nom de la table alteree par la migration.
pub const TABLE_SESSION_CONTEXT_EPOCH: &str = "session_context_epoch";

/// Nom de la colonne ajoutee par la migration.
pub const COLUMN_AGENT: &str = "agent";

/// Valeur par defaut posee par l'ordre sur la nouvelle colonne.
pub const AGENT_DEFAULT: &str = "build";

/// L'unique ordre SQL de la migration, tel que la source le passe a `tx.run`.
pub const STATEMENT_ADD_AGENT: &str =
    r#"ALTER TABLE `session_context_epoch` ADD `agent` text DEFAULT 'build' NOT NULL;"#;

/// Les ordres de la migration, dans l'ordre d'execution de `up`.
pub const STATEMENTS: &[&str] = &[STATEMENT_ADD_AGENT];

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

/// Vrai si l'ordre ajoute la colonne `colonne` a la table `table`.
pub fn adds_column(statement: &str, table: &str, colonne: &str) -> bool {
    statement.contains("ALTER TABLE")
        && statement.contains(table)
        && statement.contains(colonne)
        && statement.contains("ADD")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(id(), "20260605042240_add_context_epoch_agent");
        assert_eq!(MIGRATION_ID, id());
    }

    #[test]
    fn la_migration_ne_contient_qu_un_seul_ordre() {
        assert_eq!(statement_count(), 1);
        assert_eq!(statements().len(), 1);
    }

    #[test]
    fn l_ordre_ajoute_agent_a_session_context_epoch() {
        let ordre = statements()[0];
        assert!(adds_column(ordre, TABLE_SESSION_CONTEXT_EPOCH, COLUMN_AGENT));
        assert_eq!(STATEMENT_ADD_AGENT, ordre);
    }

    #[test]
    fn l_ordre_pose_un_defaut_build_non_null() {
        let ordre = statements()[0];
        assert!(ordre.contains("DEFAULT 'build'"));
        assert!(ordre.contains("NOT NULL"));
        assert_eq!(AGENT_DEFAULT, "build");
    }

    #[test]
    fn la_colonne_est_de_type_texte() {
        assert!(statements()[0].contains("`agent` text"));
    }

    #[test]
    fn une_autre_colonne_n_est_pas_ajoutee() {
        assert!(!adds_column(statements()[0], TABLE_SESSION_CONTEXT_EPOCH, "snapshot"));
        assert!(!adds_column(statements()[0], "credential", COLUMN_AGENT));
    }

    #[test]
    fn l_ordre_ne_cree_ni_table_ni_index() {
        let ordre = statements()[0];
        assert!(!ordre.contains("CREATE TABLE"));
        assert!(!ordre.contains("CREATE INDEX"));
        assert!(!ordre.contains("DROP"));
    }
}
