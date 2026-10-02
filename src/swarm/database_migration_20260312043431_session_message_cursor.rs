//! Portage de `packages/core/src/database/migration/20260312043431_session_message_cursor.ts`.
//!
//! La source tient en 16 lignes et exporte une migration `DatabaseMigration`
//! qui execute quatre `tx.run` dans l ordre : suppression de l index
//! `message_session_idx`, suppression de l index `part_message_idx`, creation
//! de l index `message_session_time_created_id_idx` sur `message`, puis
//! creation de l index `part_message_id_id_idx` sur `part`.
//!
//! Sans pilote SQL dans ce crate, le portage est en donnees pures :
//! identifiant, quatre instructions dans l ordre, listes des index
//! supprimes et crees, et helpers de lecture. Aucune base n est ouverte ici.

/// Identifiant de la migration, tel qu ecrit dans la source.
pub const MIGRATION_ID: &str = "20260312043431_session_message_cursor";

/// Les quatre instructions, dans l ordre d execution de `up`.
pub const STATEMENTS: [&str; 4] = [
    "DROP INDEX IF EXISTS `message_session_idx`;",
    "DROP INDEX IF EXISTS `part_message_idx`;",
    "CREATE INDEX `message_session_time_created_id_idx` ON `message` (`session_id`,`time_created`,`id`);",
    "CREATE INDEX `part_message_id_id_idx` ON `part` (`message_id`,`id`);",
];

/// Index supprimes, dans l ordre.
pub const INDEX_SUPPRIMES: [&str; 2] = ["message_session_idx", "part_message_idx"];

/// Index crees, dans l ordre.
pub const INDEX_CREES: [&str; 2] =
    ["message_session_time_created_id_idx", "part_message_id_id_idx"];

/// Le nombre d instructions de la migration.
pub fn nombre_d_instructions() -> usize {
    STATEMENTS.len()
}

/// Vrai si l instruction est une suppression d index.
pub fn est_suppression(instruction: &str) -> bool {
    instruction.starts_with("DROP INDEX")
}

/// Vrai si l instruction est une creation d index.
pub fn est_creation(instruction: &str) -> bool {
    instruction.starts_with("CREATE INDEX")
}

/// Les colonnes couvertes par l index donne, ou `None` si inconnu.
pub fn colonnes_de_index(nom: &str) -> Option<Vec<&'static str>> {
    match nom {
        "message_session_time_created_id_idx" => {
            Some(vec!["session_id", "time_created", "id"])
        }
        "part_message_id_id_idx" => Some(vec!["message_id", "id"]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260312043431_session_message_cursor");
    }

    #[test]
    fn la_migration_execute_quatre_instructions() {
        assert_eq!(nombre_d_instructions(), 4);
    }

    #[test]
    fn les_suppressions_ouvrent_et_les_creations_ferment() {
        assert!(est_suppression(STATEMENTS[0]));
        assert!(est_suppression(STATEMENTS[1]));
        assert!(est_creation(STATEMENTS[2]));
        assert!(est_creation(STATEMENTS[3]));
    }

    #[test]
    fn les_index_supprimes_sont_les_anciens_curseurs() {
        assert_eq!(
            INDEX_SUPPRIMES.to_vec(),
            vec!["message_session_idx", "part_message_idx"]
        );
        assert!(STATEMENTS[0].contains("message_session_idx"));
        assert!(STATEMENTS[1].contains("part_message_idx"));
    }

    #[test]
    fn les_index_crees_portent_les_bonnes_colonnes() {
        assert_eq!(
            colonnes_de_index("message_session_time_created_id_idx"),
            Some(vec!["session_id", "time_created", "id"])
        );
        assert_eq!(
            colonnes_de_index("part_message_id_id_idx"),
            Some(vec!["message_id", "id"])
        );
        assert_eq!(colonnes_de_index("message_session_idx"), None);
    }

    #[test]
    fn les_suppressions_sont_idempotentes_par_if_exists() {
        assert!(STATEMENTS[0].contains("IF EXISTS"));
        assert!(STATEMENTS[1].contains("IF EXISTS"));
    }
}
