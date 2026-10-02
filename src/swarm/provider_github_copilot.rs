//! Portage de `packages/core/src/plugin/provider/github-copilot.ts`.
//!
//! ## Ce que fait la source
//!
//! Le plugin `GithubCopilotPlugin` (defini par `id: "github-copilot"`)
//! enregistre **trois** crochets :
//!
//! 1. **`ctx.catalog.transform`.** Il cherche le fournisseur
//!    `ProviderV2.ID.githubCopilot` ; s'il existe et que sa liste de modeles
//!    contient `gpt-5-chat-latest`, il desactive ce modele (`enabled = false`).
//!    Le commentaire de la source explique pourquoi : cet alias chat-only entre
//!    en conflit avec la route Responses Copilot de GPT-5, donc on le cache
//!    uniquement pour Copilot, pas pour tout le catalogue.
//! 2. **`ctx.aisdk.sdk`.** Filtre sur `evt.package === "@ai-sdk/github-copilot"`,
//!    puis pose `evt.sdk = mod.createOpenaiCompatible(evt.options)`, ou `mod`
//!    est `core/src/github-copilot/copilot-provider`, porte ailleurs dans ce
//!    depot Rust (voir `copilot_openai_config.rs` et voisins). Meme decoupage
//!    que `provider_mistral.rs` : la fabrique est injectee, pas reimplementee.
//! 3. **`ctx.aisdk.language`.** Choisit le modele de langage a construire :
//!    - si le SDK n'a ni `responses` ni `chat`, on passe par `languageModel` ;
//!    - sinon `options.endpoint === "responses"` avec un `sdk.responses`
//!      present gagne, puis `options.endpoint === "chat"` avec un
//!      `sdk.chat` present ;
//!    - sinon, par defaut : les modeles `gpt-N` avec `N >= 5`, hors variantes
//!      `gpt-5-mini` (qui restent en chat-completions), prennent la route
//!      Responses ; tous les autres prennent `chat`.
//!
//! ## Choix de portage
//!
//! - Le champ `effect` est une fonction, sans representation JSON : comme pour
//!   Mistral et Vercel, le plugin est porte comme fonctions pures, une par
//!   crochet.
//! - La regex `/^gpt-(\d+)/` est reecrite sans le crate `regex` : on lit les
//!   chiffres qui suivent le prefixe `gpt-`. `match[1]` est reconverti en
//!   nombre via `Number(...)`, ce qui correspond a un `u32` : la source ne
//!   compare qu'a `5`.
//! - Le test de veracite `evt.sdk.responses` dans le ternaire final devient
//!   `Option::is_some` : en JavaScript, un objet present est toujours truthy,
//!   donc les deux formulations coincident.
//! - La branche finale `evt.sdk.chat(...)` est inconditionnelle en TS ; si le
//!   SDK n'a pas de `chat`, l'original leverait. Le portage rend la decision,
//!   c'est l'appelant qui construit : meme contrat, meme responsabilite.
//!
//! ## Noms de champs
//!
//! Tous les noms camelCase de la source sont reecrits explicitement avec
//! `#[serde(rename = ...)]` : `providerID` (avec le D majuscule), `api`, `id`,
//! `endpoint`, `responses`, `chat`, `language`, `enabled`. Les tests
//! font des allers-retours serde sur ces noms exacts, c'est la le point de
//! rupture habituel de ce portage.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "github-copilot"` dans la source.
pub const PLUGIN_ID: &str = "github-copilot";

/// Le fournisseur vise par la transformation du catalogue.
///
/// En TS : `ProviderV2.ID.githubCopilot`. La definition de `ProviderV2.ID` n'a
/// pas ete trouvee dans l'arbre source fourni ; la valeur reprend l'identifiant
/// kebab-case du plugin, convention des autres portages.
pub const PROVIDER_ID: &str = "github-copilot";

/// Le seul nom de paquet npm auquel le crochet `sdk` repond.
pub const PACKAGE: &str = "@ai-sdk/github-copilot";

/// Le nom de la fabrique exportee par `copilot-provider`.
pub const FACTORY: &str = "createOpenaiCompatible";

