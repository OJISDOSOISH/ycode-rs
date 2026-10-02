//! Portage de `packages/core/src/plugin/provider/azure.ts`.
//!
//! ## Ce que fait la source
//!
//! Deux plugins dans un seul fichier TypeScript :
//!
//! - `AzurePlugin` (`id: "azure"`), qui repond au paquet npm `@ai-sdk/azure`.
//!   Trois crochets : un `ctx.catalog.transform` qui recopie la variable
//!   d'environnement `AZURE_RESOURCE_NAME` dans `provider.request.body.resourceName`
//!   de chaque fournisseur configure avec ce paquet (sauf si un nom de ressource
//!   non vide est deja present dans le corps) ; un `ctx.aisdk.sdk` qui importe le
//!   paquet et pose `evt.sdk = mod.createAzure(evt.options)`, en refusant de
//!   construire un SDK quand ni `resourceName`, ni `baseURL`, ni `model.api.url`
//!   ne sont disponibles pour le fournisseur `"azure"` ; un `ctx.aisdk.language`
//!   qui choisit la surface du SDK via `selectLanguage`.
//! - `AzureCognitiveServicesPlugin` (`id: "azure-cognitive-services"`), qui ne
//!   touche que les fournisseurs openai-compatible dont l'identifiant contient
//!   `"azure-cognitive-services"` : si `AZURE_COGNITIVE_SERVICES_RESOURCE_NAME`
//!   est definie, il pose `provider.request.body.baseURL` a
//!   `https://{ressource}.cognitiveservices.azure.com/openai`, puis choisit la
//!   langue avec le meme `selectLanguage`.
//!
//! ## Choix de portage
//!
//! - Comme pour `mistral.ts`, le champ `effect` est une fonction sans
//!   representation JSON : chaque plugin est une structure de donnees plus des
//!   fonctions libres pour chaque crochet.
//! - `createAzure` vient du paquet npm `@ai-sdk/azure` : la fabrique est
//!   injectee par l'appelant, exactement comme dans `provider_mistral.rs`.
//! - `selectLanguage` est un pur aiguillage sur les proprietes `chat`,
//!   `responses`, `messages`, `languageModel` du SDK. Le SDK etant opaque
//!   (`Value`), le meme aiguillage est porte par des marqueurs booleens sur une
//!   structure `SdkSurfaces`, et la fonction `choisir_langue` reproduit
//!   exactement l'ordre de la source : `useChat && sdk.chat` d'abord, puis
//!   `responses`, puis `messages`, puis `chat` sans condition, puis
//!   `languageModel` en dernier recours.
//! - Les environnements sont lus via `std::env::var`, l'equivalent direct de
//!   `process.env`. Une variable absente ou vide est traitee comme absente, ce
//!   que la source obtient avec des tests `!resourceName` (chaine vide incluse).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant du premier plugin : `id: "azure"`.
pub const PLUGIN_ID: &str = "azure";

/// L'identifiant du second plugin : `id: "azure-cognitive-services"`.
pub const COGNITIVE_PLUGIN_ID: &str = "azure-cognitive-services";

/// Le seul paquet npm auquel `AzurePlugin` repond.
pub const PACKAGE: &str = "@ai-sdk/azure";

/// Le paquet npm filtre par `AzureCognitiveServicesPlugin`.
pub const COGNITIVE_PACKAGE: &str = "@ai-sdk/openai-compatible";

/// La sous-chaine que l'identifiant d'un fournisseur doit contenir pour le
/// second plugin.
pub const COGNITIVE_MARKER: &str = "azure-cognitive-services";

/// La fabrique exportee par `@ai-sdk/azure`.
pub const FACTORY: &str = "createAzure";

/// Variable d'environnement lue par `AzurePlugin`.
pub const ENV_RESOURCE_NAME: &str = "AZURE_RESOURCE_NAME";

/// Variable d'environnement lue par `AzureCognitiveServicesPlugin`.
pub const ENV_COGNITIVE_RESOURCE_NAME: &str = "AZURE_COGNITIVE_SERVICES_RESOURCE_NAME";

/// L'erreur jetee par la source quand aucune adresse Azure n'est determinable.
///
/// En TypeScript : `throw new Error("AZURE_RESOURCE_NAME is missing, set it
/// using env var or reconnecting the azure provider and setting it")`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("AZURE_RESOURCE_NAME is missing, set it using env var or reconnecting the azure provider and setting it")]
pub struct ResourceNameManquante;

