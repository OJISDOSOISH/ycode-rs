//! Portage de `packages/core/src/plugin/provider/sap-ai-core.ts`.
//!
//! ## Ce que fait la source
//!
//! `SapAICorePlugin` est un plugin declare par
//! `define({ id: "sap-ai-core", effect })`, et son effet enregistre **deux**
//! crochets sur `ctx.aisdk`, tous deux filtres sur
//! `evt.model.providerID === "sap-ai-core"`.
//!
//! Le crochet `sdk` :
//! 1. lit `process.env.AICORE_SERVICE_KEY`, sinon `evt.options.serviceKey`
//!    **seulement si c'est une chaine** (`typeof === "string"`) ;
//! 2. si cette cle existe et que `AICORE_SERVICE_KEY` n'est pas deja defini,
//!    elle est ecrite dans l'environnement ;
//! 3. resout le chemin du paquet : `evt.package` s'il commence par
//!    `"file://"`, sinon le `entrypoint` renvoye par `npm.add` ;
//! 4. importe dynamiquement ce chemin, cherche la premiere export dont le nom
//!    commence par `"create"` et l'appelle avec
//!    `{ deploymentId, resourceGroup }` (depuis l'environnement) si une cle de
//!    service existe, `{}` sinon.
//!
//! Le crochet `language` est une ligne : `evt.language = evt.sdk(evt.model.api.id)`.
//!
//! ## Choix de portage
//!
//! - Comme dans `mistral.rs`, le champ `effect` est une fonction sans
//!   representation JSON : `SapAiCorePlugin` porte l'`id`, les fonctions
//!   `on_sdk_event` et `on_language_event` portent les effets.
//! - `process.env` n'existe pas en Rust : l'environnement est un parametre
//!   explicite `Environnement`, que `on_sdk_event` met a jour (`&mut`) quand la
//!   source ecrirait `process.env.AICORE_SERVICE_KEY = ...`. Le test verifie ce
//!   write-back sans toucher a l'environnement reel du processus.
//! - `npm.add` et l'import dynamique sont du monde JavaScript : ils sont
//!   injectes. `installer` tient lieu de `npm.add` et renvoie le `entrypoint`
//!   (`Option`, car la source teste `if (!installedPath)`), `fabrique` tient
//!   lieu du module importe appele sur ses parametres. `trouver_export`
//!   reproduit la recherche de la premiere export `create*`.
//! - `evt.model` reste opaque en `Value` : le plugin lit seulement
//!   `providerID` et `api.id`, extraits par des accesseurs tolerants qui
//!   renvoient `None` sur toute forme inattendue.
//! - Les parametres `{ deploymentId, resourceGroup }` sont une structure
//!   serialisee : en JavaScript, un champ `undefined` disparait du JSON, donc
//!   chaque `Option` est `skip_serializing_if`. Sans cle de service, la source
//!   passe `{}` : les deux champs a `None` serialisent exactement `{}`.
//!
//! ## Le piege `?` contre `??`
//!
//! Ici il est reel et porte par `resoudre_service_key` : la source ecrit
//! `process.env.AICORE_SERVICE_KEY ?? (typeof evt.options.serviceKey ===
//! "string" ? ... : undefined)`. Un `??` ne remplace que `null`/`undefined` :
//! une chaine vide dans l'environnement **gagne** et masque l'option. Le test
//! `une_cle_vide_dans_l_environnement_gagne` fige ce cas.
//!
//! ## Noms de champs
//!
//! Les noms camelCase du contrat TypeScript sont ecrits explicitement :
//! `serviceKey`, `deploymentId`, `resourceGroup`, `providerID`, `api`, `id`,
//! `package`, `options`, `sdk`, `language`. Un test compare le JSON produit
//! champ par champ, parce que c'est le point de rupture habituel de ce
//! portage.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "sap-ai-core"` dans `define`.
pub const PLUGIN_ID: &str = "sap-ai-core";

