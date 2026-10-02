//! Portage de `github-copilot/responses/tool/web-search-preview.ts`.
//!
//! La source fait 103 lignes : schema des arguments (`searchContextSize`,
//! `userLocation`) et fabrique d outil id `"openai.web_search_preview"`.
//! C est la variante preview de `web-search.ts` : memes arguments, pas de
//! sortie typess, meme entree outil (`action` discriminee).
//!
//! Seule la partie pure est portee ici, sans `@ai-sdk/provider-utils`.
//!
//! Volontairement non porte : `createProviderToolFactory`, la validation
//! zod et l execution de la recherche.

use serde::{Deserialize, Serialize};

/// Identifiant de l outil tel qu enregistre par la fabrique.
pub const ID_OUTIL: &str = "openai.web_search_preview";

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

/// Arguments de la preview de recherche web.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgumentsPreview {
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
    fn l_identifiant_est_celui_de_la_preview() {
        assert_eq!(ID_OUTIL, "openai.web_search_preview");
    }

    #[test]
    fn la_preview_partage_les_meme_tailles_que_la_recherche() {
        assert_eq!(serde_json::to_string(&TailleContexte::Basse).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&TailleContexte::Moyenne).unwrap(), "\"medium\"");
        assert_eq!(serde_json::to_string(&TailleContexte::Haute).unwrap(), "\"high\"");
    }

    #[test]
    fn les_arguments_vides_ne_serialisent_rien() {
        let args = ArgumentsPreview::default();
        let valeur = serde_json::to_value(&args).unwrap();
        assert_eq!(valeur, serde_json::json!({}));
    }

    #[test]
    fn les_cles_gardent_leur_camel_case() {
        let args = ArgumentsPreview {
            taille_contexte: Some(TailleContexte::Basse),
            localisation: Some(Localisation::approximative()),
        };
        let valeur = serde_json::to_value(&args).unwrap();
        assert!(valeur.get("searchContextSize").is_some());
        assert!(valeur.get("userLocation").is_some());
        assert!(valeur.get("search_context_size").is_none());
        assert!(valeur.get("user_location").is_none());
    }

    #[test]
    fn les_trois_actions_se_distinguent_sur_le_tag_type() {
        let recherche = ActionRecherche::Recherche { query: None };
        let valeur = serde_json::to_value(&recherche).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("search"));
        let page = ActionRecherche::OuvrirPage {
            url: "https://exemple.fr".to_string(),
        };
        let valeur = serde_json::to_value(&page).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("open_page"));
    }
}
