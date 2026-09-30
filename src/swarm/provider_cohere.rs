//! Portage de `packages/core/src/plugin/provider/cohere.ts`.
//!
//! ## Ce que dit la source
//!
//! Quinze lignes. Tout tient dans un objet `{ id, effect }` passe a `define`,
//! qui, dans `plugin/internal.ts`, ne fait que renvoyer son argument. L'effet
//! enregistre **un seul** gestionnaire sur le canal `aisdk.sdk` :
//!
//! ```text
//! if (evt.package !== "@ai-sdk/cohere") return
//! const mod = yield* Effect.promise(() => import("@ai-sdk/cohere"))
//! evt.sdk = mod.createCohere(evt.options)
//! ```
//!
//! Trois details comptent :
//!
//! - le filtre est une **comparaison de chaine exacte**, pas un prefixe ni une
//!   recherche. `"@ai-sdk/Cohere"`, `"@ai-sdk/cohere-extra"` et `""` sont tous
//!   ignores ;
//! - le gestionnaire **mute l'evenement** et ne renvoie rien. C'est
//!   `AISDK.runSDK` qui rend l'evenement apres avoir fait tourner tous les
//!   gestionnaires. D'ou un `&mut SdkEvent` plutot qu'un retour de valeur ;
//! - l'import dynamique du paquet npm n'a pas d'equivalent Rust : il est
//!   represente par le trait [`CohereModule`], que l'hote fournit. C'est le
//!   seul endroit ou le portage ne peut pas etre litteral, et il est dit ici
//!   pour que la relecture ne le prenne pas pour une invention.
//!
//! ## Noms de champs
//!
//! L'evenement porte `model`, `package`, `options` et `sdk`. Aucun de ces quatre
//! noms n'est en camelCase : ils sont donc deja identiques en Rust, et aucun
//! `#[serde(rename = ...)]` n'est necessaire. Un test verifie quand meme
//! l'ensemble des cles JSON, champ par champ, parce que c'est l'erreur la plus
//! frequente de ce portage.
//!
//! Deux chaines distinctes coexistent dans ce fichier et se ressemblent :
//! `id` vaut `"cohere"`, tandis que le paquet reconnu vaut `"@ai-sdk/cohere"`.
//! Les confondre casse silencieusement le filtrage. Elles sont portees comme
//! deux constantes separees, et un test les verifie.
//!
//! Le contenu d'`options` est un `Record<string, any>` recopie tel quel dans
//! `createCohere` : le plugin n'ajoute, ne retire et ne renomme aucune cle. Les
//! noms d'options restent ceux de l'appelant, `apiKey` et `baseURL` compris.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Options d'un evenement SDK : dictionnaire libre recopie sans transformation.
///
/// Le TS ecrit `Record<string, any>`. Les valeurs y sont des `any` JavaScript ;
/// ici une `Value` JSON, ce qui couvre tout ce que le plugin transporte
/// (`name`, `apiKey`, `baseURL`, `timeout`...). Le `fetch` que
/// `AISDK.prepareOptions` glisse dans ces options n'a pas de representation JSON :
/// c'est une limite assumee ici, a traiter dans le portage de `aisdk.ts`, et
/// non dans ce fichier. `BTreeMap` donne un ordre de cles deterministe, la
/// ou l'objet JavaScript en garde un ordre d'insertion.
pub type Options = BTreeMap<String, Value>;

/// L'evenement du canal `aisdk.sdk`, tel que `AISDK.SDKEvent` le decrit.
///
/// Les trois premiers champs sont `readonly` en TypeScript : presents de bout
/// en bout, et jamais modifies par ce plugin. Seul `sdk` est ecrit, et il est
/// facultatif.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// `ModelV2.Info`, le modele demande. Ce plugin ne le lit jamais : il reste
    /// une `Value` plutot qu'un struct complet, dont le portage est ailleurs.
    pub model: Value,
    /// Le paquet npm demande, compare caractere par caractere a
    /// [`CoherePlugin::PACKAGE`].
    pub package: String,
    /// Les options, recopiees telles quelles dans `createCohere`.
    pub options: Options,
    /// Le SDK construit. Absent tant qu'aucun gestionnaire n'a repondu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

impl SdkEvent {
    /// Evenement sans SDK, la forme d'entree du canal.
    pub fn new(model: Value, package: impl Into<String>, options: Options) -> Self {
        Self { model, package: package.into(), options, sdk: None }
    }

    /// Le SDK construit, ou `None` quand aucun gestionnaire n'a repondu.
    ///
    /// `evt.sdk` vaut `undefined` tant que personne n'a parle, et le test
    /// d'origine compte justement sur cette absence. Une chaine vide n'est pas
    /// un SDK absent : elle reste un SDK.
    pub fn sdk(&self) -> Option<&Value> {
        self.sdk.as_ref()
    }
}