/// L'alias chat-only que la transformation desactive pour Copilot.
pub const HIDDEN_MODEL_ID: &str = "gpt-5-chat-latest";

// ---------------------------------------------------------------------------
// Crochet 1 : transformation du catalogue
// ---------------------------------------------------------------------------

/// Un modele, tel que la mise a jour du catalogue l'ecrit.
///
/// La source ne lit jamais les autres champs du modele ; `enabled` est le seul
/// qu'elle modifie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogModelEntry {
    /// Identifiant du modele, cle de `evt.model.update`.
    #[serde(rename = "id")]
    pub id: String,
    /// Drapeau pose a `false` par la transformation.
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

/// La reference `item.provider.id` lue par la transformation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogProviderRef {
    /// Identifiant du fournisseur, recopie vers `evt.model.update`.
    #[serde(rename = "id")]
    pub id: String,
}

/// Un fournisseur tel que `evt.provider.get()` le renvoie.
///
/// Version reduite : seul l'identifiant du fournisseur et l'ensemble des
/// modeles sont lus par ce plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogProviderItem {
    /// La reference `item.provider`.
    #[serde(rename = "provider")]
    pub provider: CatalogProviderRef,
    /// La liste `item.models`, dont la source ne teste que la presence.
    #[serde(rename = "models")]
    pub models: Vec<String>,
}

/// L'evenement recu par le crochet `ctx.catalog.transform`.
///
/// En TS, `evt.provider` et `evt.model` sont deux vues sur le meme catalogue ;
/// ici elles sont portees cote a cote dans une seule structure, ce qui est
/// inobservable depuis ce fichier.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CatalogEvent {
    /// Les fournisseurs connus du catalogue.
    #[serde(rename = "providers")]
    pub providers: Vec<CatalogProviderItem>,
    /// Les modeles, cibles de `evt.model.update`.
    #[serde(rename = "models")]
    pub models: Vec<CatalogModelEntry>,
}

impl CatalogEvent {
    /// `evt.provider.get(providerID)` : un fournisseur, ou `None`.
    pub fn get(&self, provider_id: &str) -> Option<&CatalogProviderItem> {
        self.providers.iter().find(|item| item.provider.id == provider_id)
    }

    /// `evt.model.update(providerID, modelID, fn)` : applique `update` au
    /// modele nomme, et renvoie `true` s'il existait.
    pub fn update_model<F>(&mut self, provider_id: &str, model_id: &str, update: F) -> bool
    where
        F: FnOnce(&mut CatalogModelEntry),
    {
        let _ = provider_id;
        match self.models.iter_mut().find(|model| model.id == model_id) {
            Some(model) => {
                update(model);
                true
            }
            None => false,
        }
    }
}

/// Regle du crochet `ctx.catalog.transform`.
///
/// Reproduit la source : chercher `ProviderV2.ID.githubCopilot`, sortir si le
/// fournisseur est absent ou si `gpt-5-chat-latest` n'est pas dans ses modeles,
/// sinon desactiver ce modele et rendre `true`. Renvoie `false` quand la source
/// n'aurait rien fait.
pub fn transform_catalogue(evt: &mut CatalogEvent) -> bool {
    let Some(item) = evt.get(PROVIDER_ID) else {
        return false;
    };
    if !item.models.iter().any(|model| model == HIDDEN_MODEL_ID) {
        return false;
    }
    let provider_id = item.provider.id.clone();
    evt.update_model(&provider_id, HIDDEN_MODEL_ID, |model| model.enabled = false)
}

// ---------------------------------------------------------------------------
// Crochet 2 : construction du SDK
// ---------------------------------------------------------------------------

