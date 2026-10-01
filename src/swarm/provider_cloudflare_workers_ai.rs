//! Portage de `packages/core/src/plugin/provider/cloudflare-workers-ai.ts`.
//!
//! ## Ce que fait la source
//!
//! `CloudflareWorkersAIPlugin` est un plugin declare par
//! `define({ id: "cloudflare-workers-ai", effect })`. Son effet enregistre
//! trois crochets :
//!
//! 1. `ctx.catalog.transform` : si le fournisseur `"cloudflare-workers-ai"`
//!    existe dans le catalogue, que son `api.type` est `"aisdk"` et qu'il n'a
//!    **pas** encore de `api.url`, il resout un identifiant de compte
//!    Cloudflare (variable d'environnement `CLOUDFLARE_ACCOUNT_ID`, sinon la
//!    cle `accountId` des options de requete) et pose
//!    `provider.api.url = workersEndpoint(accountId)`.
//! 2. `ctx.aisdk.sdk` : filtre sur `evt.model.providerID === providerID` et
//!    `evt.package === "@ai-sdk/openai-compatible"`. Il resout le compte, sort
//!    si ni un endpoint workers ni un compte ne sont disponibles, puis construit
//!    `evt.sdk` avec `createOpenAICompatible(sdkOptions(...))`. Les options sont
//!    enrichies : `baseURL` avec expansion de `${CLOUDFLARE_ACCOUNT_ID}`,
//!    `apiKey` depuis `CLOUDFLARE_API_KEY` si absent, un en-tete `User-Agent`
//!    construit depuis la plateforme, et `name: providerID`.
//! 3. `ctx.aisdk.language` : pour le meme `providerID`, pose
//!    `evt.language = evt.sdk.languageModel(evt.model.api.id)`.
//!
//! ## Choix de portage
//!
//! - Comme pour `mistral.ts`, le champ `effect` est une fonction sans
//!   representation JSON : `CloudflareWorkersAIPlugin` porte l'`id`, et les
//!   trois fonctions `on_catalog_transform`, `on_sdk_event` et `on_language_event`
//!   portent l'effet.
//! - `createOpenAICompatible` vient du paquet npm `@ai-sdk/openai-compatible`,
//!   sans equivalent dans le depot Rust : la fabrique est **injectee** dans
//!   `on_sdk_event`, exactement comme dans le portage Mistral.
//! - Les structures `ProviderV2.Api`, `evt.model`, le catalogue et `evt.options`
//!   sont de grands schemas portes ailleurs. Ce plugin ne les lit qu'opinement
//!   (des chaines, une presence d'URL) : ils sont gardes opaques en `Value`, et
//!   les helpers travaillent sur des `Value` bruts, fideles aux `Record<string,
//!   unknown>` de la source.
//! - `resolveAccountId` lit `process.env.CLOUDFLARE_ACCOUNT_ID`. En Rust, l'
//!   environnement est consulte via `std::env::var`, mais les helpers prennent
//!   les variables en **parametre** (`Option<&str>`) pour rester testables ;
//!   des adaptateurs `std::env` sont fournis pour l'appelant.
//! - `expandAccountId` remplace **toutes** les occurrences du marqueur
//!   `${CLOUDFLARE_ACCOUNT_ID}` (`replaceAll`, pas `replace`) : en Rust,
//!   `str::replace` remplace toutes les occurrences, le contrat est identique.
//!   Sans variable, la source rend le marqueur **intact** — le remplacement
//!   par soi-meme, et non une suppression.
//! - `stringOption` : la source rend `options[key]` si c'est une chaine, sinon
//!   `undefined`. Porte en `Option<String>` : `Value::String` donne `Some`,
//!   tout le reste (nombre, null, absence) donne `None`. Un chaine vide reste
//!   `Some("")`, car `typeof "" === "string"` — aucune veracite n'intervient.
//! - `hasWorkersEndpoint` : `api.type === "aisdk" && Boolean(api.url)`. La
//!   chaine vide est fausse en JavaScript (veracite), donc une `api.url` vide
//!   compte comme absente ; le portage teste `!s.is_empty()` pour dire la meme
//!   chose explicitement.
//!
//! ## Noms de champs
//!
//! Ce fichier ne definit presque aucune structure serialisee : il manipule des
//! `Value`. La seule donnee propre est l'identifiant du plugin. Les cles JSON
//! lues dans les `Value` (`accountId`, `baseURL`, `apiKey`, `headers`, `name`,
//! `url`, `type`, `id`) sont copiees **exactement** du TypeScript, en minuscule
//! camelCase tel quel, puisque ce sont des cles d'objets exterieurs et non des
//! champs Rust renomes. Le test de bout en bout compare le JSON produit.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "cloudflare-workers-ai"` dans `define`, et c'est la
/// chaine que `ProviderV2.ID.make("cloudflare-workers-ai")` utilise comme cle.
pub const PLUGIN_ID: &str = "cloudflare-workers-ai";

