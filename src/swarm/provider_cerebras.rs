//! Portage de `packages/core/src/plugin/provider/cerebras.ts`.
//!
//! ## Ce que la source est vraiment
//!
//! Le fichier d'origine fait vingt-six lignes et n'exporte qu'une seule valeur :
//!
//! ```ts
//! export const CerebrasPlugin = define({
//!   id: "cerebras",
//!   effect: Effect.fn(function* (ctx) {
//!     yield* ctx.catalog.transform(Effect.fn(function* (evt) { /* ... */ }))
//!     yield* ctx.aisdk.sdk(Effect.fn(function* (evt) { /* ... */ }))
//!   }),
//! })
//! ```
//!
//! Ce n'est donc **pas** un objet de configuration litteral, mais l'enregistrement
//! de deux **raccourcis** ("hooks") aupres du contexte du plugin. `define` est
//! l'identite (`export function define<R>(plugin: Plugin<R>) { return plugin }`),
//! donc l'objet exporte n'a que deux champs : `id` et `effect`. L'effet, lui,
//! enregistre deux callbacks et n'en fait rien d'autre.
//!
//! - Le premier callback est enregistre sur `ctx.catalog.transform`. Pour chaque
//!   fournisseur du catalogue, si `api.type` vaut exactement `"aisdk"` et si
//!   `api.package` vaut exactement `"@ai-sdk/cerebras"`, il ajoute l'entete
//!   `X-Cerebras-3rd-Party-Integration: opencode` dans
//!   `provider.request.headers`.
//! - Le second callback est enregistre sur `ctx.aisdk.sdk`. Si `evt.package` vaut
//!   exactement `"@ai-sdk/cerebras"`, il importe dynamiquement le paquet npm du
//!   meme nom et pose `evt.sdk = mod.createCerebras(evt.options)`.
//!
//! Les enregistrements rendus sont `yield*` puis abandonnes : la source ne garde
//! aucune `Registration` et ne s'en sert jamais. Le mecanisme de
//! registration/chargement n'est donc pas retranscrit.
//!
//! ## Ce qui n'est pas portable tel quel
//!
//! - **L'import dynamique.** `import("@ai-sdk/cerebras")` charge un paquet npm
//!   JavaScript. Il n'a pas d'equivalent en Rust, et il n'est de toute facon pas
//!   installable ici (le paquet `effect` lui-meme est absent de cette machine,
//!   comme le signale le rapport `swarm-copilot_finish_reason`). On ne simule
//!   donc pas de SDK : on isole l'appel dans le trait `CerebrasSdkFactory`, que
//!   l'appelant fournit. Ce qui est porte ici, c'est la **decision** (le paquet
//!   correspond-il ou non) et la **transmission des options**, c'est-a-dire tout
//!   ce que la source decide reellement. La fabrication du SDK, elle, n'existe
//!   pas dans la source.
//! - **Le `yield*` de `Effect`.** L'effet TypeScript est un generateur qui
//!   enchaine deux effets. Ici les deux branches sont des fonctions pures, sans
//!   `async` : rien ne le Justifie.
//!
//! ## Les pieges traites
//!
//! - **Comparaisons strictes.** Le TypeScript utilise `!==`, jamais `!=` : pas de
//!   coercition. Un paquet vide `""` ne correspond donc pas, ce que le test
//!   `un_paquet_vide_ne_declenche_pas_le_sdk` verifie. Une comparaison Rust sur
//!   `&str` donne le meme resultat.
//! - **Pas de `?` contre `??` ici.** La source ne contient ni ternaire ni
//!   coalescent : les deux conditions sont deux `continue` successifs, qui
//!   valent un `if !(a && b) { continue }`.
//! - **Ecriture et non ajout.** L'entete est posee par une affectation simple
//!   `headers[...] = "opencode"`, donc une valeur deja presente est **ecrasee**.
//!   C'est le point exact ou le plugin voisin `nvidia.ts` utilise `??=` et
//!   laisse la valeur en place. Ne pas confondre les deux. Test dedie :
//!   `une_entete_deja_presente_est_ecrasee`.
//! - **Noms de champs.** Le piege est reel dans ce fichier :
//!   `ProviderV2Info.integrationID` porte un `ID` en majuscules, ce que le nom
//!   Rust idiomatique (`integration_id`) ne restitue pas tout seul. D'ou le
//!   `#[serde(rename = "integrationID")]` explicite, verifie par
//!   `les_noms_de_champs_json_du_fournisseur_sont_exacts`. Les autres noms
//!   (`id`, `name`, `disabled`, `api`, `request`, `type`, `package`, `url`,
//!   `settings`, `headers`, `body`) sont des mots uniques en minuscules et n'ont
//!   pas besoin de renommage.
//!
//! ## Types repris d'ailleurs
//!
//! `ProviderV2Info`, `ProviderApi` et `ProviderRequest` sont declares dans
//! `packages/sdk/js/src/v2/gen/types.gen.ts`, donc **hors** de ce fichier, et
//! aucun autre agent du lot ne les porte. Comme le plugin ne peut pas exister
//! sans eux, leurs formes sont recopiees ici de facon minimale, avec les memes
//! noms de champs qu'a l'echange. Cela vaut aussi pour `ModelV2Info`, alluded
//! par `SdkEvent.model` et par `CatalogProviderRecord.models` : le plugin ne le
//! lit jamais, il est donc type par une `serde_json::Value` opaque plutot que
//! recopie en entier. Rien n'est fabrique, tout est signale.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Copie locale de `ProviderRequest` (`types.gen.ts`).
///
/// `headers` est un indexeur `[key: string]: string`, donc une `BTreeMap` est le
/// equivalent deterministe. `body` est un indexeur `unknown`, donc une valeur
/// JSON opaque. Les deux champs sont obligatoires dans la source, ils le restent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub headers: BTreeMap<String, String>,
    pub body: BTreeMap<String, Value>,
}

