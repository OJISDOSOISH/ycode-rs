//! Resolution d'un modele pour une session, d'apres
//! `opencode/packages/core/src/session/runner/model.ts`.
//!
//! Cette module decide quel protocole HTTP employer et comment s authentifier.
//! Trois decisions, dans cet ordre :
//!
//! 1. La **variante** demandee (effort de raisonnement) est appliquee.
//! 2. Le protocole est choisi d apres le couple type / package de l API.
//! 3. La **cle d API est retiree du corps de requete** avant l envoi.
//!
//! Le point 3 est le plus important et le plus facile a oublier. Dans le TS,
//! `withDefaults` fait `Object.fromEntries(entries.filter(([key]) => key !==
//! "apiKey"))` : la cle est lue, puis retirees du corps. Sans cette etape, la
//! cle d API partirait dans le corps JSON de chaque requete, donc dans les
//! journaux du provider, les rapports d erreur et les traces de debogage.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Type d'API d'un modele.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiType {
    /// Route passant par le SDK AI interne, avec un paquet sous-jacent.
    #[serde(rename = "aisdk")]
    AiSdk {
        /// Paquet AI SDK : `openai`, `anthropic` ou `openai-compatible`.
        package: String,
    },
    /// Autre type, non gere par ce portage.
    #[serde(other)]
    Other,
}

/// Limites d'un modele.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Fenetre de contexte, en tokens.
    pub context: u64,
    /// Sortie maximale, en tokens.
    pub output: u64,
}

/// Requete associee a un modele : en-tetes, corps, identifiants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Request {
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: BTreeMap<String, serde_json::Value>,
    /// Variante par defaut du modele.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Description d'un API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Api {
    #[serde(rename = "type")]
    pub kind: ApiType,
    /// URL de base, obligatoire pour un endpoint compatible OpenAI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Identifiant du modele vu par le provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Une variante de raisonnement d'un modele.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: BTreeMap<String, serde_json::Value>,
}

/// Un modele du catalogue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    pub api: Api,
    pub request: Request,
    pub limit: Limits,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

/// Selection de modele d'une session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionModelSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Erreurs de resolution, equivalents des quatre `TaggedErrorClass` du TS.
///
/// On les regroupe en un `enum` : l appelant veut savoir « pourquoi ca n a pas
/// marche », pas avoir quatre types distincts a faire circuler.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("aucun modele disponible pour la session {session_id}")]
    NotSelected { session_id: String },
    #[error("modele indisponible : {provider_id}/{model_id}")]
    Unavailable { provider_id: String, model_id: String },
    #[error("variante indisponible pour {provider_id}/{model_id} : {variant}")]
    VariantUnavailable { provider_id: String, model_id: String, variant: String },
    #[error("API non supportee pour {provider_id}/{model_id} : {api}")]
    UnsupportedApi { provider_id: String, model_id: String, api: String },
}

/// Protocole retenu apres resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// API Responses, style OpenAI.
    OpenAiResponses,
    /// API Messages, style Anthropic.
    AnthropicMessages,
    /// Chat completions compatible OpenAI.
    OpenAiCompatibleChat,
}

impl Protocol {
    /// Comment la cle d API est transmise.
    ///
    /// Anthropic ne prend pas de jeton porteur : la cle va dans un en-tete
    /// dedie. Utiliser un jeton bearer chez Anthropic est une erreur silencieuse
    /// qui se manifeste par un 401.
    pub fn auth_style(self) -> AuthStyle {
        match self {
            Protocol::AnthropicMessages => AuthStyle::Header("x-api-key".to_string()),
            _ => AuthStyle::Bearer,
        }
    }
}

/// Style d'authentification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStyle {
    Bearer,
    Header(String),
}

/// Ce qu'un modele resolu produit.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub protocol: Protocol,
    pub model_id: String,
    pub endpoint: Option<String>,
    /// En-tetes, cle d API exclue.
    pub headers: BTreeMap<String, String>,
    /// Corps de requete, cle d API exclue.
    pub body: BTreeMap<String, serde_json::Value>,
    /// Cle d API, a transmettre par le canal d'authentification.
    pub api_key: Option<String>,
    pub limits: Limits,
}

