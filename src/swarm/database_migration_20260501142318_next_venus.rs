//! Portage de `packages/core/src/database/migration/20260501142318_next_venus.ts`.
//!
//! La source tient douze lignes : un identifiant et deux `ALTER TABLE` qui
//! ajoutent chacun une colonne texte nullable a la table `session` :
//!
//! ```sql
//! ALTER TABLE `session` ADD `agent` text;
//! ALTER TABLE `session` ADD `model` text;
//! ```
//!
//! Il n'y a ni valeur par defaut ni contrainte `NOT NULL` : les lignes
//! existantes recoivent `NULL` sur les deux colonnes. Comme `Cargo.toml` ne
//! declare aucun pilote SQL, ce fichier ne touche a aucune base ; il porte
//! l'identifiant, les deux instructions dans leur ordre d'execution, et la
//! forme de la rustine que la migration applique, sous forme de donnees pures
//! et de fonctions pures testables sans base.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260501142318_next_venus";

/// Nom de la table visee par les deux instructions.
pub const TABLE_SESSION: &str = "session";

/// Premiere instruction de `up`, dans l'ordre de la source.
pub const ADD_AGENT_SQL: &str = "ALTER TABLE `session` ADD `agent` text;";

/// Seconde instruction de `up`, dans l'ordre de la source.
pub const ADD_MODEL_SQL: &str = "ALTER TABLE `session` ADD `model` text;";

/// Les instructions SQL de `up`, dans leur ordre d'execution.
pub const STATEMENTS: [&str; 2] = [ADD_AGENT_SQL, ADD_MODEL_SQL];

/// Les colonnes ajoutees, dans l'ordre des instructions.
pub const COLONNES_AJOUTEES: [&str; 2] = ["agent", "model"];

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

/// La rustine qu'un `ALTER TABLE ... ADD COLUMN` nullable applique aux lignes
/// existantes : les deux nouvelles colonnes valent `NULL`, donc `None`.
///
/// `agent` et `model` sont des textes libres sans valeur par defaut ; une
/// chaine vide reste une valeur presente, distincte de l'absence portee par
/// `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionAgentModel {
    /// Nouvelle colonne `agent`, `text` nullable.
    pub agent: Option<String>,
    /// Nouvelle colonne `model`, `text` nullable.
    pub model: Option<String>,
}

impl SessionAgentModel {
    /// La ligne telle que la migration la laisse sur une session existante :
    /// les deux colonnes a `NULL`.
    pub fn ligne_existante() -> Self {
        Self { agent: None, model: None }
    }

    /// Vrai quand la rustine ne porte aucune valeur, c'est-a-dire quand elle
    /// decrit une ligne existante qui n'a pas encore ete reecrite.
    pub fn est_vide(&self) -> bool {
        self.agent.is_none() && self.model.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_exactement_celui_de_la_source() {
        assert_eq!(id(), "20260501142318_next_venus");
        assert_eq!(MIGRATION_ID, "20260501142318_next_venus");
    }

    #[test]
    fn les_deux_instructions_ciblent_la_table_session_dans_l_ordre() {
        assert_eq!(nombre_instructions(), 2);
        assert_eq!(instructions().len(), 2);
        assert_eq!(instructions()[0], ADD_AGENT_SQL);
        assert_eq!(instructions()[1], ADD_MODEL_SQL);
        for instruction in instructions() {
            assert!(
                instruction.contains("`session`"),
                "instruction hors table session : {instruction}"
            );
        }
    }

    #[test]
    fn la_premiere_instruction_ajoute_agent_en_texte_nullable() {
        assert!(ADD_AGENT_SQL.contains("ADD `agent`"));
        assert!(ADD_AGENT_SQL.contains("text"));
        assert!(!ADD_AGENT_SQL.contains("NOT NULL"));
        assert!(!ADD_AGENT_SQL.contains("DEFAULT"));
    }

    #[test]
    fn la_seconde_instruction_ajoute_model_en_texte_nullable() {
        assert!(ADD_MODEL_SQL.contains("ADD `model`"));
        assert!(ADD_MODEL_SQL.contains("text"));
        assert!(!ADD_MODEL_SQL.contains("NOT NULL"));
        assert!(!ADD_MODEL_SQL.contains("DEFAULT"));
    }

    #[test]
    fn les_colonnes_ajoutees_sont_agent_puis_model() {
        assert_eq!(COLONNES_AJOUTEES, ["agent", "model"]);
        assert_eq!(TABLE_SESSION, "session");
    }

    #[test]
    fn une_ligne_existante_recoit_null_sur_les_deux_colonnes() {
        let ligne = SessionAgentModel::ligne_existante();
        assert_eq!(ligne.agent, None);
        assert_eq!(ligne.model, None);
        assert!(ligne.est_vide());
        assert_eq!(ligne, SessionAgentModel::default());
    }

    #[test]
    fn une_chaine_vide_reste_une_valeur_presente_et_non_une_absence() {
        let ligne = SessionAgentModel {
            agent: Some(String::new()),
            model: Some(String::new()),
        };
        assert!(!ligne.est_vide());
        assert_ne!(ligne, SessionAgentModel::ligne_existante());
    }

    #[test]
    fn une_rustine_partiellement_renseignee_n_est_pas_vide() {
        let ligne = SessionAgentModel {
            agent: Some("plan".to_string()),
            model: None,
        };
        assert!(!ligne.est_vide());
        let ligne = SessionAgentModel {
            agent: None,
            model: Some("gpt-5".to_string()),
        };
        assert!(!ligne.est_vide());
    }
}
