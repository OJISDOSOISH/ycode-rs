//! Portage de `packages/core/src/database/migration/20260622142730_simplify_session_context_epoch.ts`.
//!
//! La source fait neuf lignes : un identifiant de migration et une fonction
//! `up` qui execute trois `ALTER TABLE ... DROP COLUMN` sur la table
//! `session_context_epoch`. Aucune lecture, aucun test TypeScript.
//!
//! Ce fichier ne touche a aucune base : `Cargo.toml` ne declare aucun pilote
//! SQL. Il porte le contrat observable : l identifiant, la table visee, les
//! trois colonnes supprimees dans l ordre, et les trois instructions exactes.

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260622142730_simplify_session_context_epoch";

/// Table visee par les trois instructions.
pub const TABLE: &str = "session_context_epoch";

/// Colonnes supprimees, dans l ordre d execution de `up`.
pub const COLONNES_SUPPRIMEES: [&str; 3] = ["agent", "replacement_seq", "revision"];

/// Construit l instruction `ALTER TABLE ... DROP COLUMN` pour une colonne.
pub fn instruction_pour(colonne: &str) -> String {
    format!("ALTER TABLE `{}` DROP COLUMN `{}`;", TABLE, colonne)
}

/// Les trois instructions de `up`, dans l ordre.
pub fn instructions() -> Vec<String> {
    COLONNES_SUPPRIMEES.iter().map(|c| instruction_pour(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_la_source() {
        assert_eq!(MIGRATION_ID, "20260622142730_simplify_session_context_epoch");
    }

    #[test]
    fn la_table_visee_est_session_context_epoch() {
        assert_eq!(TABLE, "session_context_epoch");
    }

    #[test]
    fn les_trois_colonnes_sont_supprimees_dans_l_ordre() {
        assert_eq!(COLONNES_SUPPRIMEES, ["agent", "replacement_seq", "revision"]);
    }

    #[test]
    fn chaque_instruction_est_un_drop_column_sur_la_table() {
        let liste = instructions();
        assert_eq!(liste.len(), 3);
        assert_eq!(liste[0], "ALTER TABLE `session_context_epoch` DROP COLUMN `agent`;");
        assert_eq!(liste[1], "ALTER TABLE `session_context_epoch` DROP COLUMN `replacement_seq`;");
        assert_eq!(liste[2], "ALTER TABLE `session_context_epoch` DROP COLUMN `revision`;");
    }

    #[test]
    fn instruction_pour_cite_la_colonne_demandee() {
        assert_eq!(
            instruction_pour("agent"),
            "ALTER TABLE `session_context_epoch` DROP COLUMN `agent`;"
        );
    }

    #[test]
    fn aucune_instruction_ne_touche_une_autre_table() {
        for instruction in instructions() {
            assert!(instruction.contains("`session_context_epoch`"));
            assert!(!instruction.contains("`session_input`"));
            assert!(!instruction.contains("`session_message`"));
        }
    }

    #[test]
    fn rejouer_instructions_rend_la_meme_liste() {
        assert_eq!(instructions(), instructions());
    }
}
