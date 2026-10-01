//! Portage de `packages/core/src/plugin/provider/opencode.ts`.
//!
//! ## Ce que fait la source
//!
//! `OpencodePlugin` est un plugin declare par
//! `define({ id: "opencode", effect })`. Son effet :
//!
//! 1. enregistre deux methodes d'integration OAuth sur l'integration
//!    `"opencode"` : une methode `device` (flux OAuth Device Grant contre
//!    `https://opencode.ai/console`, client_id `opencode-cli`) et une methode
//!    `key` (cle d'API) ;
//! 2. recharge la configuration distante `GET {server}/api/config` a chaque
//!    `ConnectionUpdated` (sous semaphore), et la transforme en entrees du
//!    catalogue : fournisseurs, modeles, variantes, couts, limites ;
//! 3. si aucune cle n'existe (ni `OPENCODE_API_KEY`, ni connexion active, ni
//!    `apiKey` dans le corps de requete du fournisseur), pose
//!    `provider.request.body.apiKey = "public"` et desactive les modeles dont
//!    le cout d'entree est strictement positif.
//!
//! ## Choix de portage
//!
//! - La mecanique Effect (`Effect.gen`, `Stream`, `Semaphore`, `Scope`, le
//!   client HTTP injecte) n'a pas d'equivalent dans le depot Rust et n'est pas
//!   une donnee serialisable. Ce fichier porte ce qui est un **contrat JSON** :
//!   les schemas `Device`, `Token`, `DeviceToken`, `User`, `Org`, la reponse
//!   distante `RemoteResponse`, les metadonnees du credential, et les deux
//!   fonctions pures `remoteCost` et `withoutCredentials`. Le polling HTTP et
//!   les transformations de catalogue appartiennent aux portages de
//!   `integration.ts`, `catalog.ts` et `credential.ts`.
//! - Les schemas TypeScript ont deja des cles snake_case (`device_code`,
//!   `expires_in`, `tool_call`, `cache_read`, `context_over_200k`) : le
//!   `#[serde(rename)]` explicite est pose quand meme, pour figer le contrat.
//!   Les seuls vrai camelCase sont cote metadonnees : `accountID`, `orgID`,
//!   `orgName`.
//! - `DeviceToken` est une `Schema.Union([Token, TokenPending])` distinguee par
//!   la presence de `"access_token" in result` dans la source : une enum non
//!   etiquetee serde fait exactement ca.
//! - `withoutCredentials` filtre les cles `apiKey` et `headers` du corps de
//!   requete. Les corps restent opaques (`Value`) : la source ne les lit
//!   jamais champ par champ, elle les transmet a `Object.assign` et au
//!   `lowerer` de `ConfigProviderOptionsV1`, porte ailleurs.
//! - `remoteCost` produit un tableau d'un ou deux paliers : le palier de base,
//!   et, si `context_over_200k` est present, un palier de contexte a
//!   200_000 jetons. Les `cache_read` / `cache_write` absents valent 0.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'hote de la console OpenCode, valeur de `defaultServer`.
pub const DEFAULT_SERVER: &str = "https://opencode.ai/console";

/// La valeur de `clientID` envoyee au flux device.
pub const CLIENT_ID: &str = "opencode-cli";

/// L'identifiant de methode OAuth, `Integration.MethodID.make("device")`.
pub const METHOD_ID: &str = "device";

/// L'identifiant d'integration, `Integration.ID.make("opencode")`.
pub const INTEGRATION_ID: &str = "opencode";

/// La premiere etape du flux device, `GET /auth/device/code`.
///
/// Les noms sont ceux du schema `Device` de la source, deja en snake_case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    #[serde(rename = "device_code")]
    pub device_code: String,
    #[serde(rename = "user_code")]
    pub user_code: String,
    #[serde(rename = "verification_uri_complete")]
    pub verification_uri_complete: String,
    #[serde(rename = "expires_in")]
    pub expires_in: f64,
    #[serde(rename = "interval")]
    pub interval: f64,
}

