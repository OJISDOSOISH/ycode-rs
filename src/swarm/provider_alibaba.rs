//! Portage de `plugin/provider/alibaba.ts`.
//!
//! La source fait quinze lignes et ne declare pas un fournisseur complet. Elle
//! enregistre un unique crochet sur le domaine `aisdk.sdk` du contexte de
//! plugin, et ce crochet ne fait quelque chose que pour un seul paquet npm.
//!
//! Ce que dit la source, sans interpretation :
//!
//! - l identifiant du plugin est la chaine `alibaba` ;
//! - l effet du plugin enregistre un seul crochet via `ctx.aisdk.sdk(...)` ;
//! - ce crochet teste `evt.package !== "@ai-sdk/alibaba"` et sort sans rien
//!   faire des que le test est vrai ;
//! - sinon il charge dynamiquement le paquet `@ai-sdk/alibaba` et ecrase
//!   `evt.sdk` avec le resultat de `createAlibaba(evt.options)`.
//!
//! Deux points de fidelite.
//!
//! - Le test de la source est une inegalite **stricte**, pas un test de
//!   veracite. Une chaine vide pour `package` ne declenche donc pas le crochet,
//!   elle ne fait simplement rien. C est l inverse d un ternaire, qui
//!   traiterait `""` comme une absence.
//! - Le seul champ ecrit par le crochet est `sdk`. `model`, `package` et
//!   `options` sont marques `readonly` dans le type de la source et restent
//!   inchanges. En particulier les valeurs d `options` sont transmises telles
//!   quelles a la fabrique, chaine vide comprise, puisque le seul acces se fait
//!   par `evt.options` sans test.
//!
//! L appel `createAlibaba` est une fonction exportee par un paquet npm charge
//! dynamiquement. Rust ne peut pas l appeler a la compilation, ni l emuler sans
//! reimplementer le SDK Alibaba. Le champ `sdk` est donc represente par une
//! poignee `AlibabaSdk` qui **decrit** la liaison demandee (paquet, fabrique,
//! options recues) sans l instancier. Un runtime JavaScript, ou une
//! implementation Rust du SDK, prendra cette poignee et construira l objet.
//! Tout le comportement de la source, lui, est decide ici : c est la selection
//! du paquet, et rien d autre.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// L identifiant du plugin, ecrit dans `id` par la source.
pub const PLUGIN_ID: &str = "alibaba";

/// Le seul paquet npm que ce crochet intercepte.
pub const PACKAGE: &str = "@ai-sdk/alibaba";

/// La fabrique exportee par le paquet, appelee avec `evt.options`.
pub const FACTORY: &str = "createAlibaba";

/// Le plugin exporte par la source sous le nom `AlibabaPlugin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlibabaPlugin {
    pub id: String,
}

impl AlibabaPlugin {
    pub fn new() -> Self {
        Self {
            id: PLUGIN_ID.to_string(),
        }
    }

    /// Ce que l effet du plugin enregistre : le unique crochet `aisdk.sdk`.
    pub fn hook(&self) -> AlibabaSdkHook {
        AlibabaSdkHook::new()
    }
}

impl Default for AlibabaPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Le crochet enregistre par `ctx.aisdk.sdk(...)`.
///
/// En TypeScript c est une fonction anonyme. Ici c est une donnee : le test que
/// la source fait sur `evt.package` ne depend que d une chaine, et la fabrique
/// qu elle appelle ensuite est une constante. Les deux sont donc lisibles sans
/// executer de code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlibabaSdkHook {
    /// Le paquet compare, par egalite stricte.
    pub package: String,
    /// La fabrique a appeler si le test passe.
    pub factory: String,
}

impl AlibabaSdkHook {
    pub fn new() -> Self {
        Self {
            package: PACKAGE.to_string(),
            factory: FACTORY.to_string(),
        }
    }

    /// Le test exact de la source : `evt.package !== "@ai-sdk/alibaba"`.
    ///
    /// Renvoie `true` quand le crochet doit agir. Une chaine vide renvoie
    /// `false`, comme en JavaScript, ou seule l egalite stricte compte.
    pub fn applies_to(&self, event: &AlibabaSdkEvent) -> bool {
        event.package == self.package
    }

    /// La liaison que le crochet produirait, ou `None` s il sort sans rien
    /// faire parce que le paquet ne correspond pas.
    pub fn resolve(&self, event: &AlibabaSdkEvent) -> Option<AlibabaSdk> {
        if !self.applies_to(event) {
            return None;
        }
        Some(AlibabaSdk {
            package: self.package.clone(),
            factory: self.factory.clone(),
            options: event.options.clone(),
        })
    }