/// L'identifiant de fournisseur auquel le plugin repond.
///
/// La source compare `evt.model.providerID` a `ProviderV2.ID.make("sap-ai-core")` :
/// une egalite stricte de chaine, sensible a la casse.
pub const PROVIDER_ID: &str = "sap-ai-core";

/// Variables d'environnement lues par le plugin.
pub const ENV_SERVICE_KEY: &str = "AICORE_SERVICE_KEY";
pub const ENV_DEPLOYMENT_ID: &str = "AICORE_DEPLOYMENT_ID";
pub const ENV_RESOURCE_GROUP: &str = "AICORE_RESOURCE_GROUP";

/// Le plugin SAP AI Core, partie donnee.
///
/// En TypeScript, `define({ id: "sap-ai-core", effect })`. Le champ `effect`
/// est une fonction et n'a pas de place dans une structure serialisable : il
/// est porte par `on_sdk_event` et `on_language_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SapAiCorePlugin {
    /// La valeur de `id`, telle qu'elle circule dans l'enregistrement.
    #[serde(rename = "id")]
    pub id: String,
}

impl SapAiCorePlugin {
    /// Construit le plugin avec son identifiant officiel.
    pub fn new() -> Self {
        SapAiCorePlugin {
            id: PLUGIN_ID.to_string(),
        }
    }
}

impl Default for SapAiCorePlugin {
    fn default() -> Self {
        SapAiCorePlugin::new()
    }
}

/// L'environnement vu par le plugin, en lieu et place de `process.env`.
///
/// Seules les trois variables `AICORE_*` sont lues ; `on_sdk_event` peut
/// ecrire `service_key`, exactement comme la source ecrit
/// `process.env.AICORE_SERVICE_KEY` quand elle n'est pas deja definie.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Environnement {
    pub service_key: Option<String>,
    pub deployment_id: Option<String>,
    pub resource_group: Option<String>,
}

/// L'evenement recu par le crochet `sdk`.
///
/// Meme forme que `SDKEvent` dans `core/src/aisdk.ts` (voir `mistral.rs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkHookEvent {
    /// Le modele demande. Le filtre lit `providerID`.
    #[serde(rename = "model")]
    pub model: Value,
    /// Le nom (ou l'URL `file://`) du paquet npm a importer.
    #[serde(rename = "package")]
    pub package: String,
    /// Les options transmises par l'appelant. Le plugin lit `serviceKey`.
    #[serde(rename = "options")]
    pub options: Value,
    /// Le SDK construit. Absent tant qu'aucun plugin n'en a pose un.
    #[serde(rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

/// L'evenement recu par le crochet `language`.
///
/// En TypeScript, `evt.sdk` est le SDK deja construit (une fonction) et
/// `evt.language` le resultat. Le SDK etant un appel, il est injecte dans
/// `on_language_event` et n'apparait pas dans la structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageHookEvent {
    /// Le modele demande. Le plugin lit `api.id`.
    #[serde(rename = "model")]
    pub model: Value,
    /// Le modele de langage construit. Absent tant que le crochet n'a pas tourne.
    #[serde(rename = "language", skip_serializing_if = "Option::is_none")]
    pub language: Option<Value>,
}

/// Les parametres passes a la fabrique du paquet.
///
/// La source appelle `mod[match]({ deploymentId, resourceGroup })` quand une
/// cle de service existe, `{}` sinon. En JavaScript, un champ `undefined`
/// n'apparait pas dans le JSON : chaque `Option` est donc absente du JSON
/// quand elle vaut `None`, et une structure vide serialise exactement `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParametresFabrique {
    #[serde(rename = "deploymentId", skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<String>,
    #[serde(rename = "resourceGroup", skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,
}

