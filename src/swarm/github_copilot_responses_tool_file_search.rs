//! Portage de `github-copilot/responses/tool/file-search.ts`.
//!
//! La source fait 127 lignes : schemas zod des arguments (`vectorStoreIds`,
//! `maxNumResults`, `ranking`, `filters`) et de la sortie (`queries`,
//! `results`), plus la fabrique d outil id `"openai.file_search"`.
//!
//! Seule la partie pure est portee ici, sans `@ai-sdk/provider-utils` :
//! - l identifiant de l outil ;
//! - les filtres de comparaison et composes ;
//! - les arguments, le classement et la sortie avec leurs cles exactes.
//!
//! Volontairement non porte : `createProviderToolFactoryWithOutputSchema`,
//! la validation zod et l execution de l outil.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifiant de l outil tel qu enregistre par la fabrique.
pub const ID_OUTIL: &str = "openai.file_search";

/// Operateurs de comparaison autorises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperateurComparaison {
    #[serde(rename = "eq")]
    Egal,
    #[serde(rename = "ne")]
    Differend,
    #[serde(rename = "gt")]
    Superieur,
    #[serde(rename = "gte")]
    SuperieurOuEgal,
    #[serde(rename = "lt")]
    Inferieur,
    #[serde(rename = "lte")]
    InferieurOuEgal,
}

/// Valeur d un filtre de comparaison : chaine, nombre ou booleen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValeurFiltre {
    Texte(String),
    Nombre(f64),
    Booleen(bool),
}

/// Filtre de comparaison `{ key, type, value }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiltreComparaison {
    pub key: String,
    #[serde(rename = "type")]
    pub operateur: OperateurComparaison,
    pub value: ValeurFiltre,
}

/// Filtre compose `{ type: and|or, filters: [...] }`, recursif.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiltreCompose {
    #[serde(rename = "type")]
    pub jonction: Jonction,
    pub filters: Vec<Filtre>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Jonction {
    #[serde(rename = "and")]
    Et,
    #[serde(rename = "or")]
    Ou,
}

/// Un filtre, comparaison ou compose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Filtre {
    Comparaison(FiltreComparaison),
    Compose(FiltreCompose),
}

/// Options de classement de la recherche.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Classement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "scoreThreshold")]
    pub seuil_score: Option<f64>,
}

/// Arguments de la recherche de fichiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArgumentsRecherche {
    #[serde(rename = "vectorStoreIds")]
    pub entrepots: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxNumResults")]
    pub max_resultats: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking: Option<Classement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<Filtre>,
}

/// Une ligne de resultat, cles `fileId` en camelCase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LigneResultat {
    pub attributes: BTreeMap<String, serde_json::Value>,
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub filename: String,
    pub score: f64,
    pub text: String,
}

/// Sortie de l outil : requetes + resultats nullable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortieRecherche {
    pub queries: Vec<String>,
    pub results: Option<Vec<LigneResultat>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(ID_OUTIL, "openai.file_search");
    }

    #[test]
    fn les_operateurs_se_serialisent_en_minuscules() {
        assert_eq!(serde_json::to_string(&OperateurComparaison::Egal).unwrap(), "\"eq\"");
        assert_eq!(serde_json::to_string(&OperateurComparaison::SuperieurOuEgal).unwrap(), "\"gte\"");
        assert_eq!(serde_json::to_string(&Jonction::Et).unwrap(), "\"and\"");
        assert_eq!(serde_json::to_string(&Jonction::Ou).unwrap(), "\"or\"");
    }

    #[test]
    fn les_arguments_gardent_la_casse_vector_store_ids() {
        let args = ArgumentsRecherche {
            entrepots: vec!["vs-1".to_string()],
            max_resultats: None,
            ranking: None,
            filters: None,
        };
        let valeur = serde_json::to_value(&args).unwrap();
        assert!(valeur.get("vectorStoreIds").is_some());
        assert!(valeur.get("vector_store_ids").is_none());
        assert!(valeur.get("maxNumResults").is_none());
    }

    #[test]
    fn la_ligne_de_resultat_garde_file_id_en_camel_case() {
        let ligne = LigneResultat {
            attributes: BTreeMap::new(),
            file_id: "f-1".to_string(),
            filename: "doc.txt".to_string(),
            score: 0.9,
            text: "extrait".to_string(),
        };
        let valeur = serde_json::to_value(&ligne).unwrap();
        assert_eq!(valeur.get("fileId").and_then(|v| v.as_str()), Some("f-1"));
        assert!(valeur.get("file_id").is_none());
    }

    #[test]
    fn un_filtre_compose_imbrique_se_lit_et_s_ecrit() {
        let filtre = Filtre::Compose(FiltreCompose {
            jonction: Jonction::Et,
            filters: vec![Filtre::Comparaison(FiltreComparaison {
                key: "auteur".to_string(),
                operateur: OperateurComparaison::Egal,
                value: ValeurFiltre::Texte("moi".to_string()),
            })],
        });
        let valeur = serde_json::to_value(&filtre).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("and"));
        let relue: Filtre = serde_json::from_value(valeur).unwrap();
        assert_eq!(relue, filtre);
    }
}
