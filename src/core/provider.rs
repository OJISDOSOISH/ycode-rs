//! Portage Rust de `opencode/packages/core/src/provider.ts` (25 lignes).
//!
//! La source ne contient aucune logique : elle reexporte les definitions de
//! `packages/schema/src/provider.ts` sous les noms `ID`, `AISDK`, `Native`,
//! `Api`, `Request`, `Info`, plus deux utilitaires de types `MutableApi` et
//! `MutableInfo` qui n existent qu a la compilation TypeScript.
//!
//! Notes de portage :
//! - `export * as ProviderV2 from "./provider"` est un auto reexport
//!   d espace de noms, idiome present dans tout le depot. En Rust c est le
//!   module lui-meme, rien a ecrire.
//! - `Provider.ID` est un `string` marque a la compilation seulement. A
//!   l execution c est une chaine. Traduit par `pub type ProviderId = String`
//!   plus des constantes pour les valeurs statiques connues.
//! - `MutableApi` / `MutableInfo` (`Types.DeepMutable` + `settings: any`) : en
//!   Rust toute valeur possedee est mutable via `mut`, il n y a pas
//!   d equivalent a porter. Les structs ci-dessous sont deja mutables.
//! - `Schema.Record(...)` devient `BTreeMap` (deterministe, regle du swarm).
//! - `Schema.Unknown` / `Schema.Json` deviennent `serde_json::Value`.
//! - `Info.empty(id)` (statics du schema, lignes 64-71) devient
//!   `Info::empty`, fonction pure sans `async`.
//! - La definition reelle portee est celle de
//!   `packages/schema/src/provider.ts:1-72`, lue en entier. Sans elle ce
//!   fichier serait vide, ce que la regle "ne rien inventer" decourage moins
//!   qu une duplication honnete et signalee (precedent `swarm-core_file`) :
//!   si un autre agent porte `packages/schema/src/provider.ts`, les deux
//!   definitions devront etre fusionnees et celle-ci supprimee.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Identifiant de fournisseur. Marque `ProviderV2.ID` en TS, chaine nue en JSON.
pub type ProviderId = String;

/// Valeur de repli `ID.opencode`.
pub const PROVIDER_OPENCODE: &str = "opencode";
/// Valeur statique `ID.anthropic`.
pub const PROVIDER_ANTHROPIC: &str = "anthropic";
/// Valeur statique `ID.openai`.
pub const PROVIDER_OPENAI: &str = "openai";
/// Valeur statique `ID.google`.
pub const PROVIDER_GOOGLE: &str = "google";
/// Valeur statique `ID.googleVertex` : attention au tiret, ce n est pas `googleVertex`.
pub const PROVIDER_GOOGLE_VERTEX: &str = "google-vertex";
/// Valeur statique `ID.githubCopilot`.
pub const PROVIDER_GITHUB_COPILOT: &str = "github-copilot";
/// Valeur statique `ID.amazonBedrock`.
pub const PROVIDER_AMAZON_BEDROCK: &str = "amazon-bedrock";
/// Valeur statique `ID.azure`.
pub const PROVIDER_AZURE: &str = "azure";
/// Valeur statique `ID.openrouter`.
pub const PROVIDER_OPENROUTER: &str = "openrouter";
/// Valeur statique `ID.mistral`.
pub const PROVIDER_MISTRAL: &str = "mistral";
/// Valeur statique `ID.gitlab`.
pub const PROVIDER_GITLAB: &str = "gitlab";

/// Union taggee `Api` sur le champ `type` (`Schema.toTaggedUnion("type")`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderApi {
    #[serde(rename = "aisdk")]
    Aisdk(AisdkApiContent),
    #[serde(rename = "native")]
    Native(NativeApiContent),
}

/// Contenu de la variante `aisdk` sans le tag (le tag est porte par l enum).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AisdkApiContent {
    #[serde(rename = "package")]
    pub package: String,
    #[serde(rename = "url")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "settings")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, serde_json::Value>>,
}

/// Contenu de la variante `native` sans le tag (le tag est porte par l enum).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeApiContent {
    #[serde(rename = "url")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "settings")]
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// Requete par defaut attachee a un fournisseur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    #[serde(rename = "headers")]
    pub headers: BTreeMap<String, String>,
    #[serde(rename = "body")]
    pub body: BTreeMap<String, serde_json::Value>,
}

