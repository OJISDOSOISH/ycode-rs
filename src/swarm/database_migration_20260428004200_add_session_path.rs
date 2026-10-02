//! Portage de `packages/core/src/database/migration/20260428004200_add_session_path.ts`.
//!
//! La migration tient en un seul ordre :
//!
//! ```sql
//! ALTER TABLE `session` ADD `path` text;
//! ```
//!
//! La colonne est nullable et sans defaut : les lignes existantes recoivent
//! `NULL`, et c'est un etat normal (session sans chemin associe), pas une
//! faute de donnee. Sans pilote SQL dans ce crate, ce fichier porte
//! l'identifiant, le nom de table, le nom de colonne, le texte SQL exact et
//! deux fonctions pures qui decrivent l'effet sur une ligne vue comme
//! `Option<String>`.

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260428004200_add_session_path";

/// Table visee par la migration.
pub const TABLE_NAME: &str = "session";

/// Colonne ajoutee par la migration.
pub const ADDED_COLUMN: &str = "path";

/// Ordre SQL unique de la migration.
pub const ALTER_TABLE_SQL: &str = "ALTER TABLE `session` ADD `path` text;";

/// Vrai si une valeur de `path` est renseignee.
///
/// Comme pour la migration `add_icon_url_override`, la decision porte sur la
/// nullite et non sur le contenu : une chaine vide est une valeur, pas une
/// absence.
pub fn chemin_est_renseigne(path: Option<&str>) -> bool {
    path.is_some()
}

/// Valeur lue apres migration pour une ligne existante.
///
/// Les lignes qui precedaient la migration n'ont pas de `path` : elles se
/// lisent `None`. La fonction existe pour figer ce point : `ALTER TABLE ADD`
/// sans defaut remplit de `NULL`, jamais de chaine vide.
pub fn chemin_apres_migration_pour_ligne_existante() -> Option<String> {
    None
}

/// Nombre d'ordres SQL emis par `up`.
pub fn nombre_ordres() -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260428004200_add_session_path");
    }

    #[test]
    fn la_table_visee_est_session_et_la_colonne_path() {
        assert_eq!(TABLE_NAME, "session");
        assert_eq!(ADDED_COLUMN, "path");
    }

    #[test]
    fn l_ordre_sql_ajoute_une_colonne_texte_nullable() {
        assert!(ALTER_TABLE_SQL.contains("ALTER TABLE `session`"));
        assert!(ALTER_TABLE_SQL.contains("ADD `path` text"));
        assert!(!ALTER_TABLE_SQL.contains("NOT NULL"));
        assert!(!ALTER_TABLE_SQL.contains("DEFAULT"));
    }

    #[test]
    fn une_ligne_existante_se_lit_absente_apres_migration() {
        assert_eq!(chemin_apres_migration_pour_ligne_existante(), None);
    }

    #[test]
    fn une_chaine_vide_est_renseignee_et_non_absente() {
        assert!(chemin_est_renseigne(Some("")));
        assert!(chemin_est_renseigne(Some("/tmp/travail")));
        assert!(!chemin_est_renseigne(None));
    }

    #[test]
    fn un_chemin_renseigne_survit_a_la_copie() {
        let chemin = Some("/tmp/travail");
        assert!(chemin_est_renseigne(chemin));
        assert_eq!(chemin, Some("/tmp/travail"));
    }

    #[test]
    fn le_nombre_d_ordres_est_d_un_seul() {
        assert_eq!(nombre_ordres(), 1);
    }
}
