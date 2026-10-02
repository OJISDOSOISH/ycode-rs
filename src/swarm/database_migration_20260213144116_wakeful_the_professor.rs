//! Portage Rust de `packages/core/src/database/migration/20260213144116_wakeful_the_professor.ts`.
//!
//! La source cree la table `control_account` (cle primaire composee sur
//! `email` et `url`), en un seul ordre `CREATE TABLE`. Ce module fige l `id`
//! et cet ordre unique comme des donnees pures, sans pilote SQL.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce fichier
//! ne peut ni ouvrir de base ni executer quoi que ce soit.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260213144116_wakeful_the_professor";

/// Unique ordre de la migration : creation de `control_account`.
pub const STATEMENT_CREATE_CONTROL_ACCOUNT: &str =
    "CREATE TABLE `control_account` (`email` text NOT NULL, `url` text NOT NULL, `access_token` text NOT NULL, `refresh_token` text NOT NULL, `token_expiry` integer, `active` integer NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, CONSTRAINT `control_account_pk` PRIMARY KEY(`email`, `url`))";

/// Les ordres SQL de la migration, dans leur ordre d execution.
pub const STATEMENTS: [&str; 1] = [STATEMENT_CREATE_CONTROL_ACCOUNT];

/// Nombre d ordres executes par la migration.
pub fn statement_count() -> usize {
    STATEMENTS.len()
}

/// Nom de la table creee par la migration.
pub fn created_table() -> &'static str {
    "control_account"
}

/// Les deux colonnes de la cle primaire composee, dans leur ordre.
pub fn primary_key_columns() -> Vec<&'static str> {
    vec!["email", "url"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(MIGRATION_ID, "20260213144116_wakeful_the_professor");
    }

    #[test]
    fn la_migration_execute_un_seul_ordre() {
        assert_eq!(statement_count(), 1);
        assert_eq!(STATEMENTS.len(), 1);
    }

    #[test]
    fn l_ordre_cree_la_table_control_account() {
        assert!(STATEMENT_CREATE_CONTROL_ACCOUNT.contains("CREATE TABLE"));
        assert!(STATEMENT_CREATE_CONTROL_ACCOUNT.contains("`control_account`"));
    }

    #[test]
    fn la_cle_primaire_est_le_couple_email_url() {
        assert_eq!(primary_key_columns(), vec!["email", "url"]);
        assert!(STATEMENT_CREATE_CONTROL_ACCOUNT.contains("`control_account_pk`"));
        assert!(STATEMENT_CREATE_CONTROL_ACCOUNT.contains("PRIMARY KEY(`email`, `url`)"));
    }

    #[test]
    fn les_colonnes_obligatoires_sont_not_null() {
        for colonne in ["`email`", "`url`", "`access_token`", "`refresh_token`", "`active`"] {
            assert!(
                STATEMENT_CREATE_CONTROL_ACCOUNT.contains(&format!("{colonne} text NOT NULL"))
                    || STATEMENT_CREATE_CONTROL_ACCOUNT.contains(&format!("{colonne} integer NOT NULL")),
                "la colonne {colonne} doit etre NOT NULL"
            );
        }
    }

    #[test]
    fn aucun_ordre_n_est_vide() {
        for stmt in STATEMENTS {
            assert!(!stmt.trim().is_empty());
        }
    }
}