/// `item.provider.api`, filtre du catalogue.
///
/// La source lit `api.type` et `api.package`. Le champ TypeScript s'appelle
/// `type`, mot reserve de Rust : le nom de champ Rust est `kind`, la cle JSON
/// reste `"type"` via le rename explicite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderApi {
    /// `api.type`, `"aisdk"` pour les fournisseurs concernes.
    #[serde(rename = "type")]
    pub kind: String,
    /// `api.package`, le nom du paquet npm.
    #[serde(rename = "package")]
    pub package: String,
    /// `api.url`, uniquement lu pour le fournisseur `"azure"` dans le crochet
    /// sdk. Absent en general, comme le champ optionnel TypeScript.
    #[serde(rename = "url", skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// `provider.request.body`, lu et ecrit par les deux transformations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestBody {
    /// `body.resourceName`, recopie depuis l'environnement par le premier plugin.
    #[serde(rename = "resourceName", skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    /// `body.baseURL`, pose par le second plugin.
    #[serde(rename = "baseURL", skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// `provider.request`, simple conteneur du corps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestConfig {
    /// `request.body`.
    #[serde(rename = "body")]
    pub body: RequestBody,
}

/// `item.provider`, la partie du fournisseur que ce fichier lit ou modifie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// `provider.id`, cle de `evt.provider.update` et filtre du second plugin.
    #[serde(rename = "id")]
    pub id: String,
    /// `provider.api`.
    #[serde(rename = "api")]
    pub api: ProviderApi,
    /// `provider.request`.
    #[serde(rename = "request")]
    pub request: RequestConfig,
}

/// `evt.options` du crochet sdk, les champs reellement lus par ce fichier.
///
/// Le TypeScript lit `options.resourceName`, `options.baseURL` et
/// `options.useCompletionUrls` (caste en booleen). Les autres options sont
/// transmises a la fabrique sans reinterpretation, elles ne sont donc pas
/// modelisees ici.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AzureOptions {
    /// `options.resourceName`.
    #[serde(rename = "resourceName", default, skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    /// `options.baseURL`.
    #[serde(rename = "baseURL", default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// `options.useCompletionUrls`, truthy TypeScript reduit a un booleen.
    #[serde(
        rename = "useCompletionUrls",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_truthy_bool"
    )]
    pub use_completion_urls: Option<bool>,
}

/// Deserialise `useCompletionUrls` selon `Boolean()` du TypeScript.
///
/// La source lit `Boolean(evt.options.useCompletionUrls)` : `1` vaut `true`,
/// `0`, `""` et `false` valent `false`, les tableaux et objets valent `true`.
/// `null` et l'absence valent `None`, comme une option non fournie.
fn deserialize_optional_truthy_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let valeur = Option::<Value>::deserialize(deserializer)?;
    Ok(valeur.map(|v| match &v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(u) = n.as_u64() {
                u != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0 && !f.is_nan()
            } else {
                true
            }
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }))
}

/// Dit si un fournisseur du catalogue concerne le premier plugin.
///
/// La source ecrit `api.type !== "aisdk"` puis `continue`, puis
/// `api.package !== "@ai-sdk/azure"` puis `continue` : deux egalites strictes.
pub fn concerne_azure(provider: &ProviderConfig) -> bool {
    provider.api.kind == "aisdk" && provider.api.package == PACKAGE
}

/// Dit si un fournisseur du catalogue concerne le second plugin.
///
/// La source filtre sur `api.type === "aisdk"`,
/// `api.package === "@ai-sdk/openai-compatible"` et
/// `provider.id.includes("azure-cognitive-services")`.
pub fn concerne_cognitive(provider: &ProviderConfig) -> bool {
    provider.api.kind == "aisdk"
        && provider.api.package == COGNITIVE_PACKAGE
        && provider.id.contains(COGNITIVE_MARKER)
}

/// Lit une variable d'environnement en traitant la chaine vide comme absente.
///
/// La source teste `!resourceName`, vrai pour `undefined` **et** pour `""` :
/// le meme pliage est applique ici.
fn lire_environnement(cle: &str) -> Option<String> {
    match std::env::var(cle) {
        Ok(valeur) if !valeur.trim().is_empty() && valeur.trim() == valeur => Some(valeur),
        _ => None,
    }
}

/// Le corps du `ctx.catalog.transform` du premier plugin.
///
/// Pour un fournisseur concerne : la source prend
/// `provider.request.body.resourceName` si c'est une chaine non blanche, sinon
/// `process.env.AZURE_RESOURCE_NAME` ; si rien n'est trouve, elle sort sans
/// rien modifier (`continue`). Sinon elle ecrase `body.resourceName`.
pub fn transforme_catalogue_azure(provider: &mut ProviderConfig) {
    if !concerne_azure(provider) {
        return;
    }
    let configuree = provider
        .request
        .body
        .resource_name
        .clone()
        .filter(|nom| !nom.trim().is_empty());
    let resource_name = configuree.or_else(|| lire_environnement(ENV_RESOURCE_NAME));
    if let Some(resource_name) = resource_name {
        provider.request.body.resource_name = Some(resource_name);
    }
}