    /// L equivalent de l affectation `evt.sdk = mod.createAlibaba(evt.options)`.
    ///
    /// Les autres champs de l evenement ne sont pas lus pour etre reecrits :
    /// ils sont en lecture seule dans le type de la source et restent donc
    /// inchanges.
    pub fn apply(&self, event: &mut AlibabaSdkEvent) {
        if let Some(sdk) = self.resolve(event) {
            event.sdk = Some(sdk);
        }
    }
}

impl Default for AlibabaSdkHook {
    fn default() -> Self {
        Self::new()
    }
}

/// L evenement recu par un crochet `aisdk.sdk`.
///
/// Les quatre champs sont ceux de `sdk` dans `AISDKHooks`, y compris leur
/// orthographe. Aucun n est en camelCase, donc aucun renommage n est necessaire
/// ; le test de serialisation verrouille malgre tout les quatre noms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlibabaSdkEvent {
    /// `readonly model: ModelV2Info`. Le crochet ne lit jamais ce champ, il est
    /// donc garde opaque plutot que recopie du type du SDK, qui n est pas celui
    /// de ce fichier.
    pub model: serde_json::Value,
    /// `readonly package: string`, le nom du paquet npm demande.
    pub package: String,
    /// `readonly options: Record<string, any>`, transmis tel quel a la fabrique.
    pub options: BTreeMap<String, serde_json::Value>,
    /// `sdk?: any`. Absent tant qu aucun crochet n a produit d instance.
    ///
    /// `default` et `skip_serializing_if` vont de pair : la cle disparait du
    /// JSON quand il n y a pas d instance, et elle se relit comme absente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<AlibabaSdk>,
}

