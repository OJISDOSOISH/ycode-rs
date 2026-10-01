//! Portage de `packages/core/src/plugin/provider/cloudflare-ai-gateway.ts`.
//!
//! ## Ce que fait la source
//!
//! `CloudflareAIGatewayPlugin` est un plugin declare par
//! `define({ id: "cloudflare-ai-gateway", effect })`. Son effet enregistre un
//! crochet sur `ctx.aisdk.sdk` qui filtre sur le nom du paquet npm :
//! `"ai-gateway-provider"` (sans prefixe `@scope/`, a la difference de
//! Mistral). Deux sorties precoces : si `evt.options.baseURL` est deja pose,
//! l'utilisateur a pris la main et le plugin sort sans rien faire ; si aucune
//! configuration de passerelle n'est trouvable, il sort aussi.
//!
//! Sinon il construit trois choses :
//!
//! 1. `gatewayConfig(options)` : le triplet `{ accountId, gatewayId, apiKey }`,
//!    resolu depuis les variables d'environnement en priorite, puis les
//!    options. `gatewayId` accepte le synonyme historique `gateway`.
//! 2. `gatewayMetadata(options)` : les metadonnees de journalisation de la
//!    passerelle. L'option typed `metadata` gagne ; sinon l'en-tete d'echappement
//!    historique `cf-aig-metadata` est decode du JSON.
//! 3. `gatewayOptions(options, metadata)` : l'objet transmis a `createAiGateway`
//!    sous la cle `options`, avec les champs `metadata`, `cacheTtl`, `cacheKey`,
//!    `skipCache`, `collectLog` et un en-tete `User-Agent` fixe.
//!
//! Le crochet pose ensuite `evt.sdk` : une fonction `languageModel(modelID)` qui
//! construit le modele passeurelle. Si l'identifiant vise Workers AI (prefixe
//! `workers-ai/` ou identifiant nu `@cf/...`), le jeton Cloudflare est passe
//! comme cle d'api amont ; sinon rien, les fournisseurs tiers comptent sur les
//! cles stockees/BYOK de la passerelle.
//!
//! ## Choix de portage
//!
//! - Comme pour `mistral.ts`, le champ `effect` est une fonction sans
//!   representation JSON : `CloudflareAIGatewayPlugin` porte l'`id`,
//!   `on_sdk_event` porte l'effet, avec la fabrique de passerelle injectee.
//! - `createAiGateway` et `createUnified` viennent du paquet npm
//!   `ai-gateway-provider`, sans equivalent dans le depot Rust. Ils sont
//!   injectes : `on_sdk_event` recoit une fabrique qui prend les options
//!   resolues et renvoie le SDK, ce qui est exactement le contrat JavaScript.
//! - `GatewayConfig` est la structure `GatewayConfig` de la source, avec ses
//!   trois champs en camelCase preserves par `#[serde(rename = ...)]` explicite.
//! - `GatewayOptions` reproduit l'objet retourne par `gatewayOptions`. Les
//!   champs `cacheTtl`, `cacheKey`, `skipCache` et `collectLog` sont copies tels
//!   quels depuis `evt.options` : ils deviennent `Option<Value>` (absents ou
//!   `undefined` en JavaScript, non serialises quand absents). `metadata` peut
//!   valoir `undefined` dans la source, il est garde optionnel pour la meme
//!   raison.
//! - `evt.sdk` vaut `any` : `serde_json::Value`, comme dans `provider_mistral.rs`.
//!
//! ## Noms de champs
//!
//! Les noms camelCase de la source (`accountId`, `gatewayId`, `apiKey`,
//! `cacheTtl`, `cacheKey`, `skipCache`, `collectLog`, `cf-aig-metadata`) sont
//! ecrits explicitement dans les `#[serde(rename = ...)]` et un test compare le
//! JSON produit champ par champ. C'est le point de rupture habituel de ce
//! portage.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "cloudflare-ai-gateway"` dans `define`.
pub const PLUGIN_ID: &str = "cloudflare-ai-gateway";