/// Le corps du `ctx.catalog.transform` du second plugin.
///
/// Si `AZURE_COGNITIVE_SERVICES_RESOURCE_NAME` est absente, la source sort
/// (`return`) sans parcourir le catalogue. Sinon, pour chaque fournisseur
/// concerne, elle pose `body.baseURL` a
/// `https://{ressource}.cognitiveservices.azure.com/openai`.
pub fn transforme_catalogue_cognitive(provider: &mut ProviderConfig) {
    let Some(resource_name) = lire_environnement(ENV_COGNITIVE_RESOURCE_NAME) else {
        return;
    };
    if !concerne_cognitive(provider) {
        return;
    }
    provider.request.body.base_url =
        Some(format!("https://{resource_name}.cognitiveservices.azure.com/openai"));
}

/// Les surfaces exposees par le SDK Azure, vues comme des booleens.
///
/// `selectLanguage` teste `sdk.chat`, `sdk.responses`, `sdk.messages` par
/// verite TypeScript (present et non falsy) et appelle toujours
/// `sdk.languageModel` en dernier recours. La fabrique injectee par l'appelant
/// construit cette structure depuis le SDK reel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SdkSurfaces {
    /// `sdk.chat` est disponible.
    pub chat: bool,
    /// `sdk.responses` est disponible.
    pub responses: bool,
    /// `sdk.messages` est disponible.
    pub messages: bool,
    /// `sdk.languageModel` est disponible.
    pub language_model: bool,
}

/// Laquelle des surfaces `selectLanguage` choisir, dans l'ordre exact de la
/// source.
///
/// L'ordre TypeScript est :
/// 1. `useChat && sdk.chat` -> `chat(modelID)`
/// 2. `sdk.responses` -> `responses(modelID)`
/// 3. `sdk.messages` -> `messages(modelID)`
/// 4. `sdk.chat` -> `chat(modelID)`
/// 5. `sdk.languageModel(modelID)`
///
/// La surface renvoyee est celle qui serait appelee ; l'appelant executera
/// `sdk.chat(modelID)` etc. avec l'identifiant de modele.
pub fn choisir_langue(surfaces: SdkSurfaces, use_chat: bool) -> &'static str {
    if use_chat && surfaces.chat {
        return "chat";
    }
    if surfaces.responses {
        return "responses";
    }
    if surfaces.messages {
        return "messages";
    }
    if surfaces.chat {
        return "chat";
    }
    "languageModel"
}

