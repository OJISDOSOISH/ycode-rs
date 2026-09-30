//! Portage Rust de `opencode/packages/core/src/config/command.ts`.
//!
//! La source TypeScript est minuscule : douze lignes, un seul export de fond.
//!
//! 1. `export * as ConfigCommand from "./command"` est un reexport d'espace de
//!    noms qui pointe sur le fichier lui-meme. C'est du code mort pour nous :
//!    en Rust le module `config_command` est deja l'espace de noms, donc il
//!    n'y a rien a retranscrire.
//!
//! 2. `Info` est une classe de schema `effect/Schema`. Une `Schema.Class` est
//!    simplement un type de donnees : pas de methode, pas de comportement.
//!    Elle devient donc un `struct` derive `Serialize, Deserialize`.
//!
//! Pieges a ecarter explicitement :
//!
//! - **Aucun renommage de champ n'est necessaire ici.** Tous les cinq champs
//!   optionnels sont des mots uniques en minuscules (`description`, `agent`,
//!   `model`, `variant`, `subtask`), donc le nom Rust est deja identique au nom
//!   TypeScript et aucun `#[serde(rename)]` n'est requis. Si quelqu'un ajoute
//!   un jour un champ en camelCase, il faudra l'ajouter ici.
//! - **Aucun ternaire `?` ni coalescent `??` dans la source.** Donc aucune
//!   divergence possible entre chaine vide et `undefined`. Une chaine vide
//!   envoyee par le TypeScript est un `String` valide et reste un `Some("")`
//!   en Rust : c'est le comportement correct.
//! - **Les cinq champs optionnels sont absents de la sortie JSON quand ils
//!   sont `None`** (`Schema.optional` = cle facultative), d'ou
//!   `skip_serializing_if`.

use serde::{Deserialize, Serialize};

/// Identifiant du schema dans le registre `effect/Schema`.
///
/// En TS : `Schema.Class<Info>("ConfigV2.Command")`.
/// Ce nom sert a l'introspection des schemas cote serveur OpenCode ; il n'a
/// aucune contrepartie necessaire dans la structure Rust, mais on le garde
/// pour que la correspondance avec la source reste lisible.
pub const SCHEMA_IDENTIFIER: &str = "ConfigV2.Command";

/// Definition d'une commande personnalisee, lue depuis la configuration.
///
/// En TS : `export class Info extends Schema.Class<Info>("ConfigV2.Command")`.
///
/// Seuls `template` est obligatoire. Les cinq autres champs sont des
/// surcharges facultatives du comportement par defaut d'une commande.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Corps de la commande, envoye tel quel a l'agent. Champ obligatoire.
    pub template: String,

    /// Texte affiche dans la liste des commandes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Nom de l'agent a utiliser pour cette commande.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,

    /// Reference du modele a utiliser, de la forme `provider/model`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Variante du modele (par exemple le niveau de raisonnement).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,

    /// Vrai si la commande ouvre une sous-tache plutot qu'une session mere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask: Option<bool>,
}

impl Info {
    /// Construit une commande avec seulement `template`, tous les champs
    /// optionnels a `None`. C'est le cas minimal valide.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            description: None,
            agent: None,
            model: None,
            variant: None,
            subtask: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une commande avec seulement le champ obligatoire se deserialize, et
    /// les cinq champs optionnels restent a `None`.
    #[test]
    fn une_commande_avec_seulement_le_template_se_deserialize() {
        let info: Info = serde_json::from_str(r#"{"template":"resume"}"#).unwrap();

        assert_eq!(info.template, "resume");
        assert_eq!(info.description, None);
        assert_eq!(info.agent, None);
        assert_eq!(info.model, None);
        assert_eq!(info.variant, None);
        assert_eq!(info.subtask, None);
    }

    /// Une commande avec les six champs fournis se deserialize integralement.
    #[test]
    fn les_six_champs_se_deserialize_entierement() {
        let info: Info = serde_json::from_str(
            r#"{"template":"run","description":"Lance le projet","agent":"build","model":"anthropic/claude","variant":"high","subtask":true}"#,
        )
        .unwrap();

        assert_eq!(info.template, "run");
        assert_eq!(info.description.as_deref(), Some("Lance le projet"));
        assert_eq!(info.agent.as_deref(), Some("build"));
        assert_eq!(info.model.as_deref(), Some("anthropic/claude"));
        assert_eq!(info.variant.as_deref(), Some("high"));
        assert_eq!(info.subtask, Some(true));
    }

    /// Un champ optionnel absent n'apparait pas dans le JSON produit, et un
    /// champ optionnel present apparait avec exactement le nom du TypeScript.
    /// Ce test verrouille les noms de champs a l'echange.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let info = Info::new("resume");
        let json = serde_json::to_value(&info).unwrap();
        let objet = json.as_object().unwrap();

        // Un seul champ obligatoire, tous les optionnels absents.
        assert_eq!(objet.len(), 1);
        assert_eq!(objet.get("template").and_then(|v| v.as_str()), Some("resume"));

        let complet = Info {
            template: "run".to_string(),
            description: Some("d".to_string()),
            agent: Some("build".to_string()),
            model: Some("m".to_string()),
            variant: Some("high".to_string()),
            subtask: Some(false),
        };
        // `to_value` renvoie une `Value`, qui n'a ni `contains_key` ni `len` :
        // il faut descendre jusqu'a l'objet JSON qu'elle contient.
        let json = serde_json::to_value(&complet).unwrap();
        let objet = json.as_object().unwrap();

        for nom in ["template", "description", "agent", "model", "variant", "subtask"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert_eq!(objet.len(), 6);
    }

    /// Une chaine vide est une chaine valide : elle ne disparait pas. C'est le
    /// cas limite a ne pas confondre avec une valeur `undefined`.
    #[test]
    fn une_chaine_vide_est_conservee_comme_valeur_presente() {
        let info: Info =
            serde_json::from_str(r#"{"template":"","description":"","agent":""}"#).unwrap();

        assert_eq!(info.template, "");
        assert_eq!(info.description.as_deref(), Some(""));
        assert_eq!(info.agent.as_deref(), Some(""));

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json.get("description").and_then(|v| v.as_str()), Some(""));
        assert!(json.get("model").is_none());
    }

    /// Sans `template`, la commande est invalide : le deserialiseur echoue.
    /// Et `subtask` doit rester un booleen, pas une chaine.
    #[test]
    fn une_commande_invalide_est_refusee() {
        assert!(serde_json::from_str::<Info>(r#"{"description":"pas de template"}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"template":"run","subtask":"oui"}"#).is_err());
    }
}