/// Le jeton emis par `POST /auth/device/token`, schema `Token`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
    #[serde(rename = "expires_in")]
    pub expires_in: f64,
}

/// La reponse d'attente du flux device, schema `TokenPending`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenPending {
    #[serde(rename = "error")]
    pub error: String,
}

/// L'union `DeviceToken = Schema.Union([Token, TokenPending])`.
///
/// La source distingue les deux par `"access_token" in result` : enum non
/// etiquetee, qui essaie `Token` d'abord, comme la garde JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeviceToken {
    /// Un jeton complet : la branche `"access_token" in result`.
    Jeton(Token),
    /// Une attente ou une erreur : la branche `result.error`.
    EnAttente(TokenPending),
}

/// Le profil utilisateur, schema `User`, `GET /api/user`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "email")]
    pub email: String,
}

/// Une organisation, schema `Org`, `GET /api/orgs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Org {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
}

/// La reponse distante, schema `RemoteResponse = { config: ConfigV1.Info }`.
///
/// Seul `config.provider` est lu par la source : le reste de `ConfigV1.Info`
/// reste opaque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteResponse {
    /// La carte `providerID -> ConfigProviderV1.Info` de `config.provider`.
    #[serde(rename = "config")]
    pub config: RemoteConfig,
}

/// Le contenu utile de `ConfigV1.Info` : uniquement sa carte `provider`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// `config.provider`, carte `providerID -> ConfigProviderV1.Info`.
    #[serde(rename = "provider")]
    pub provider: Value,
}

/// Un item de la carte `provider` distante, `ConfigProviderV1.Info`.
///
/// La source lit `name`, `npm`, `api`, `options` et `models` ; le reste du
/// schema `ConfigProviderV1` n'est pas lu ici et n'est pas recopie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteProviderItem {
    /// Le nom affiche. Absent possible : `if (item.name !== undefined)`.
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// L'URL d'API passee telle quelle au fournisseur.
    #[serde(rename = "api", skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    /// Le paquet npm du SDK : present => `type: "aisdk"`, absent => `native`.
    #[serde(rename = "npm", skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    /// Les options du fournisseur, corps opaque filtre par `withoutCredentials`.
    #[serde(rename = "options", skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
    /// La carte `modelID -> config` des modeles distants.
    #[serde(rename = "models", skip_serializing_if = "Option::is_none")]
    pub models: Option<std::collections::BTreeMap<String, RemoteModelConfig>>,
}

/// Les modalites d'un modele distant, `config.modalities`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteModalities {
    #[serde(rename = "input", skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<String>>,
    #[serde(rename = "output", skip_serializing_if = "Option::is_none")]
    pub output: Option<Vec<String>>,
}

/// Le cout brut d'un modele distant, `ConfigProviderV1.Model["cost"]`.
///
/// Les cles sont deja en snake_case dans la source TypeScript
/// (`cache_read`, `cache_write`, `context_over_200k`) : les renames les
/// figent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteCost {
    #[serde(rename = "input")]
    pub input: f64,
    #[serde(rename = "output")]
    pub output: f64,
    #[serde(rename = "cache_read", skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(rename = "cache_write", skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    #[serde(rename = "context_over_200k", skip_serializing_if = "Option::is_none")]
    pub context_over_200k: Option<RemoteCostOver200k>,
}

/// La surcouche de cout au-dela de 200k de contexte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteCostOver200k {
    #[serde(rename = "input")]
    pub input: f64,
    #[serde(rename = "output")]
    pub output: f64,
    #[serde(rename = "cache_read", skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(rename = "cache_write", skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

/// Le cache d'un palier de cout produit : `{ read, write }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalierCache {
    #[serde(rename = "read")]
    pub read: f64,
    #[serde(rename = "write")]
    pub write: f64,
}

/// L'etiquette de palier de contexte : `{ type: "context", size: 200_000 }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalierTier {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "size")]
    pub size: f64,
}