/// Les erreurs que le crochet `sdk` peut lever.
///
/// Ce sont les deux `throw new Error(...)` de la source, mot pour mot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Erreur {
    #[error("le paquet {0} n'a pas de point d'entree d'import")]
    PointEntreeManquant(String),
    #[error("le paquet {0} n'exporte aucune fabrique de fournisseur")]
    FabriqueManquante(String),
}

/// Dit si ce plugin repond a cet identifiant de fournisseur.
pub fn applies_to(provider_id: &str) -> bool {
    provider_id == PROVIDER_ID
}

/// Extrait `providerID` du modele, s'il est une chaine.
pub fn extraire_provider_id(model: &Value) -> Option<&str> {
    model.get("providerID")?.as_str()
}

/// Extrait `serviceKey` des options, uniquement si c'est une chaine.
///
/// La source ecrit `typeof evt.options.serviceKey === "string" ? ... :
/// undefined` : un nombre ou un objet ne compte pas, meme serialisable.
pub fn extraire_service_key(options: &Value) -> Option<&str> {
    options.get("serviceKey")?.as_str()
}

/// Resout la cle de service : l'environnement d'abord, l'option ensuite.
///
/// C'est le `??` de la source : `process.env.AICORE_SERVICE_KEY ??
/// evt.options.serviceKey`. Un `??` ne remplace que `null`/`undefined`,
/// donc une chaine vide dans l'environnement gagne et masque l'option.
/// Resout la cle de service : l'environnement d'abord, l'option ensuite.
///
/// C'est le `??` de la source : `process.env.AICORE_SERVICE_KEY ??
/// evt.options.serviceKey`. Un `??` ne remplace que `null`/`undefined`,
/// donc une chaine vide dans l'environnement gagne et masque l'option.
///
/// Le retour est PROPRE, pas emprunte. Les deux branches originate de deux
/// objets differents -- `env` et `options` -- et un `?` ne peut pas designer
/// les deux. Emporter la chaine evite d immibiliser un des deux, ce qui
/// empechait ensuite d ecrire `env.service_key = ...`.
pub fn resoudre_service_key(options: &Value, env: &Environnement) -> Option<String> {
    match &env.service_key {
        Some(cle) => Some(cle.clone()),
        None => extraire_service_key(options).map(str::to_string),
    }
}

/// Extrait `api.id` du modele, s'il est une chaine.
pub fn extraire_api_id(model: &Value) -> Option<&str> {
    model.get("api")?.get("id")?.as_str()
}

/// Trouve la premiere export dont le nom commence par `"create"`.
///
/// La source ecrit `Object.keys(mod).find((name) => name.startsWith("create"))` :
/// ordre des cles du module, prefixe sensible a la casse.
pub fn trouver_export<'a>(exports: &[&'a str]) -> Option<&'a str> {
    exports.iter().copied().find(|nom| nom.starts_with("create"))
}

/// Resout le chemin du paquet a importer.
///
/// La source ecrit `evt.package.startsWith("file://") ? evt.package :
/// (npm.add(evt.package)).entrypoint`. `None` represente le cas ou
/// `npm.add` ne renvoie pas de point d'entree : c'est le `throw` du
/// `if (!installedPath)`, porte par `Erreur::PointEntreeManquant`.
pub fn resoudre_chemin_paquet(package: &str, entree_npm: Option<&str>) -> Option<String> {
    if package.starts_with("file://") {
        Some(package.to_string())
    } else {
        entree_npm.map(|entree| entree.to_string())
    }
}