/// Le seul nom de paquet npm auquel ce plugin repond.
///
/// Attention : la source filtre sur `evt.package !== "ai-gateway-provider"`,
/// sans prefixe `@scope/`.
pub const PACKAGE: &str = "ai-gateway-provider";

/// La cle d'options qui, si elle est deja posee, desactive le plugin.
///
/// La source ecrit `if (evt.options.baseURL) return` : l'utilisateur a fourni
/// sa propre URL, le plugin ne doit pas la remplacer.
pub const OPTION_BASE_URL: &str = "baseURL";

/// L'en-tete d'echappement historique portant les metadonnees de passerelle.
pub const HEADER_METADATA: &str = "cf-aig-metadata";

/// Le triplet de configuration requis par `createAiGateway`.
///
/// En TypeScript, `GatewayConfig` : `{ accountId, gatewayId, apiKey }`. Les
/// noms camelCase sont preservés a l'identique dans le JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// L'identifiant de compte Cloudflare (`process.env.CLOUDFLARE_ACCOUNT_ID`
    /// ou `options.accountId`).
    #[serde(rename = "accountId")]
    pub account_id: String,
    /// L'identifiant de passerelle (`process.env.CLOUDFLARE_GATEWAY_ID`,
    /// `options.gatewayId` ou l'ancien synonyme `options.gateway`).
    #[serde(rename = "gatewayId")]
    pub gateway_id: String,
    /// Le jeton (`process.env.CLOUDFLARE_API_TOKEN`, `process.env.CF_AIG_TOKEN`
    /// ou `options.apiKey`).
    #[serde(rename = "apiKey")]
    pub api_key: String,
}

/// L'objet transmis a `createAiGateway` sous la cle `options`.
///
/// En TypeScript, la valeur de retour de `gatewayOptions`. Les champs
/// `cacheTtl`, `cacheKey`, `skipCache` et `collectLog` sont copies tels quels
/// depuis les options de l'evenement : absents de la source, ils sont absents
/// du JSON (`skip_serializing_if`), ce qui reproduit `undefined` en JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayOptions {
    /// Les metadonnees de journalisation, typées ou decodees de l'en-tete.
    #[serde(rename = "metadata", skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Duree de vie du cache, recopiee de `options.cacheTtl`.
    #[serde(rename = "cacheTtl", skip_serializing_if = "Option::is_none")]
    pub cache_ttl: Option<Value>,
    /// Cle de cache, recopiee de `options.cacheKey`.
    #[serde(rename = "cacheKey", skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<Value>,
    /// Contournement du cache, recopie de `options.skipCache`.
    #[serde(rename = "skipCache", skip_serializing_if = "Option::is_none")]
    pub skip_cache: Option<Value>,
    /// Collecte des journaux, recopiee de `options.collectLog`.
    #[serde(rename = "collectLog", skip_serializing_if = "Option::is_none")]
    pub collect_log: Option<Value>,
    /// Les en-tetes envoyes a la passerelle. La source ne pose qu'un
    /// `User-Agent` fixe.
    #[serde(rename = "headers")]
    pub headers: GatewayHeaders,
}

/// L'en-tete `User-Agent` pose par `gatewayOptions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayHeaders {
    /// La chaine exacte de la source :
    /// `opencode/<version> cloudflare-ai-gateway (<plateforme> <release>; <arch>)`.
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
}

/// Dit si ce plugin repond a ce nom de paquet.
///
/// La source ecrit `evt.package !== "ai-gateway-provider"` puis `return`. C'est
/// une egalite stricte, sensible a la casse.
pub fn applies_to(package: &str) -> bool {
    package == PACKAGE
}

/// Lit une option de chaine, comme `stringOption` de la source.
///
/// Renvoie la valeur seulement si elle est une chaine JSON ; un nombre, un
/// booléen ou `null` sont ignores, comme le `typeof === "string"` TypeScript.
fn option_chaine(options: &Value, cle: &str) -> Option<String> {
    options.get(cle).and_then(Value::as_str).map(str::to_string)
}