/// Le module npm `@ai-sdk/cohere`, seul morceau de JavaScript que ce plugin
/// utilise.
///
/// L'original fait `await import("@ai-sdk/cohere")` au premier evenement
/// correspondant, puis appelle `createCohere` dessus. Rust n'a pas d'import
/// dynamique : l'hote fournit l'implementation, et le plugin se contente de
/// l'appeler. Une seule methode est portee, `createCohere`, parce que c'est la
/// seule que la source utilise.
pub trait CohereModule {
    /// `createCohere(options)`. Les options sont fournies en lecture seule,
    /// comme en JavaScript, ou l'objet n'est jamais reecrit.
    fn create_cohere(&self, options: &Options) -> Value;
}

/// Le plugin Cohere, equivalent de `CoherePlugin`.
///
/// `define` ne fait que renvoyer son argument : tout le comportement est dans
/// [`CoherePlugin::effect`]. L'effet d'origine ne fait rien d'autre que
/// enregistrer ce gestionnaire ; l'enregistrement lui-meme appartient au service
/// `AISDK`, qui n'est pas porte ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoherePlugin;

impl CoherePlugin {
    /// `id` du plugin. Attention : c'est `"cohere"`, **sans** le prefixe de
    /// scope npm, alors que le paquet reconnu est `"@ai-sdk/cohere"`. Ce sont
    /// deux chaines differentes, et le portage les garde separees.
    pub const ID: &'static str = "cohere";

    /// Le seul paquet npm que ce plugin reconnait.
    pub const PACKAGE: &'static str = "@ai-sdk/cohere";

    /// Le gestionnaire enregistre sur le canal `aisdk.sdk`.
    ///
    /// Ne renvoie rien, comme l'original. Son seul effet observable est la
    /// mutation de `event.sdk`, qui reste intact quand le paquet ne correspond
    /// pas.
    pub fn effect<M: CohereModule + ?Sized>(module: &M, event: &mut SdkEvent) {
        if !is_cohere_package(&event.package) {
            return;
        }
        let sdk = module.create_cohere(&event.options);
        event.sdk = Some(sdk);
    }
}

