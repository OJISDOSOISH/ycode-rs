//! Portage de `packages/core/src/database/migration/20260228203230_blue_harpoon.ts`.
//!
//! La source tient en 30 lignes et exporte une migration `DatabaseMigration`
//! avec un `id` et une fonction `up(tx)` qui execute deux `tx.run` dans
//! l ordre : creation de la table `account`, puis creation de la table
//! `account_state` avec une cle etrangere vers `account`.
//!
//! Il n y a ni branche, ni boucle, ni test TypeScript pour ce module : le
//! comportement observable tient en trois proprietes, porte ici en donnees
//! pures sans pilote SQL (le crate ne declare ni `sqlx`, ni `rusqlite`, ni
//! `diesel`, donc aucune requete n est executee) :
//!
//! 1. l identifiant exact de la migration ;
//! 2. les deux instructions DDL, dans l ordre d execution ;
//! 3. les invariants que ces DDL imposent : noms des tables, noms des
//!    colonnes dans leur ordre, cle etrangere et sa regle de suppression.
//!
//! Ce fichier n ecrit aucune migration et n ouvre aucune base. Il decrit ce
//! que la migration d origine a ecrit, pour qu un branchement SQL futur n ait
//! pas a le deviner.

/// Identifiant de la migration, tel qu ecrit dans la source.
pub const MIGRATION_ID: &str = "20260228203230_blue_harpoon";

/// Premiere instruction : creation de la table `account`.
pub const STATEMENT_CREATE_ACCOUNT: &str = "CREATE TABLE `account` (`id` text PRIMARY KEY, `email` text NOT NULL, `url` text NOT NULL, `access_token` text NOT NULL, `refresh_token` text NOT NULL, `token_expiry` integer, `selected_org_id` text, `time_created` integer NOT NULL, `time_updated` integer NOT NULL);";

/// Seconde instruction : creation de la table `account_state`.
pub const STATEMENT_CREATE_ACCOUNT_STATE: &str = "CREATE TABLE `account_state` (`id` integer PRIMARY KEY NOT NULL, `active_account_id` text, FOREIGN KEY (`active_account_id`) REFERENCES `account`(`id`) ON UPDATE no action ON DELETE set null);";

/// Les instructions de la migration, dans l ordre d execution de `up`.
pub const STATEMENTS: [&str; 2] = [STATEMENT_CREATE_ACCOUNT, STATEMENT_CREATE_ACCOUNT_STATE];

/// Nom de la premiere table creee.
pub const TABLE_ACCOUNT: &str = "account";

/// Nom de la seconde table creee.
pub const TABLE_ACCOUNT_STATE: &str = "account_state";

/// Colonnes de `account`, dans l ordre du `CREATE TABLE`.
pub const ACCOUNT_COLUMNS: [&str; 9] = [
    "id",
    "email",
    "url",
    "access_token",
    "refresh_token",
    "token_expiry",
    "selected_org_id",
    "time_created",
    "time_updated",
];

/// Colonnes de `account_state`, dans l ordre du `CREATE TABLE`.
pub const ACCOUNT_STATE_COLUMNS: [&str; 2] = ["id", "active_account_id"];

/// Les tables creees par la migration, dans l ordre d execution.
pub fn tables_creees() -> Vec<&'static str> {
    vec![TABLE_ACCOUNT, TABLE_ACCOUNT_STATE]
}

/// Le nombre d instructions de la migration.
pub fn nombre_d_instructions() -> usize {
    STATEMENTS.len()
}

/// Vrai si l instruction donnee cree la table donnee.
pub fn instruction_cree_table(instruction: &str, table: &str) -> bool {
    instruction.contains(&format!("CREATE TABLE `{}`", table))
}

/// Vrai si la migration declare une cle etrangere de `account_state`
/// vers `account` avec `ON DELETE set null`.
pub fn declare_cle_vers_account() -> bool {
    STATEMENT_CREATE_ACCOUNT_STATE.contains("REFERENCES `account`(`id`)")
        && STATEMENT_CREATE_ACCOUNT_STATE.contains("ON DELETE set null")
}

/// Les colonnes de `account_state` telles que la migration les a creees.
pub fn colonnes_account_state() -> Vec<&'static str> {
    ACCOUNT_STATE_COLUMNS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260228203230_blue_harpoon");
    }

    #[test]
    fn la_migration_execute_exactement_deux_instructions() {
        assert_eq!(nombre_d_instructions(), 2);
        assert_eq!(STATEMENTS.len(), 2);
    }

    #[test]
    fn l_ordre_d_execution_cree_account_puis_account_state() {
        assert!(instruction_cree_table(&STATEMENTS[0], "account"));
        assert!(instruction_cree_table(&STATEMENTS[1], "account_state"));
        assert_eq!(tables_creees(), vec!["account", "account_state"]);
    }

    #[test]
    fn la_table_account_expose_ses_neuf_colonnes_dans_l_ordre() {
        assert_eq!(
            ACCOUNT_COLUMNS.to_vec(),
            vec![
                "id",
                "email",
                "url",
                "access_token",
                "refresh_token",
                "token_expiry",
                "selected_org_id",
                "time_created",
                "time_updated"
            ]
        );
    }

    #[test]
    fn la_table_account_state_ne_porte_que_deux_colonnes() {
        assert_eq!(colonnes_account_state(), vec!["id", "active_account_id"]);
    }

    #[test]
    fn la_cle_etrangere_pointe_vers_account_en_set_null() {
        assert!(declare_cle_vers_account());
    }

    #[test]
    fn une_instruction_ne_cree_pas_une_table_absente() {
        assert!(!instruction_cree_table(&STATEMENTS[0], "account_state"));
        assert!(!instruction_cree_table(&STATEMENTS[1], "event"));
    }

    #[test]
    fn la_colonne_temporelle_est_bien_selected_org_id() {
        assert!(STATEMENT_CREATE_ACCOUNT.contains("`selected_org_id` text"));
        assert!(ACCOUNT_COLUMNS.contains(&"selected_org_id"));
    }
}