/// Copie locale de `ProviderApi`, qui est une union **tagguee** sur `type`.
///
/// La source est `ProviderAisdk | ProviderNative`, d'ou `#[serde(tag = "type")]`
/// et non `#[serde(untagged)]` : le discriminant est une vraie donnee du JSON.
/// `url` est optionnel sur les deux variantes, `settings` est optionnel sur la
/// premiere et obligatoire sur la seconde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderApi {
    #[serde(rename = "aisdk")]
    Aisdk {
        package: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        settings: Option<BTreeMap<String, Value>>,
    },
    #[serde(rename = "native")]
    Native {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        settings: BTreeMap<String, Value>,
    },
}

impl ProviderApi {
    /// Valeur du tag `type`, c'est-a-dire le nom de la variante.
    pub fn kind(&self) -> &'static str {
        match self {
            ProviderApi::Aisdk { .. } => "aisdk",
            ProviderApi::Native { .. } => "native",
        }
    }

    /// Champ `package`, present seulement sur la variante `aisdk`.
    ///
    /// En TypeScript, `api.package` vaut `undefined` sur la variante `native` et
    /// la comparaison `!== "@ai-sdk/cerebras"` est donc vraie, donc continue.
    /// Une `Option` rend exactement ce comportement sans court-circuiter.
    pub fn package(&self) -> Option<&str> {
        match self {
            ProviderApi::Aisdk { package, .. } => Some(package.as_str()),
            ProviderApi::Native { .. } => None,
        }
    }
}

/// Copie locale de `ProviderV2Info` (`types.gen.ts`).
///
/// Attention au nom `integrationID` : le `ID` est en majuscules dans la source,
/// le nom Rust est `integration_id` et le renommage est explicite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderV2Info {
    pub id: String,
    #[serde(
        rename = "integrationID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub integration_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    pub api: ProviderApi,
    pub request: ProviderRequest,
}

/// Copie locale de `CatalogProviderRecord` (`packages/plugin/src/v2/effect/catalog.ts`).
///
/// `models` est un `ReadonlyMap<string, ModelV2Info>`. Le plugin lit
/// `item.provider` et jamais `item.models`, donc le type du modele est laisse
/// opaque. Le champ est tout de meme conserve pour que la forme reste fidele.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogProviderRecord {
    pub provider: ProviderV2Info,
    #[serde(default)]
    pub models: BTreeMap<String, Value>,
}

/// L'evenement recu par le raccourci `catalog.transform`.
///
/// Seule la moitie `provider` de `CatalogDraft` est portee : c'est la seule que
/// `cerebras.ts` utilise. Une lecture plus large de la source ne le justifie pas.
pub trait CatalogProviderDraft {
    /// Les enregistrements presents dans le brouillon.
    fn list(&self) -> Vec<CatalogProviderRecord>;