/// L'evenement recu par le crochet `aisdk.sdk`.
///
/// Meme forme que l'evenement porté par `provider_mistral.rs` : `{ model,
/// package, options, sdk? }`. Le champ `model` n'est pas lu par ce plugin et
/// reste opaque.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SdkHookEvent {
    /// Le modele demande. Jamais lu par ce plugin, donc garde opaque.
    #[serde(rename = "model")]
    pub model: Value,
    /// Le nom du paquet npm cherche. Seul champ que le filtre lit.
    #[serde(rename = "package")]
    pub package: String,
    /// Les options a transmettre a la fabrique, sans reinterpretation.
    #[serde(rename = "options")]
    pub options: Value,
    /// Le SDK construit. Absent tant qu'aucun plugin n'en a pose un.
    #[serde(rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

/// Dit si ce plugin repond a ce nom de paquet.
///
/// La source ecrit `evt.package !== "@ai-sdk/github-copilot"` puis `return` :
/// egalite stricte, sensible a la casse.
pub fn applies_to(package: &str) -> bool {
    package == PACKAGE
}

/// Le corps du crochet enregistre par le plugin.
///
/// `factory` tient lieu de `createOpenaiCompatible` de
/// `core/src/github-copilot/copilot-provider`, porte ailleurs dans ce depot.
/// Si le nom de paquet ne correspond pas, la fabrique n'est pas appelee.
pub fn on_sdk_event<F>(event: &mut SdkHookEvent, factory: F)
where
    F: FnOnce(&Value) -> Value,
{
    if !applies_to(&event.package) {
        return;
    }
    let sdk = factory(&event.options);
    event.sdk = Some(sdk);
}

// ---------------------------------------------------------------------------
// Crochet 3 : choix du modele de langage
// ---------------------------------------------------------------------------

/// La reference `evt.model` lue par le crochet `language`.
///
/// Seuls `providerID` et `api.id` sont lus ; le filtre sur le fournisseur et
/// l'identifiant d'api sont reproduits tels quels.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageModelRef {
    /// `evt.model.providerID`.
    #[serde(rename = "providerID")]
    pub provider_id: String,
    /// `evt.model.api`.
    #[serde(rename = "api")]
    pub api: LanguageApiRef,
}

/// La reference `evt.model.api` lue par le crochet `language`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageApiRef {
    /// `evt.model.api.id`, l'identifiant d'api teste contre la regex.
    #[serde(rename = "id")]
    pub id: String,
}

/// Les options `evt.options` lues par le crochet `language`.
///
/// Seul `endpoint` est lu ; le champ est absent ou d'une autre valeur pour tout
/// ce qui n'est pas `"responses"` ou `"chat"`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageOptions {
    /// `evt.options.endpoint`, absent tant que personne ne l'a fixe.
    #[serde(rename = "endpoint", skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// La forme `evt.sdk` lue par le crochet `language`.
///
/// `languageModel` est une fonction du SDK : sans representation JSON, elle est
/// designee par la variante `LanguageChoice::LanguageModel` plutot que portee
/// ici. `responses` et `chat` sont opaques : le plugin ne fait que tester leur
/// presence et les appeler avec l'identifiant d'api.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageSdk {
    /// `evt.sdk.responses`, absent ou present.
    #[serde(rename = "responses", skip_serializing_if = "Option::is_none")]
    pub responses: Option<Value>,
    /// `evt.sdk.chat`, absent ou present.
    #[serde(rename = "chat", skip_serializing_if = "Option::is_none")]
    pub chat: Option<Value>,
}

/// L'evenement recu par le crochet `aisdk.language`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageHookEvent {
    /// Le modele demande, filtre sur le fournisseur Copilot.
    #[serde(rename = "model")]
    pub model: LanguageModelRef,
    /// Les options, dont `endpoint`.
    #[serde(rename = "options")]
    pub options: LanguageOptions,
    /// Le SDK construit par le crochet `sdk`.
    #[serde(rename = "sdk")]
    pub sdk: LanguageSdk,
    /// `evt.language`, la decision ecrite par le crochet. Absent tant que le
    /// crochet n'a pas tourne, comme une propriete non assignee en JS.
    #[serde(rename = "language", skip_serializing_if = "Option::is_none")]
    pub language: Option<Value>,
}