/// Le corps du crochet enregistre par le plugin sur `ctx.aisdk.sdk`.
///
/// - `installer` tient lieu de `npm.add` : il recoit le nom du paquet et
///   renvoie son point d'entree, ou `None` s'il n'y en a pas.
/// - `fabrique` tient lieu du module importe : il recoit le chemin resolu et
///   les parametres, et renvoie le SDK. La recherche d'export `create*` reste
///   de la responsabilite de l'appelant via `trouver_export`, qui renvoie
///   `Erreur::FabriqueManquante` a reproduire.
///
/// Si l'identifiant de fournisseur ne correspond pas, rien n'est touche et
/// `Ok(None)` est renvoie, comme le `return` de la source. Un SDK deja
/// present serait remplace, comme le fait une affectation en JavaScript.
pub fn on_sdk_event<I, F>(
    event: &mut SdkHookEvent,
    env: &mut Environnement,
    installer: I,
    fabrique: F,
) -> Result<Option<Value>, Erreur>
where
    I: FnOnce(&str) -> Option<String>,
    F: FnOnce(&str, &ParametresFabrique) -> Value,
{
    let Some(provider_id) = extraire_provider_id(&event.model) else {
        return Ok(None);
    };
    if !applies_to(provider_id) {
        return Ok(None);
    }

    // `serviceKey && !process.env.AICORE_SERVICE_KEY` : la cle resolue est
    // ecrite dans l'environnement seulement si elle existe et qu'il n'en a
    // pas deja une. Une chaine vide compte comme absente (`&&` sur une
    // chaine vide est falsy en JavaScript).
    let cle = resoudre_service_key(&event.options, env);
    // `Option<String>` is not `Copy`, so the value is borrowed for both the test
    // and the hand-off rather than moved by the first one.
    let cle_pleine = cle.as_ref().is_some_and(|cle| !cle.is_empty());
    if cle_pleine && env.service_key.is_none() {
        env.service_key = cle;
    }

    let entree = if event.package.starts_with("file://") {
        None
    } else {
        installer(&event.package)
    };
    let chemin = resoudre_chemin_paquet(&event.package, entree.as_deref())
        .ok_or_else(|| Erreur::PointEntreeManquant(event.package.clone()))?;

    // Une cle (resolue) impose les parametres d'environnement ; sinon `{}`.
    let parametres = if cle_pleine {
        ParametresFabrique {
            deployment_id: env.deployment_id.clone(),
            resource_group: env.resource_group.clone(),
        }
    } else {
        ParametresFabrique {
            deployment_id: None,
            resource_group: None,
        }
    };

    let sdk = fabrique(&chemin, &parametres);
    event.sdk = Some(sdk.clone());
    Ok(Some(sdk))
}

