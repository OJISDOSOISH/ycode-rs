//! Portage de `packages/core/src/database/migration/20260423070820_add_icon_url_override.ts`.
//!
//! La migration ajoute la colonne `icon_url_override` a la table `project`
//! puis recopie `icon_url` dedans pour les lignes qui en ont une :
//!
//! ```sql
//! ALTER TABLE `project` ADD `icon_url_override` text;
//! UPDATE `project` SET `icon_url_override` = `icon_url` WHERE `icon_url` IS NOT NULL;
//! ```
//!
//! Le point delicat est le `WHERE ... IS NOT NULL` : seules les lignes dont
//! `icon_url` est renseigne sont touchees. Une ligne sans `icon_url` garde
//! `icon_url_override` a `NULL`, elle ne recoit ni chaine vide ni copie.
//! Sans pilote SQL dans ce crate, ce fichier porte l'identifiant, les deux
//! ordres SQL et la regle de recopie sous forme d'une fonction pure sur
//! `Option<&str>`.

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260423070820_add_icon_url_override";

/// Table visee par la migration.
pub const TABLE_NAME: &str = "project";

/// Colonne ajoutee par la migration.
pub const ADDED_COLUMN: &str = "icon_url_override";

/// Colonne source de la recopie.
pub const SOURCE_COLUMN: &str = "icon_url";

/// Premier ordre : ajout de la colonne (nullable, sans defaut).
pub const ALTER_TABLE_SQL: &str = "ALTER TABLE `project` ADD `icon_url_override` text;";

/// Second ordre : recopie conditionnelle.
pub const BACKFILL_SQL: &str =
    "UPDATE `project` SET `icon_url_override` = `icon_url` WHERE `icon_url` IS NOT NULL;";

/// Vrai si la ligne est concernee par la recopie.
///
/// Transcription directe du `WHERE icon_url IS NOT NULL` : la decision porte
/// sur la nullite, jamais sur le contenu. Une chaine vide est renseignee,
/// donc elle est recopiee.
pub fn est_concernee_par_recopie(icon_url: Option<&str>) -> bool {
    icon_url.is_some()
}

/// Valeur ecrite dans `icon_url_override` pour une ligne.
///
/// `None` reste `None` (ligne non touchee par l'`UPDATE`) ; `Some(v)` donne
/// `Some(v)`, y compris quand `v` est vide. Aucune normalisation n'est
/// appliquee : la migration recopie octet pour octet.
pub fn valeur_override(icon_url: Option<&str>) -> Option<String> {
    icon_url.map(|valeur| valeur.to_string())
}

/// Applique la regle de recopie a une tranche de lignes.
///
/// Rend, dans l'ordre, les rangs des lignes touchees. C'est l'equivalent en
/// memoire du `WHERE` : les rangs absents correspondent aux lignes que
/// l'`UPDATE` laisse a `NULL`.
pub fn rangs_touches(icon_urls: &[Option<&str>]) -> Vec<usize> {
    icon_urls
        .iter()
        .enumerate()
        .filter(|(_, icon_url)| est_concernee_par_recopie(**icon_url))
        .map(|(rang, _)| rang)
        .collect()
}

/// Nombre d'ordres SQL emis par `up`.
pub fn nombre_ordres() -> usize {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260423070820_add_icon_url_override");
    }

    #[test]
    fn la_colonne_ajoutee_est_nullable_sans_defaut() {
        assert_eq!(ADDED_COLUMN, "icon_url_override");
        assert!(ALTER_TABLE_SQL.contains("ADD `icon_url_override` text"));
        assert!(!ALTER_TABLE_SQL.contains("NOT NULL"));
        assert!(!ALTER_TABLE_SQL.contains("DEFAULT"));
    }

    #[test]
    fn la_recopie_ne_touche_que_les_lignes_renseignees() {
        assert!(est_concernee_par_recopie(Some("https://ex/icon.png")));
        assert!(est_concernee_par_recopie(Some("")));
        assert!(!est_concernee_par_recopie(None));
    }

    #[test]
    fn une_chaine_vide_est_recopiee_et_non_ignoree() {
        // Le piege `?` contre `??` : un test de veracite ignorerait `""`,
        // mais le `WHERE IS NOT NULL` de la source la retient.
        assert_eq!(valeur_override(Some("")), Some(String::new()));
        assert!(est_concernee_par_recopie(Some("")));
    }

    #[test]
    fn une_valeur_absente_reste_absente() {
        assert_eq!(valeur_override(None), None);
    }

    #[test]
    fn la_recopie_conserve_la_valeur_octet_pour_octet() {
        let url = "https://ex/icon.png";
        assert_eq!(valeur_override(Some(url)), Some(url.to_string()));
    }

    #[test]
    fn les_rangs_touches_suivent_l_ordre_d_entree() {
        let lignes: Vec<Option<&str>> = vec![Some("a"), None, Some(""), None, Some("b")];
        assert_eq!(rangs_touches(&lignes), vec![0, 2, 4]);
    }

    #[test]
    fn une_tranche_vide_ne_touche_rien() {
        let lignes: Vec<Option<&str>> = Vec::new();
        assert!(rangs_touches(&lignes).is_empty());
    }

    #[test]
    fn une_tranche_sans_url_ne_touche_rien() {
        let lignes: Vec<Option<&str>> = vec![None, None];
        assert!(rangs_touches(&lignes).is_empty());
    }

    #[test]
    fn le_backfill_filtre_sur_is_not_null_et_reprend_la_bonne_colonne() {
        assert!(BACKFILL_SQL.contains("SET `icon_url_override` = `icon_url`"));
        assert!(BACKFILL_SQL.contains("WHERE `icon_url` IS NOT NULL"));
        assert_eq!(TABLE_NAME, "project");
        assert_eq!(SOURCE_COLUMN, "icon_url");
    }

    #[test]
    fn le_nombre_d_ordres_est_de_deux() {
        assert_eq!(nombre_ordres(), 2);
    }
}