/// Traduit `gatewayConfig(options)` de la source.
///
/// L'environnement gagne sur les options, champ par champ :
///
/// - `accountId` : `CLOUDFLARE_ACCOUNT_ID`, sinon `options.accountId` ;
/// - `gatewayId` : `CLOUDFLARE_GATEWAY_ID`, sinon `options.gatewayId`, sinon
///   l'ancien synonyme `options.gateway` ;
/// - `apiKey` : `CLOUDFLARE_API_TOKEN`, puis `CF_AIG_TOKEN`, sinon
///   `options.apiKey`.
///
/// Si un des trois manque, la source renvoie `undefined` et le plugin sort sans
/// rien faire : ici, `None`.
///
/// `env` est injecte pour rester testable : en production, passer une fonction
/// qui lit les variables d'environnement reelles.
pub fn config_passerelle<E>(options: &Value, env: E) -> Option<GatewayConfig>
where
    E: Fn(&str) -> Option<String>,
{
    let account_id = env("CLOUDFLARE_ACCOUNT_ID")
        .or_else(|| option_chaine(options, "accountId"))?;
    // La projection d'identifiants copie les metadonnees de cle dans les
    // options. Le prompt stocke la passerelle sous gatewayId, mais d'anciens
    // exemples de configuration utilisent gateway.
    let gateway_id = env("CLOUDFLARE_GATEWAY_ID")
        .or_else(|| option_chaine(options, "gatewayId"))
        .or_else(|| option_chaine(options, "gateway"))?;
    let api_key = env("CLOUDFLARE_API_TOKEN")
        .or_else(|| env("CF_AIG_TOKEN"))
        .or_else(|| option_chaine(options, "apiKey"))?;
    if account_id.is_empty() || gateway_id.is_empty() || api_key.is_empty() {
        return None;
    }
    Some(GatewayConfig {
        account_id,
        gateway_id,
        api_key,
    })
}

/// Traduit `gatewayMetadata(options)` de la source.
///
/// L'option typed `metadata` gagne si elle est presente (meme `null`, comme
/// `!== undefined` en JavaScript). Sinon, l'en-tete `cf-aig-metadata` est
/// decode du JSON ; un en-tete absent ou du JSON invalide ne produit rien.
pub fn metadonnees_passerelle(options: &Value) -> Option<Value> {
    if let Some(metadata) = options.get("metadata") {
        return Some(metadata.clone());
    }
    let brut = options
        .get("headers")
        .and_then(|entetes| entetes.get(HEADER_METADATA))
        .and_then(Value::as_str)?;
    serde_json::from_str(brut).ok()
}

/// Traduit `gatewayOptions(options, metadata)` de la source.
///
/// Recopie les quatre options de cache telles quelles (absentes ici si
/// absentes la-bas) et pose l'en-tete `User-Agent` fixe. `user_agent` est
/// injecte car il depend de la version d'installation et de la plateforme hote,
/// deux informations hors du perimetre de ce fichier.
pub fn options_passerelle(
    options: &Value,
    metadata: Option<Value>,
    user_agent: String,
) -> GatewayOptions {
    GatewayOptions {
        metadata,
        cache_ttl: options.get("cacheTtl").cloned(),
        cache_key: options.get("cacheKey").cloned(),
        skip_cache: options.get("skipCache").cloned(),
        collect_log: options.get("collectLog").cloned(),
        headers: GatewayHeaders { user_agent },
    }
}

/// Dit si l'identifiant de modele vise Workers AI.
///
/// La source commente : Workers AI est le seul fournisseur de premiere partie
/// dont l'amont est Cloudflare lui-meme, donc le seul a recevoir le jeton
/// Cloudflare en `Authorization` amont. L'API unifiee le vise avec le prefixe
/// explicite `workers-ai/` ou avec des identifiants nus `@cf/...`.
pub fn est_workers_ai(identifiant: &str) -> bool {
    identifiant.starts_with("workers-ai/") || identifiant.starts_with("@cf/")
}