/// La valeur que le crochet ecrase dans `evt.sdk`.
///
/// La source y met l instance renvoyee par `createAlibaba`. Cette instance vient
/// d un paquet npm charge dynamiquement, elle ne peut donc pas etre construite en
/// Rust. On conserve ici la description exacte de la liaison demandee, et c est
/// le seul ecart de la source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlibabaSdk {
    /// Le paquet depuis lequel la fabrique a ete importee.
    pub package: String,
    /// Le nom de la fabrique appelee, `createAlibaba` pour ce plugin.
    pub factory: String,
    /// Les options recues de l evenement, recopiees telles quelles.
    pub options: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un evenement minimal dont seul le nom du paquet varie.
    fn evenement(package: &str) -> AlibabaSdkEvent {
        AlibabaSdkEvent {
            model: serde_json::json!({ "id": "qwen-max" }),
            package: package.to_string(),
            options: BTreeMap::new(),
            sdk: None,
        }
    }

    #[test]
    fn le_plugin_s_identifie_par_la_chaine_alibaba() {
        let plugin = AlibabaPlugin::new();
        assert_eq!(plugin.id, "alibaba");
        assert_eq!(
            serde_json::to_value(&plugin).unwrap(),
            serde_json::json!({ "id": "alibaba" })
        );
        assert_eq!(AlibabaPlugin::default().id, "alibaba");
    }

    #[test]
    fn le_crochet_enregistre_le_paquet_et_sa_fabrique() {
        let crochet = AlibabaPlugin::new().hook();
        assert_eq!(crochet.package, "@ai-sdk/alibaba");
        assert_eq!(crochet.factory, "createAlibaba");
        assert_eq!(AlibabaSdkHook::default().factory, "createAlibaba");
    }

    #[test]
    fn un_evenement_du_paquet_alibaba_declenche_le_crochet() {
        let crochet = AlibabaPlugin::new().hook();
        let mut event = evenement("@ai-sdk/alibaba");
        assert!(crochet.applies_to(&event));
        crochet.apply(&mut event);
        let sdk = event.sdk.expect("le crochet aurait du produire une liaison");
        assert_eq!(sdk.package, "@ai-sdk/alibaba");
        assert_eq!(sdk.factory, "createAlibaba");
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_laisse_l_evenement_intact() {
        let crochet = AlibabaPlugin::new().hook();
        let mut event = evenement("@ai-sdk/openai");
        let avant = event.clone();
        assert!(!crochet.applies_to(&event));
        assert!(crochet.resolve(&event).is_none());
        crochet.apply(&mut event);
        assert_eq!(event, avant);
    }

    #[test]
    fn un_nom_de_paquet_vide_ne_declenche_pas_le_crochet() {
        // Le test de la source est une inegalite stricte, pas un test de
        // veracite : la chaine vide n est pas traitee comme une absence.
        let crochet = AlibabaPlugin::new().hook();
        let mut event = evenement("");
        let avant = event.clone();
        assert!(!crochet.applies_to(&event));
        crochet.apply(&mut event);
        assert_eq!(event, avant);
    }

    #[test]
    fn le_nom_du_paquet_est_compare_strictement() {
        let crochet = AlibabaPlugin::new().hook();
        assert!(crochet.applies_to(&evenement("@ai-sdk/alibaba")));
        assert!(!crochet.applies_to(&evenement("@ai-sdk/ALIBABA")));
        assert!(!crochet.applies_to(&evenement("alibaba")));
        assert!(!crochet.applies_to(&evenement("@ai/alibaba")));
        // Une espace en trop suffit a faire echouer l egalite.
        assert!(!crochet.applies_to(&evenement("@ai-sdk/alibaba ")));
    }

    #[test]
    fn appliquer_le_crochet_ecrase_une_liaison_deja_presente() {
        let crochet = AlibabaPlugin::new().hook();
        let mut event = evenement("@ai-sdk/alibaba");
        event.sdk = Some(AlibabaSdk {
            package: "ancien".to_string(),
            factory: "ancienneFabrique".to_string(),
            options: BTreeMap::new(),
        });
        crochet.apply(&mut event);
        let sdk = event.sdk.unwrap();
        assert_eq!(sdk.package, "@ai-sdk/alibaba");
        assert_eq!(sdk.factory, "createAlibaba");
    }

    #[test]
    fn les_options_sont_transmises_telles_elles_chaine_vide_comprise() {
        let crochet = AlibabaPlugin::new().hook();
        let mut event = evenement("@ai-sdk/alibaba");
        event.options.insert("baseURL".to_string(), serde_json::json!(""));
        event
            .options
            .insert("apiKey".to_string(), serde_json::json!("cle"));
        let avant = event.options.clone();
        crochet.apply(&mut event);
        let sdk = event.sdk.unwrap();
        // Aucune valeur n est filtree, normalisee ou remplacee.
        assert_eq!(sdk.options, avant);
        assert_eq!(sdk.options.get("baseURL"), Some(&serde_json::json!("")));
    }

    #[test]
    fn le_crochet_ne_reecrit_ni_le_modele_ni_le_paquet() {
        let crochet = AlibabaPlugin::new().hook();
        let mut event = evenement("@ai-sdk/alibaba");
        let modele_avant = event.model.clone();
        let paquet_avant = event.package.clone();
        let options_avant = event.options.clone();
        crochet.apply(&mut event);
        assert_eq!(event.model, modele_avant);
        assert_eq!(event.package, paquet_avant);
        assert_eq!(event.options, options_avant);
    }

    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_de_la_source() {
        let mut event = evenement("@ai-sdk/alibaba");
        AlibabaPlugin::new().hook().apply(&mut event);
        let objet = serde_json::to_value(&event).unwrap();
        let objet = objet.as_object().expect("un evenement serialise en objet");
        let mut noms: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        noms.sort();
        assert_eq!(noms, vec!["model", "options", "package", "sdk"]);
    }

    #[test]
    fn le_champ_sdk_est_absaut_tant_qu_aucun_crochet_n_a_agi() {
        let objet = serde_json::to_value(evenement("@ai-sdk/alibaba")).unwrap();
        let objet = objet.as_object().expect("un evenement serialise en objet");
        assert!(!objet.contains_key("sdk"));
        let mut noms: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        noms.sort();
        assert_eq!(noms, vec!["model", "options", "package"]);
    }

    #[test]
    fn un_evenement_se_relit_depuis_son_json() {
        let mut event = evenement("@ai-sdk/alibaba");
        event
            .options
            .insert("apiKey".to_string(), serde_json::json!("cle"));
        AlibabaPlugin::new().hook().apply(&mut event);
        let json = serde_json::to_string(&event).unwrap();
        let relu: AlibabaSdkEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, event);
    }

    #[test]
    fn un_evenement_sans_la_cle_sdk_se_relit_avec_sdk_absent() {
        // En TS le champ est `sdk?`, donc la cle peut ne pas etre presente.
        let json = r#"{"model":{"id":"qwen-max"},"package":"@ai-sdk/alibaba","options":{}}"#;
        let event: AlibabaSdkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.package, "@ai-sdk/alibaba");
        assert!(event.sdk.is_none());
    }

    #[test]
    fn un_evenement_complet_se_relit_avec_ses_quatre_champs() {
        let json = concat!(
            r#"{"model":{"id":"qwen-max"},"package":"@ai-sdk/alibaba","#,
            r#""options":{"apiKey":"cle"},"#,
            r#""sdk":{"package":"@ai-sdk/alibaba","factory":"createAlibaba","options":{"apiKey":"cle"}}}"#
        );
        let event: AlibabaSdkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.package, "@ai-sdk/alibaba");
        assert_eq!(event.model["id"], serde_json::json!("qwen-max"));
        assert_eq!(
            event.options.get("apiKey"),
            Some(&serde_json::json!("cle"))
        );
        let sdk = event.sdk.expect("le champ sdk etait present dans le json");
        assert_eq!(sdk.factory, "createAlibaba");
    }
}