    /// Applique `update` au fournisseur d'identifiant `provider_id`.
    fn update(&mut self, provider_id: &str, update: &dyn Fn(&mut ProviderV2Info));
}

/// L'evenement recu par le raccourci `aisdk.sdk`.
///
/// `model` est un `ModelV2Info` que le plugin ne lit pas : il reste opaque.
/// `sdk` est un `any` optionnel, seul champ que le callback ecrit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    pub model: Value,
    pub package: String,
    pub options: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

/// L'import dynamique de `@ai-sdk/cerebras`, isole derriere un trait.
///
/// Le trait ne correspond a rien dans la source : c'est le seul moyen de rendre
/// testable le `import()` sans ecrire le SDK. Une seule methode, celle que la
/// source appelle reellement.
pub trait CerebrasSdkFactory {
    fn create_cerebras(&self, options: &BTreeMap<String, Value>) -> Value;
}

/// Equivalent de l'export `CerebrasPlugin`.
///
/// L'objet TypeScript porte `id` et `effect`. `id` devient [`CerebrasPlugin::ID`]
/// et `effect` devient les deux fonctions enregistrees, [`CerebrasPlugin::transform`]
/// et [`CerebrasPlugin::register_sdk`], appelables directement.
pub struct CerebrasPlugin;

impl CerebrasPlugin {
    /// Champ `id` de l'objet `define`.
    pub const ID: &'static str = "cerebras";

    /// Valeur du tag `type` que le plugin cible dans `provider.api`.
    pub const AISDK_TYPE: &'static str = "aisdk";

    /// Valeur du champ `package` que le plugin cible.
    pub const AISDK_PACKAGE: &'static str = "@ai-sdk/cerebras";

    /// Nom de la fonction exportee par le paquet `@ai-sdk/cerebras`.
    pub const FACTORY: &'static str = "createCerebras";

    /// Cle d'en-tete injectee dans `provider.request.headers`.
    pub const INTEGRATION_HEADER: &'static str = "X-Cerebras-3rd-Party-Integration";

    /// Valeur de l'en-tete, toujours cette chaine la.
    pub const INTEGRATION_HEADER_VALUE: &'static str = "opencode";