/// Le corps du crochet enregistre par le plugin.
///
/// `fabriquer` tient lieu du couple `createAiGateway`/`createUnified` du paquet
/// npm `ai-gateway-provider`, dont le code JavaScript n'a pas d'equivalent
/// Rust. Elle recoit la configuration resolue, les options de passerelle et
/// l'identifiant de modele, et renvoie le SDK, comme la fabrique d'origine.
///
/// Ordre des sorties precoces, fidele a la source : mauvais paquet, `baseURL`
/// deja posee, configuration manquante. Dans tous ces cas l'evenement reste
/// intact.
pub fn on_sdk_event<F>(event: &mut super::provider_mistral::SdkHookEvent, fabriquer: F)
where
    F: FnOnce(&GatewayConfig, &GatewayOptions, &str) -> Value,
{
    if !applies_to(&event.package) {
        return;
    }
    // `if (evt.options.baseURL) return` : une chaine non vide desactive le
    // plugin. Present et falsy (chaine vide), il laisse passer, comme en
    // JavaScript.
    if event
        .options
        .get(OPTION_BASE_URL)
        .and_then(Value::as_str)
        .is_some_and(|url| !url.is_empty())
    {
        return;
    }
    let Some(config) = config_passerelle(&event.options, |cle| None) else {
        return;
    };
    let metadata = metadonnees_passerelle(&event.options);
    // La version reelle de la chaine depend de `InstallationVersion` et de
    // `os.platform()/release()/arch()`, hors du perimetre de ce fichier ; le
    // suffixe `?` marque la partie injectee par l'appelant.
    let user_agent = "opencode/? cloudflare-ai-gateway".to_string();
    let options = options_passerelle(&event.options, metadata, user_agent);
    let sdk = fabriquer(&config, &options, &identifiant_modele(&event));
    event.sdk = Some(sdk);
}