/// Ce que le crochet `language` decide de construire.
///
/// Les trois voies de la source, dans l'ordre ou elle les teste. L'appelant
/// construit effectivement le modele : `languageModel`, `responses` et `chat`
/// sont des fonctions du SDK JavaScript, sans equivalent serialisable ici.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageChoice {
    /// `evt.sdk.languageModel(evt.model.api.id)` : le SDK ne sait faire ni
    /// Responses ni chat.
    LanguageModel,
    /// `evt.sdk.responses(evt.model.api.id)` : la route Responses.
    Responses,
    /// `evt.sdk.chat(evt.model.api.id)` : la route chat-completions.
    Chat,
}

/// Le numero `N` d'un identifiant d'api `gpt-N...`.
///
/// Equivalent de la regex `/^gpt-(\d+)/` sans le crate `regex` : l'identifiant
/// doit commencer par `gpt-`, suivi d'au moins un chiffre, puis de ce qui vient
/// (points, tirets, rien). `Number(match[1])` en TS accepte les chiffres en
/// tete exactement comme ce decoupage ; un identifiant sans chiffre ne
/// correspond pas.
pub fn numero_gpt(api_id: &str) -> Option<u32> {
    let reste = api_id.strip_prefix("gpt-")?;
    let chiffres: String = reste.chars().take_while(|c| c.is_ascii_digit()).collect();
    if chiffres.is_empty() {
        return None;
    }
    chiffres.parse().ok()
}

/// Dit si l'identifiant d'api est une variante `gpt-5-mini`.
///
/// La source teste `evt.model.api.id.startsWith("gpt-5-mini")`.
pub fn est_gpt5_mini(api_id: &str) -> bool {
    api_id.starts_with("gpt-5-mini")
}

/// Dit si ce crochet repond a ce fournisseur.
///
/// La source ecrit `evt.model.providerID !== ProviderV2.ID.githubCopilot` puis
/// `return` : egalite stricte, sensible a la casse.
pub fn language_applies_to(provider_id: &str) -> bool {
    provider_id == PROVIDER_ID
}

/// Le corps du crochet `aisdk.language`, sans son filtre de fournisseur.
///
/// Reproduit l'ordre des tests de la source :
/// 1. ni `responses` ni `chat` presents -> `languageModel` ;
/// 2. `endpoint === "responses"` et `sdk.responses` present -> Responses ;
/// 3. `endpoint === "chat"` et `sdk.chat` present -> chat ;
/// 4. sinon, `gpt-N` avec `N >= 5` hors `gpt-5-mini` et `sdk.responses`
///    present -> Responses, sinon chat.
///
/// La comparaison `Number(match[1]) >= 5` est un `u32 >= 5` : les identifiants
/// sans numero ne passent pas, `numero_gpt` renvoyant `None`.
pub fn choix_langage(event: &LanguageHookEvent) -> LanguageChoice {
    if event.sdk.responses.is_none() && event.sdk.chat.is_none() {
        return LanguageChoice::LanguageModel;
    }
    if event.options.endpoint.as_deref() == Some("responses") && event.sdk.responses.is_some() {
        return LanguageChoice::Responses;
    }
    if event.options.endpoint.as_deref() == Some("chat") && event.sdk.chat.is_some() {
        return LanguageChoice::Chat;
    }
    let reponses_gagnent = match numero_gpt(&event.model.api.id) {
        Some(numero) => numero >= 5 && !est_gpt5_mini(&event.model.api.id),
        None => false,
    };
    if reponses_gagnent && event.sdk.responses.is_some() {
        LanguageChoice::Responses
    } else {
        LanguageChoice::Chat
    }
}