/// Un palier de cout produit par `remoteCost`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalierCout {
    /// Present uniquement sur le palier `context_over_200k`.
    #[serde(rename = "tier", skip_serializing_if = "Option::is_none")]
    pub tier: Option<PalierTier>,
    #[serde(rename = "input")]
    pub input: f64,
    #[serde(rename = "output")]
    pub output: f64,
    #[serde(rename = "cache")]
    pub cache: PalierCache,
}

/// La configuration d'un modele distant, `ConfigProviderV1.Model`.
///
/// La source lit `family`, `name`, `id`, `provider`, `tool_call`,
/// `modalities`, `headers`, `options`, `variants`, `release_date`, `cost`,
/// `status` et `limit`. Les cles deja snake_case en TypeScript
/// (`tool_call`, `release_date`) gardent leur nom via rename explicite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteModelConfig {
    #[serde(rename = "family", skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Le fournisseur propre au modele, lu pour `provider.npm` / `provider.api`.
    #[serde(rename = "provider", skip_serializing_if = "Option::is_none")]
    pub provider: Option<RemoteModelProvider>,
    /// Support des outils. Absent possible : `if (config.tool_call !== undefined)`.
    #[serde(rename = "tool_call", skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<bool>,
    #[serde(rename = "modalities", skip_serializing_if = "Option::is_none")]
    pub modalities: Option<RemoteModalities>,
    /// En-tetes fusionnes par `Object.assign(model.request.headers, ...)`.
    #[serde(rename = "headers", skip_serializing_if = "Option::is_none")]
    pub headers: Option<Value>,
    /// Corps de requete opaque, filtre par `withoutCredentials`.
    #[serde(rename = "options", skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
    /// Variantes : carte `variantID -> options`, transformee en tableau par la
    /// source. Gardee en carte ici, la transformation est du cote catalogue.
    #[serde(rename = "variants", skip_serializing_if = "Option::is_none")]
    pub variants: Option<std::collections::BTreeMap<String, Value>>,
    /// Date de sortie, analysee par `Date.parse` cote source.
    #[serde(rename = "release_date", skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(rename = "cost", skip_serializing_if = "Option::is_none")]
    pub cost: Option<RemoteCost>,
    /// `"active"` par defaut ; `"deprecated"` desactive le modele.
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Les limites, recopiees telles quelles (`{ ...config.limit }`).
    #[serde(rename = "limit", skip_serializing_if = "Option::is_none")]
    pub limit: Option<Value>,
}

/// Le sous-objet `provider` d'un modele distant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteModelProvider {
    #[serde(rename = "npm", skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    #[serde(rename = "api", skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
}

/// Les metadonnees du credential OAuth, objet `metadata` de `Credential.OAuth`.
///
/// C'est ici que vivent les seuls vrais camelCase du contrat :
/// `accountID`, `orgID`, `orgName`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataOAuth {
    /// Le serveur utilise pour le flux, sinon `defaultServer`.
    #[serde(rename = "server", skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(rename = "accountID", skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(rename = "email", skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "orgID", skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    #[serde(rename = "orgName", skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
}

/// L'erreur d'echec du flux device, `Device authorization failed: ${error}`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("Device authorization failed: {error}")]
pub struct ErreurAuthDevice {
    /// La valeur de `result.error` renvoyee par le serveur.
    pub error: String,
}

/// Retranche `apiKey` et `headers` d'un corps de requete.
///
/// La source ecrit :
/// `Object.entries(body ?? {}).filter(([key]) => key !== "apiKey" && key !== "headers")`.
/// Un corps absent ou non-objet revient a un objet vide, comme
/// `Object.entries(undefined ?? {})`.
pub fn sans_credentials(body: Option<&Value>) -> Value {
    let Some(Value::Object(carte)) = body else {
        return serde_json::json!({});
    };
    Value::Object(
        carte
            .iter()
            .filter(|(cle, _)| cle.as_str() != "apiKey" && cle.as_str() != "headers")
            .map(|(cle, valeur)| (cle.clone(), valeur.clone()))
            .collect(),
    )
}