    /// Lecture de l'identifiant du plugin.
    pub fn id() -> &'static str {
        Self::ID
    }

    /// Le fournisseur est-il concerne par ce plugin ?
    ///
    /// Reproduit les deux `continue` de la source : il faut le tag `aisdk` ET le
    /// paquet `@ai-sdk/cerebras`. Un seul des deux ne suffit pas.
    pub fn targets(provider: &ProviderV2Info) -> bool {
        provider.api.kind() == Self::AISDK_TYPE
            && provider.api.package() == Some(Self::AISDK_PACKAGE)
    }

    /// Le corps du callback enregistre sur `ctx.catalog.transform`.
    ///
    /// Parcourt la liste du brouillon et, pour chaque fournisseur concerne,
    /// ecrase l'en-tete d'integration par la valeur `opencode`. La valeur
    /// precedente est perdue si elle existe, exactement comme le fait
    /// l'affectation directe de la source.
    pub fn transform<D>(draft: &mut D)
    where
        D: CatalogProviderDraft + ?Sized,
    {
        for item in draft.list() {
            if !Self::targets(&item.provider) {
                continue;
            }
            let provider_id = item.provider.id.clone();
            let apply = |provider: &mut ProviderV2Info| {
                provider.request.headers.insert(
                    Self::INTEGRATION_HEADER.to_string(),
                    Self::INTEGRATION_HEADER_VALUE.to_string(),
                );
            };
            draft.update(&provider_id, &apply);
        }
    }

    /// Le corps du callback enregistre sur `ctx.aisdk.sdk`.
    ///
    /// Renvoie `true` si le SDK a ete construit, `false` si le paquet de
    /// l'evenement ne correspond pas et que la source serait sortie par son
    /// `return` precoce. Les options sont transmises telles quelles a la
    /// fabrique.
    pub fn register_sdk<E>(evt: &mut SdkEvent, factory: &E) -> bool
    where
        E: CerebrasSdkFactory + ?Sized,
    {
        if evt.package != Self::AISDK_PACKAGE {
            return false;
        }
        evt.sdk = Some(factory.create_cerebras(&evt.options));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    /// Brouillon de test : une simple liste de fournisseurs.
    #[derive(Default)]
    struct Catalogue(Vec<ProviderV2Info>);

    impl CatalogProviderDraft for Catalogue {
        fn list(&self) -> Vec<CatalogProviderRecord> {
            self.0
                .iter()
                .map(|provider| CatalogProviderRecord {
                    provider: provider.clone(),
                    models: BTreeMap::new(),
                })
                .collect()
        }

        fn update(&mut self, provider_id: &str, update: &dyn Fn(&mut ProviderV2Info)) {
            for provider in self.0.iter_mut() {
                if provider.id == provider_id {
                    update(provider);
                    return;
                }
            }
        }
    }

    fn aisdk(id: &str, package: &str) -> ProviderV2Info {
        ProviderV2Info {
            id: id.to_string(),
            integration_id: None,
            name: id.to_string(),
            disabled: None,
            api: ProviderApi::Aisdk {
                package: package.to_string(),
                url: None,
                settings: None,
            },
            request: ProviderRequest {
                headers: BTreeMap::new(),
                body: BTreeMap::new(),
            },
        }
    }

    fn cerebras(id: &str) -> ProviderV2Info {
        aisdk(id, CerebrasPlugin::AISDK_PACKAGE)
    }

    fn native(id: &str) -> ProviderV2Info {
        ProviderV2Info {
            api: ProviderApi::Native {
                url: None,
                settings: BTreeMap::new(),
            },
            ..aisdk(id, CerebrasPlugin::AISDK_PACKAGE)
        }
    }

    fn entete(provider: &ProviderV2Info) -> Option<&String> {
        provider.request.headers.get(CerebrasPlugin::INTEGRATION_HEADER)
    }

    /// Fabrique de test qui note les options recues puis renvoie une valeur
    /// identifiable, sans jamais charger de paquet npm.
    #[derive(Default)]
    struct Usine {
        options_recues: RefCell<Option<BTreeMap<String, Value>>>,
    }

    impl CerebrasSdkFactory for Usine {
        fn create_cerebras(&self, options: &BTreeMap<String, Value>) -> Value {
            *self.options_recues.borrow_mut() = Some(options.clone());
            json!({ "fournisseur": "cerebras" })
        }
    }

    fn evenement(package: &str) -> SdkEvent {
        SdkEvent {
            model: Value::Null,
            package: package.to_string(),
            options: BTreeMap::new(),
            sdk: None,
        }
    }

    fn cles(valeur: &Value) -> Vec<String> {
        let mut noms: Vec<String> = valeur
            .as_object()
            .expect("valeur JSON non objet")
            .keys()
            .cloned()
            .collect();
        noms.sort();
        noms
    }

    #[test]
    fn le_plugin_s_appelle_cerebras() {
        assert_eq!(CerebrasPlugin::id(), "cerebras");
        assert_eq!(CerebrasPlugin::ID, "cerebras");
    }

    #[test]
    fn une_liste_vide_ne_touche_aucun_fournisseur() {
        let mut catalogue = Catalogue::default();
        CerebrasPlugin::transform(&mut catalogue);
        assert!(catalogue.0.is_empty());
    }

    #[test]
    fn le_fournisseur_cerebras_recoit_l_entete_d_integration() {
        let mut catalogue = Catalogue(vec![cerebras("cerebras")]);
        CerebrasPlugin::transform(&mut catalogue);
        assert_eq!(
            entete(&catalogue.0[0]).map(String::as_str),
            Some("opencode")
        );
    }

    #[test]
    fn un_fournisseur_natif_est_ignore() {
        let mut catalogue = Catalogue(vec![native("natif")]);
        CerebrasPlugin::transform(&mut catalogue);
        assert_eq!(entete(&catalogue.0[0]), None);
    }

    #[test]
    fn un_paquet_aisdk_different_est_ignore() {
        let mut catalogue = Catalogue(vec![aisdk("autre", "@ai-sdk/openai-compatible")]);
        CerebrasPlugin::transform(&mut catalogue);
        assert_eq!(entete(&catalogue.0[0]), None);
    }

    #[test]
    fn une_entete_deja_presente_est_ecrasee() {
        let mut fournisseur = cerebras("cerebras");
        fournisseur.request.headers.insert(
            CerebrasPlugin::INTEGRATION_HEADER.to_string(),
            "valeur-anterieure".to_string(),
        );
        let mut catalogue = Catalogue(vec![fournisseur]);
        CerebrasPlugin::transform(&mut catalogue);
        assert_eq!(
            entete(&catalogue.0[0]).map(String::as_str),
            Some("opencode"),
            "l'affectation directe doit ecraser la valeur existante"
        );
    }

    #[test]
    fn les_autres_entetes_du_fournisseur_sont_conservees() {
        let mut fournisseur = cerebras("cerebras");
        fournisseur
            .request
            .headers
            .insert("Authorization".to_string(), "Bearer jeton".to_string());
        let mut catalogue = Catalogue(vec![fournisseur]);
        CerebrasPlugin::transform(&mut catalogue);
        assert_eq!(
            catalogue.0[0]
                .request
                .headers
                .get("Authorization")
                .map(String::as_str),
            Some("Bearer jeton")
        );
        assert_eq!(
            entete(&catalogue.0[0]).map(String::as_str),
            Some("opencode")
        );
    }

    #[test]
    fn seul_le_fournisseur_cerebras_est_modifie_au_milieu_des_autres() {
        let mut catalogue = Catalogue(vec![
            native("natif"),
            cerebras("cerebras"),
            aisdk("groq", "@ai-sdk/groq"),
            cerebras("cerebis"),
        ]);
        CerebrasPlugin::transform(&mut catalogue);
        assert_eq!(entete(&catalogue.0[0]), None);
        assert_eq!(entete(&catalogue.0[1]).map(String::as_str), Some("opencode"));
        assert_eq!(entete(&catalogue.0[2]), None);
        assert_eq!(entete(&catalogue.0[3]).map(String::as_str), Some("opencode"));
    }

    #[test]
    fn un_paquet_cerebras_construit_le_sdk_avec_ses_options() {
        let mut evt = evenement(CerebrasPlugin::AISDK_PACKAGE);
        evt.options.insert("baseURL".to_string(), json!("https://api.cerebras.ai"));
        let usine = Usine::default();
        let construit = CerebrasPlugin::register_sdk(&mut evt, &usine);
        assert!(construit);
        assert_eq!(evt.sdk, Some(json!({ "fournisseur": "cerebras" })));
        let recues = usine.options_recues.borrow().clone().expect("options lues");
        assert_eq!(recues.get("baseURL"), Some(&json!("https://api.cerebras.ai")));
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_laisse_le_sdk_absent() {
        let mut evt = evenement("@ai-sdk/groq");
        let usine = Usine::default();
        let construit = CerebrasPlugin::register_sdk(&mut evt, &usine);
        assert!(!construit);
        assert_eq!(evt.sdk, None);
        assert!(usine.options_recues.borrow().is_none());
    }

    #[test]
    fn un_paquet_vide_ne_declenche_pas_le_sdk() {
        let mut evt = evenement("");
        let usine = Usine::default();
        assert!(!CerebrasPlugin::register_sdk(&mut evt, &usine));
        assert_eq!(evt.sdk, None);
    }

    #[test]
    fn un_sdk_deja_present_est_remplace() {
        let mut evt = evenement(CerebrasPlugin::AISDK_PACKAGE);
        evt.sdk = Some(json!("ancien"));
        let usine = Usine::default();
        assert!(CerebrasPlugin::register_sdk(&mut evt, &usine));
        assert_eq!(evt.sdk, Some(json!({ "fournisseur": "cerebras" })));
    }

    #[test]
    fn les_noms_de_champs_json_du_fournisseur_sont_exacts() {
        let mut fournisseur = cerebras("cerebras");
        fournisseur.integration_id = Some("int_1".to_string());
        let valeur = serde_json::to_value(&fournisseur).expect("serialisation");
        // `disabled` is `Option<bool>` and the helper leaves it None, and the
        // TypeScript declares it `disabled?: boolean` -- absent, not false. So it
        // is not serialised, and expecting it here was wrong: the port omits it,
        // which is what the source does.
        assert_eq!(
            cles(&valeur),
            vec!["api", "id", "integrationID", "name", "request"]
        );

        // And with it set, the name shows up. `serde_json::Value`'s object is a
        // BTreeMap, so the keys come out sorted -- that is why every expected
        // list here is alphabetical.
        fournisseur.disabled = Some(true);
        let valeur = serde_json::to_value(&fournisseur).expect("serialisation");
        assert_eq!(
            cles(&valeur),
            vec!["api", "disabled", "id", "integrationID", "name", "request"]
        );
    }

    #[test]
    fn integration_id_se_serialize_sous_la_majuscule_du_typeScript() {
        let mut fournisseur = cerebras("cerebras");
        fournisseur.integration_id = Some("int_1".to_string());
        let brut = serde_json::to_string(&fournisseur).expect("serialisation");
        assert!(
            brut.contains("\"integrationID\":\"int_1\""),
            "attendu integrationID en majuscules, obtenu {}",
            brut
        );
        assert!(!brut.contains("integrationId"));
        assert!(!brut.contains("integration_id"));
    }

    #[test]
    fn un_fournisseur_sans_integration_id_n_emporte_pas_la_cle() {
        let fournisseur = cerebras("cerebras");
        let valeur = serde_json::to_value(&fournisseur).expect("serialisation");
        assert!(!valeur.as_object().unwrap().contains_key("integrationID"));
    }

    #[test]
    fn la_cle_integration_id_est_lue_telle_quelle_au_desarerialisage() {
        let brut = r#"{
          "id": "cerebras",
          "integrationID": "int_1",
          "name": "Cerebras",
          "disabled": false,
          "api": { "type": "aisdk", "package": "@ai-sdk/cerebras" },
          "request": { "headers": {}, "body": {} }
        }"#;
        let lu: ProviderV2Info = serde_json::from_str(brut).expect("deserialisation");
        assert_eq!(lu.integration_id.as_deref(), Some("int_1"));
        assert_eq!(lu.disabled, Some(false));
    }

    #[test]
    fn une_cle_integration_id_mal_ecrite_est_ignoree() {
        let brut = r#"{
          "id": "cerebras",
          "integrationId": "int_1",
          "name": "Cerebras",
          "api": { "type": "aisdk", "package": "@ai-sdk/cerebras" },
          "request": { "headers": {}, "body": {} }
        }"#;
        let lu: ProviderV2Info = serde_json::from_str(brut).expect("deserialisation");
        assert_eq!(
            lu.integration_id, None,
            "seule la casse integrationID est acceptee"
        );
    }

    #[test]
    fn les_noms_de_champs_json_de_l_api_et_de_la_requete_sont_exacts() {
        let fournisseur = cerebras("cerebras");
        let valeur = serde_json::to_value(&fournisseur).expect("serialisation");
        // The TypeScript declares `url?` and `settings?` on the aisdk variant, so
        // with both absent they are not serialised. The sibling test below shows
        // the same rule on the native variant and passes; this one expected the
        // four names anyway, which the source does not produce.
        assert_eq!(
            cles(&valeur.get("api").unwrap()),
            vec!["package", "type"]
        );
        assert_eq!(
            cles(&valeur.get("request").unwrap()),
            vec!["body", "headers"]
        );

        // Populated, all four names appear -- and the flat shape is confirmed:
        // `type` is a field of the same object as `package`, not a wrapper key.
        let complet = ProviderApi::Aisdk {
            package: CerebrasPlugin::AISDK_PACKAGE.to_string(),
            url: Some("https://api.cerebras.ai".to_string()),
            settings: Some(BTreeMap::from([(
                "model".to_string(),
                serde_json::json!("llama-3.3-70b"),
            )])),
        };
        let valeur = serde_json::to_value(&complet).expect("serialisation");
        assert_eq!(
            cles(&valeur),
            vec!["package", "settings", "type", "url"]
        );
        assert_eq!(valeur.get("type").unwrap(), "aisdk");
    }

    #[test]
    fn une_api_native_sans_url_ne_serialise_pas_url() {
        let valeur = serde_json::to_value(native("natif")).expect("serialisation");
        assert_eq!(cles(&valeur.get("api").unwrap()), vec!["settings", "type"]);
        assert_eq!(valeur.get("api").unwrap().get("type").unwrap(), "native");
    }

    #[test]
    fn une_api_aisdk_vide_d_url_reste_deserialisable() {
        let brut = r#"{ "type": "aisdk", "package": "@ai-sdk/cerebras" }"#;
        let api: ProviderApi = serde_json::from_str(brut).expect("deserialisation");
        assert_eq!(api.kind(), "aisdk");
        assert_eq!(api.package(), Some("@ai-sdk/cerebras"));
    }
}