/// Le test exact de la source, `evt.package !== "@ai-sdk/cohere"`.
///
/// Egalite de chaine, pas prefixe. Une chaine vide est acceptee en parametre
/// et ne correspond a rien, donc elle doit etre rejettee. Aucun test de
/// veracite ici : c'est le piege `?` contre `??`, et la source n'utilise que
/// l'egalite stricte.
pub fn is_cohere_package(package: &str) -> bool {
    package == CoherePlugin::PACKAGE
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    /// Le module de `@ai-sdk/cohere` simule. Il note les options recues et
    /// renvoie un SDK qui les recopie, comme le `mock.module` du test
    /// d'origine. C'est aussi le seul moyen de verifier que `createCohere` n'a
    /// pas ete appele quand le filtre rejette l'evenement.
    #[derive(Default)]
    struct FauxModule {
        appels: RefCell<Vec<Options>>,
    }

    impl CohereModule for FauxModule {
        fn create_cohere(&self, options: &Options) -> Value {
            self.appels.borrow_mut().push(options.clone());
            let nom = options.get("name").cloned().unwrap_or(Value::Null);
            json!({ "name": nom })
        }
    }

    /// Un evenement minimal : un modele vide, un paquet impose, et la seule
    /// option `name`, celle que `AISDK.prepareOptions` ajoute toujours.
    fn evenement(package: &str) -> SdkEvent {
        let mut options = Options::new();
        options.insert("name".to_string(), Value::String("cohere".to_string()));
        SdkEvent::new(Value::Null, package, options)
    }

    // -------------------------------------------------------------- le filtrage

    #[test]
    fn un_evenement_d_un_autre_paquet_laisse_le_sdk_absent() {
        let module = FauxModule::default();
        let mut event = evenement("@ai-sdk/openai-compatible");

        CoherePlugin::effect(&module, &mut event);

        assert!(event.sdk().is_none(), "le champ sdk doit rester absent");
        let vide: bool = module.appels.borrow().is_empty();
        assert!(vide, "createCohere ne doit pas etre appele pour un autre paquet");
    }

    #[test]
    fn seule_une_difference_de_casse_desactive_deja_le_plugin() {
        // Le test d'origine compte sur un `@ai-sdk/openai-compatible` qui passe
        // au travers. Ici on verrouille le bord le plus proche : une simple
        // difference de casse, que ni un prefixe ni une recherche ne tolereraient.
        assert!(!is_cohere_package("@ai-sdk/Cohere"));
        assert!(!is_cohere_package("@ai-sdk/COHERE"));
        assert!(!is_cohere_package("@ai-sdk/cohere-extra"));
        assert!(is_cohere_package("@ai-sdk/cohere"));
    }

    #[test]
    fn un_nom_de_paquet_vide_ou_sans_le_scope_ne_declenche_rien() {
        for autre in ["", "cohere", "ai-sdk/cohere", "@cohere", "@ai-sdk/"] {
            assert!(!is_cohere_package(autre), "{} ne doit pas correspondre", autre);

            let module = FauxModule::default();
            let mut event = evenement(autre);
            CoherePlugin::effect(&module, &mut event);
            assert!(event.sdk().is_none(), "{} ne doit pas construire de SDK", autre);
        }
    }

    #[test]
    fn un_sdk_deja_present_survit_a_un_evenement_ignore() {
        // Un gestionnaire qui ne filtre pas ne doit rien ecraser, meme si un
        // autre a deja parle. Le `return` de la source sort avant l'ecriture.
        let module = FauxModule::default();
        let mut event = evenement("@ai-sdk/openai");
        event.sdk = Some(json!({ "existant": true }));

        CoherePlugin::effect(&module, &mut event);

        let attendu = json!({ "existant": true });
        assert_eq!(event.sdk(), Some(&attendu));
    }

    // ------------------------------------------------------ la construction du SDK

    #[test]
    fn un_evenement_du_paquet_cohere_construit_le_sdk_avec_ses_Options() {
        let module = FauxModule::default();
        let mut options = Options::new();
        options.insert("name".to_string(), json!("custom-cohere"));
        options.insert("apiKey".to_string(), json!("test"));
        options.insert("baseURL".to_string(), json!("https://cohere.example"));
        let mut event = SdkEvent::new(Value::Null, "@ai-sdk/cohere", options);

        CoherePlugin::effect(&module, &mut event);

        let attendu = json!({ "name": "custom-cohere" });
        assert_eq!(event.sdk(), Some(&attendu));

        // Les options arrivees dans `createCohere` sont celles de l'evenement,
        // ni ajoutees, ni retirees, ni renommees.
        let recus: Vec<Options> = module.appels.borrow().clone();
        assert_eq!(recus.len(), 1, "createCohere est appele une seule fois");
        let premier = &recus[0];
        assert_eq!(premier["name"], json!("custom-cohere"));
        assert_eq!(premier["apiKey"], json!("test"), "le nom apiKey doit survivre");
        assert_eq!(premier["baseURL"], json!("https://cohere.example"), "le nom baseURL doit survivre");
        assert_eq!(premier.len(), 3);
    }

    #[test]
    fn des_options_vides_donnent_pourtant_un_sdk() {
        // `createCohere` est appele sans condition sur les options : un objet
        // vide n'est pas une raison de passer son chemin.
        let module = FauxModule::default();
        let mut event = SdkEvent::new(Value::Null, "@ai-sdk/cohere", Options::new());

        CoherePlugin::effect(&module, &mut event);

        let recus: Vec<Options> = module.appels.borrow().clone();
        assert_eq!(recus.len(), 1);
        assert_eq!(recus[0].len(), 0);
        assert!(event.sdk().is_some());
    }

    // --------------------------------------------------------- noms de champs

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let module = FauxModule::default();
        let mut event = evenement("@ai-sdk/cohere");
        CoherePlugin::effect(&module, &mut event);

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["package"], json!("@ai-sdk/cohere"), "le champ doit s'appeler package");
        assert_eq!(json["options"]["name"], json!("cohere"), "le champ doit s'appeler options");
        assert_eq!(json["model"], Value::Null, "le champ doit s'appeler model");
        assert_eq!(json["sdk"], json!({ "name": "cohere" }), "le champ doit s'appeler sdk");

        // Aucune variante ni en majuscule, ni en camelCase, ni en scorie
        // Strong: ce sont les formes qui casseraient l'echange sans message.
        for interdit in [
            "Package", "Options", "Model", "Sdk", "SDK", "packageName", "optionsSDK", "npm", "nPm",
        ] {
            assert!(json.get(interdit).is_none(), "{} ne doit pas exister", interdit);
        }
    }

    #[test]
    fn un_evenement_sans_sdk_ne_serie_pas_la_cle_sdk() {
        let event = evenement("@ai-sdk/cohere");

        let json = serde_json::to_value(&event).unwrap();

        // Pas de `sdk: null` : le TS ecrit `undefined`, donc rien du tout.
        assert!(json.get("sdk").is_none(), "l'absence de SDK ne doit pas devenir null");
        assert_eq!(json.as_object().map(|objet| objet.len()), Some(3));
    }

    #[test]
    fn un_evenement_relu_reprend_le_sdk_deja_construit() {
        let json = json!({
            "model": { "id": "command-r-plus" },
            "package": "@ai-sdk/cohere",
            "options": { "name": "cohere" },
            "sdk": { "name": "cohere" }
        });

        let event: SdkEvent = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(event.package, "@ai-sdk/cohere");
        assert_eq!(event.sdk, Some(json!({ "name": "cohere" })));
        assert_eq!(serde_json::to_value(&event).unwrap(), json);
    }

    // ------------------------------------------------------------------ constantes

    #[test]
    fn l_identifiant_du_plugin_et_le_nom_du_paquet_sont_deux_chaines_differentes() {
        assert_eq!(CoherePlugin::ID, "cohere");
        assert_eq!(CoherePlugin::PACKAGE, "@ai-sdk/cohere");
        assert_ne!(CoherePlugin::ID, CoherePlugin::PACKAGE);
    }
}
