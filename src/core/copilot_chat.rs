//! Portage Rust de `openai-compatible-chat-language-model.ts` (chat Copilot).
//!
//! Logique metier pure uniquement, decoupage en fonctions pures.
//! Volontairement non porte : transport HTTP async, SSE, TransformStream,
//! `doGenerate`, `doStream`, `postJsonToApi`, `combineHeaders`, gestionnaires
//! de reponse, `generateId` aleatoire, `headers()`, `url()`, `fetch`,
//! `metadataExtractor`, conversion des messages et preparation des outils.
//! Ces parties dependent du runtime async et sont signalees comme sautees.
//!
//! Piege `?` contre `??` : un texte vide est ignore dans le contenu genere
//! (`length > 0`), alors qu-un token manquant (`null`) garde `None` sans
//! ecraser les details deja accumules. Les deux cas sont testes.
//!
//! Piege des noms : `reasoningOpaque`, `toolCallId`, `toolName` gardent leur
//! casse exacte via `rename`. Les champs snake_case de l-API passent bruts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Chemin d-API utilise par `doGenerate` et `doStream`.
pub const CHEMIN_COMPLETIONS: &str = "/chat/completions";

/// Identifiants stables des parties logiques du flux (constantes d-origine).
pub const ID_RAISONNEMENT: &str = "reasoning-0";
pub const ID_TEXTE: &str = "txt-0";

/// Repli deterministe quand un appel d-outil arrive sans `id`.
///
/// L-original appelle `generateId()` (aleatoire). Ici on ne fabrique pas
/// d-aleatoire dans la logique pure : l-appelant remplace cette valeur.
pub const ID_OUTIL_REPLI: &str = "appel-sans-id";

// ---------------------------------------------------------------------------
// Config phenotypique minimale (les closures JS ne sont pas portables)
// ---------------------------------------------------------------------------

/// Sous-ensemble portable de `OpenAICompatibleChatConfig`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigChatCopilot {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_structured_outputs: Option<bool>,
}

impl ConfigChatCopilot {
    pub fn supporte_structured(&self) -> bool {
        self.supports_structured_outputs.unwrap_or(false)
    }

    pub fn inclut_usage(&self) -> bool {
        self.include_usage.unwrap_or(false)
    }
}

/// `this.config.provider.split(".")[0].trim()` en pur.
pub fn nom_options_fournisseur(provider: &str) -> String {
    provider.split('.').next().unwrap_or("").trim().to_string()
}

// ---------------------------------------------------------------------------
// Avertissements
// ---------------------------------------------------------------------------

/// `SharedV3Warning` limite au cas `unsupported` utilise ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Avertissement {
    #[serde(rename = "unsupported")]
    NonSupporte {
        feature: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
}

/// `topK` n-est jamais supporte : present => un avertissement.
pub fn avertir_top_k(top_k: Option<f64>) -> Option<Avertissement> {
    if top_k.is_some() {
        Some(Avertissement::NonSupporte {
            feature: "topK".to_string(),
            details: None,
        })
    } else {
        None
    }
}