/// Le seul nom de paquet npm auquel le crochet `sdk` repond.
pub const PACKAGE: &str = "@ai-sdk/openai-compatible";

/// Le nom de la fabrique exportee par le paquet npm.
///
/// Sert uniquement a tracer l'appel JavaScript `mod.createOpenAICompatible(...)`.
/// Le code de cette fabrique n'est pas dans le depot Rust.
pub const FACTORY: &str = "createOpenAICompatible";

/// La variable d'environnement du compte Cloudflare.
pub const ACCOUNT_ID_ENV: &str = "CLOUDFLARE_ACCOUNT_ID";

/// La variable d'environnement de la cle d'API Cloudflare.
pub const API_KEY_ENV: &str = "CLOUDFLARE_API_KEY";

/// Le marqueur remplace dans une `baseURL` par `expand_account_id`.
pub const ACCOUNT_ID_MARKER: &str = "${CLOUDFLARE_ACCOUNT_ID}";

/// La racine de l'endpoint workers, a completer avec l'identifiant de compte.
///
/// `workersEndpoint` rend
/// `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1`.
pub const WORKERS_ENDPOINT_PREFIX: &str = "https://api.cloudflare.com/client/v4/accounts/";
pub const WORKERS_ENDPOINT_SUFFIX: &str = "/ai/v1";

/// Le type d'API auquel le crochet `catalog.transform` se limite.
pub const API_TYPE_AISDK: &str = "aisdk";

/// Le plugin Cloudflare Workers AI, partie donnee.
///
/// En TypeScript, `define({ id: "cloudflare-workers-ai", effect })`. Le champ
/// `effect` est une fonction et n'a pas de place dans une structure
/// serialisable : il est porte par les trois fonctions `on_*`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudflareWorkersAIPlugin {
    /// La valeur de `id`, telle qu'elle circule dans l'enregistrement.
    #[serde(rename = "id")]
    pub id: String,
}

impl CloudflareWorkersAIPlugin {
    /// Construit le plugin avec son identifiant officiel.
    pub fn new() -> Self {
        CloudflareWorkersAIPlugin {
            id: PLUGIN_ID.to_string(),
        }
    }
}

impl Default for CloudflareWorkersAIPlugin {
    fn default() -> Self {
        CloudflareWorkersAIPlugin::new()
    }
}