/// Le portage de `remoteCost` : un ou deux paliers de cout.
///
/// Sans `context_over_200k`, un palier de base. Avec, un second palier
/// etiquete `{ type: "context", size: 200_000 }`. Les caches absents valent 0
/// (`input.cache_read ?? 0`).
pub fn cout_distant(entree: &RemoteCost) -> Vec<PalierCout> {
    let base = PalierCout {
        tier: None,
        input: entree.input,
        output: entree.output,
        cache: PalierCache {
            read: entree.cache_read.unwrap_or(0.0),
            write: entree.cache_write.unwrap_or(0.0),
        },
    };
    let Some(au_dela) = &entree.context_over_200k else {
        return vec![base];
    };
    vec![
        base,
        PalierCout {
            tier: Some(PalierTier {
                kind: "context".to_string(),
                size: 200_000.0,
            }),
            input: au_dela.input,
            output: au_dela.output,
            cache: PalierCache {
                read: au_dela.cache_read.unwrap_or(0.0),
                write: au_dela.cache_write.unwrap_or(0.0),
            },
        },
    ]
}

/// Dit si la metadonnee `server` du credential remplace `defaultServer`.
///
/// La source ecrit `typeof credential.metadata?.server === "string" ? ...` :
/// seule une chaine compte, pas un nombre ni un objet.
pub fn serveur_du_credential(metadata: Option<&MetadataOAuth>) -> &str {
    match metadata.and_then(|m| m.server.as_deref()) {
        Some(serveur) => serveur,
        None => DEFAULT_SERVER,
    }
}