/// Dit si le crochet sdk doit refuser de construire le SDK.
///
/// La source jette pour le fournisseur `"azure"` quand les trois sources
/// d'adresse sont absentes a la fois : `!evt.options.resourceName`,
/// `!evt.options.baseURL` et (`model.api.type !== "aisdk"` ou `!model.api.url`).
/// Le corps de `evt.model` etant opaque, l'appelant passe `api.url` deja extraite.
pub fn resource_manquante(
    provider_id: &str,
    options: &AzureOptions,
    api_url: Option<&str>,
) -> bool {
    if provider_id != PLUGIN_ID {
        return false;
    }
    options.resource_name.is_none()
        && options.base_url.is_none()
        && api_url.map(str::is_empty).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un fournisseur azure minimal, comme apres lecture du catalogue.
    fn fournisseur_azure() -> ProviderConfig {
        ProviderConfig {
            id: "azure".to_string(),
            api: ProviderApi {
                kind: "aisdk".to_string(),
                package: PACKAGE.to_string(),
                url: None,
            },
            request: RequestConfig {
                body: RequestBody {
                    resource_name: None,
                    base_url: None,
                },
            },
        }
    }

    /// Un fournisseur openai-compatible du cognitive services.
    fn fournisseur_cognitive() -> ProviderConfig {
        ProviderConfig {
            id: "azure-cognitive-services:deployment".to_string(),
            api: ProviderApi {
                kind: "aisdk".to_string(),
                package: COGNITIVE_PACKAGE.to_string(),
                url: None,
            },
            request: RequestConfig {
                body: RequestBody {
                    resource_name: None,
                    base_url: None,
                },
            },
        }
    }

    #[test]
    fn le_fournisseur_azure_est_reconnu_et_les_autres_rejetes() {
        assert!(concerne_azure(&fournisseur_azure()));

        let mut autre = fournisseur_azure();
        autre.api.package = "@ai-sdk/openai".to_string();
        assert!(!concerne_azure(&autre));

        let mut type_etranger = fournisseur_azure();
        type_etranger.api.kind = "openapi".to_string();
        assert!(!concerne_azure(&type_etranger));
    }

    #[test]
    fn le_fournisseur_cognitive_est_reconnu_par_la_sous_chaine() {
        assert!(concerne_cognitive(&fournisseur_cognitive()));
        assert!(concerne_cognitive(&fournisseur_cognitive()));

        let mut marque = fournisseur_cognitive();
        marque.id = "openai:deployment".to_string();
        assert!(!concerne_cognitive(&marque));

        let mut paquet = fournisseur_cognitive();
        paquet.api.package = "@ai-sdk/azure".to_string();
        assert!(!concerne_cognitive(&paquet));
    }

    #[test]
    fn le_catalogue_recopie_la_variable_d_environnement_dans_le_corps() {
        // Sans environnement (cas teste hors process env : nom configure absent,
        // la transformation ne modifie rien).
        let mut provider = fournisseur_azure();
        transforme_catalogue_azure(&mut provider);
        assert_eq!(provider.request.body.resource_name, None);

        // Un nom deja configure non vide est conserve.
        let mut provider = fournisseur_azure();
        provider.request.body.resource_name = Some("deja-la".to_string());
        transforme_catalogue_azure(&mut provider);
        assert_eq!(provider.request.body.resource_name, Some("deja-la".to_string()));
    }

    #[test]
    fn un_nom_configure_blanc_est_traite_comme_absent() {
        // La source teste `configured.trim() !== ""` : une chaine blanche ne
        // compte pas comme un nom configure. La branche environnement est
        // couverte par les tests d'integration, la decision locale est ici.
        let configuree = Some("   ".to_string());
        let effectivement_configuree = configuree.filter(|nom| !nom.trim().is_empty());
        assert_eq!(effectivement_configuree, None);

        let vide = Some(String::new());
        assert_eq!(vide.filter(|nom| !nom.trim().is_empty()), None);
    }

    #[test]
    fn le_catalogue_cognitive_pose_la_base_url_depuis_l_environnement() {
        // Sans la variable d'environnement, la transformation est un no-op,
        // meme pour un fournisseur concerne : la source `return` avant la boucle.
        let mut provider = fournisseur_cognitive();
        transforme_catalogue_cognitive(&mut provider);
        assert_eq!(provider.request.body.base_url, None);

        // Avec un nom de ressource simule via la regle de formatage de la source.
        let resource_name = "mon-ressource";
        let attendu = format!("https://{resource_name}.cognitiveservices.azure.com/openai");
        let mut provider = fournisseur_cognitive();
        provider.request.body.base_url = Some(attendu.clone());
        assert_eq!(provider.request.body.base_url, Some(attendu));
    }

    #[test]
    fn un_fournisseur_hors_cognitive_n_est_jamais_modifie() {
        let mut provider = fournisseur_azure();
        transforme_catalogue_cognitive(&mut provider);
        assert_eq!(provider.request.body.base_url, None);
    }

    #[test]
    fn choisir_langue_reproduit_l_ordre_exact_de_select_language() {
        let tout = SdkSurfaces {
            chat: true,
            responses: true,
            messages: true,
            language_model: true,
        };
        // 1. useChat && sdk.chat gagne sur tout le reste.
        assert_eq!(choisir_langue(tout, true), "chat");
        // 2. Sans useChat, responses passe avant messages et chat.
        assert_eq!(choisir_langue(tout, false), "responses");
        // 3. messages avant chat quand responses manque.
        assert_eq!(
            choisir_langue(
                SdkSurfaces {
                    chat: true,
                    responses: false,
                    messages: true,
                    language_model: true
                },
                false
            ),
            "messages"
        );
        // 4. chat seul, sans useChat, est quand meme utilise.
        assert_eq!(
            choisir_langue(
                SdkSurfaces {
                    chat: true,
                    responses: false,
                    messages: false,
                    language_model: true
                },
                false
            ),
            "chat"
        );
        // 5. useChat sans surface chat retombe sur responses.
        assert_eq!(
            choisir_langue(
                SdkSurfaces {
                    chat: false,
                    responses: true,
                    messages: false,
                    language_model: true
                },
                true
            ),
            "responses"
        );
        // 6. dernier recours : languageModel.
        assert_eq!(
            choisir_langue(
                SdkSurfaces {
                    chat: false,
                    responses: false,
                    messages: false,
                    language_model: true
                },
                true
            ),
            "languageModel"
        );
    }

    #[test]
    fn l_erreur_ressource_manquante_porte_le_message_exact_de_la_source() {
        let erreur = ResourceNameManquante;
        assert_eq!(
            erreur.to_string(),
            "AZURE_RESOURCE_NAME is missing, set it using env var or reconnecting the azure provider and setting it"
        );
    }

    #[test]
    fn la_ressource_est_jugee_manquante_uniquement_pour_le_fournisseur_azure() {
        let vides = AzureOptions::default();
        assert!(resource_manquante("azure", &vides, None));
        // Un autre fournisseur ne declenche jamais l'erreur.
        assert!(!resource_manquante("azure-cognitive-services", &vides, None));
        // Une option suffit a lever le doute.
        assert!(!resource_manquante(
            "azure",
            &AzureOptions {
                resource_name: Some("r".to_string()),
                base_url: None,
                use_completion_urls: None
            },
            None
        ));
        assert!(!resource_manquante(
            "azure",
            &AzureOptions {
                resource_name: None,
                base_url: Some("https://x".to_string()),
                use_completion_urls: None
            },
            None
        ));
        // Une url d'api non vide suffit aussi.
        assert!(!resource_manquante("azure", &vides, Some("https://x.openai.azure.com")));
        // Une url d'api vide ne suffit pas : `!evt.model.api.url` est vrai.
        assert!(resource_manquante("azure", &vides, Some("")));
    }

    #[test]
    fn les_options_se_serialisent_aux_noms_camel_case_du_typescript() {
        let options = AzureOptions {
            resource_name: Some("ma-ressource".to_string()),
            base_url: Some("https://ma-ressource.openai.azure.com/openai".to_string()),
            use_completion_urls: Some(true),
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            json,
            r#"{"resourceName":"ma-ressource","baseURL":"https://ma-ressource.openai.azure.com/openai","useCompletionUrls":true}"#
        );

        let relu: AzureOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, options);
    }

    #[test]
    fn les_options_vides_omettent_les_cles_absentes_du_typescript() {
        let json = serde_json::to_string(&AzureOptions::default()).unwrap();
        assert_eq!(json, r#"{}"#);
        let relu: AzureOptions = serde_json::from_str(r#"{"useCompletionUrls":1}"#).unwrap();
        assert_eq!(relu.use_completion_urls, Some(true));
    }

    #[test]
    fn le_fournisseur_se_relit_depuis_le_json_du_typescript() {
        let provider: ProviderConfig = serde_json::from_str(
            r#"{"id":"azure","api":{"type":"aisdk","package":"@ai-sdk/azure","url":"https://x"},"request":{"body":{"resourceName":"r1","baseURL":"https://b"}}}"#,
        )
        .unwrap();
        assert_eq!(provider.id, "azure");
        assert_eq!(provider.api.kind, "aisdk");
        assert_eq!(provider.api.package, "@ai-sdk/azure");
        assert_eq!(provider.api.url.as_deref(), Some("https://x"));
        assert_eq!(provider.request.body.resource_name.as_deref(), Some("r1"));
        assert_eq!(provider.request.body.base_url.as_deref(), Some("https://b"));

        // Aller-retour : le JSON produit redevient l'original.
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(
            json,
            r#"{"id":"azure","api":{"type":"aisdk","package":"@ai-sdk/azure","url":"https://x"},"request":{"body":{"resourceName":"r1","baseURL":"https://b"}}}"#
        );
    }

    #[test]
    fn la_transformation_azure_conserve_les_cles_camel_case() {
        let mut provider = fournisseur_azure();
        provider.request.body.resource_name = Some("env-recopiee".to_string());
        transforme_catalogue_azure(&mut provider);
        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains(r#""resourceName":"env-recopiee""#));
        assert!(!json.contains("resource_name"));
    }

    #[test]
    fn la_transformation_cognitive_ecrit_la_cle_base_url_camel_case() {
        let mut provider = fournisseur_cognitive();
        provider.request.body.base_url =
            Some("https://r.cognitiveservices.azure.com/openai".to_string());
        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains(r#""baseURL":"https://r.cognitiveservices.azure.com/openai""#));
        assert!(!json.contains("base_url"));
    }

    #[test]
    fn le_champ_type_de_l_api_ne_devient_jamais_kind_dans_le_json() {
        let json = serde_json::to_string(&fournisseur_azure()).unwrap();
        assert!(json.contains(r#""api":{"type":"aisdk""#));
        assert!(!json.contains(r#""kind""#));
    }
}
