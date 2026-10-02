//! Portage de `packages/core/src/database/migration/20260507164347_add_workspace_time.ts`.
//!
//! La source tient onze lignes : un identifiant et un seul `ALTER TABLE` qui
//! ajoute une colonne entiere non nulle avec defaut zero a `workspace` :
//!
//! ```sql
//! ALTER TABLE `workspace` ADD `time_used` integer NOT NULL DEFAULT 0;
//! ```
//!
//! Contrairement aux colonnes texte nullables des migrations voisines, les
//! lignes existantes recoivent ici `0`, pas `NULL`. Comme `Cargo.toml` ne
//! declare aucun pilote SQL, ce fichier ne touche a aucune base ; il porte
//! l'identifiant, l'instruction unique, et la valeur par defaut appliquee,
//! sous forme de donnees pures testables sans base.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260507164347_add_workspace_time";

/// Nom de la table visee par l'instruction.
pub const TABLE_WORKSPACE: &str = "workspace";

/// Nom de la colonne ajoutee.
pub const COLONNE_TIME_USED: &str = "time_used";

/// L'unique instruction de `up`.
pub const ADD_TIME_USED_SQL: &str =
    "ALTER TABLE `workspace` ADD `time_used` integer NOT NULL DEFAULT 0;";

/// Les instructions SQL de `up`, dans leur ordre d'execution.
pub const STATEMENTS: [&str; 1] = [ADD_TIME_USED_SQL];

/// Valeur par defaut de la colonne pour les lignes existantes et les lignes
/// futures sans valeur explicite.
pub const TIME_USED_DEFAUT: i64 = 0;

/// L'identifiant de la migration.
pub fn id() -> &'static str {
    MIGRATION_ID
}

/// Les instructions SQL, dans l'ordre d'execution.
pub fn instructions() -> &'static [&'static str] {
    &STATEMENTS
}

/// Le nombre d'instructions executees par `up`.
pub fn nombre_instructions() -> usize {
    STATEMENTS.len()
}

/// La valeur que la migration ecrit sur les lignes existantes.
///
/// C'est le `DEFAULT 0` de l'instruction : une colonne `NOT NULL` sans valeur
/// par defaut refuserait la reecriture des lignes existantes, et la source a
/// donc du en fournir une.
pub fn valeur_lignes_existantes() -> i64 {
    TIME_USED_DEFAUT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_exactement_celui_de_la_source() {
        assert_eq!(id(), "20260507164347_add_workspace_time");
        assert_eq!(MIGRATION_ID, "20260507164347_add_workspace_time");
    }

    #[test]
    fn l_instruction_unique_cible_la_table_workspace() {
        assert_eq!(nombre_instructions(), 1);
        assert_eq!(instructions()[0], ADD_TIME_USED_SQL);
        assert!(ADD_TIME_USED_SQL.contains("`workspace`"));
        assert_eq!(TABLE_WORKSPACE, "workspace");
    }

    #[test]
    fn l_instruction_ajoute_time_used_en_entier_non_nul_avec_defaut_zero() {
        assert!(ADD_TIME_USED_SQL.contains("ADD `time_used`"));
        assert!(ADD_TIME_USED_SQL.contains("integer"));
        assert!(ADD_TIME_USED_SQL.contains("NOT NULL"));
        assert!(ADD_TIME_USED_SQL.contains("DEFAULT 0"));
        assert_eq!(COLONNE_TIME_USED, "time_used");
    }

    #[test]
    fn les_lignes_existantes_recoivent_zero_et_non_null() {
        assert_eq!(valeur_lignes_existantes(), 0);
        assert_eq!(TIME_USED_DEFAUT, 0);
    }

    #[test]
    fn le_defaut_est_zero_et_non_negatif() {
        assert!(TIME_USED_DEFAUT >= 0);
    }

    #[test]
    fn une_seule_instruction_comme_dans_la_source() {
        assert_eq!(STATEMENTS.len(), 1);
        assert_eq!(instructions().len(), 1);
    }
}