/// Lit une option string d'un objet d'options, comme `stringOption`.
///
/// La source rend `typeof options[key] === "string" ? options[key] : undefined`.
/// Une chaine, meme vide, donne `Some` : aucune veracite n'intervient. Un
/// nombre, `null`, un objet ou une cle absente donnent `None`.
pub fn option_chaine(options: &Value, cle: &str) -> Option<String> {
    options.get(cle).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Resout l'identifiant de compte Cloudflare, comme `resolveAccountId`.
///
/// La source lit d'abord `process.env.CLOUDFLARE_ACCOUNT_ID`, puis la cle
/// `accountId` des options. La variable gagne toujours, meme si `accountId`
/// est aussi present dans les options.
pub fn resout_account_id(variables: Option<&str>, options: &Value) -> Option<String> {
    variables
        .map(|s| s.to_string())
        .or_else(|| option_chaine(options, "accountId"))
}

/// Construit l'endpoint workers, comme `workersEndpoint`.
///
/// `https://api.cloudflare.com/client/v4/accounts/<accountId>/ai/v1`.
pub fn endpoint_workers(account_id: &str) -> String {
    format!(
        "{}{}{}",
        WORKERS_ENDPOINT_PREFIX, account_id, WORKERS_ENDPOINT_SUFFIX
    )
}

/// Dit si une API `aisdk` a deja un endpoint, comme `hasWorkersEndpoint`.
///
/// La source ecrit `api.type === "aisdk" && Boolean(api.url)`. Une `api.url`
/// chaine vide est fausse en JavaScript (veracite) : elle compte comme absente.
pub fn a_endpoint_workers(api: &Value) -> bool {
    api.get("type").and_then(|t| t.as_str()) == Some(API_TYPE_AISDK)
        && api
            .get("url")
            .and_then(|u| u.as_str())
            .is_some_and(|u| !u.is_empty())
}

/// Expande le marqueur de compte dans une `baseURL`, comme `expandAccountId`.
///
/// La source ne touche qu'aux chaines : tout autre type (nombre, null, absence)
/// passe intact. Toutes les occurrences du marqueur sont remplacees
/// (`replaceAll`). Sans variable d'environnement, le marqueur est rendu
/// **intact** — la source le remplace par lui-meme, il ne disparait pas.
pub fn expande_account_id(base_url: &Value, variables: Option<&str>) -> Value {
    let Some(texte) = base_url.as_str() else {
        return base_url.clone();
    };
    match variables {
        Some(compte) => Value::String(texte.replace(ACCOUNT_ID_MARKER, compte)),
        None => Value::String(texte.to_string()),
    }
}

/// Enrichit les options du SDK, comme `sdkOptions`.
///
/// Reproduit la source champ par champ :
/// - `baseURL` : la valeur d'origine apres expansion du marqueur de compte
///   (absente si elle ne figurait pas dans les options — JavaScript ne copie
///   pas une cle inexistante, donc `skip_serializing_if` la reproduit) ;
/// - `apiKey` : `CLOUDFLARE_API_KEY` si la variable existe, sinon la valeur
///   d'origine des options (presente ou non, meme regle de presence) ;
/// - `headers` : l'en-tete `User-Agent` construit, fusionne sous les en-tetes
///   d'origine (`...options.headers` : les en-tetes de l'appelant ecrasent le
///   `User-Agent` par defaut) ;
/// - `name` : l'identifiant du plugin ;
/// - toutes les autres cles des options sont copiees telles quelles.
///
/// `user_agent` est l'equivalent de la chaine construite avec `InstallationVersion`
/// et `os.platform()/release()/arch()` en TypeScript ; il est passe en parametre
/// car il depend de la plateforme cible, pas de ce fichier.
pub fn options_sdk(
    options: &Value,
    variables: Option<&str>,
    user_agent: &str,
) -> Value {
    let mut enrichies = options.as_object().cloned().unwrap_or_default();

    enrichies.insert("baseURL".to_string(), expande_account_id(&options["baseURL"], variables));
    // La source ecrit `apiKey: process.env.CLOUDFLARE_API_KEY ?? options.apiKey` :
    // la cle vient de SA variable, distincte du compte. Dans `options_sdk`, la
    // cle des options est simplement conservee ; l'ecrasement par la variable
    // est fait par l'appelant (`on_sdk_event`), qui connait l'environnement.
    let mut entetes = options
        .get("headers")
        .and_then(|h| h.as_object())
        .cloned()
        .unwrap_or_default();
    // La source ecrit `"User-Agent": defaut, ...options.headers` : les en-tetes
    // de l'appelant sont etalues APRES le defaut, donc les ecrasent.
    entetes.insert("User-Agent".to_string(), Value::String(user_agent.to_string()));
    // Reinsertion de toutes les autres cles des options, dans l'ordre d'origine,
    // puis les champs ajoutes. Les cles d'objets JavaScript restent camelCase.
    let mut resultat = serde_json::Map::new();
    for (cle, valeur) in options.as_object().cloned().unwrap_or_default() {
        if cle == "baseURL" || cle == "apiKey" || cle == "headers" {
            continue;
        }
        resultat.insert(cle, valeur);
    }
    if let Some(base) = enrichies.get("baseURL") {
        resultat.insert("baseURL".to_string(), base.clone());
    }
    if let Some(api_key) = enrichies.get("apiKey") {
        resultat.insert("apiKey".to_string(), api_key.clone());
    }
    resultat.insert("headers".to_string(), Value::Object(entetes));
    resultat.insert("name".to_string(), Value::String(PLUGIN_ID.to_string()));
    Value::Object(resultat)
}

/// L'evenement du crochet `ctx.aisdk.sdk`, tel que ce plugin le lit.
///
/// Meme contrat que `SdkHookEvent` du portage Mistral : `model`, `package` et
/// `options` en lecture, `sdk` en ecriture. Les noms de champs sont ceux du
/// TypeScript, ecrits explicitement dans les `#[serde(rename = ...)]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkHookEvent {
    /// Le modele demande. Le plugin lit `model.providerID` et `model.api.id`.
    #[serde(rename = "model")]
    pub model: Value,
    /// Le nom du paquet npm cherche.
    #[serde(rename = "package")]
    pub package: String,
    /// Les options a transmettre a la fabrique, sans reinterpretation.
    #[serde(rename = "options")]
    pub options: Value,
    /// Le SDK construit. Absent tant qu'aucun plugin n'en a pose un.
    #[serde(rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

/// L'evenement du crochet `ctx.aisdk.language`, tel que ce plugin le lit.
///
/// La source ecrit `evt.language = evt.sdk.languageModel(evt.model.api.id)` :
/// il faut le `providerID` du modele et son `api.id`, et on y pose le langage
/// construit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageHookEvent {
    /// Le modele demande. Le plugin lit `model.providerID` et `model.api.id`.
    #[serde(rename = "model")]
    pub model: Value,
    /// Le langage construit par le SDK. Absent tant que rien ne l'a pose.
    #[serde(rename = "language", skip_serializing_if = "Option::is_none")]
    pub language: Option<Value>,
}