/// Dit si un modele distant est desactive parce que deporte.
///
/// La source ecrit `model.enabled = config.status !== "deprecated"` : tout
/// statut autre que la chaine exacte `"deprecated"` laisse le modele active.
pub fn est_deporte(statut: Option<&str>) -> bool {
    statut == Some("deprecated")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_device_se_relit_aux_noms_de_champs_typescript() {
        let json = r#"{
            "device_code": "abc",
            "user_code": "WXYZ-TQVH",
            "verification_uri_complete": "https://opencode.ai/device?code=abc",
            "expires_in": 600,
            "interval": 5
        }"#;
        let device: Device = serde_json::from_str(json).unwrap();
        assert_eq!(device.device_code, "abc");
        assert_eq!(device.user_code, "WXYZ-TQVH");
        assert_eq!(device.expires_in, 600.0);
        assert_eq!(device.interval, 5.0);
        let texte = serde_json::to_string(&device).unwrap();
        assert!(texte.contains(r#""device_code":"abc""#));
        assert!(texte.contains(r#""verification_uri_complete""#));
        assert!(texte.contains(r#""expires_in":600"#));
    }

    #[test]
    fn le_token_se_relit_aux_noms_de_champs_typescript() {
        let token: Token = serde_json::from_str(
            r#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#,
        )
        .unwrap();
        assert_eq!(token.access_token, "a");
        assert_eq!(token.refresh_token, "r");
        assert_eq!(token.expires_in, 3600.0);
        let texte = serde_json::to_string(&token).unwrap();
        assert_eq!(
            texte,
            r#"{"access_token":"a","refresh_token":"r","expires_in":3600.0}"#
        );
    }

    #[test]
    fn l_union_device_token_distingue_par_la_presence_d_access_token() {
        let jeton: DeviceToken =
            serde_json::from_str(r#"{"access_token":"a","refresh_token":"r","expires_in":60}"#)
                .unwrap();
        assert_eq!(
            jeton,
            DeviceToken::Jeton(Token {
                access_token: "a".to_string(),
                refresh_token: "r".to_string(),
                expires_in: 60.0,
            })
        );
        let en_attente: DeviceToken = serde_json::from_str(r#"{"error":"authorization_pending"}"#)
            .unwrap();
        assert_eq!(
            en_attente,
            DeviceToken::EnAttente(TokenPending {
                error: "authorization_pending".to_string(),
            })
        );
    }

    #[test]
    fn user_et_org_gardent_leurs_noms_courts() {
        let user: User = serde_json::from_str(r#"{"id":"u1","email":"a@b.c"}"#).unwrap();
        let org: Org = serde_json::from_str(r#"{"id":"o1","name":"Org"}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&user).unwrap(),
            r#"{"id":"u1","email":"a@b.c"}"#
        );
        assert_eq!(serde_json::to_string(&org).unwrap(), r#"{"id":"o1","name":"Org"}"#);
    }

    #[test]
    fn les_metadonnees_portent_les_camelcase_typescript() {
        let meta = MetadataOAuth {
            server: Some("https://console.exemple".to_string()),
            account_id: Some("u1".to_string()),
            email: Some("a@b.c".to_string()),
            org_id: Some("o1".to_string()),
            org_name: Some("Org".to_string()),
        };
        let texte = serde_json::to_string(&meta).unwrap();
        assert!(texte.contains(r#""accountID":"u1""#));
        assert!(texte.contains(r#""orgID":"o1""#));
        assert!(texte.contains(r#""orgName":"Org""#));
        let relu: MetadataOAuth = serde_json::from_str(&texte).unwrap();
        assert_eq!(relu, meta);
    }

    #[test]
    fn les_metadonnees_partielles_se_relisent_et_ne_serialisent_pas_les_absents() {
        let meta: MetadataOAuth = serde_json::from_str(r#"{"orgName":"Org"}"#).unwrap();
        assert_eq!(meta.account_id, None);
        assert_eq!(
            serde_json::to_string(&meta).unwrap(),
            r#"{"orgName":"Org"}"#
        );
    }

    #[test]
    fn sans_credentials_retranche_apikey_et_headers() {
        let corps = serde_json::json!({
            "apiKey": "secret",
            "headers": { "x-a": "b" },
            "model": "opencode"
        });
        assert_eq!(
            sans_credentials(Some(&corps)),
            serde_json::json!({ "model": "opencode" })
        );
    }

    #[test]
    fn sans_credentials_renvoie_un_objet_vide_quand_le_corps_est_absent_ou_non_objet() {
        assert_eq!(sans_credentials(None), serde_json::json!({}));
        assert_eq!(sans_credentials(Some(&Value::Null)), serde_json::json!({}));
        assert_eq!(
            sans_credentials(Some(&serde_json::json!("chaine"))),
            serde_json::json!({})
        );
    }

    #[test]
    fn sans_credentials_ne_confond_pas_la_casse_des_cles_filtrees() {
        let corps = serde_json::json!({ "apiKey": "a", "apikey": "b", "Headers": "c" });
        assert_eq!(
            sans_credentials(Some(&corps)),
            serde_json::json!({ "apikey": "b", "Headers": "c" })
        );
    }

    #[test]
    fn cout_distant_sans_context_over_200k_produit_un_seul_palier() {
        let cout: RemoteCost = serde_json::from_str(
            r#"{"input":1,"output":2,"cache_read":0.5,"cache_write":0.75}"#,
        )
        .unwrap();
        let paliers = cout_distant(&cout);
        assert_eq!(paliers.len(), 1);
        assert_eq!(paliers[0].tier, None);
        assert_eq!(paliers[0].input, 1.0);
        assert_eq!(paliers[0].cache.read, 0.5);
        assert_eq!(paliers[0].cache.write, 0.75);
    }

    #[test]
    fn cout_distant_avec_context_over_200k_produit_le_palier_de_contexte() {
        let cout: RemoteCost = serde_json::from_str(
            r#"{"input":1,"output":2,"context_over_200k":{"input":3,"output":4,"cache_write":9}}"#,
        )
        .unwrap();
        let paliers = cout_distant(&cout);
        assert_eq!(paliers.len(), 2);
        assert_eq!(
            paliers[1].tier,
            Some(PalierTier {
                kind: "context".to_string(),
                size: 200_000.0,
            })
        );
        assert_eq!(paliers[1].input, 3.0);
        assert_eq!(paliers[1].output, 4.0);
        assert_eq!(paliers[1].cache.read, 0.0);
        assert_eq!(paliers[1].cache.write, 9.0);
    }

    #[test]
    fn les_paliers_de_cout_se_serialisent_aux_noms_typescript() {
        let cout: RemoteCost =
            serde_json::from_str(r#"{"input":1,"output":2,"context_over_200k":{"input":3,"output":4}}"#)
                .unwrap();
        let texte = serde_json::to_string(&cout_distant(&cout)).unwrap();
        assert!(texte.contains(r#""cache":{"read":0.0,"write":0.0}"#));
        assert!(texte.contains(r#""tier":{"type":"context","size":200000.0}"#));
    }

    #[test]
    fn l_item_de_fournisseur_distant_se_relit_avec_sa_carte_de_modeles() {
        let json = r#"{
            "name": "OpenCode",
            "npm": "@ai-sdk/opencode",
            "api": "https://api.exemple",
            "options": { "apiKey": "x" },
            "models": {
                "m1": {
                    "family": "f",
                    "tool_call": true,
                    "release_date": "2024-01-02",
                    "status": "deprecated",
                    "cost": { "input": 1, "output": 2 }
                }
            }
        }"#;
        let item: RemoteProviderItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.name.as_deref(), Some("OpenCode"));
        assert_eq!(item.npm.as_deref(), Some("@ai-sdk/opencode"));
        let modeles = item.models.unwrap();
        let m1 = modeles.get("m1").unwrap();
        assert_eq!(m1.tool_call, Some(true));
        assert_eq!(m1.release_date.as_deref(), Some("2024-01-02"));
        assert_eq!(m1.status.as_deref(), Some("deprecated"));
        assert!(est_deporte(m1.status.as_deref()));
        assert_eq!(m1.cost.as_ref().unwrap().input, 1.0);
    }

    #[test]
    fn la_reponse_distante_expose_la_carte_provider() {
        let json = r#"{"config":{"provider":{"opencode":{"npm":"@ai-sdk/opencode"}}}}"#;
        let remote: RemoteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            remote.config.provider,
            serde_json::json!({ "opencode": { "npm": "@ai-sdk/opencode" } })
        );
    }

    #[test]
    fn le_serveur_par_defaut_ne_est_remplace_que_par_une_chaine() {
        let meta: MetadataOAuth =
            serde_json::from_str(r#"{"server":"https://autre.serveur"}"#).unwrap();
        assert_eq!(serveur_du_credential(Some(&meta)), "https://autre.serveur");
        let vide: MetadataOAuth = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(serveur_du_credential(Some(&vide)), DEFAULT_SERVER);
        assert_eq!(serveur_du_credential(None), DEFAULT_SERVER);
    }

    #[test]
    fn seul_le_statut_deprecated_exact_desactive_le_modele() {
        assert!(est_deporte(Some("deprecated")));
        assert!(!est_deporte(Some("active")));
        assert!(!est_deporte(Some("Deprecated")));
        assert!(!est_deporte(None));
    }

    #[test]
    fn l_erreur_d_authorisation_reproduit_le_message_typescript() {
        let erreur = ErreurAuthDevice {
            error: "expired_token".to_string(),
        };
        assert_eq!(
            erreur.to_string(),
            "Device authorization failed: expired_token"
        );
    }

    #[test]
    fn les_constantes_sont_celles_de_la_source() {
        assert_eq!(DEFAULT_SERVER, "https://opencode.ai/console");
        assert_eq!(CLIENT_ID, "opencode-cli");
        assert_eq!(METHOD_ID, "device");
        assert_eq!(INTEGRATION_ID, "opencode");
    }
}
