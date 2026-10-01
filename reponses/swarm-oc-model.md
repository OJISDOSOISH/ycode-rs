# swarm-oc-model

source : opencode/packages/core/src/model.ts
cible : src/swarm/oc_model.rs (a creer par l agent principal, non cree ici)
taille : 9147 octets
tests : 6

===DEBUT===
```rust
//! Portage Rust de `opencode/packages/core/src/model.ts`.
//!
//! Le fichier TS est presque un reexport du schema
//! `packages/schema/src/model.ts` (ID, VariantID, Family, Capabilities,
//! Cost, Ref, Api, Info) avec deux ajouts reels :
//! - `MutableInfo`, qui n a pas de sens en Rust (tout est mutable par
//!   defaut via `mut`), donc porte comme simple alias de `ModelInfo`.
//! - `parse`, qui coupe `fournisseur/modele` sur le premier `/`.
//!
//! Les structs ci-dessous portent le schema source pour garder la
//! compatibilite JSON avec le TypeScript. Un seul champ est en camelCase
//! dans la source (`providerID`, plus `modelID` pour le retour de `parse`)
//! et porte donc un `rename` explicite. Tout champ optionnel du TS est un
//! `Option` avec `skip_serializing_if`.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Identifiant de modele, marque opaque en TS (`ModelV2.ID`).
pub type ModelId = String;
/// Identifiant de fournisseur, marque opaque en TS (`ProviderV2.ID`).
pub type ProviderId = String;
/// Identifiant de variante de modele.
pub type VariantId = String;
/// Famille de modeles, par exemple une gamme d un meme fournisseur.
pub type Family = String;

/// Reference compacte vers un modele : son id plus son fournisseur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub id: ModelId,
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<VariantId>,
}

/// Capacites declarees d un modele.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub tools: bool,
    pub input: Vec<String>,
    pub output: Vec<String>,
}

/// Palier de cout lie a la taille de contexte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostTier {
    #[serde(rename = "type")]
    pub tier_type: String,
    pub size: i64,
}

/// Cache de cout en lecture et en ecriture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostCache {
    pub read: f64,
    pub write: f64,
}

/// Cout d usage d un modele.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<CostTier>,
    pub input: f64,
    pub output: f64,
    pub cache: CostCache,
}

/// Variante `aisdk` de l API d un modele (id + champs Provider.AISDK).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AisdkApi {
    pub id: ModelId,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, serde_json::Value>>,
}

/// Variante `native` de l API d un modele (id + champs Provider.Native).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeApi {
    pub id: ModelId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// Union taggee sur `type`, comme `Schema.toTaggedUnion("type")` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModelApi {
    #[serde(rename = "aisdk")]
    Aisdk(AisdkApi),
    #[serde(rename = "native")]
    Native(NativeApi),
}

/// Corps de requete commun (champs Provider.Request + variante optionnelle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub headers: BTreeMap<String, String>,
    pub body: BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Variante declaree dans `Info.variants` : id + champs Provider.Request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelVariant {
    pub id: VariantId,
    pub headers: BTreeMap<String, String>,
    pub body: BTreeMap<String, serde_json::Value>,
}

/// Date de publication, en secondes depuis l epoch (Schema.Finite en TS).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTime {
    pub released: f64,
}

/// Statut de publication d un modele.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelStatus {
    #[serde(rename = "alpha")]
    Alpha,
    #[serde(rename = "beta")]
    Beta,
    #[serde(rename = "deprecated")]
    Deprecated,
    #[serde(rename = "active")]
    Active,
}

/// Limites de contexte, d entree et de sortie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelLimit {
    pub context: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<i64>,
    pub output: i64,
}

/// Description complete d un modele, miroir de `Model.Info` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: ModelId,
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<Family>,
    pub name: String,
    pub api: ModelApi,
    pub capabilities: Capabilities,
    pub request: ModelRequest,
    pub variants: Vec<ModelVariant>,
    pub time: ModelTime,
    pub cost: Vec<Cost>,
    pub status: ModelStatus,
    pub enabled: bool,
    pub limit: ModelLimit,
}

/// Equivalent de `MutableInfo` en TS : en Rust la mutation passe par
/// `mut`, il n y a donc rien a changer sur la structure.
pub type MutableInfo = ModelInfo;

/// Resultat de `parse` : les deux moities de `fournisseur/modele`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedModelRef {
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    #[serde(rename = "modelID")]
    pub model_id: ModelId,
}

/// Decoupe `fournisseur/modele` sur le premier `/`.
///
/// Replique exacte du TS :
/// `const [providerID, ...modelID] = input.split("/")`
/// puis `modelID.join("/")`.
/// - sans `/`, le modele vaut `""` et tout l entree est le fournisseur.
/// - avec plusieurs `/`, seul le premier separe, le reste reste colle.
/// - `""` donne fournisseur `""` et modele `""`.
pub fn parse(input: &str) -> ParsedModelRef {
    match input.find('/') {
        None => ParsedModelRef {
            provider_id: input.to_string(),
            model_id: String::new(),
        },
        Some(idx) => ParsedModelRef {
            provider_id: input[..idx].to_string(),
            model_id: input[idx + 1..].to_string(),
        },
    }
}

/// Construit un `ModelInfo` par defaut, miroir de `Info.empty` en TS.
///
/// Valeurs reprises du schema : nom = id du modele, api native sans
/// reglages, capacites vides sans outils, requete vide, aucun variant,
/// aucun cout, statut `active`, active, limites de contexte a zero.
pub fn empty_info(provider_id: &str, model_id: &str) -> ModelInfo {
    ModelInfo {
        id: model_id.to_string(),
        provider_id: provider_id.to_string(),
        family: None,
        name: model_id.to_string(),
        api: ModelApi::Native(NativeApi {
            id: model_id.to_string(),
            url: None,
            settings: BTreeMap::new(),
        }),
        capabilities: Capabilities {
            tools: false,
            input: Vec::new(),
            output: Vec::new(),
        },
        request: ModelRequest {
            headers: BTreeMap::new(),
            body: BTreeMap::new(),
            variant: None,
        },
        variants: Vec::new(),
        time: ModelTime { released: 0.0 },
        cost: Vec::new(),
        status: ModelStatus::Active,
        enabled: true,
        limit: ModelLimit {
            context: 0,
            input: None,
            output: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_entree_vide_donne_deux_chaines_vides() {
        let out = parse("");
        assert_eq!(out.provider_id, "");
        assert_eq!(out.model_id, "");
    }

    #[test]
    fn une_entree_sans_slash_donne_un_modele_vide() {
        let out = parse("anthropic");
        assert_eq!(out.provider_id, "anthropic");
        assert_eq!(out.model_id, "");
    }

    #[test]
    fn un_slash_unique_separe_fournisseur_et_modele() {
        let out = parse("anthropic/claude-sonnet");
        assert_eq!(out.provider_id, "anthropic");
        assert_eq!(out.model_id, "claude-sonnet");
    }

    #[test]
    fn plusieurs_slash_recolle_le_modele() {
        // Seul le premier `/` separe, comme `split` + `join("/")` en TS.
        let out = parse("a/b/c");
        assert_eq!(out.provider_id, "a");
        assert_eq!(out.model_id, "b/c");
    }

    #[test]
    fn un_info_vide_porte_les_defauts_attendus() {
        let info = empty_info("anthropic", "claude");
        assert_eq!(info.name, "claude");
        assert_eq!(info.provider_id, "anthropic");
        assert_eq!(info.status, ModelStatus::Active);
        assert!(info.enabled);
        assert!(info.capabilities.input.is_empty());
        assert!(!info.capabilities.tools);
        assert_eq!(info.limit.context, 0);
        assert_eq!(info.limit.output, 0);
        assert_eq!(info.limit.input, None);
        assert_eq!(info.family, None);
        assert!(info.variants.is_empty());
        assert!(info.cost.is_empty());
    }

    #[test]
    fn la_serialisation_garde_les_noms_camel_case() {
        let info = empty_info("anthropic", "claude");
        let json = serde_json::to_value(&info).expect("serialisation JSON");
        assert_eq!(json["providerID"], serde_json::json!("anthropic"));
        assert!(json.get("provider_id").is_none());
        assert_eq!(json["api"]["type"], serde_json::json!("native"));
        let back: ModelInfo = serde_json::from_value(json).expect("retour JSON");
        assert_eq!(back, info);
        let parsed = parse("anthropic/claude-sonnet");
        let v = serde_json::to_value(&parsed).expect("serialisation du parse");
        assert_eq!(v["providerID"], serde_json::json!("anthropic"));
        assert_eq!(v["modelID"], serde_json::json!("claude-sonnet"));
    }
}
```
===FIN===

CONFIANCE : moyenne
POINT FAIBLE : le type exact de `Model.Api` depend du spread `...Provider.AISDK.fields` et `...Provider.Native.fields` que j ai recopie depuis `packages/schema/src/provider.ts` sans acces au typage `effect/Schema` exact ; si un champ (par exemple `settings` requis ou optionnel, ou un champ supplementaire de `AISDK`) differe, la variante serde casse a l echange JSON.