/// Avertissement quand un schema JSON est demande sans sorties structurees.
pub fn avertir_format_reponse(type_est_json: bool, schema_present: bool, supporte: bool) -> Option<Avertissement> {
    if type_est_json && schema_present && !supporte {
        Some(Avertissement::NonSupporte {
            feature: "responseFormat".to_string(),
            details: Some(
                "JSON response format schema is only supported with structuredOutputs".to_string(),
            ),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Format de reponse demande
// ---------------------------------------------------------------------------

/// Corps interne de `{ type: "json_schema", json_schema: {...} }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaJsonDemande {
    pub schema: Value,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Union taggee sur `type`, renommage explicite par variante.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FormatReponseDemande {
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(rename = "json_schema")]
    JsonSchema {
        #[serde(rename = "json_schema")]
        json_schema: SchemaJsonDemande,
    },
}

/// Selection du format : `json` + schema + support => `json_schema`,
/// `json` sinon => `json_object`, autre type => `None`.
pub fn choisir_format_reponse(
    type_est_json: bool,
    schema: Option<Value>,
    nom: Option<&str>,
    description: Option<String>,
    supporte: bool,
) -> Option<FormatReponseDemande> {
    if !type_est_json {
        return None;
    }
    match (supporte, schema) {
        (true, Some(s)) => Some(FormatReponseDemande::JsonSchema {
            json_schema: SchemaJsonDemande {
                schema: s,
                name: nom.unwrap_or("response").to_string(),
                description,
            },
        }),
        _ => Some(FormatReponseDemande::JsonObject),
    }
}

// ---------------------------------------------------------------------------
// Arguments de requete (partie pure de `getArgs`)
// ---------------------------------------------------------------------------

/// Corps JSON envoye a `/chat/completions`, sans `messages` opaques.
/// `messages`, `tools` et `tool_choice` restent `Value` car leurs schemas
/// vivent dans d-autres fichiers (non inventes ici).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArgsRequeteChat {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<FormatReponseDemande>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
    /// `compatibleOptions.thinking_budget` copie tel quel par `getArgs`.
    ///
    /// Le TS le type en `z.number()` dans `openaiCompatibleProviderOptions`,
    /// donc un NOMBRE sur le fil : `Option<String>` mettrait `"2048"` entre
    /// guillemets dans le corps de la requete. Type aligne sur la structure
    /// d options portee dans `github_copilot_chat_openai_compatible_chat_options.rs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<f64>,
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    /// Cles supplementaires du fournisseur, deja filtre des cles connues.
    #[serde(default, flatten)]
    pub extras: BTreeMap<String, Value>,
}

/// `stream_options` vaut `{ include_usage: true }` seulement en mode strict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamOptions {
    #[serde(rename = "include_usage")]
    pub include_usage: bool,
}

pub fn options_flux(include_usage: bool) -> Option<StreamOptions> {
    if include_usage {
        Some(StreamOptions { include_usage: true })
    } else {
        None
    }
}

/// Garde les entrees `providerOptions[nom]` dont la cle n-est pas connue.
///
/// Miroir pur de `Object.entries(...).filter(([k]) => !shape.includes(k))`.
/// `?? {}` : une map absente donne une map vide.
pub fn filtrer_extras(
    brutes: Option<&BTreeMap<String, Value>>,
    cles_connues: &[&str],
) -> BTreeMap<String, Value> {
    match brutes {
        None => BTreeMap::new(),
        Some(m) => m
            .iter()
            .filter(|(k, _)| !cles_connues.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Usage de tokens (schema limite d-origine)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailsPromptTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailsCompletionTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_prediction_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_prediction_tokens: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<DetailsPromptTokens>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<DetailsCompletionTokens>,
}

// ---------------------------------------------------------------------------
// Reponse non flux (schema limite d-origine)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FonctionAppelOutil {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelOutilReponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub function: FonctionAppelOutil,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageAssistant {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_opaque: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<AppelOutilReponse>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoixReponse {
    pub message: MessageAssistant,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReponseChat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub choices: Vec<ChoixReponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageTokens>,
}

// ---------------------------------------------------------------------------
// Morceau de flux (schema limite d-origine, union avec erreur)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FonctionDeltaOutil {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaAppelOutil {
    pub index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub function: FonctionDeltaOutil,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaMorceau {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_opaque: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<DeltaAppelOutil>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoixMorceau {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<DeltaMorceau>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MorceauChat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub choices: Vec<ChoixMorceau>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageTokens>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErreurInterne {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MorceauErreur {
    pub error: ErreurInterne,
}

/// Union sans tag discriminant (`z.union([morceau, errorSchema])`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MorceauOuErreur {
    Morceau(MorceauChat),
    Erreur(MorceauErreur),
}

// ---------------------------------------------------------------------------
// Meta Copilot (casse exacte verifiee par test)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetaCopilot {
    #[serde(rename = "reasoningOpaque", skip_serializing_if = "Option::is_none")]
    pub reasoning_opaque: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnveloppeMetaCopilot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copilot: Option<MetaCopilot>,
}

pub fn enveloppe_opaque(reasoning_opaque: Option<&str>) -> Option<EnveloppeMetaCopilot> {
    reasoning_opaque.map(|s| EnveloppeMetaCopilot {
        copilot: Some(MetaCopilot {
            reasoning_opaque: Some(s.to_string()),
        }),
    })
}

// ---------------------------------------------------------------------------
// Contenu genere (partie pure de `doGenerate`)
// ---------------------------------------------------------------------------

/// Contenus `LanguageModelV3Content` limites a ce que le fichier produit.
/// Union taggee sur `type`, renommage explicite par variante.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContenuGenere {
    #[serde(rename = "text")]
    Texte {
        text: String,
        #[serde(rename = "reasoningOpaque", skip_serializing_if = "Option::is_none")]
        reasoning_opaque: Option<String>,
    },
    #[serde(rename = "reasoning")]
    Raisonnement {
        text: String,
        #[serde(rename = "reasoningOpaque", skip_serializing_if = "Option::is_none")]
        reasoning_opaque: Option<String>,
    },
    #[serde(rename = "tool-call")]
    AppelOutil {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        input: String,
        #[serde(rename = "reasoningOpaque", skip_serializing_if = "Option::is_none")]
        reasoning_opaque: Option<String>,
    },
}

/// Assemble texte, raisonnement puis appels d-outils.
///
/// Regle d-origine : `!= null && length > 0`, donc `Some("")` est ignore
/// (chaine vide falsy), alors que `None` reste absent (nullish).
pub fn assembler_contenu_genere(message: &MessageAssistant) -> Vec<ContenuGenere> {
    let mut out = Vec::new();
    let opaque = message.reasoning_opaque.clone();

    if let Some(t) = message.content.as_deref() {
        if !t.is_empty() {
            out.push(ContenuGenere::Texte {
                text: t.to_string(),
                reasoning_opaque: opaque.clone(),
            });
        }
    }

    if let Some(r) = message.reasoning_text.as_deref() {
        if !r.is_empty() {
            out.push(ContenuGenere::Raisonnement {
                text: r.to_string(),
                reasoning_opaque: opaque.clone(),
            });
        }
    }

    if let Some(appels) = message.tool_calls.as_deref() {
        for appel in appels {
            out.push(ContenuGenere::AppelOutil {
                tool_call_id: appel
                    .id
                    .clone()
                    .unwrap_or_else(|| ID_OUTIL_REPLI.to_string()),
                tool_name: appel.function.name.clone(),
                input: appel.function.arguments.clone(),
                reasoning_opaque: opaque.clone(),
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Raison de fin
// ---------------------------------------------------------------------------

/// Valeurs unifiees emises vers l-exterieur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FinUnifie {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Longueur,
    #[serde(rename = "content-filter")]
    FiltreContenu,
    #[serde(rename = "tool-calls")]
    AppelsOutil,
    #[serde(rename = "error")]
    Erreur,
    #[serde(rename = "other")]
    Autre,
}

/// Etat initial du flux : `{ unified: "other", raw: undefined }`.
pub fn fin_initiale() -> (FinUnifie, Option<String>) {
    (FinUnifie::Autre, None)
}

/// Etat d-erreur du flux : `{ unified: "error", raw: undefined }`.
pub fn fin_erreur() -> (FinUnifie, Option<String>) {
    (FinUnifie::Erreur, None)
}

/// Convertit une raison brute OpenAI en raison unifiee.
///
/// Reconstitution locale de `mapOpenAICompatibleFinishReason` (fichier
/// voisin non lu ici) : `stop`, `length`, `tool_calls` / `function_call`,
/// `content_filter`, le reste donne `other`. Le brut est conserve tel quel.
pub fn convertir_fin(brute: Option<&str>) -> (FinUnifie, Option<String>) {
    match brute {
        Some("stop") => (FinUnifie::Stop, Some("stop".to_string())),
        Some("length") => (FinUnifie::Longueur, Some("length".to_string())),
        Some("tool_calls") | Some("function_call") => {
            (FinUnifie::AppelsOutil, brute.map(|s| s.to_string()))
        }
        Some("content_filter") => (FinUnifie::FiltreContenu, brute.map(|s| s.to_string())),
        Some(autre) => (FinUnifie::Autre, Some(autre.to_string())),
        None => (FinUnifie::Autre, None),
    }
}

// ---------------------------------------------------------------------------
// Usage genere (parties pures de `doGenerate` et du `flush`)
// ---------------------------------------------------------------------------

/// `usage.inputTokens` of the TS `doGenerate` return value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntreesGenerees {
    #[serde(rename = "total", skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    #[serde(rename = "noCache", skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<i64>,
    #[serde(rename = "cacheRead", skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<i64>,
    #[serde(rename = "cacheWrite", skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<i64>,
}

/// `usage.outputTokens` of the TS `doGenerate` return value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SortiesGenerees {
    #[serde(rename = "total", skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    #[serde(rename = "text", skip_serializing_if = "Option::is_none")]
    pub text: Option<i64>,
    #[serde(rename = "reasoning", skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<i64>,
}

/// The `usage` block the TS returns, nested exactly like it.
///
/// It used to be a flat struct with French field names, deriving Serialize:
/// harmless while nothing serialised it, a wire bug the moment the response
/// boundary is wired up. Names and nesting now match the TS, so the mapping is
/// a plain `serde_json::to_value` instead of a hand-written translation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UsageGenere {
    #[serde(rename = "inputTokens")]
    pub input_tokens: EntreesGenerees,
    #[serde(rename = "outputTokens")]
    pub output_tokens: SortiesGenerees,
}

/// `noCache = total - cached` seulement quand les deux sont connus.
/// Miroir exact du calcul du `flush` du flux.
pub fn calculer_sans_cache(prompt_tokens: Option<i64>, cached_tokens: Option<i64>) -> Option<i64> {
    match (prompt_tokens, cached_tokens) {
        (Some(total), Some(cached)) => Some(total - cached),
        _ => None,
    }
}

/// Usage non flux : lecture directe, `noCache` reste indefini.
pub fn usage_non_flux(usage: Option<&UsageTokens>) -> UsageGenere {
    match usage {
        None => UsageGenere::default(),
        Some(u) => UsageGenere {
            input_tokens: EntreesGenerees {
                total: u.prompt_tokens,
                no_cache: None,
                cache_read: u.prompt_tokens_details.as_ref().and_then(|d| d.cached_tokens),
                cache_write: None,
            },
            output_tokens: SortiesGenerees {
                total: u.completion_tokens,
                text: None,
                reasoning: u
                    .completion_tokens_details
                    .as_ref()
                    .and_then(|d| d.reasoning_tokens),
            },
        },
    }
}

/// Accumulateur d-usage du flux : le dernier morceau gagne.
///
/// Les compteurs directs suivent `?? undefined` (absent => `None`, meme si
/// un morceau precedent avait une valeur). Les details ne sont ecrases que
/// quand le morceau les fournit (`!= null`), sinon ils survivent.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccumulateurUsage {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub accepted_prediction_tokens: Option<i64>,
    pub rejected_prediction_tokens: Option<i64>,
}

impl AccumulateurUsage {
    pub fn nouveau() -> Self {
        Self::default()
    }

    pub fn appliquer(&mut self, usage: Option<&UsageTokens>) {
        let Some(u) = usage else { return };
        self.prompt_tokens = u.prompt_tokens;
        self.completion_tokens = u.completion_tokens;
        self.total_tokens = u.total_tokens;
        if let Some(d) = u.prompt_tokens_details.as_ref() {
            if d.cached_tokens.is_some() {
                self.cached_tokens = d.cached_tokens;
            }
        }
        if let Some(d) = u.completion_tokens_details.as_ref() {
            if d.reasoning_tokens.is_some() {
                self.reasoning_tokens = d.reasoning_tokens;
            }
            if d.accepted_prediction_tokens.is_some() {
                self.accepted_prediction_tokens = d.accepted_prediction_tokens;
            }
            if d.rejected_prediction_tokens.is_some() {
                self.rejected_prediction_tokens = d.rejected_prediction_tokens;
            }
        }
    }

    pub fn vers_usage_genere(&self) -> UsageGenere {
        UsageGenere {
            input_tokens: EntreesGenerees {
                total: self.prompt_tokens,
                no_cache: calculer_sans_cache(self.prompt_tokens, self.cached_tokens),
                cache_read: self.cached_tokens,
                cache_write: None,
            },
            output_tokens: SortiesGenerees {
                total: self.completion_tokens,
                text: None,
                reasoning: self.reasoning_tokens,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers de flux purs (sans TransformStream)
// ---------------------------------------------------------------------------

/// Equivalent de `isParsableJson` : le texte doit parser en JSON.
pub fn est_json_analysable(texte: &str) -> bool {
    serde_json::from_str::<Value>(texte).is_ok()
}

/// Fusionne un delta d-arguments sur l-existant (`?? ""`).
pub fn fusionner_arguments(existant: &str, delta: Option<&str>) -> String {
    format!("{}{}", existant, delta.unwrap_or(""))
}

/// Un seul `reasoning_opaque` par reponse, sinon erreur.
///
/// Miroir du `throw InvalidResponseDataError` quand deux valeurs arrivent.
pub fn controler_opaque_unique(
    existant: Option<&str>,
    nouveau: Option<&str>,
) -> Result<Option<String>, String> {
    match (existant, nouveau) {
        (Some(_), Some(_)) => Err(
            "Multiple reasoning_opaque values received in a single response. Only one thinking part per response is supported.".to_string(),
        ),
        (Some(e), None) => Ok(Some(e.to_string())),
        (_, Some(n)) => Ok(Some(n.to_string())),
        (None, None) => Ok(None),
    }
}

/// Valide la creation d-un appel d-outil (messages d-erreur d-origine).
pub fn valider_nouvel_appel_outil(id: Option<&str>, nom: Option<&str>) -> Result<(), String> {
    if id.is_none() {
        return Err("Expected 'id' to be a string.".to_string());
    }
    if nom.is_none() {
        return Err("Expected 'function.name' to be a string.".to_string());
    }
    Ok(())
}

/// Appel d-outil en cours d-accumulation dans le flux.
#[derive(Debug, Clone, PartialEq)]
pub struct AppelOutilEnCours {
    pub id: String,
    pub nom: String,
    pub arguments: String,
    pub termine: bool,
}

/// Etat des appels d-outils indexes par `index` (deterministe).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccumulateurAppelsOutil {
    pub appels: BTreeMap<u64, AppelOutilEnCours>,
}

impl AccumulateurAppelsOutil {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Cree l-entree `index`. Retourne `true` si l-appel est deja complet
    /// (fournisseur qui envoie tout dans un seul morceau JSON valide).
    pub fn demarrer(
        &mut self,
        index: u64,
        id: Option<&str>,
        nom: Option<&str>,
        arguments: Option<&str>,
    ) -> Result<bool, String> {
        if self.appels.contains_key(&index) {
            return Err(format!("index d-appel d-outil deja utilise : {index}"));
        }
        valider_nouvel_appel_outil(id, nom)?;
        let args = arguments.unwrap_or("").to_string();
        let termine = !args.is_empty() && est_json_analysable(&args);
        self.appels.insert(
            index,
            AppelOutilEnCours {
                id: id.unwrap_or(ID_OUTIL_REPLI).to_string(),
                nom: nom.unwrap_or("").to_string(),
                arguments: args,
                termine,
            },
        );
        Ok(termine)
    }

    /// Ajoute un delta a une entree existante non terminee.
    ///
    /// Retourne `None` quand il faut ignorer le delta (index inconnu ou
    /// appel deja termine, soit les deux `continue` d-origine), sinon
    /// `Some(true)` si l-appel vient de se terminer.
    pub fn ajouter_arguments(&mut self, index: u64, delta: Option<&str>) -> Option<bool> {
        let appel = self.appels.get_mut(&index)?;
        if appel.termine {
            return None;
        }
        appel.arguments = fusionner_arguments(&appel.arguments, delta);
        if !appel.nom.is_empty()
            && !appel.arguments.is_empty()
            && est_json_analysable(&appel.arguments)
        {
            appel.termine = true;
            Some(true)
        } else {
            Some(false)
        }
    }

    /// Indices des appels non termines (emis de force au `flush`).
    pub fn inacheves(&self) -> Vec<u64> {
        self.appels
            .iter()
            .filter(|(_, a)| !a.termine)
            .map(|(i, _)| *i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_nom_de_fournisseur_coupe_au_point_et_rogne() {
        assert_eq!(nom_options_fournisseur("copilot.chat"), "copilot");
        assert_eq!(nom_options_fournisseur("  copilot . x"), "copilot");
        assert_eq!(nom_options_fournisseur("copilot"), "copilot");
        assert_eq!(nom_options_fournisseur(""), "");
    }

    #[test]
    fn le_corps_de_requete_tient_les_seize_cles_du_ts() {
        // Les cles de `args` dans getArgs (openai-compatible-chat-language-model.ts),
        // nom pour nom. Une cle snake_case perdue en camelCase, ou l inverse,
        // passerait inapercue jusqu a l'appel de l'API.
        let args = ArgsRequeteChat {
            model: "m".to_string(),
            user: Some("u".to_string()),
            max_tokens: Some(16),
            temperature: Some(0.5),
            top_p: Some(0.9),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.2),
            response_format: Some(FormatReponseDemande::JsonSchema {
                json_schema: SchemaJsonDemande {
                    schema: json!({ "type": "object" }),
                    name: "response".to_string(),
                    description: None,
                },
            }),
            stop: Some(vec!["x".to_string()]),
            seed: Some(7),
            reasoning_effort: Some("high".to_string()),
            verbosity: Some("low".to_string()),
            thinking_budget: Some(2048.0),
            messages: vec![],
            tools: Some(json!([{ "type": "function" }])),
            tool_choice: Some(json!("auto")),
            extras: BTreeMap::new(),
        };
        let corps = serde_json::to_value(&args).unwrap();
        for cle in [
            "model", "user", "max_tokens", "temperature", "top_p", "frequency_penalty",
            "presence_penalty", "response_format", "stop", "seed", "reasoning_effort", "verbosity",
            "thinking_budget", "messages", "tools", "tool_choice",
        ] {
            assert!(corps.get(cle).is_some(), "cle absente du corps : {}", cle);
        }
        // `thinking_budget` est un nombre cote TS (`z.number()`), pas une
        // chaine : c est le piege que ce test verrouille.
        assert!(corps["thinking_budget"].is_number());
        assert!(corps["response_format"]["json_schema"]["schema"].is_object());
        assert_eq!(corps["response_format"]["type"], json!("json_schema"));

        // Une option a None disparait du corps, comme le `undefined` que
        // `JSON.stringify` laisse tomber cote TS.
        let sans_outils = ArgsRequeteChat {
            tools: None,
            tool_choice: None,
            ..args
        };
        let corps = serde_json::to_value(&sans_outils).unwrap();
        assert!(corps.get("tools").is_none());
        assert!(corps.get("tool_choice").is_none());
    }

    #[test]
    fn un_top_k_present_donne_un_avertissement() {
        assert!(avertir_top_k(None).is_none());
        let a = avertir_top_k(Some(1.0)).expect("attendu un avertissement");
        match a {
            Avertissement::NonSupporte { feature, .. } => assert_eq!(feature, "topK"),
        }
    }

    #[test]
    fn un_schema_sans_support_donne_json_object_et_avertit() {
        let a = avertir_format_reponse(true, true, false);
        assert!(a.is_some());
        let f = choisir_format_reponse(true, Some(json!({"type": "object"})), None, None, false);
        assert_eq!(f, Some(FormatReponseDemande::JsonObject));
        assert!(choisir_format_reponse(false, None, None, None, true).is_none());
    }

    #[test]
    fn un_schema_supporte_donne_un_json_schema_nomme() {
        let f = choisir_format_reponse(
            true,
            Some(json!({"type": "object"})),
            None,
            Some("desc".to_string()),
            true,
        );
        match f {
            Some(FormatReponseDemande::JsonSchema { json_schema }) => {
                assert_eq!(json_schema.name, "response");
                assert_eq!(json_schema.description, Some("desc".to_string()));
            }
            autre => panic!("format inattendu : {autre:?}"),
        }
    }

    #[test]
    fn un_contenu_vide_ne_donne_aucun_element() {
        let msg = MessageAssistant {
            role: Some("assistant".to_string()),
            content: Some("".to_string()),
            reasoning_text: Some("".to_string()),
            reasoning_opaque: None,
            tool_calls: None,
        };
        assert!(assembler_contenu_genere(&msg).is_empty());
        let plein = MessageAssistant {
            role: Some("assistant".to_string()),
            content: Some("bonjour".to_string()),
            reasoning_text: Some("ref".to_string()),
            reasoning_opaque: Some("opaque-1".to_string()),
            tool_calls: Some(vec![AppelOutilReponse {
                id: None,
                function: FonctionAppelOutil {
                    name: "lire".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        };
        let c = assembler_contenu_genere(&plein);
        assert_eq!(c.len(), 3);
        match &c[2] {
            ContenuGenere::AppelOutil { tool_call_id, .. } => {
                assert_eq!(tool_call_id, ID_OUTIL_REPLI)
            }
            autre => panic!("contenu inattendu : {autre:?}"),
        }
    }

    #[test]
    fn la_serialisation_garde_la_casse_exacte_des_noms() {
        let c = ContenuGenere::AppelOutil {
            tool_call_id: "a1".to_string(),
            tool_name: "lire".to_string(),
            input: "{}".to_string(),
            reasoning_opaque: Some("o".to_string()),
        };
        let v = serde_json::to_value(&c).expect("serialisation");
        assert_eq!(v.get("toolCallId").and_then(|x| x.as_str()), Some("a1"));
        assert_eq!(v.get("toolName").and_then(|x| x.as_str()), Some("lire"));
        assert_eq!(v.get("reasoningOpaque").and_then(|x| x.as_str()), Some("o"));
        assert!(v.get("tool_call_id").is_none());
        let m = EnveloppeMetaCopilot {
            copilot: Some(MetaCopilot { reasoning_opaque: Some("o".to_string()) }),
        };
        let mv = serde_json::to_value(&m).expect("serialisation");
        assert_eq!(
            mv.pointer("/copilot/reasoningOpaque").and_then(|x| x.as_str()),
            Some("o")
        );
    }

    #[test]
    fn le_sans_cache_soustrait_le_cache_seulement_si_connu() {
        assert_eq!(calculer_sans_cache(Some(100), Some(30)), Some(70));
        assert_eq!(calculer_sans_cache(Some(100), None), None);
        assert_eq!(calculer_sans_cache(None, Some(30)), None);
        let mut acc = AccumulateurUsage::nouveau();
        acc.appliquer(Some(&UsageTokens {
            prompt_tokens: Some(100),
            completion_tokens: Some(20),
            total_tokens: Some(120),
            prompt_tokens_details: Some(DetailsPromptTokens { cached_tokens: Some(30) }),
            completion_tokens_details: Some(DetailsCompletionTokens {
                reasoning_tokens: Some(5),
                accepted_prediction_tokens: None,
                rejected_prediction_tokens: None,
            }),
        }));
        // Un morceau sans details ne doit pas effacer le cache connu.
        acc.appliquer(Some(&UsageTokens {
            prompt_tokens: Some(110),
            completion_tokens: None,
            total_tokens: None,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        }));
        let u = acc.vers_usage_genere();
        assert_eq!(u.input_tokens.total, Some(110));
        assert_eq!(u.input_tokens.cache_read, Some(30));
        assert_eq!(u.input_tokens.no_cache, Some(80));
        assert_eq!(u.output_tokens.reasoning, Some(5));
    }

    #[test]
    fn l_usage_genere_s_ecrit_en_arbre_comme_le_ts() {
        // The TS returns { inputTokens: { total, noCache, cacheRead, cacheWrite },
        // outputTokens: { total, text, reasoning } }: nested, camelCase, and an
        // absent field disappears instead of turning into null.
        let acc = AccumulateurUsage {
            prompt_tokens: Some(10),
            completion_tokens: Some(4),
            cached_tokens: Some(3),
            reasoning_tokens: Some(1),
            ..AccumulateurUsage::default()
        };
        let v = serde_json::to_value(acc.vers_usage_genere()).unwrap();
        assert_eq!(v["inputTokens"]["total"], json!(10));
        assert_eq!(v["inputTokens"]["cacheRead"], json!(3));
        assert_eq!(v["inputTokens"]["noCache"], json!(7));
        assert!(v["inputTokens"].get("cacheWrite").is_none());
        assert_eq!(v["outputTokens"]["total"], json!(4));
        assert_eq!(v["outputTokens"]["reasoning"], json!(1));
        assert!(v["outputTokens"].get("text").is_none());
        // No French name may survive on the wire.
        assert!(v.get("entree_total").is_none());
        assert!(v.get("sortie_total").is_none());
    }

    #[test]
    fn l_accumulateur_fusionne_jusqu_au_json_complet() {
        let mut acc = AccumulateurAppelsOutil::nouveau();
        assert!(valider_nouvel_appel_outil(None, Some("lire")).is_err());
        assert!(valider_nouvel_appel_outil(Some("a"), None).is_err());
        let fini = acc.demarrer(0, Some("a"), Some("lire"), Some("{\"x\":")).expect("demarrage");
        assert!(!fini);
        assert_eq!(controler_opaque_unique(Some("a"), Some("b")).is_err(), true);
        assert_eq!(
            controler_opaque_unique(None, Some("b")).expect("opaque"),
            Some("b".to_string())
        );
        assert!(!est_json_analysable("{\"x\":"));
        let termine = acc.ajouter_arguments(0, Some(" 1}")).expect("delta");
        assert!(termine);
        assert!(acc.inacheves().is_empty());
        assert!(acc.ajouter_arguments(0, Some("ignore")).is_none());
        assert!(acc.ajouter_arguments(9, Some("{}")).is_none());
        assert_eq!(convertir_fin(Some("tool_calls")).0, FinUnifie::AppelsOutil);
        assert_eq!(convertir_fin(Some("inconnu")).0, FinUnifie::Autre);
        assert_eq!(convertir_fin(None).1, None);
        let brutes: BTreeMap<String, Value> =
            [("user".to_string(), json!("u")), ("x".to_string(), json!(1))]
                .into_iter()
                .collect();
        let garde = filtrer_extras(Some(&brutes), &["user"]);
        assert!(garde.contains_key("x"));
        assert!(!garde.contains_key("user"));
        assert!(filtrer_extras(None, &["user"]).is_empty());
        assert!(options_flux(false).is_none());
        assert_eq!(
            options_flux(true),
            Some(StreamOptions { include_usage: true })
        );
    }
}
