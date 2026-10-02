//! Portage de `packages/core/src/database/migration/20260504145000_add_sync_owner.ts`.
//!
//! La source tient onze lignes : un identifiant et un seul `ALTER TABLE` qui
//! ajoute une colonne texte nullable a la table `event_sequence` :
//!
//! ```sql
//! ALTER TABLE `event_sequence` ADD `owner_id` text;
//! ```
//!
//! Sans `NOT NULL` ni valeur par defaut, les lignes existantes recoivent
//! `NULL`. Comme `Cargo.toml` ne declare aucun pilote SQL, ce fichier ne
//! touche a aucune base ; il porte l'identifiant, l'instruction unique, et la
//! forme de la rustine appliquee, sous forme de donnees pures testables sans
//! base.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260504145000_add_sync_owner";

/// Nom de la table visee par l'instruction.
pub const TABLE_EVENT_SEQUENCE: &str = "event_sequence";

/// Nom de la colonne ajoutee.
pub const COLONNE_OWNER_ID: &str = "owner_id";

/// L'unique instruction de `up`.
pub const ADD_OWNER_ID_SQL: &str = "ALTER TABLE `event_sequence` ADD `owner_id` text;";

/// Les instructions SQL de `up`, dans leur ordre d'execution.
pub const STATEMENTS: [&str; 1] = [ADD_OWNER_ID_SQL];

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

/// La rustine qu'un `ALTER TABLE ... ADD COLUMN` nullable applique : la
/// nouvelle colonne vaut `NULL`, donc `None`, sur les lignes existantes.
///
/// Une chaine vide reste une valeur presente, distincte de l'absence.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SequenceOwner {
    /// Nouvelle colonne `owner_id`, `text` nullable.
    pub owner_id: Option<String>,
}

impl SequenceOwner {
    /// La ligne telle que la migration la laisse sur un agregat existant.
    pub fn ligne_existante() -> Self {
        Self { owner_id: None }
    }

    /// Vrai quand la rustine ne porte aucune valeur.
    pub fn est_vide(&self) -> bool {
        self.owner_id.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_exactement_celui_de_la_source() {
        assert_eq!(id(), "20260504145000_add_sync_owner");
        assert_eq!(MIGRATION_ID, "20260504145000_add_sync_owner");
    }

    #[test]
    fn l_instruction_unique_cible_event_sequence() {
        assert_eq!(nombre_instructions(), 1);
        assert_eq!(instructions()[0], ADD_OWNER_ID_SQL);
        assert!(ADD_OWNER_ID_SQL.contains("`event_sequence`"));
        assert_eq!(TABLE_EVENT_SEQUENCE, "event_sequence");
    }

    #[test]
    fn l_instruction_ajoute_owner_id_en_texte_nullable() {
        assert!(ADD_OWNER_ID_SQL.contains("ADD `owner_id`"));
        assert!(ADD_OWNER_ID_SQL.contains("text"));
        assert!(!ADD_OWNER_ID_SQL.contains("NOT NULL"));
        assert!(!ADD_OWNER_ID_SQL.contains("DEFAULT"));
        assert_eq!(COLONNE_OWNER_ID, "owner_id");
    }

    #[test]
    fn une_ligne_existante_recoit_null_sur_owner_id() {
        let ligne = SequenceOwner::ligne_existante();
        assert_eq!(ligne.owner_id, None);
        assert!(ligne.est_vide());
        assert_eq!(ligne, SequenceOwner::default());
    }

    #[test]
    fn une_chaine_vide_reste_une_valeur_presente_et_non_une_absence() {
        let ligne = SequenceOwner { owner_id: Some(String::new()) };
        assert!(!ligne.est_vide());
        assert_ne!(ligne, SequenceOwner::ligne_existante());
    }

    #[test]
    fn un_proprietaire_renseigne_n_est_pas_vide() {
        let ligne = SequenceOwner { owner_id: Some("own_1".to_string()) };
        assert!(!ligne.est_vide());
        assert_eq!(ligne.owner_id.as_deref(), Some("own_1"));
    }
}