/// Nom affichable d'une API, pour les messages d'erreur.
///
/// Le TS produit `aisdk:@ai-sdk/openai` pour le SDK et `aisdk` sinon. On
/// reproduit exactement, parce que ces noms apparaissent dans les messages
/// visibles par l'utilisateur.
pub fn api_name(api: &Api) -> String {
    match &api.kind {
        ApiType::AiSdk { package } => format!("aisdk:{package}"),
        ApiType::Other => "aisdk".to_string(),
    }
}

/// Le modele supporte-t-il le routage ?
///
/// Une API compatible OpenAI sans URL de base n'a nulle part ou envoyer la
/// requete : elle est donc refusee, plutot que d'echouer plus tard avec une
/// erreur de connexion incomprehensible.
pub fn supported(model: &ModelInfo) -> bool {
    match &model.api.kind {
        ApiType::AiSdk { package } => match package.as_str() {
            "openai" | "anthropic" => true,
            "openai-compatible" => model.api.url.is_some(),
            _ => false,
        },
        ApiType::Other => false,
    }
}

/// Applique une variante au modele.
///
/// `"default"` et l'absence de variante designent la variante par defaut du
/// modele, et ne sont jamaisConsideres comme une erreur : demander la variante
/// par defaut ne doit pas echouer meme si elle n'apparait pas dans la liste.
pub fn with_variant(model: &ModelInfo, variant_id: Option<&str>) -> Result<ModelInfo, ModelError> {
    let explicit = variant_id.filter(|v| *v != "default");

    let id = explicit.or(model.request.variant.as_deref());
    let Some(id) = id else {
        return Ok(model.clone());
    };

    let Some(variant) = model.variants.iter().find(|v| v.id == id) else {
        // Pas de variante, et l'utilisateur n'en a pas demande une : on garde
        // le modele tel quel.
        if explicit.is_none() {
            return Ok(model.clone());
        }
        return Err(ModelError::VariantUnavailable {
            provider_id: model.provider_id.clone(),
            model_id: model.id.clone(),
            variant: id.to_string(),
        });
    };

    // Les en-tetes et le corps de la variante sont fusionnes par-dessus ceux du
    // modele : la variante gagne. C'est l'ordre du TS (`Object.assign` sur le
    // brouillon).
    let mut merged = model.clone();
    merged.request.headers.extend(variant.headers.clone());
    merged.request.body.extend(variant.body.clone());
    Ok(merged)
}

