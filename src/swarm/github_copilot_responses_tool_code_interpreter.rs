//! Portage de `github-copilot/responses/tool/code-interpreter.ts`.
//!
//! La source fait 87 lignes : trois schemas zod (`input`, `output`, `args`),
//! une fabrique d outil (`codeInterpreterToolFactory`, id
//! `"openai.code_interpreter"`) et son aide `codeInterpreter(args = {})`.
//!
//! Seule la partie pure est portee ici, sans `@ai-sdk/provider-utils` :
//! - l identifiant de l outil ;
//! - les types d entree, de sortie et d arguments ;
//! - la resolution de l argument par defaut (`{}`).
//!
//! Volontairement non porte : `createProviderToolFactoryWithOutputSchema`,
//! la validation zod et l execution de l outil.

use serde::{Deserialize, Serialize};

/// Identifiant de l outil tel qu enregistre par la fabrique.
pub const ID_OUTIL: &str = "openai.code_interpreter";

/// Conteneur : soit un id direct, soit un objet `{ fileIds }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Conteneur {
    Id(String),
    Fichiers { #[serde(rename = "fileIds")] file_ids: Vec<String> },
}

/// Arguments de `codeInterpreter(args = {})`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgumentsInterprete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<Conteneur>,
}

impl ArgumentsInterprete {
    /// L argument par defaut de la source : `{}`.
    pub fn par_defaut() -> Self {
        ArgumentsInterprete { container: None }
    }
}

/// Entree de l outil : `{ code, containerId }`, les deux lisibles cote API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntreeInterprete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(rename = "containerId")]
    pub container_id: String,
}

/// Une sortie d interprete : journaux ou image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SortieInterprete {
    #[serde(rename = "logs")]
    Journaux { logs: String },
    #[serde(rename = "image")]
    Image { url: String },
}

/// Sortie de l outil : `{ outputs }`, nullable cote source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortieOutil {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<SortieInterprete>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(ID_OUTIL, "openai.code_interpreter");
    }

    #[test]
    fn l_argument_par_defaut_est_vide() {
        assert_eq!(ArgumentsInterprete::par_defaut(), ArgumentsInterprete { container: None });
        assert_eq!(ArgumentsInterprete::default(), ArgumentsInterprete::par_defaut());
    }

    #[test]
    fn le_conteneur_accepte_un_id_ou_des_fichiers() {
        let direct: Conteneur = serde_json::from_str("\"cont-1\"").unwrap();
        assert_eq!(direct, Conteneur::Id("cont-1".to_string()));
        let fichiers: Conteneur = serde_json::from_str(r#"{"fileIds":["f1","f2"]}"#).unwrap();
        assert_eq!(
            fichiers,
            Conteneur::Fichiers {
                file_ids: vec!["f1".to_string(), "f2".to_string()]
            }
        );
    }

    #[test]
    fn l_entree_garde_la_casse_container_id() {
        let entree = EntreeInterprete {
            code: Some("print(1)".to_string()),
            container_id: "cont-1".to_string(),
        };
        let valeur = serde_json::to_value(&entree).unwrap();
        assert_eq!(valeur.get("containerId").and_then(|v| v.as_str()), Some("cont-1"));
        assert!(valeur.get("container_id").is_none());
        assert!(valeur.get("containerid").is_none());
    }

    #[test]
    fn la_sortie_distribue_journaux_et_image_sur_le_tag_type() {
        let journaux = SortieInterprete::Journaux {
            logs: "ok".to_string(),
        };
        let valeur = serde_json::to_value(&journaux).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("logs"));
        let image = SortieInterprete::Image {
            url: "https://img".to_string(),
        };
        let valeur = serde_json::to_value(&image).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("image"));
    }

    #[test]
    fn une_sortie_sans_contenu_se_lit_et_s_ecrit() {
        let sortie = SortieOutil { outputs: None };
        let valeur = serde_json::to_value(&sortie).unwrap();
        assert!(valeur.get("outputs").is_none());
        let relue: SortieOutil = serde_json::from_value(valeur).unwrap();
        assert_eq!(relue, sortie);
    }
}
