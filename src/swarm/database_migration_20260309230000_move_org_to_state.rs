//! Portage de `packages/core/src/database/migration/20260309230000_move_org_to_state.ts`.
//!
//! La source tient en 15 lignes et exporte une migration `DatabaseMigration`
//! qui execute trois `tx.run` dans l ordre : ajout de `active_org_id` sur
//! `account_state`, recopie de `selected_org_id` depuis `account` vers
//! `account_state` par sous-requete, puis suppression de `selected_org_id`
//! sur `account`.
//!
//! Sans pilote SQL dans ce crate, le portage est en donnees pures :
//! identifiant, trois instructions dans l ordre, et helpers qui nomment la
//! colonne ajoutee, la colonne supprimee et la condition de jointure de la
//! recopie. Aucune base n est ouverte ici.

/// Identifiant de la migration, tel qu ecrit dans la source.
pub const MIGRATION_ID: &str = "20260309230000_move_org_to_state";

/// Les trois instructions, dans l ordre d execution de `up`.
pub const STATEMENTS: [&str; 3] = [
    "ALTER TABLE `account_state` ADD `active_org_id` text;",
    "UPDATE `account_state` SET `active_org_id` = (SELECT `selected_org_id` FROM `account` WHERE `account`.`id` = `account_state`.`active_account_id`);",
    "ALTER TABLE `account` DROP COLUMN `selected_org_id`;",
];

/// Colonne ajoutee sur `account_state`.
pub const COLONNE_AJOUTEE: &str = "active_org_id";

/// Colonne source lue puis supprimee sur `account`.
pub const COLONNE_SUPPRIMEE: &str = "selected_org_id";

/// Le nombre d instructions de la migration.
pub fn nombre_d_instructions() -> usize {
    STATEMENTS.len()
}

/// Vrai si l instruction est la recopie par sous-requete.
pub fn est_la_recopie(instruction: &str) -> bool {
    instruction.contains("UPDATE `account_state`")
        && instruction.contains("SELECT `selected_org_id`")
}

/// Vrai si la recopie joint sur le compte actif.
pub fn joint_sur_compte_actif(instruction: &str) -> bool {
    instruction.contains("`account`.`id` = `account_state`.`active_account_id`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260309230000_move_org_to_state");
    }

    #[test]
    fn la_migration_execute_trois_instructions() {
        assert_eq!(nombre_d_instructions(), 3);
        assert_eq!(STATEMENTS.len(), 3);
    }

    #[test]
    fn l_ajout_ouvre_la_sequence_et_la_suppression_la_ferme() {
        assert!(STATEMENTS[0].contains("ADD `active_org_id`"));
        assert!(STATEMENTS[2].contains("DROP COLUMN `selected_org_id`"));
    }

    #[test]
    fn la_recopie_est_au_milieu_et_joint_sur_le_compte_actif() {
        assert!(est_la_recopie(STATEMENTS[1]));
        assert!(joint_sur_compte_actif(STATEMENTS[1]));
        assert!(!est_la_recopie(STATEMENTS[0]));
        assert!(!est_la_recopie(STATEMENTS[2]));
    }

    #[test]
    fn les_noms_de_colonnes_sont_coherents_avec_les_instructions() {
        assert!(STATEMENTS[0].contains(COLONNE_AJOUTEE));
        assert!(STATEMENTS[1].contains(COLONNE_SUPPRIMEE));
        assert!(STATEMENTS[2].contains(COLONNE_SUPPRIMEE));
    }

    #[test]
    fn la_colonne_ajoutee_est_nullable_comme_dans_la_source() {
        assert!(!STATEMENTS[0].contains("NOT NULL"));
    }
}