/// Resout un modele : variante, puis protocole, puis authentification.
///
/// C'est l equivalents de `resolve` dans le TS : `withVariant` puis
/// `fromCatalogModel`.
pub fn resolve(model: &ModelInfo, session: &SessionModelSelection) -> Result<Resolved, ModelError> {
    let model = with_variant(model, session.variant.as_deref())?;

    let protocol = match &model.api.kind {
        ApiType::AiSdk { package } => match package.as_str() {
            "openai" => Protocol::OpenAiResponses,
            "anthropic" => Protocol::AnthropicMessages,
            "openai-compatible" if model.api.url.is_some() => Protocol::OpenAiCompatibleChat,
            other => {
                return Err(ModelError::UnsupportedApi {
                    provider_id: model.provider_id.clone(),
                    model_id: model.id.clone(),
                    api: format!("aisdk:{other}"),
                })
            }
        },
        ApiType::Other => {
            return Err(ModelError::UnsupportedApi {
                provider_id: model.provider_id.clone(),
                model_id: model.id.clone(),
                api: api_name(&model.api),
            })
        }
    };

    // La cle est lue puis retiree du corps. Elle ne doit jamais partir dans la
    // charge utile de la requete.
    let api_key = model.request.body.get("apiKey").and_then(|v| v.as_str()).map(str::to_string);

    let mut body = model.request.body.clone();
    body.remove("apiKey");

    Ok(Resolved {
        protocol,
        model_id: model.api.id.clone().unwrap_or_else(|| model.id.clone()),
        endpoint: model.api.url.clone(),
        headers: model.request.headers.clone(),
        body,
        api_key,
        limits: model.limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(package: &str, url: Option<&str>) -> ModelInfo {
        ModelInfo {
            id: "gpt-4".into(),
            provider_id: "openai".into(),
            api: Api { kind: ApiType::AiSdk { package: package.into() }, url: url.map(str::to_string), id: Some("gpt-4".into()) },
            request: Request::default(),
            limit: Limits { context: 200_000, output: 8_000 },
            variants: vec![],
        }
    }

    fn with_key(m: &mut ModelInfo, key: &str) {
        m.request.body.insert("apiKey".into(), serde_json::json!(key));
    }

    #[test]
    fn la_cle_d_api_est_retiree_du_corps_de_requete() {
        // Le point le plus important du module : la cle ne doit pas partir dans
        // le JSON de la requete, sinon elle finit dans les journaux du provider.
        let mut m = model("openai", None);
        with_key(&mut m, "sk-secret");
        let r = resolve(&m, &SessionModelSelection::default()).unwrap();
        assert_eq!(r.api_key.as_deref(), Some("sk-secret"));
        assert!(!r.body.contains_key("apiKey"), "la cle ne doit pas rester dans le corps");
    }

    #[test]
    fn les_autres_champs_du_corps_sont_conserveres() {
        let mut m = model("openai", None);
        m.request.body.insert("temperature".into(), serde_json::json!(0.7));
        with_key(&mut m, "sk-secret");
        let r = resolve(&m, &SessionModelSelection::default()).unwrap();
        assert_eq!(r.body.get("temperature"), Some(&serde_json::json!(0.7)));
    }

    #[test]
    fn le_protocole_suit_le_paquet() {
        assert_eq!(resolve(&model("openai", None), &SessionModelSelection::default()).unwrap().protocol, Protocol::OpenAiResponses);
        assert_eq!(resolve(&model("anthropic", None), &SessionModelSelection::default()).unwrap().protocol, Protocol::AnthropicMessages);
        let c = model("openai-compatible", Some("http://localhost:1234/v1"));
        assert_eq!(resolve(&c, &SessionModelSelection::default()).unwrap().protocol, Protocol::OpenAiCompatibleChat);
    }

    #[test]
    fn anthropic_utilise_un_en_tete_dedie() {
        // Un jeton bearer chez Anthropic echoue en 401 sans message clair.
        assert_eq!(Protocol::AnthropicMessages.auth_style(), AuthStyle::Header("x-api-key".into()));
        assert_eq!(Protocol::OpenAiResponses.auth_style(), AuthStyle::Bearer);
    }

    #[test]
    fn un_compatible_sans_url_est_refuse() {
        // Aucune ou envoyer : mieux vaut une erreur explicite qu une connexion
        // impossible plus tard.
        let c = model("openai-compatible", None);
        assert!(!supported(&c));
        assert!(matches!(
            resolve(&c, &SessionModelSelection::default()),
            Err(ModelError::UnsupportedApi { .. })
        ));
    }

    #[test]
    fn un_paquet_inconnu_est_refuse_nomme() {
        let m = model("mystere", None);
        let e = resolve(&m, &SessionModelSelection::default()).unwrap_err();
        match e {
            ModelError::UnsupportedApi { api, .. } => assert_eq!(api, "aisdk:mystere"),
            other => panic!("attendu UnsupportedApi, obtenu {other:?}"),
        }
    }

    #[test]
    fn la_variante_est_fusionnee_par_dessus_le_modele() {
        let mut m = model("openai", None);
        m.request.body.insert("reasoning_effort".into(), serde_json::json!("medium"));
        m.variants.push(Variant {
            id: "high".into(),
            headers: BTreeMap::new(),
            body: BTreeMap::from([("reasoning_effort".into(), serde_json::json!("high"))]),
        });
        let session = SessionModelSelection { variant: Some("high".into()) };
        let r = resolve(&m, &session).unwrap();
        assert_eq!(r.body.get("reasoning_effort"), Some(&serde_json::json!("high")));
    }

    #[test]
    fn demander_la_variante_par_defaut_ne_donne_pas_d_erreur() {
        // "default" designe la variante par defaut, meme absente de la liste.
        let m = model("openai", None);
        for v in [Some("default"), None] {
            let session = SessionModelSelection { variant: v.map(str::to_string) };
            assert!(resolve(&m, &session).is_ok());
        }
    }

    #[test]
    fn une_variante_inexistante_demandee_explicitement_echoue() {
        let m = model("openai", None);
        let session = SessionModelSelection { variant: Some("impossible".into()) };
        assert!(matches!(resolve(&m, &session), Err(ModelError::VariantUnavailable { .. })));
    }

    #[test]
    fn les_identifiants_sont_en_camelcase_dans_le_json() {
        let m = model("openai", None);
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["providerID"], "openai");
        assert!(v.get("provider_id").is_none());
    }
}