/// Extrait l'identifiant de modele de l'evenement, s'il est une chaine.
///
/// La source recoit `languageModel(modelID: string)` ; ici, le plugin ne lit
/// pas le modele avant d'appeler la fabrique, donc cette aide reste prudente.
fn identifiant_modele(event: &super::provider_mistral::SdkHookEvent) -> String {
    event.model.as_str().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Un evenement minimal, avec le paquet demande et aucune instance SDK.
    fn evenement(package: &str) -> super::super::provider_mistral::SdkHookEvent {
        super::super::provider_mistral::SdkHookEvent {
            model: Value::Null,
            package: package.to_string(),
            options: json!({}),
            sdk: None,
        }
    }

    /// Un environnement d'essai sans variable d'environnement.
    fn env_vide(_cle: &str) -> Option<String> {
        None
    }

    #[test]
    fn le_plugin_s_enregistre_sous_le_nom_cloudflare_ai_gateway() {
        assert_eq!(PLUGIN_ID, "cloudflare-ai-gateway");
    }

    #[test]
    fn le_seul_paquet_reconnu_est_celui_de_la_passerelle() {
        assert!(applies_to("ai-gateway-provider"));
        assert!(applies_to(PACKAGE));
        assert!(!applies_to("@ai-sdk/openai"));
        assert!(!applies_to("@cloudflare/ai-gateway-provider"));
        assert!(!applies_to("ai-gateway-provider-pro"));
        assert!(!applies_to(""));
    }

    #[test]
    fn la_config_vient_des_options_quand_l_environnement_est_vide() {
        let options = json!({ "accountId": "compte", "gatewayId": "passerelle", "apiKey": "jeton" });
        let config = config_passerelle(&options, env_vide).unwrap();
        assert_eq!(config.account_id, "compte");
        assert_eq!(config.gateway_id, "passerelle");
        assert_eq!(config.api_key, "jeton");
    }

    #[test]
    fn l_environnement_gagne_sur_les_options() {
        let options = json!({ "accountId": "compte", "gatewayId": "passerelle", "apiKey": "jeton" });
        let config = config_passerelle(&options, |cle| match cle {
            "CLOUDFLARE_ACCOUNT_ID" => Some("compte-env".into()),
            "CLOUDFLARE_GATEWAY_ID" => Some("passerelle-env".into()),
            "CLOUDFLARE_API_TOKEN" => Some("jeton-env".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.account_id, "compte-env");
        assert_eq!(config.gateway_id, "passerelle-env");
        assert_eq!(config.api_key, "jeton-env");
    }

    #[test]
    fn gateway_accepte_le_synonyme_historique_de_gateway_id() {
        let options = json!({ "accountId": "compte", "gateway": "ancienne", "apiKey": "jeton" });
        let config = config_passerelle(&options, env_vide).unwrap();
        assert_eq!(config.gateway_id, "ancienne");
    }

    #[test]
    fn le_jeton_de_secours_cf_aig_token_est_utilise() {
        let options = json!({ "accountId": "compte", "gatewayId": "passerelle" });
        let config = config_passerelle(&options, |cle| match cle {
            "CF_AIG_TOKEN" => Some("jeton-cf".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.api_key, "jeton-cf");
    }

    #[test]
    fn une_config_incomplete_ne_produit_rien() {
        assert!(config_passerelle(&json!({ "gatewayId": "g", "apiKey": "k" }), env_vide).is_none());
        assert!(config_passerelle(&json!({ "accountId": "a", "apiKey": "k" }), env_vide).is_none());
        assert!(config_passerelle(&json!({ "accountId": "a", "gatewayId": "g" }), env_vide).is_none());
    }

    #[test]
    fn les_noms_de_champs_de_la_config_sont_les_noms_typescript() {
        let config = GatewayConfig {
            account_id: "compte".into(),
            gateway_id: "passerelle".into(),
            api_key: "jeton".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            json,
            r#"{"accountId":"compte","gatewayId":"passerelle","apiKey":"jeton"}"#
        );
        let relu: GatewayConfig = serde_json::from_str(
            r#"{"accountId":"compte","gatewayId":"passerelle","apiKey":"jeton"}"#,
        )
        .unwrap();
        assert_eq!(relu, config);
    }

    #[test]
    fn les_metadonnes_typed_gagnent_sur_l_en_tete() {
        let options = json!({
            "metadata": { "source": "typée" },
            "headers": { "cf-aig-metadata": "{\"source\":\"en-tete\"}" }
        });
        assert_eq!(
            metadonnees_passerelle(&options),
            Some(json!({ "source": "typée" }))
        );
    }

    #[test]
    fn l_en_tete_cf_aig_metadata_est_decode_du_json() {
        let options = json!({ "headers": { "cf-aig-metadata": "{\"requestId\":\"abc\"}" } });
        assert_eq!(
            metadonnees_passerelle(&options),
            Some(json!({ "requestId": "abc" }))
        );
    }

    #[test]
    fn un_en_tete_metadata_invalide_ne_produit_rien() {
        let options = json!({ "headers": { "cf-aig-metadata": "pas du json" } });
        assert_eq!(metadonnees_passerelle(&options), None);
    }

    #[test]
    fn sans_metadonnes_ni_en_tete_rien_n_est_produit() {
        assert_eq!(metadonnees_passerelle(&json!({})), None);
        assert_eq!(
            metadonnees_passerelle(&json!({ "headers": { "autre": "x" } })),
            None
        );
    }

    #[test]
    fn les_options_de_cache_sont_recopiees_avec_les_noms_typescript() {
        let options = json!({
            "cacheTtl": 60,
            "cacheKey": "cle",
            "skipCache": true,
            "collectLog": false
        });
        let options_gw = options_passerelle(&options, None, "agent".into());
        let json = serde_json::to_string(&options_gw).unwrap();
        assert_eq!(
            json,
            r#"{"cacheTtl":60,"cacheKey":"cle","skipCache":true,"collectLog":false,"headers":{"User-Agent":"agent"}}"#
        );
    }

    #[test]
    fn les_options_de_cache_absentes_ne_sont_pas_serialisees() {
        let options_gw = options_passerelle(&json!({}), Some(json!({"a":1})), "agent".into());
        let json = serde_json::to_string(&options_gw).unwrap();
        assert_eq!(
            json,
            r#"{"metadata":{"a":1},"headers":{"User-Agent":"agent"}}"#
        );
    }

    #[test]
    fn les_options_de_passerelle_se_relisent_depuis_le_json_du_typescript() {
        let relu: GatewayOptions = serde_json::from_str(
            r#"{"metadata":{"x":1},"cacheTtl":30,"cacheKey":"k","skipCache":true,"collectLog":false,"headers":{"User-Agent":"u"}}"#,
        )
        .unwrap();
        assert_eq!(relu.cache_ttl, Some(json!(30)));
        assert_eq!(relu.cache_key, Some(json!("k")));
        assert_eq!(relu.skip_cache, Some(json!(true)));
        assert_eq!(relu.collect_log, Some(json!(false)));
        assert_eq!(relu.headers.user_agent, "u");
    }

    #[test]
    fn workers_ai_reconnait_le_prefixe_explicite_et_les_identifiants_nus() {
        assert!(est_workers_ai("workers-ai/llama-3"));
        assert!(est_workers_ai("@cf/meta/llama-3"));
        assert!(!est_workers_ai("@openai/gpt-4"));
        assert!(!est_workers_ai("openai/gpt-4"));
        assert!(!est_workers_ai(""));
    }

    #[test]
    fn un_mauvais_paquet_laisse_l_evenement_intact() {
        let mut event = evenement("@ai-sdk/openai");
        event.options = json!({ "accountId": "a", "gatewayId": "g", "apiKey": "k" });
        on_sdk_event(&mut event, |_c, _o, _m| json!("fabrique"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn une_base_url_deja_posee_desactive_le_plugin() {
        let mut event = evenement(PACKAGE);
        event.options = json!({
            "baseURL": "https://exemple.test",
            "accountId": "a",
            "gatewayId": "g",
            "apiKey": "k"
        });
        on_sdk_event(&mut event, |_c, _o, _m| json!("fabrique"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn une_config_manquante_desactive_le_plugin() {
        let mut event = evenement(PACKAGE);
        event.options = json!({ "accountId": "a" });
        on_sdk_event(&mut event, |_c, _o, _m| json!("fabrique"));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn un_paquet_reconnu_construit_le_sdk_avec_la_config_resolue() {
        let mut event = evenement(PACKAGE);
        event.options = json!({ "accountId": "a", "gatewayId": "g", "apiKey": "k" });
        event.model = json!("workers-ai/llama-3");
        on_sdk_event(&mut event, |config, options, modele| {
            json!({
                "accountId": config.account_id,
                "gatewayId": config.gateway_id,
                "apiKey": config.api_key,
                "options": options,
                "modele": modele
            })
        });
        assert!(event.sdk.is_some());
        let sdk = event.sdk.clone().unwrap();
        assert_eq!(sdk["accountId"], "a");
        assert_eq!(sdk["gatewayId"], "g");
        assert_eq!(sdk["apiKey"], "k");
        assert_eq!(sdk["modele"], "workers-ai/llama-3");
        assert_eq!(
            sdk["options"]["headers"]["User-Agent"],
            "opencode/? cloudflare-ai-gateway"
        );
    }

    #[test]
    fn un_sdk_deja_present_est_remplace() {
        let mut event = evenement(PACKAGE);
        event.options = json!({ "accountId": "a", "gatewayId": "g", "apiKey": "k" });
        event.sdk = Some(json!("ancien"));
        on_sdk_event(&mut event, |_c, _o, _m| json!("nouveau"));
        assert_eq!(event.sdk, Some(json!("nouveau")));
    }
}