/// Fiche fournisseur complete (`ProviderV2.Info`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInfo {
    #[serde(rename = "id")]
    pub id: ProviderId,
    #[serde(rename = "integrationID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<String>,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "disabled")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(rename = "api")]
    pub api: ProviderApi,
    #[serde(rename = "request")]
    pub request: ProviderRequest,
}

impl ProviderInfo {
    /// Construit une fiche vide pour un identifiant donne.
    ///
    /// Miroir exact du `statics.empty` du schema : `name` reprend `id`,
    /// l api est `native` avec des reglages vides, la requete est vide.
    /// `integrationID` et `disabled` restent absents.
    pub fn empty(id: ProviderId) -> Self {
        ProviderInfo {
            name: id.clone(),
            id,
            integration_id: None,
            disabled: None,
            api: ProviderApi::Native(NativeApiContent {
                url: None,
                settings: BTreeMap::new(),
            }),
            request: ProviderRequest {
                headers: BTreeMap::new(),
                body: BTreeMap::new(),
            },
        }
    }

    /// Vrai si le fournisseur est desactive (`disabled: true` en TS).
    ///
    /// En TS `disabled` est optionnel : absent vaut non desactive, comme
    /// `?? false`. Un ternaire sur la veracite donnerait le meme resultat
    /// ici car `false` est falsy, donc pas de piege `?` contre `??`.
    pub fn is_disabled(&self) -> bool {
        self.disabled.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_info_vide_reprend_l_id_comme_nom() {
        let info = ProviderInfo::empty("openai".to_string());
        assert_eq!(info.id, "openai");
        assert_eq!(info.name, "openai");
        assert_eq!(info.integration_id, None);
        assert_eq!(info.disabled, None);
    }

    #[test]
    fn un_info_vide_utilise_une_api_native_sans_reglage() {
        let info = ProviderInfo::empty("openai".to_string());
        match &info.api {
            ProviderApi::Native(content) => {
                assert_eq!(content.url, None);
                assert!(content.settings.is_empty());
            }
            other => panic!("attendu native, obtenu {:?}", other),
        }
        assert!(info.request.headers.is_empty());
        assert!(info.request.body.is_empty());
    }

    #[test]
    fn un_fournisseur_absent_ou_faux_n_est_pas_desactive() {
        let mut info = ProviderInfo::empty("openai".to_string());
        assert!(!info.is_disabled());
        info.disabled = Some(false);
        assert!(!info.is_disabled());
        info.disabled = Some(true);
        assert!(info.is_disabled());
    }

    #[test]
    fn l_api_aisdk_serialise_le_tag_aisdk() {
        let api = ProviderApi::Aisdk(AisdkApiContent {
            package: "@ai-sdk/openai".to_string(),
            url: None,
            settings: None,
        });
        let json = serde_json::to_value(&api).unwrap();
        assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("aisdk"));
        assert_eq!(
            json.get("package").and_then(|v| v.as_str()),
            Some("@ai-sdk/openai")
        );
    }

    #[test]
    fn le_champ_integration_id_garde_sa_casse() {
        let mut info = ProviderInfo::empty("openai".to_string());
        info.integration_id = Some("int-1".to_string());
        let json = serde_json::to_value(&info).unwrap();
        // Piege `integrationID` contre `integrationId` : la majuscule compte.
        assert_eq!(
            json.get("integrationID").and_then(|v| v.as_str()),
            Some("int-1")
        );
        assert!(json.get("integrationId").is_none());
        assert!(json.get("integration_id").is_none());
    }

    #[test]
    fn l_url_absente_n_est_pas_serialisee() {
        let api = ProviderApi::Native(NativeApiContent {
            url: None,
            settings: BTreeMap::new(),
        });
        let json = serde_json::to_value(&api).unwrap();
        assert!(json.get("url").is_none());
        assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("native"));
    }

    #[test]
    fn un_type_d_api_inconnu_est_refuse_a_la_lecture() {
        let raw = serde_json::json!({ "type": "inconnu", "settings": {} });
        let parsed: Result<ProviderApi, _> = serde_json::from_value(raw);
        assert!(parsed.is_err());
    }

    #[test]
    fn un_info_lu_depuis_le_json_reprend_ses_champs() {
        let raw = serde_json::json!({
            "id": "anthropic",
            "name": "Anthropic",
            "api": { "type": "native", "settings": {} },
            "request": { "headers": {}, "body": {} }
        });
        let info: ProviderInfo = serde_json::from_value(raw).unwrap();
        assert_eq!(info.id, "anthropic");
        assert_eq!(info.name, "Anthropic");
        assert!(!info.is_disabled());
    }
}