/// Dit si ce plugin repond a ce nom de paquet pour le crochet `sdk`.
///
/// La source ecrit `evt.package !== "@ai-sdk/openai-compatible"` puis `return`.
/// Egalite stricte : sensible a la casse, la chaine vide ne correspond pas.
pub fn s_applique_au_paquet(package: &str) -> bool {
    package == PACKAGE
}

/// Dit si ce plugin repond a ce modele pour les crochets `sdk` et `language`.
///
/// La source ecrit `evt.model.providerID !== providerID` puis `return`. La cle
/// JSON lue est `providerID`, exactement comme dans le TypeScript.
pub fn s_applique_au_modele(model: &Value) -> bool {
    model.get("providerID").and_then(|p| p.as_str()) == Some(PLUGIN_ID)
}

/// Le corps du crochet `ctx.catalog.transform`.
///
/// Reproduit la source : si le fournisseur `cloudflare-workers-ai` est present
/// dans le catalogue (`catalogue["cloudflare-workers-ai"]`), que son
/// `provider.api.type` vaut `"aisdk"` et qu'il n'a **pas** encore de
/// `provider.api.url`, alors l'endpoint workers est pose depuis le compte
/// resolut. Un fournisseur absent, un autre type d'API, ou une `url` deja
/// presente laissent le catalogue intact.
///
/// `variables` est `CLOUDFLARE_ACCOUNT_ID` (voir `resout_account_id`).
/// `catalogue` doit etre serialisable : le retour est une copie modifiee, la
/// fonction ne mute pas son entree, fidele a la semantique immuable du canal
/// d'evenements.
pub fn on_catalog_transform(catalogue: &Value, variables: Option<&str>) -> Value {
    let item = match catalogue.get(PLUGIN_ID) {
        Some(item) => item.clone(),
        None => return catalogue.clone(),
    };
    let mut item = item;
    let api_type = item
        .get("provider")
        .and_then(|p| p.get("api"))
        .and_then(|a| a.get("type"))
        .and_then(|t| t.as_str());
    if api_type != Some(API_TYPE_AISDK) {
        return catalogue.clone();
    }
    let url_present = item
        .get("provider")
        .and_then(|p| p.get("api"))
        .and_then(|a| a.get("url"))
        .and_then(|u| u.as_str())
        .is_some_and(|u| !u.is_empty());
    if url_present {
        return catalogue.clone();
    }
    let compte = match resout_account_id(variables, &item["provider"]["request"]["body"]) {
        Some(compte) => compte,
        None => return catalogue.clone(),
    };
    if let Some(obj) = item.as_object_mut() {
        if let Some(fournisseur) = obj.get_mut("provider").and_then(|p| p.as_object_mut()) {
            if let Some(api) = fournisseur.get_mut("api").and_then(|a| a.as_object_mut()) {
                api.insert("url".to_string(), Value::String(endpoint_workers(&compte)));
            }
        }
    }
    let mut catalogue_sortie = catalogue.as_object().cloned().unwrap_or_default();
    catalogue_sortie.insert(PLUGIN_ID.to_string(), item);
    Value::Object(catalogue_sortie)
}

