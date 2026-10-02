//! Portage de `github-copilot/responses/tool/web-search.ts`.
//!
//! La source fait 102 lignes : schema des arguments (`filters`,
//! `searchContextSize`, `userLocation`), schema de l entree outil
//! (`action` discriminee `search | open_page | find`) et fabrique d outil id
//! `"openai.web_search"`.
//!
//! Seule la partie pure est portee ici, sans `@ai-sdk/provider-utils` :
//! types d arguments, de localisation et d action avec leurs cles exactes.
//!
//! Volontairement non porte : `createProviderToolFactory`, la validation
//! zod et l execution de la recherche.

use serde::{Deserialize, Serialize};

/// Identifiant de l outil tel qu enregistre par la fabrique.
pub const ID_OUTIL: &str = "openai.web_search";

/// Taille du contexte de recherche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailleContexte {
    #[serde(rename = "low")]
    Basse,
    #[serde(rename = "medium")]
    Moyenne,
    #[serde(rename = "high")]
    Haute,
}

/// Localisation approximative de l utilisateur.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localisation {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl Localisation {
    pub fn approximative() -> Self {
        Localisation {
            kind: "approximate".to_string(),
            country: None,
            city: None,
            region: None,
            timezone: None,
        }
    }
}

/// Filtres de recherche : domaines autorises.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiltresRecherche {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "allowedDomains")]
    pub domaines_autorises: Option<Vec<String>>,
}

/// Arguments de la recherche web.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgumentsRecherche {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<FiltresRecherche>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "searchContextSize")]
    pub taille_contexte: Option<TailleContexte>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "userLocation")]
    pub localisation: Option<Localisation>,
}

/// Action d outil : recherche, ouverture de page ou motif dans une page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionRecherche {
    #[serde(rename = "search")]
    Recherche {
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
    },
    #[serde(rename = "open_page")]
    OuvrirPage { url: String },
    #[serde(rename = "find")]
    Trouver { url: String, pattern: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(ID_OUTIL, "openai.web_search");
    }

    #[test]
    fn les_tailles_se_serialisent_en_minuscules() {
        assert_eq!(serde_json::to_string(&TailleContexte::Basse).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&TailleContexte::Moyenne).unwrap(), "\"medium\"");
        assert_eq!(serde_json::to_string(&TailleContexte::Haute).unwrap(), "\"high\"");
    }

    #[test]
    fn la_localisation_par_defaut_est_approximative() {
        let loc = Localisation::approximative();
        assert_eq!(loc.kind, "approximate");
        let valeur = serde_json::to_value(&loc).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("approximate"));
    }

    #[test]
    fn les_arguments_gardent_leur_camel_case() {
        let args = ArgumentsRecherche {
            taille_contexte: Some(TailleContexte::Haute),
            ..ArgumentsRecherche::default()
        };
        let valeur = serde_json::to_value(&args).unwrap();
        assert_eq!(valeur.get("searchContextSize").and_then(|v| v.as_str()), Some("high"));
        assert!(valeur.get("search_context_size").is_none());
        assert!(valeur.get("userLocation").is_none());
    }

    #[test]
    fn les_trois_actions_se_distinguent_sur_le_tag_type() {
        let recherche = ActionRecherche::Recherche { query: Some("rust".to_string()) };
        let valeur = serde_json::to_value(&recherche).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("search"));
        let page = ActionRecherche::OuvrirPage {
            url: "https://exemple.fr".to_string(),
        };
        let valeur = serde_json::to_value(&page).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("open_page"));
        let motif = ActionRecherche::Trouver {
            url: "https://exemple.fr".to_string(),
            pattern: "bonjour".to_string(),
        };
        let valeur = serde_json::to_value(&motif).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("find"));
    }

    #[test]
    fn les_domaines_autorises_gardent_leur_casse() {
        let filtres = FiltresRecherche {
            domaines_autorises: Some(vec!["exemple.fr".to_string()]),
        };
        let valeur = serde_json::to_value(&filtres).unwrap();
        assert!(valeur.get("allowedDomains").is_some());
        assert!(valeur.get("allowed_domains").is_none());
    }
}