/// Le corps du crochet enregistre par le plugin sur `ctx.aisdk.language`.
///
/// La source ecrit `evt.language = evt.sdk(evt.model.api.id)` : une ligne.
/// `fabrique` tient lieu du SDK appele sur l'identifiant du modele. Renvoie
/// `Ok(None)` si le fournisseur ne correspond pas, et une erreur si le
/// modele n'a pas d'`api.id` chaine, cas que la source ne filtre pas
/// (`evt.sdk(undefined)` en JavaScript).
pub fn on_language_event<F>(
    event: &mut LanguageHookEvent,
    fabrique: F,
) -> Result<Option<Value>, Erreur>
where
    F: FnOnce(&str) -> Value,
{
    let Some(provider_id) = extraire_provider_id(&event.model) else {
        return Ok(None);
    };
    if !applies_to(provider_id) {
        return Ok(None);
    }

    let api_id = extraire_api_id(&event.model)
        .ok_or_else(|| Erreur::FabriqueManquante(event.model.to_string()))?;
    let language = fabrique(api_id);
    event.language = Some(language.clone());
    Ok(Some(language))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un evenement minimal, avec le fournisseur demande.
    fn evenement() -> SdkHookEvent {
        SdkHookEvent {
            model: serde_json::json!({ "providerID": "sap-ai-core" }),
            package: "@ai-sdk/sap-ai-core".to_string(),
            options: serde_json::json!({}),
            sdk: None,
        }
    }

    /// Un evenement de langage minimal.
    fn evenement_language() -> LanguageHookEvent {
        LanguageHookEvent {
            model: serde_json::json!({
                "providerID": "sap-ai-core",
                "api": { "id": "gpt-4o" }
            }),
            language: None,
        }
    }

    #[test]
    fn le_plugin_s_enregistre_sous_le_nom_sap_ai_core() {
        assert_eq!(SapAiCorePlugin::new().id, "sap-ai-core");
        assert_eq!(SapAiCorePlugin::default().id, "sap-ai-core");
        assert_eq!(
            serde_json::to_string(&SapAiCorePlugin::new()).unwrap(),
            r#"{"id":"sap-ai-core"}"#
        );
    }

    #[test]
    fn le_seul_fournisseur_reconnu_est_sap_ai_core() {
        assert!(applies_to("sap-ai-core"));
        assert!(applies_to(PROVIDER_ID));
        assert!(!applies_to("sap-ai"));
        assert!(!applies_to("SAP-AI-CORE"));
    }

    #[test]
    fn un_fournisseur_inconnu_ne_declenche_rien() {
        let mut event = evenement();
        event.model = serde_json::json!({ "providerID": "openai" });
        let mut env = Environnement::default();
        let mut installe = false;
        let resultat = on_sdk_event(
            &mut event,
            &mut env,
            |_paquet| {
                installe = true;
                Some("/mod".to_string())
            },
            |_chemin, _parametres| serde_json::json!("sdk"),
        );
        assert_eq!(resultat, Ok(None));
        assert!(!installe);
        assert_eq!(event.sdk, None);
        assert_eq!(env, Environnement::default());
    }

    #[test]
    fn la_cle_de_service_vient_des_options_si_l_environnement_est_vide() {
        let mut event = evenement();
        event.options = serde_json::json!({ "serviceKey": "depuis-les-options" });
        let mut env = Environnement::default();
        on_sdk_event(&mut event, &mut env, |_paquet| Some("/mod".to_string()), |chemin, _parametres| {
            serde_json::json!(chemin)
        })
        .unwrap();
        assert_eq!(env.service_key.as_deref(), Some("depuis-les-options"));
    }

    #[test]
    fn l_environnement_gagne_sur_l_option() {
        let mut event = evenement();
        event.options = serde_json::json!({ "serviceKey": "depuis-les-options" });
        let mut env = Environnement {
            service_key: Some("depuis-l-environnement".to_string()),
            ..Environnement::default()
        };
        on_sdk_event(&mut event, &mut env, |_paquet| Some("/mod".to_string()), |_chemin, _parametres| {
            serde_json::json!("sdk")
        })
        .unwrap();
        assert_eq!(env.service_key.as_deref(), Some("depuis-l-environnement"));
    }

    #[test]
    fn une_cle_vide_dans_l_environnement_gagne() {
        // Un `??` ne remplace que null/undefined : une chaine vide gagne.
        // Mais elle est falsy pour le write-back : elle n'est pas re-ecrite
        // et la fabrique recoit `{}`.
        let mut event = evenement();
        event.options = serde_json::json!({ "serviceKey": "depuis-les-options" });
        let mut env = Environnement {
            service_key: Some(String::new()),
            ..Environnement::default()
        };
        let mut parametres_recus: Option<Value> = None;
        on_sdk_event(&mut event, &mut env, |_paquet| Some("/mod".to_string()), |_chemin, parametres| {
            parametres_recus = Some(serde_json::to_value(parametres).unwrap());
            serde_json::json!("sdk")
        })
        .unwrap();
        assert_eq!(parametres_recus, Some(serde_json::json!({})));
    }

    #[test]
    fn une_cle_non_chaine_dans_les_options_est_ignoree() {
        let mut event = evenement();
        event.options = serde_json::json!({ "serviceKey": 42 });
        let mut env = Environnement::default();
        on_sdk_event(&mut event, &mut env, |_paquet| Some("/mod".to_string()), |_chemin, _parametres| {
            serde_json::json!("sdk")
        })
        .unwrap();
        assert_eq!(env.service_key, None);
    }

    #[test]
    fn une_cle_des_options_est_ecrite_seulement_si_absente_de_l_environnement() {
        // Imitation : l'environnement a deja une cle, posee avant le plugin.
        let mut event = evenement();
        event.options = serde_json::json!({ "serviceKey": "depuis-les-options" });
        let mut env = Environnement {
            service_key: Some("deja-define".to_string()),
            ..Environnement::default()
        };
        on_sdk_event(&mut event, &mut env, |_paquet| Some("/mod".to_string()), |_chemin, _parametres| {
            serde_json::json!("sdk")
        })
        .unwrap();
        assert_eq!(env.service_key.as_deref(), Some("deja-define"));
    }

    #[test]
    fn une_url_file_est_utilisee_telle_quelle_sans_appeler_npm() {
        let mut event = evenement();
        event.package = "file:///chemin/local".to_string();
        let mut appele = false;
        on_sdk_event(&mut event, &mut Environnement::default(), |_paquet| {
            appele = true;
            None
        }, |chemin, _parametres| serde_json::json!(chemin))
        .unwrap();
        assert!(!appele);
        assert_eq!(event.sdk, Some(serde_json::json!("file:///chemin/local")));
    }

    #[test]
    fn un_paquet_sans_point_d_entree_est_une_erreur() {
        let mut event = evenement();
        let resultat = on_sdk_event(
            &mut event,
            &mut Environnement::default(),
            |_paquet| None,
            |_chemin, _parametres| serde_json::json!("sdk"),
        );
        assert_eq!(
            resultat,
            Err(Erreur::PointEntreeManquant("@ai-sdk/sap-ai-core".to_string()))
        );
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn la_fabrique_recoit_les_parametres_d_environnement_avec_une_cle() {
        let mut event = evenement();
        event.options = serde_json::json!({ "serviceKey": "cle" });
        let mut env = Environnement {
            deployment_id: Some("dep-1".to_string()),
            resource_group: Some("rg-1".to_string()),
            ..Environnement::default()
        };
        let mut parametres_recus: Option<ParametresFabrique> = None;
        on_sdk_event(&mut event, &mut env, |_paquet| Some("/mod".to_string()), |_chemin, parametres| {
            parametres_recus = Some(parametres.clone());
            serde_json::json!("sdk")
        })
        .unwrap();
        assert_eq!(
            parametres_recus,
            Some(ParametresFabrique {
                deployment_id: Some("dep-1".to_string()),
                resource_group: Some("rg-1".to_string()),
            })
        );
    }

    #[test]
    fn les_parametres_sans_environnement_serialisent_comme_un_objet_vide() {
        let parametres = ParametresFabrique {
            deployment_id: None,
            resource_group: None,
        };
        assert_eq!(serde_json::to_string(&parametres).unwrap(), "{}");
    }

    #[test]
    fn les_noms_de_champs_des_parametres_sont_camel_case() {
        let parametres = ParametresFabrique {
            deployment_id: Some("dep-1".to_string()),
            resource_group: Some("rg-1".to_string()),
        };
        assert_eq!(
            serde_json::to_string(&parametres).unwrap(),
            r#"{"deploymentId":"dep-1","resourceGroup":"rg-1"}"#
        );
        let relus: ParametresFabrique =
            serde_json::from_str(r#"{"deploymentId":"dep-1","resourceGroup":"rg-1"}"#).unwrap();
        assert_eq!(relus, parametres);
    }

    #[test]
    fn les_noms_de_champs_des_options_sont_camel_case_en_aller_retour() {
        // Le contrat TS est `serviceKey`, jamais `service_key`.
        let json = r#"{"serviceKey":"cle"}"#;
        let options: Value = serde_json::from_str(json).unwrap();
        assert_eq!(extraire_service_key(&options), Some("cle"));
        assert_eq!(
            serde_json::to_string(&options).unwrap(),
            r#"{"serviceKey":"cle"}"#
        );
    }

    #[test]
    fn la_recherche_d_export_trouve_la_premiere_fabrique_create() {
        assert_eq!(trouver_export(&["createSapAiCore", "autre"]), Some("createSapAiCore"));
        assert_eq!(trouver_export(&["autre", "createMistral"]), Some("createMistral"));
        assert_eq!(trouver_export(&["CreateSapAiCore"]), None);
        assert_eq!(trouver_export(&[]), None);
    }

    #[test]
    fn un_sdk_deja_present_est_remplace() {
        let mut event = evenement();
        event.sdk = Some(serde_json::json!("ancien"));
        on_sdk_event(&mut event, &mut Environnement::default(), |_paquet| Some("/mod".to_string()), |_chemin, _parametres| {
            serde_json::json!("nouveau")
        })
        .unwrap();
        assert_eq!(event.sdk, Some(serde_json::json!("nouveau")));
    }

    #[test]
    fn le_crochet_language_pose_le_modele_du_champ_api() {
        let mut event = evenement_language();
        let resultat =
            on_language_event(&mut event, |api_id| serde_json::json!({ "modelId": api_id }))
                .unwrap();
        assert_eq!(resultat, Some(serde_json::json!({ "modelId": "gpt-4o" })));
        assert_eq!(event.language, Some(serde_json::json!({ "modelId": "gpt-4o" })));
    }

    #[test]
    fn le_crochet_language_ignorer_les_autres_fournisseurs() {
        let mut event = evenement_language();
        event.model = serde_json::json!({
            "providerID": "openai",
            "api": { "id": "gpt-4o" }
        });
        let resultat =
            on_language_event(&mut event, |_api_id| serde_json::json!("appele")).unwrap();
        assert_eq!(resultat, None);
        assert_eq!(event.language, None);
    }

    #[test]
    fn un_evenement_sdk_se_relit_depuis_le_json_du_typescript() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":{"providerID":"sap-ai-core"},"package":"@ai-sdk/sap-ai-core","options":{"serviceKey":"cle"},"sdk":"marqueur"}"#,
        )
        .unwrap();
        assert_eq!(extraire_provider_id(&event.model), Some("sap-ai-core"));
        assert_eq!(event.package, "@ai-sdk/sap-ai-core");
        assert_eq!(extraire_service_key(&event.options), Some("cle"));
        assert_eq!(event.sdk, Some(serde_json::json!("marqueur")));
    }

    #[test]
    fn un_evenement_sdk_sans_sdk_se_relit_avec_un_sdk_absent() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":null,"package":"@ai-sdk/sap-ai-core","options":{}}"#,
        )
        .unwrap();
        assert_eq!(event.sdk, None);
        assert_eq!(extraire_provider_id(&event.model), None);
    }

    #[test]
    fn un_evenement_language_se_relit_depuis_le_json_du_typescript() {
        let event: LanguageHookEvent = serde_json::from_str(
            r#"{"model":{"providerID":"sap-ai-core","api":{"id":"gpt-4o"}},"language":{"ok":true}}"#,
        )
        .unwrap();
        assert_eq!(extraire_api_id(&event.model), Some("gpt-4o"));
        assert_eq!(event.language, Some(serde_json::json!({ "ok": true })));
    }

    #[test]
    fn les_noms_de_champs_de_l_evenement_language_sont_exacts() {
        let mut event = evenement_language();
        event.language = Some(serde_json::json!("langage"));
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"model":{"api":{"id":"gpt-4o"},"providerID":"sap-ai-core"},"language":"langage"}"#
        );
        // Sans language : la cle est absente, comme un champ non pose en JS.
        let event = evenement_language();
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"model":{"api":{"id":"gpt-4o"},"providerID":"sap-ai-core"}}"#
        );
    }
}