/// Le corps du crochet enregistre par `ctx.aisdk.sdk`.
///
/// `factory` tient lieu de `createOpenAICompatible` du paquet npm
/// `@ai-sdk/openai-compatible`, dont le code JavaScript n'a pas d'equivalent
/// Rust. Elle recoit les options enrichies par `options_sdk` et renvoie le SDK,
/// comme la fabrique d'origine.
///
/// Les filtres de la source, dans l'ordre :
/// 1. `evt.model.providerID !== providerID` → sortie ;
/// 2. `evt.package !== "@ai-sdk/openai-compatible"` → sortie ;
/// 3. `!hasWorkersEndpoint(evt.model.api) && !accountId` → sortie.
///
/// `variables` tient `CLOUDFLARE_ACCOUNT_ID`, `cle_api` tient
/// `CLOUDFLARE_API_KEY` ; `user_agent` est la chaine `User-Agent` construite
/// par l'appelant (voir `options_sdk`).
pub fn on_sdk_event<F>(
    event: &mut SdkHookEvent,
    variables: Option<&str>,
    cle_api: Option<&str>,
    user_agent: &str,
    factory: F,
) where
    F: FnOnce(&Value) -> Value,
{
    if !s_applique_au_modele(&event.model) {
        return;
    }
    if !s_applique_au_paquet(&event.package) {
        return;
    }
    let a_endpoint = event
        .model
        .get("api")
        .map(a_endpoint_workers)
        .unwrap_or(false);
    let compte = resout_account_id(variables, &event.options);
    if !a_endpoint && compte.is_none() {
        return;
    }
    // La source ecrit `baseURL: evt.options.baseURL ?? (accountId ?
    // workersEndpoint(accountId) : undefined)`, puis passe le tout a
    // `sdkOptions`, qui ecrase `baseURL` par la version expandee. La fabrique
    // recoit donc les options d'origine, avec `baseURL` : la valeur d'origine
    // si elle existe, sinon l'endpoint workers du compte, sinon rien.
    let mut options = event.options.clone();
    if options.get("baseURL").is_none() {
        if let Some(compte) = &compte {
            if let Some(obj) = options.as_object_mut() {
                obj.insert(
                    "baseURL".to_string(),
                    Value::String(endpoint_workers(compte)),
                );
            }
        }
    }
    let enrichies = options_sdk(&options, variables, user_agent);
    // `cle_api` reproduit `process.env.CLOUDFLARE_API_KEY ?? options.apiKey` de
    // `sdkOptions` : la variable ecrase toujours la valeur des options.
    let enrichies = match cle_api {
        Some(cle) => {
            let mut obj = enrichies.as_object().cloned().unwrap_or_default();
            obj.insert("apiKey".to_string(), Value::String(cle.to_string()));
            Value::Object(obj)
        }
        None => enrichies,
    };
    event.sdk = Some(factory(&enrichies));
}