/// Le crochet `aisdk.language` complet : filtre de fournisseur puis decision.
///
/// Si le fournisseur n'est pas Copilot, l'evenement reste intact et `None` est
/// rendu, comme le `return` precoce de la source. Sinon la decision est posee
/// dans `evt.language` et rendue.
///
/// `language_model` tient lieu de `evt.sdk.languageModel` : la fabrique
/// recoit l'identifiant d'api et rend le modele de langage, exactement comme
/// l'appel `evt.sdk.languageModel(evt.model.api.id)` de la source.
pub fn on_language_event<F>(event: &mut LanguageHookEvent, language_model: F) -> Option<LanguageChoice>
where
    F: FnOnce(&str) -> Value,
{
    if !language_applies_to(&event.model.provider_id) {
        return None;
    }
    let choix = choix_langage(event);
    event.language = Some(match &choix {
        LanguageChoice::LanguageModel => language_model(&event.model.api.id),
        LanguageChoice::Responses => {
            let _ = &event.sdk.responses;
            Value::Null
        }
        LanguageChoice::Chat => {
            let _ = &event.sdk.chat;
            Value::Null
        }
    });
    Some(choix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn modele(id: &str, enabled: bool) -> CatalogModelEntry {
        CatalogModelEntry { id: id.to_string(), enabled }
    }

    fn catalogue_copilot() -> CatalogEvent {
        CatalogEvent {
            providers: vec![CatalogProviderItem {
                provider: CatalogProviderRef { id: PROVIDER_ID.to_string() },
                models: vec!["gpt-5-chat-latest".to_string(), "gpt-5".to_string()],
            }],
            models: vec![modele("gpt-5-chat-latest", true), modele("gpt-5", true)],
        }
    }

    fn evenement_langage(api_id: &str) -> LanguageHookEvent {
        LanguageHookEvent {
            model: LanguageModelRef {
                provider_id: PROVIDER_ID.to_string(),
                api: LanguageApiRef { id: api_id.to_string() },
            },
            options: LanguageOptions::default(),
            sdk: LanguageSdk {
                responses: Some(json!("responses-sdk")),
                chat: Some(json!("chat-sdk")),
            },
            language: None,
        }
    }

    // ------------------------------------------------- transformation catalogue

    #[test]
    fn l_alias_chat_est_desactive_pour_copilot() {
        let mut evt = catalogue_copilot();
        assert!(transform_catalogue(&mut evt));
        assert!(!evt.models[0].enabled, "gpt-5-chat-latest doit etre desactive");
        assert!(evt.models[1].enabled, "gpt-5 ne doit pas etre touche");
    }

    #[test]
    fn un_catalogue_sans_copilot_ne_change_rien() {
        let mut evt = catalogue_copilot();
        evt.providers[0].provider.id = "openai".to_string();
        assert!(!transform_catalogue(&mut evt));
        assert!(evt.models[0].enabled);
    }

    #[test]
    fn un_copilot_sans_l_alias_ne_change_rien() {
        let mut evt = catalogue_copilot();
        evt.providers[0].models = vec!["gpt-5".to_string()];
        assert!(!transform_catalogue(&mut evt));
        assert!(evt.models[0].enabled);
    }

    #[test]
    fn la_presence_de_l_alias_est_exacte() {
        let mut evt = catalogue_copilot();
        evt.providers[0].models = vec!["gpt-5-chat-latestx".to_string()];
        assert!(!transform_catalogue(&mut evt));
    }

    #[test]
    fn un_modele_absent_du_brouillon_n_est_pas_desactive() {
        let mut evt = catalogue_copilot();
        evt.models.clear();
        assert!(!transform_catalogue(&mut evt));
    }

    #[test]
    fn le_catalogue_se_relit_depuis_le_json_du_typescript() {
        let evt: CatalogEvent = serde_json::from_str(
            r#"{"providers":[{"provider":{"id":"github-copilot"},"models":["gpt-5-chat-latest"]}],"models":[{"id":"gpt-5-chat-latest","enabled":true}]}"#,
        )
        .unwrap();
        assert_eq!(evt.providers[0].provider.id, "github-copilot");
        assert_eq!(evt.models[0].id, "gpt-5-chat-latest");
        assert!(evt.models[0].enabled);
    }

    #[test]
    fn le_catalogue_se_reecrit_avec_les_memes_cles() {
        let mut evt = catalogue_copilot();
        transform_catalogue(&mut evt);
        let json = serde_json::to_string(&evt).unwrap();
        assert_eq!(
            json,
            r#"{"providers":[{"provider":{"id":"github-copilot"},"models":["gpt-5-chat-latest","gpt-5"]}],"models":[{"id":"gpt-5-chat-latest","enabled":false},{"id":"gpt-5","enabled":true}]}"#
        );
    }

    // ------------------------------------------------- construction du SDK

    #[test]
    fn le_seul_paquet_reconnu_est_celui_de_copilot() {
        assert!(applies_to("@ai-sdk/github-copilot"));
        assert!(applies_to(PACKAGE));
        assert!(!applies_to("@ai-sdk/openai"));
        assert!(!applies_to("@ai-sdk/github-copilot-provider"));
    }

    #[test]
    fn un_paquet_copilot_construit_le_sdk_avec_les_options_de_l_evenement() {
        let mut event = SdkHookEvent {
            model: Value::Null,
            package: PACKAGE.to_string(),
            options: json!({ "endpoint": "responses" }),
            sdk: None,
        };
        let mut recues: Option<Value> = None;
        on_sdk_event(&mut event, |options: &Value| {
            recues = Some(options.clone());
            json!("sdk-copilot")
        });
        assert_eq!(recues, Some(json!({ "endpoint": "responses" })));
        assert_eq!(event.sdk, Some(json!("sdk-copilot")));
    }

    #[test]
    fn un_paquet_qui_n_est_pas_copilot_laisse_le_sdk_absent() {
        let mut event = SdkHookEvent {
            model: Value::Null,
            package: "@ai-sdk/openai".to_string(),
            options: json!({}),
            sdk: None,
        };
        on_sdk_event(&mut event, |_options: &Value| json!("fabrique"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn l_evenement_sdk_se_relit_depuis_le_json_du_typescript() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":{"id":"gpt-5"},"package":"@ai-sdk/github-copilot","options":{"apiKey":"secret"},"sdk":"marqueur"}"#,
        )
        .unwrap();
        assert_eq!(event.package, "@ai-sdk/github-copilot");
        assert_eq!(event.sdk, Some(json!("marqueur")));
    }

    #[test]
    fn l_evenement_sdk_se_reecrit_avec_les_cles_typescript() {
        let event = SdkHookEvent {
            model: json!({"id": "gpt-5"}),
            package: PACKAGE.to_string(),
            options: json!({"endpoint": "responses"}),
            sdk: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"model":{"id":"gpt-5"},"package":"@ai-sdk/github-copilot","options":{"endpoint":"responses"}}"#
        );
    }

    // ------------------------------------------------- choix du modele de langage

    #[test]
    fn un_sdk_sans_responses_ni_chat_passe_par_languagemodel() {
        let mut event = evenement_langage("gpt-5");
        event.sdk = LanguageSdk::default();
        assert_eq!(choix_langage(&event), LanguageChoice::LanguageModel);
    }

    #[test]
    fn un_endpoint_responses_avec_reponses_present_prend_la_route_responses() {
        let mut event = evenement_langage("gpt-4");
        event.options.endpoint = Some("responses".to_string());
        assert_eq!(choix_langage(&event), LanguageChoice::Responses);
    }

    #[test]
    fn un_endpoint_chat_avec_chat_present_prend_la_route_chat() {
        let mut event = evenement_langage("gpt-5");
        event.options.endpoint = Some("chat".to_string());
        assert_eq!(choix_langage(&event), LanguageChoice::Chat);
    }

    #[test]
    fn un_endpoint_sans_sdk_correspondant_retombe_sur_la_regle_par_defaut() {
        let mut event = evenement_langage("gpt-5");
        event.options.endpoint = Some("responses".to_string());
        event.sdk.responses = None;
        assert_eq!(choix_langage(&event), LanguageChoice::Chat);
    }

    #[test]
    fn un_gpt5_prend_la_route_responses_par_defaut() {
        assert_eq!(choix_langage(&evenement_langage("gpt-5")), LanguageChoice::Responses);
        assert_eq!(choix_langage(&evenement_langage("gpt-6")), LanguageChoice::Responses);
    }

    #[test]
    fn gpt5_mini_reste_en_chat() {
        assert_eq!(choix_langage(&evenement_langage("gpt-5-mini")), LanguageChoice::Chat);
    }

    #[test]
    fn un_modele_avant_gpt5_reste_en_chat() {
        assert_eq!(choix_langage(&evenement_langage("gpt-4")), LanguageChoice::Chat);
        assert_eq!(choix_langage(&evenement_langage("gpt-4.1")), LanguageChoice::Chat);
    }

    #[test]
    fn un_identifiant_sans_numero_reste_en_chat() {
        assert_eq!(choix_langage(&evenement_langage("claude-sonnet")), LanguageChoice::Chat);
        assert_eq!(numero_gpt("gpt-"), None);
        assert_eq!(numero_gpt("gpt-abc"), None);
        assert_eq!(numero_gpt("o3"), None);
    }

    #[test]
    fn le_numero_gpt_lit_les_chiffres_de_tete() {
        assert_eq!(numero_gpt("gpt-5"), Some(5));
        assert_eq!(numero_gpt("gpt-5-chat-latest"), Some(5));
        assert_eq!(numero_gpt("gpt-4.1-mini"), Some(4));
    }

    #[test]
    fn le_filtre_de_fournisseur_est_strict() {
        assert!(language_applies_to("github-copilot"));
        assert!(!language_applies_to("github-copilot-chat"));
        assert!(!language_applies_to("GitHub-Copilot"));
        assert!(!language_applies_to(""));
    }

    #[test]
    fn le_crochet_ecrit_la_decision_dans_l_evenement() {
        let mut event = evenement_langage("gpt-5");
        // La fabrique passee ici tient lieu de `evt.sdk.languageModel`, que la
        // source n'appelle que sur sa premiere branche. Ici c'est la branche
        // Responses qui part : la source ecrit `evt.sdk.responses(...)`, que le
        // portage represente par `null` puisque l'appel n'a pas de forme JSON.
        // La fabrique ne doit donc pas etre appelee du tout.
        let choix = on_language_event(&mut event, |_api_id: &str| json!("fabrique languageModel")).unwrap();
        assert_eq!(choix, LanguageChoice::Responses);
        assert_eq!(event.language, Some(Value::Null));
    }

    #[test]
    fn le_crochet_appelle_languagemodel_avec_l_identifiant_d_api() {
        let mut event = evenement_langage("o3");
        event.sdk = LanguageSdk::default();
        let choix = on_language_event(&mut event, |api_id: &str| json!({ "api": api_id })).unwrap();
        assert_eq!(choix, LanguageChoice::LanguageModel);
        assert_eq!(event.language, Some(json!({ "api": "o3" })));
    }

    #[test]
    fn un_autre_fournisseur_laisse_l_evenement_intact() {
        let mut event = evenement_langage("gpt-5");
        event.model.provider_id = "openai".to_string();
        let choix = on_language_event(&mut event, |_api_id: &str| json!("construit"));
        assert_eq!(choix, None);
        assert_eq!(event.language, None);
    }

    #[test]
    fn l_evenement_language_se_relit_depuis_le_json_du_typescript() {
        let event: LanguageHookEvent = serde_json::from_str(
            r#"{"model":{"providerID":"github-copilot","api":{"id":"gpt-5-mini"}},"options":{"endpoint":"chat"},"sdk":{"chat":"chat-sdk"},"language":"construit"}"#,
        )
        .unwrap();
        assert_eq!(event.model.provider_id, "github-copilot");
        assert_eq!(event.model.api.id, "gpt-5-mini");
        assert_eq!(event.options.endpoint.as_deref(), Some("chat"));
        assert_eq!(event.sdk.chat, Some(json!("chat-sdk")));
        assert_eq!(event.sdk.responses, None);
        assert_eq!(event.language, Some(json!("construit")));
    }

    #[test]
    fn l_evenement_language_se_reecrit_avec_les_cles_typescript() {
        let event = evenement_langage("gpt-5");
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"model":{"providerID":"github-copilot","api":{"id":"gpt-5"}},"options":{},"sdk":{"responses":"responses-sdk","chat":"chat-sdk"}}"#
        );
    }
}
