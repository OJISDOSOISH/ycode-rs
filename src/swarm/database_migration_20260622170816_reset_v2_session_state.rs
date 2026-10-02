//! Portage de `packages/core/src/database/migration/20260622170816_reset_v2_session_state.ts`.
//!
//! La source fait douze lignes : un identifiant et une fonction `up` qui vide
//! l etat de session v2 : cinq `DELETE FROM`, un `UPDATE session` qui annule
//! `workspace_id`, puis un `DELETE FROM workspace`. Aucun test TypeScript.
//!
//! Ce fichier ne touche a aucune base : `Cargo.toml` ne declare aucun pilote
//! SQL. Il porte le contrat observable : l identifiant et les sept
//! instructions exactes, dans l ordre.

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260622170816_reset_v2_session_state";

/// Les sept instructions de `up`, dans l ordre d execution.
pub const INSTRUCTIONS: [&str; 7] = [
    "DELETE FROM `session_context_epoch`;",
    "DELETE FROM `session_input`;",
    "DELETE FROM `session_message`;",
    "DELETE FROM `event`;",
    "DELETE FROM `event_sequence`;",
    "UPDATE `session` SET `workspace_id` = NULL WHERE `workspace_id` IS NOT NULL;",
    "DELETE FROM `workspace`;",
];

/// Les sept instructions de `up`, dans l ordre.
pub fn instructions() -> Vec<String> {
    INSTRUCTIONS.iter().map(|s| s.to_string()).collect()
}

/// Les tables videes par `DELETE FROM`, dans l ordre.
pub fn tables_videes() -> Vec<&'static str> {
    vec![
        "session_context_epoch",
        "session_input",
        "session_message",
        "event",
        "event_sequence",
        "workspace",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_la_source() {
        assert_eq!(MIGRATION_ID, "20260622170816_reset_v2_session_state");
    }

    #[test]
    fn up_compte_sept_instructions_dans_l_ordre() {
        let liste = instructions();
        assert_eq!(liste.len(), 7);
        assert_eq!(liste[0], "DELETE FROM `session_context_epoch`;");
        assert_eq!(liste[5], "UPDATE `session` SET `workspace_id` = NULL WHERE `workspace_id` IS NOT NULL;");
        assert_eq!(liste[6], "DELETE FROM `workspace`;");
    }

    #[test]
    fn les_cinq_premieres_instructions_vident_sans_condition() {
        for instruction in instructions().iter().take(5) {
            assert!(instruction.starts_with("DELETE FROM `"));
            assert!(!instruction.contains("WHERE"));
        }
    }

    #[test]
    fn la_mise_a_jour_session_annule_workspace_id_sous_condition() {
        let maj = &instructions()[5];
        assert!(maj.contains("`session`"));
        assert!(maj.contains("SET `workspace_id` = NULL"));
        assert!(maj.contains("WHERE `workspace_id` IS NOT NULL"));
    }

    #[test]
    fn les_tables_videes_sont_les_six_attendues() {
        assert_eq!(
            tables_videes(),
            vec![
                "session_context_epoch",
                "session_input",
                "session_message",
                "event",
                "event_sequence",
                "workspace",
            ]
        );
    }

    #[test]
    fn workspace_est_vide_en_dernier_apres_avoir_ete_dereference() {
        let liste = instructions();
        assert_eq!(liste[6], "DELETE FROM `workspace`;");
        assert!(liste[5].contains("`session`"));
    }

    #[test]
    fn rejouer_instructions_rend_la_meme_liste() {
        assert_eq!(instructions(), instructions());
    }
}