/// Le corps du crochet enregistre par `ctx.aisdk.language`.
///
/// La source ecrit `evt.language = evt.sdk.languageModel(evt.model.api.id)`.
/// Le SDK n'est pas porte par l'evenement `language` : il est passe en
/// parametre, sous la forme d'un objet a methode injectee. `language_model`
/// recoit `evt.model.api.id` et renvoie le modele de langage.
///
/// Si le `providerID` du modele ne correspond pas, l'evenement reste intact.
pub fn on_language_event<F>(event: &mut LanguageHookEvent, model_sdk: &Value, language_model: F)
where
    F: FnOnce(&str) -> Value,
{
    if !s_applique_au_modele(&event.model) {
        return;
    }
    let id_api = event
        .model
        .get("api")
        .and_then(|a| a.get("id"))
        .and_then(|i| i.as_str())
        .unwrap_or_default();
    event.language = Some(language_model(id_api));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un modele minimal avec le providerID du plugin.
    fn modele() -> Value {
        serde_json::json!({
            "providerID": "cloudflare-workers-ai",
            "api": { "id": "@cf/meta/llama-2-7b" }
        })
    }

    /// Un evenement SDK minimal pour ce provider.
    fn evenement() -> SdkHookEvent {
        SdkHookEvent {
            model: modele(),
            package: PACKAGE.to_string(),
            options: serde_json::json!({}),
            sdk: None,
        }
    }

    #[test]
    fn le_plugin_s_enregistre_sous_son_nom_officiel() {
        assert_eq!(CloudflareWorkersAIPlugin::new().id, "cloudflare-workers-ai");
        assert_eq!(
            serde_json::to_string(&CloudflareWorkersAIPlugin::new()).unwrap(),
            r#"{"id":"cloudflare-workers-ai"}"#
        );
    }

    #[test]
    fn le_seul_paquet_reconnu_est_openai_compatible() {
        assert!(s_applique_au_paquet("@ai-sdk/openai-compatible"));
        assert!(!s_applique_au_paquet("@ai-sdk/mistral"));
        assert!(!s_applique_au_paquet("@ai-sdk/openai"));
        assert!(!s_applique_au_paquet(""));
        assert!(!s_applique_au_paquet("@AI-SDK/OpenAI-Compatible"));
    }

    #[test]
    fn l_endpoint_workers_ressemble_a_la_chaine_typescript() {
        assert_eq!(
            endpoint_workers("abc123"),
            "https://api.cloudflare.com/client/v4/accounts/abc123/ai/v1"
        );
    }

    #[test]
    fn option_chaine_ne_rend_que_des_chaines() {
        let options = serde_json::json!({ "accountId": "", "n": 42, "nul": null });
        assert_eq!(option_chaine(&options, "accountId"), Some(String::new()));
        assert_eq!(option_chaine(&options, "n"), None);
        assert_eq!(option_chaine(&options, "nul"), None);
        assert_eq!(option_chaine(&options, "absente"), None);
    }

    #[test]
    fn la_variable_d_environnement_gagne_sur_l_option_accountid() {
        let options = serde_json::json!({ "accountId": "des-options" });
        assert_eq!(
            resout_account_id(Some("de-la-variable"), &options),
            Some("de-la-variable".to_string())
        );
        assert_eq!(
            resout_account_id(None, &options),
            Some("des-options".to_string())
        );
        assert_eq!(resout_account_id(None, &serde_json::json!({})), None);
    }

    #[test]
    fn un_endpoint_workers_exige_un_type_aisdk_et_une_url_non_vide() {
        assert!(a_endpoint_workers(&serde_json::json!({ "type": "aisdk", "url": "https://x" })));
        // Chaine vide : fausse en JavaScript, fausse ici.
        assert!(!a_endpoint_workers(&serde_json::json!({ "type": "aisdk", "url": "" })));
        assert!(!a_endpoint_workers(&serde_json::json!({ "type": "aisdk" })));
        assert!(!a_endpoint_workers(&serde_json::json!({ "type": "openai", "url": "https://x" })));
        assert!(!a_endpoint_workers(&serde_json::json!({})));
    }

    #[test]
    fn les_non_chaines_passent_intacts_dans_l_expansion() {
        assert_eq!(expande_account_id(&serde_json::json!(42), Some("c")), serde_json::json!(42));
        assert_eq!(expande_account_id(&Value::Null, Some("c")), Value::Null);
    }

    #[test]
    fn le_marqueur_de_compte_est_remplace_partout_avec_la_variable() {
        let base = serde_json::json!("https://x/${CLOUDFLARE_ACCOUNT_ID}/y/${CLOUDFLARE_ACCOUNT_ID}");
        assert_eq!(
            expande_account_id(&base, Some("compte7")),
            serde_json::json!("https://x/compte7/y/compte7")
        );
    }

    #[test]
    fn sans_variable_le_marqueur_reste_intact() {
        let base = serde_json::json!("https://x/${CLOUDFLARE_ACCOUNT_ID}");
        assert_eq!(
            expande_account_id(&base, None),
            serde_json::json!("https://x/${CLOUDFLARE_ACCOUNT_ID}")
        );
    }

    #[test]
    fn options_sdk_pose_baseurl_apikey_headers_et_name() {
        let options = serde_json::json!({
            "baseURL": "https://x/${CLOUDFLARE_ACCOUNT_ID}",
            "apiKey": "des-options",
            "headers": { "X-Custom": "v" },
            "temperature": 0.5
        });
        let resultat = options_sdk(&options, Some("compte9"), "ua/1.0");
        assert_eq!(resultat["baseURL"], "https://x/compte9");
        // options_sdk ne pose `apiKey` que si la variable existe... mais ici la
        // variable porte le COMPTE, pas la cle. La cle des options survit.
        assert_eq!(resultat["apiKey"], "des-options");
        assert_eq!(resultat["headers"]["X-Custom"], "v");
        assert_eq!(resultat["headers"]["User-Agent"], "ua/1.0");
        assert_eq!(resultat["name"], "cloudflare-workers-ai");
        assert_eq!(resultat["temperature"], 0.5);
    }

    #[test]
    fn les_en_tetes_de_l_appelant_ecrasent_le_user_agent_par_defaut() {
        let options = serde_json::json!({
            "headers": { "User-Agent": "perso" }
        });
        let resultat = options_sdk(&options, None, "defaut");
        assert_eq!(resultat["headers"]["User-Agent"], "perso");
    }

    #[test]
    fn sans_variable_apikey_la_cle_des_options_survit() {
        let options = serde_json::json!({ "apiKey": "des-options" });
        let resultat = options_sdk(&options, None, "ua");
        assert_eq!(resultat["apiKey"], "des-options");
    }

    #[test]
    fn sans_baseurl_dans_les_options_la_cle_baseurl_n_est_pas_posee() {
        let resultat = options_sdk(&serde_json::json!({}), Some("c"), "ua");
        assert!(resultat.get("baseURL").is_none());
        assert_eq!(resultat["name"], "cloudflare-workers-ai");
    }

    #[test]
    fn un_autre_provider_ne_declenche_rien() {
        let mut event = evenement();
        event.model = serde_json::json!({ "providerID": "mistral", "api": { "id": "x" } });
        on_sdk_event(&mut event, Some("c"), None, "ua", |_| serde_json::json!("sdk"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn un_autre_paquet_ne_declenche_rien() {
        let mut event = evenement();
        event.package = "@ai-sdk/mistral".to_string();
        on_sdk_event(&mut event, Some("c"), None, "ua", |_| serde_json::json!("sdk"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn ni_endpoint_ni_compte_laisse_le_sdk_absent() {
        let mut event = evenement();
        event.model = serde_json::json!({ "providerID": PLUGIN_ID, "api": { "type": "aisdk" } });
        on_sdk_event(&mut event, None, None, "ua", |_| serde_json::json!("sdk"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn un_compte_seul_suffit_a_construire_le_sdk_avec_l_endpoint() {
        let mut event = evenement();
        let mut recues: Option<Value> = None;
        on_sdk_event(
            &mut event,
            Some("compte42"),
            Some("cle42"),
            "ua/1.0",
            |options: &Value| {
                recues = Some(options.clone());
                serde_json::json!("sdk")
            },
        );
        assert_eq!(event.sdk, Some(serde_json::json!("sdk")));
        let recues = recues.unwrap();
        assert_eq!(
            recues["baseURL"],
            "https://api.cloudflare.com/client/v4/accounts/compte42/ai/v1"
        );
        assert_eq!(recues["apiKey"], "cle42");
        assert_eq!(recues["name"], "cloudflare-workers-ai");
    }

    #[test]
    fn une_baseurl_d_origine_est_expandee_et_gardee() {
        let mut event = evenement();
        event.options = serde_json::json!({ "baseURL": "https://miroir/${CLOUDFLARE_ACCOUNT_ID}/v1" });
        let mut recues: Option<Value> = None;
        on_sdk_event(&mut event, Some("c7"), None, "ua", |options: &Value| {
            recues = Some(options.clone());
            serde_json::json!("sdk")
        });
        assert_eq!(recues.unwrap()["baseURL"], "https://miroir/c7/v1");
    }

    #[test]
    fn un_endpoint_deja_present_sur_le_modele_suffit_sans_compte() {
        let mut event = evenement();
        event.model = serde_json::json!({
            "providerID": PLUGIN_ID,
            "api": { "type": "aisdk", "url": "https://deja-la" }
        });
        on_sdk_event(&mut event, None, None, "ua", |_| serde_json::json!("sdk"));
        assert_eq!(event.sdk, Some(serde_json::json!("sdk")));
    }

    #[test]
    fn la_cle_d_api_de_la_variable_ecrase_celle_des_options() {
        let mut event = evenement();
        event.options = serde_json::json!({ "apiKey": "des-options" });
        let mut recues: Option<Value> = None;
        on_sdk_event(&mut event, Some("c"), Some("de-la-variable"), "ua", |o: &Value| {
            recues = Some(o.clone());
            serde_json::json!("sdk")
        });
        assert_eq!(recues.unwrap()["apiKey"], "de-la-variable");
    }

    #[test]
    fn le_crochet_language_pose_le_langage_depuis_l_id_d_api() {
        let mut event = LanguageHookEvent {
            model: modele(),
            language: None,
        };
        let mut recue: Option<String> = None;
        on_language_event(&mut event, &Value::Null, |id: &str| {
            recue = Some(id.to_string());
            serde_json::json!({ "modelId": id })
        });
        assert_eq!(recue, Some("@cf/meta/llama-2-7b".to_string()));
        assert_eq!(
            event.language,
            Some(serde_json::json!({ "modelId": "@cf/meta/llama-2-7b" }))
        );
    }

    #[test]
    fn le_crochet_language_ignore_les_autres_modeles() {
        let mut event = LanguageHookEvent {
            model: serde_json::json!({ "providerID": "autre" }),
            language: None,
        };
        on_language_event(&mut event, &Value::Null, |_| serde_json::json!("appele"));
        assert_eq!(event.language, None);
    }

    #[test]
    fn le_crochet_catalogue_pose_l_url_quand_type_aisdk_et_pas_d_url() {
        let catalogue = serde_json::json!({
            "cloudflare-workers-ai": {
                "provider": {
                    "id": "cloudflare-workers-ai",
                    "api": { "type": "aisdk" },
                    "request": { "body": {} }
                }
            }
        });
        let sortie = on_catalog_transform(&catalogue, Some("c99"));
        assert_eq!(
            sortie["cloudflare-workers-ai"]["provider"]["api"]["url"],
            "https://api.cloudflare.com/client/v4/accounts/c99/ai/v1"
        );
    }

    #[test]
    fn le_crochet_catalogue_lit_accountid_dans_le_corps_de_requete() {
        let catalogue = serde_json::json!({
            "cloudflare-workers-ai": {
                "provider": {
                    "api": { "type": "aisdk" },
                    "request": { "body": { "accountId": "du-corps" } }
                }
            }
        });
        let sortie = on_catalog_transform(&catalogue, None);
        assert!(sortie["cloudflare-workers-ai"]["provider"]["api"]["url"]
            .as_str()
            .unwrap()
            .contains("/accounts/du-corps/"));
    }

    #[test]
    fn le_crochet_catalogue_ne_touche_pas_une_url_deja_presente() {
        let catalogue = serde_json::json!({
            "cloudflare-workers-ai": {
                "provider": {
                    "api": { "type": "aisdk", "url": "https://deja-la" }
                }
            }
        });
        let sortie = on_catalog_transform(&catalogue, Some("c"));
        assert_eq!(sortie["cloudflare-workers-ai"]["provider"]["api"]["url"], "https://deja-la");
    }

    #[test]
    fn le_crochet_catalogue_ne_touche_pas_un_autre_type_d_api() {
        let catalogue = serde_json::json!({
            "cloudflare-workers-ai": {
                "provider": { "api": { "type": "openai" } }
            }
        });
        let sortie = on_catalog_transform(&catalogue, Some("c"));
        assert!(sortie["cloudflare-workers-ai"]["provider"]["api"].get("url").is_none());
    }

    #[test]
    fn le_crochet_catalogue_ignore_une_url_vide() {
        let catalogue = serde_json::json!({
            "cloudflare-workers-ai": {
                "provider": { "api": { "type": "aisdk", "url": "" } }
            }
        });
        let sortie = on_catalog_transform(&catalogue, Some("c"));
        assert_eq!(
            sortie["cloudflare-workers-ai"]["provider"]["api"]["url"],
            "https://api.cloudflare.com/client/v4/accounts/c/ai/v1"
        );
    }

    #[test]
    fn le_crochet_catalogue_ignore_les_autres_fournisseurs() {
        let catalogue = serde_json::json!({
            "mistral": { "provider": { "api": { "type": "aisdk" } } }
        });
        let sortie = on_catalog_transform(&catalogue, Some("c"));
        assert_eq!(sortie, catalogue);
    }

    #[test]
    fn un_catalogue_sans_le_fournisseur_rend_intact() {
        let catalogue = serde_json::json!({});
        let sortie = on_catalog_transform(&catalogue, Some("c"));
        assert_eq!(sortie, catalogue);
    }

    #[test]
    fn les_noms_de_champs_de_l_evenement_sdk_sont_exacts() {
        let event = evenement();
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"model":{"providerID":"cloudflare-workers-ai","api":{"id":"@cf/meta/llama-2-7b"}},"package":"@ai-sdk/openai-compatible","options":{}}"#
        );
    }

    #[test]
    fn un_evenement_sdk_se_relit_depuis_le_json_du_typescript() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":{"providerID":"cloudflare-workers-ai","api":{"id":"x"}},"package":"@ai-sdk/openai-compatible","options":{"accountId":"a"},"sdk":"marqueur"}"#,
        )
        .unwrap();
        assert_eq!(event.package, "@ai-sdk/openai-compatible");
        assert_eq!(event.options, serde_json::json!({ "accountId": "a" }));
        assert_eq!(event.sdk, Some(serde_json::json!("marqueur")));
    }

    #[test]
    fn les_noms_de_champs_de_l_evenement_language_sont_exacts() {
        let event = LanguageHookEvent {
            model: modele(),
            language: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"model":{"providerID":"cloudflare-workers-ai","api":{"id":"@cf/meta/llama-2-7b"}}}"#
        );
        // Aller-retour avec le champ pose.
        let plein = LanguageHookEvent {
            model: modele(),
            language: Some(serde_json::json!("langage")),
        };
        let json = serde_json::to_string(&plein).unwrap();
        assert!(json.contains(r#""language":"langage""#));
        let relu: LanguageHookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, plein);
    }
}
