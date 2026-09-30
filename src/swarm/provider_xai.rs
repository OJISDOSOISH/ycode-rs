//! Portage Rust de `opencode/packages/core/src/plugin/provider/xai.ts`.
//!
//! Vingt-deux lignes de TypeScript, dont la moitie est de l'enregistrement de
//! gestionnaires. Le fichier ne declare **aucune donnee** en dehors de son
//! identifiant et de deux constantes, et ne construit **aucune liste**. Il n'y a
//! donc rien a serialiser en JSON du cote configuration : le port tient dans un
//! petit struct de plugin, deux evenements, et deux fonctions pures qui
//! repondent a la seule question que le TS repond vraiment : est-ce que cet
//! evenement m'appartient, et avec quels arguments.
//!
//! ## Ce que dit la source
//!
//! Le plugin s'appelle `xai`. Il enregistre deux gestionnaires sur le service
//! AI SDK :
//!
//! 1. `ctx.aisdk.sdk` : si le paquet demande n'est pas `@ai-sdk/xai`, retour
//!    immediat. Sinon import dynamique du paquet et
//!    `evt.sdk = mod.createXai(evt.options)`.
//! 2. `ctx.aisdk.language` : si le `providerID` du modele n'est pas `xai`,
//!    retour immediat. Sinon `evt.language = evt.sdk.responses(evt.model.api.id)`.
//!
//! Les deux gestionnaires ont la meme forme : un filtre d'entree, puis une
//! affectation. Ce sont ces deux filtres et les arguments passes a
//! l'affectation qui sont portes ici.
//!
//! ## Ce qui n'est pas portable, et pourquoi
//!
//! L'appel a `createXai` et l'appel a `.responses` visent des objets
//! **JavaScript** : ils ne peuvent pas etre ecrits en Rust. Le plugin ne peut
//! donc pas produire un SDK ni un modele de langage reels. Plutot que de
//! inventer un equivalent qui n'existe pas, on enregistre la **description de
//! l'appel** a faire : quel paquet, quelle fabrique, quels arguments. C'est ce
//! que font les structs [`Sdk`] et [`Language`], et ce que produisent
//! [`Plugin::apply_sdk`] et [`Plugin::apply_language`]. La
//! passerelle JavaScript n'aura plus qu'a executer ces trois informations.
//!
//! Aucun comportement n'est ajoute : ni cache, ni `initError`, ni boucle
//! d'evenements. Ces choses-la vivent dans `aisdk.ts`, pas ici.
//!
//! ## Noms de champs
//!
//! C'est le point le plus delicat du fichier. Les evenements reproduisent deux
//! interfaces du TS, `AISDK.SDKEvent` et `AISDK.LanguageEvent`, et un seul de
//! leurs champs porte une casse que Rust ne peut pas ecrire tel quel :
//! `providerID`, avec le **D majuscule**. Il est donc ecrit `provider_id` en
//! Rust et porte un `#[serde(rename = "providerID")]` explicite. Les autres
//! noms (`package`, `options`, `sdk`, `language`, `model`, `api`, `id`) sont
//! deja identiques et n'ont besoin d'aucun `rename`.
//!
//! A l'inverse, `Sdk` et `Language` ne sont **pas** des interfaces du TS : ce
//! sont des enregistrements cote Rust de l'appel a faire. Leurs noms de champs
//! (`package`, `factory`, `options`, `id`) n'ont donc pas de pendant
//! JavaScript, et aucun test ne pretend le contraire.
//!
//! ## Modele volontairement partiel
//!
//! Le TS lit `evt.model.providerID` et `evt.model.api.id`, et rien d'autre. Le
//! struct [`Model`] ne porte donc que ces deux champs, malgre le fait que
//! `ModelV2.Info` en a beaucoup d'autres (`id`, `request`, `api.type`,
//! `api.url`, `api.settings`...). Ce n'est pas une perte : Serde ignore les
//! champs inconnus a la lecture, comme le decodeur de Schema, donc un
//! evenement complet venu du TS se lit sans probleme et se reecrit en ne
//! gardant que ce que le plugin regarde reellement.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Constantes du plugin
// ---------------------------------------------------------------------------

/// Identifiant du plugin, passe a `define({ id: ... })`.
///
/// En TS : `define({ id: "xai", ... })`. Ce nom sert au registre des plugins,
/// il n'est pas visible dans les evenements.
pub const ID: &str = "xai";

/// Fournisseur compare dans le gestionnaire `language`.
///
/// En TS : `ProviderV2.ID.make("xai")`. `ProviderV2.ID` est une chaine de
/// caracteres marquee, donc la valeur est exactement `"xai"`, sans
/// transformation. Cette constante n'est pas l'identifiant du plugin : c'est
/// une coincidence utile, et un test verifie qu'elles restent egales.
pub const PROVIDER_ID: &str = "xai";

/// Paquet npm qui fabrique le SDK, compare dans le gestionnaire `sdk`.
///
/// En TS : `evt.package !== "@ai-sdk/xai"` puis `import("@ai-sdk/xai")`.
pub const PACKAGE: &str = "@ai-sdk/xai";

/// Fonction exportee par le paquet, appelee avec les options.
///
/// En TS : `mod.createXai(evt.options)`.
pub const SDK_FACTORY: &str = "createXai";

/// Methode appelee sur le SDK pour obtenir le modele de langage.
///
/// En TS : `evt.sdk.responses(evt.model.api.id)`.
pub const LANGUAGE_FACTORY: &str = "responses";

/// Options d'un evenement : dictionnaire libre, cle en chaine, valeur quelconque.
///
/// En TS : `Record<string, any>` (deux fois, dans les deux evenements). Le
/// contenu n'est jamais filtre ni valide par le TS, donc il ne l'est pas ici.
pub type Options = BTreeMap<String, Value>;

// ---------------------------------------------------------------------------
// Modele, vu par les deux gestionnaires
// ---------------------------------------------------------------------------

/// L'API d'un modele, telle que le gestionnaire `language` la lit.
///
/// En TS : `evt.model.api`, et plus precisement `evt.model.api.id`, qui est
/// l'identifiant du modele chez le fournisseur. C'est le seul champ de l'API
/// que ce plugin regarde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Api {
    /// Identifiant du modele chez le fournisseur, par exemple `grok-4`.
    ///
    /// Nom identique en TS et en Rust, aucun `rename` necessaire.
    pub id: String,
}

/// Le modele qui declenche l'evenement, en version reduite.
///
/// En TS : `ModelV2.Info`. Ce plugin n'en lit que `providerID` et `api.id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    /// Fournisseur du modele. En TS la majuscule est un **D** : `providerID`.
    ///
    /// C'est le piege de casse de ce fichier. `projectID`, `providerID`,
    /// `modelID` ne s'ecrivent jamais avec un `d` minuscule, et l'oubli ne
    /// produirait aucune erreur visible depuis Rust : il apparaitrait
    /// uniquement a l'echange avec le TypeScript. D'ou le `rename` explicite
    /// et le test qui refuse `providerId` a la lecture.
    #[serde(rename = "providerID")]
    pub provider_id: String,
    /// API du modele.
    pub api: Api,
}

impl Model {
    /// Modele du fournisseur donne, pour l'API donnee.
    pub fn new(provider_id: impl Into<String>, api_id: impl Into<String>) -> Self {
        Self { provider_id: provider_id.into(), api: Api { id: api_id.into() } }
    }
}

// ---------------------------------------------------------------------------
// Ce que le plugin fait de chaque evenement
// ---------------------------------------------------------------------------

/// Description de l'appel `createXai(options)`.
///
/// Ce n'est pas une interface du TypeScript : en TS, `evt.sdk` recoit un objet
/// JavaScript opaque, type `any`. On ne peut pas fabriquer cet objet en Rust,
/// on note donc ce qu'il faut y passer pour que la passerelle JavaScript
/// obtienne l'equivalent.
///
/// Les `derive` de serialisation servent a faire circuler cette description
/// jusqu'a la passerelle. Aucun de ses noms de champs (`package`, `factory`,
/// `options`) n'a de pendant dans le TypeScript : ils ne sont pas des traduisons,
/// ils ne sont pas verifiables contre la source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sdk {
    /// Paquet a importer, c'est-a-dire [`PACKAGE`].
    pub package: String,
    /// Fabrique a appeler sur ce paquet, c'est-a-dire [`SDK_FACTORY`].
    pub factory: String,
    /// Options recues dans l'evenement, passeees telles quelles a la fabrique.
    ///
    /// Elles ne sont ni filtrees ni completees par ce plugin.
    pub options: Options,
}

/// Description de l'appel `sdk.responses(model.api.id)`.
///
/// Comme [`Sdk`], c'est un enregistrement de l'appel, pas une interface du TS :
/// en TS, `evt.language` recoit un `LanguageModelV3` produit par la bibliotheque.
/// Meme remarque sur les noms de champs, qui sont ceux de Rust et non du TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Language {
    /// Paquet qui fournit la fabrique, c'est-a-dire [`PACKAGE`].
    pub package: String,
    /// Methode a appeler, c'est-a-dire [`LANGUAGE_FACTORY`].
    pub factory: String,
    /// Identifiant passe a la fabrique, tire de `evt.model.api.id`.
    pub id: String,
}

// ---------------------------------------------------------------------------
// Les deux evenements
// ---------------------------------------------------------------------------

/// Evenement de construction d'un SDK.
///
/// En TS : `interface SDKEvent` de `aisdk.ts`. Les trois premiers champs sont
/// en lecture seule et toujours presents ; `sdk` est facultatif, c'est
/// justement ce que ce plugin vient remplir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// Modele pour lequel on construit un SDK.
    pub model: Model,
    /// Paquet demande. Le plugin ne fait rien si ce n'est pas [`PACKAGE`].
    ///
    /// Nom identique en TS et en Rust.
    pub package: String,
    /// Options a passer a la fabrique du paquet.
    pub options: Options,
    /// SDK construit. Absent tant qu'aucun plugin ne l'a produit.
    ///
    /// En TS le champ est `sdk?: SDK` avec `SDK = any`. Le type exact de
    /// l'objet JavaScript n'est pas reprenable, d'ou la description de [`Sdk`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Sdk>,
}

impl SdkEvent {
    /// Evenement de construction, sans SDK encore produit.
    pub fn new(model: Model, package: impl Into<String>, options: Options) -> Self {
        Self { model, package: package.into(), options, sdk: None }
    }
}

/// Evenement de construction d'un modele de langage.
///
/// En TS : `interface LanguageEvent` de `aisdk.ts`. Ici `sdk` est obligatoire,
/// a l'inverse du `sdk` de [`SdkEvent`] : a ce stade le SDK existe deja, c'est le
/// modele qui manque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageEvent {
    /// Modele pour lequel on construit un modele de langage.
    pub model: Model,
    /// SDK deja construit par le gestionnaire precedent. Obligatoire.
    pub sdk: Sdk,
    /// Options du modele.
    ///
    /// Presentes et obligatoires cote TS, mais **ignorees** par le plugin xai :
    /// le gestionnaire `language` ne les lit pas. On les garde quand meme, pour
    /// que la forme reste celle de l'interface d'origine.
    pub options: Options,
    /// Modele de langage construit. Absent tant qu'aucun plugin ne l'a produit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
}

impl LanguageEvent {
    /// Evenement de construction, sans modele de langage encore produit.
    pub fn new(model: Model, sdk: Sdk, options: Options) -> Self {
        Self { model, sdk, options, language: None }
    }
}

// ---------------------------------------------------------------------------
// Le plugin
// ---------------------------------------------------------------------------

/// Le plugin xai.
///
/// En TS : `define({ id: "xai", effect: ... })`. `define` ne fait que renvoyer
/// son argument, donc la seule donnee du plugin est son `id`. Le `effect`, lui,
/// enregistre deux gestionnaires et n'a pas d'equivalent : il est remplace ici
/// par les deux fonctions [`Plugin::apply_sdk`] et [`Plugin::apply_language`],
/// qui sont le corps de ces gestionnaires sans la dependance a `Effect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plugin {
    /// Identifiant du plugin.
    pub id: String,
}

impl Plugin {
    /// Le plugin tel que `define` le declare.
    pub fn xai() -> Self {
        Self { id: ID.to_string() }
    }

    /// Le gestionnaire `sdk` s'applique-t-il a cet evenement ?
    ///
    /// C'est le test `evt.package !== "@ai-sdk/xai"` du TS. La comparaison est
    /// stricte : une chaine vide ou une casse differente ne correspond pas.
    pub fn handles_sdk(&self, event: &SdkEvent) -> bool {
        event.package == PACKAGE
    }

    /// Le gestionnaire `language` s'applique-t-il a cet evenement ?
    ///
    /// C'est le test `evt.model.providerID !== ProviderV2.ID.make("xai")`.
    /// La comparaison porte sur le fournisseur, jamais sur l'identifiant du
    /// modele ni sur le nom du paquet.
    pub fn handles_language(&self, event: &LanguageEvent) -> bool {
        event.model.provider_id == PROVIDER_ID
    }

    /// Applique le gestionnaire `sdk` a l'evenement.
    ///
    /// Renvoie `false` et ne touche a rien si le paquet n'est pas le notre,
    /// comme le `return` immediat du TS. Sinon, note l'appel a
    /// `createXai(evt.options)` dans `evt.sdk` et renvoie `true`.
    ///
    /// Le TS ecrase `evt.sdk` sans regarder s'il etait deja rempli, et cette
    /// fonction fait pareil.
    pub fn apply_sdk(&self, event: &mut SdkEvent) -> bool {
        if !self.handles_sdk(event) {
            return false;
        }
        event.sdk = Some(Sdk {
            package: PACKAGE.to_string(),
            factory: SDK_FACTORY.to_string(),
            options: event.options.clone(),
        });
        true
    }

    /// Applique le gestionnaire `language` a l'evenement.
    ///
    /// Renvoie `false` et ne touche a rien si le fournisseur n'est pas le
    /// notre. Sinon, note l'appel a `responses(evt.model.api.id)` dans
    /// `evt.language` et renvoie `true`.
    pub fn apply_language(&self, event: &mut LanguageEvent) -> bool {
        if !self.handles_language(event) {
            return false;
        }
        event.language = Some(Language {
            package: PACKAGE.to_string(),
            factory: LANGUAGE_FACTORY.to_string(),
            id: event.model.api.id.clone(),
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Un SDK deja construit, pour les tests du gestionnaire `language`.
    fn sdk_deja_construit() -> Sdk {
        Sdk {
            package: PACKAGE.to_string(),
            factory: SDK_FACTORY.to_string(),
            options: Options::new(),
        }
    }

    /// Un evenement de langage du fournisseur xai, pret a etre traite.
    fn evenement_de_langage_xai() -> LanguageEvent {
        LanguageEvent::new(
            Model::new(PROVIDER_ID, "grok-4"),
            sdk_deja_construit(),
            Options::new(),
        )
    }

    // ------------------------------------------------- gestionnaire sdk

    #[test]
    fn un_evenement_du_paquet_xai_donne_un_sdk_fabrique_par_create_xai() {
        let plugin = Plugin::xai();
        let mut event = SdkEvent::new(
            Model::new(PROVIDER_ID, "grok-4"),
            PACKAGE,
            Options::new(),
        );

        let traite = plugin.apply_sdk(&mut event);

        assert!(traite, "le paquet xai doit etre pris en charge");
        let sdk = event.sdk.expect("le sdk doit etre produit");
        assert_eq!(sdk.package, "@ai-sdk/xai");
        assert_eq!(sdk.factory, "createXai");
        assert!(sdk.options.is_empty(), "des options vides restent des options");
    }

    #[test]
    fn les_options_de_l_evenement_sont_passees_telles_quelles_a_la_fabrique() {
        let plugin = Plugin::xai();
        let mut options = Options::new();
        options.insert("apiKey".to_string(), json!("secret"));
        options.insert("name".to_string(), json!("xai"));
        let mut event = SdkEvent::new(Model::new(PROVIDER_ID, "grok-4"), PACKAGE, options.clone());

        plugin.apply_sdk(&mut event);

        // Aucune cle n'est ajoutee, retiree ni renommee au passage.
        assert_eq!(event.sdk.expect("sdk attendu").options, options);
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_est_laisse_sans_sdk() {
        let plugin = Plugin::xai();
        let mut event = SdkEvent::new(
            Model::new("openai", "gpt-5"),
            "@ai-sdk/openai",
            Options::new(),
        );

        let traite = plugin.apply_sdk(&mut event);

        assert!(!traite, "un autre paquet ne doit pas etre pris en charge");
        assert!(event.sdk.is_none(), "le gestionnaire ne doit rien ecrire");
    }

    #[test]
    fn un_paquet_vide_ou_de_casse_differente_ne_ressemble_pas_au_paquet_xai() {
        let plugin = Plugin::xai();
        for package in ["", "@ai-sdk/XAI", "@ai-sdk/xai ", "xai"] {
            let mut event =
                SdkEvent::new(Model::new(PROVIDER_ID, "grok-4"), package, Options::new());
            assert!(!plugin.apply_sdk(&mut event), "le paquet {:?} ne correspond pas", package);
            assert!(event.sdk.is_none());
        }
    }

    #[test]
    fn un_sdk_deja_present_est_remplace_par_le_sdk_xai() {
        // Le TS ecrase `evt.sdk` sans condition. Un SDK deja pose par un autre
        // gestionnaire disparait donc, et la comparaison se fait sur le paquet
        // uniquement, pas sur la presence du SDK.
        let plugin = Plugin::xai();
        let mut event = SdkEvent::new(
            Model::new(PROVIDER_ID, "grok-4"),
            PACKAGE,
            Options::new(),
        );
        event.sdk = Some(Sdk {
            package: "autre".to_string(),
            factory: "autreFabrique".to_string(),
            options: Options::new(),
        });

        assert!(plugin.apply_sdk(&mut event));
        let sdk = event.sdk.expect("sdk attendu");
        assert_eq!(sdk.package, PACKAGE);
        assert_eq!(sdk.factory, SDK_FACTORY);
    }

    // -------------------------------------------- gestionnaire language

    #[test]
    fn un_evenement_du_fournisseur_xai_demande_le_modele_nomme_dans_son_api() {
        let plugin = Plugin::xai();
        let mut event = evenement_de_langage_xai();

        let traite = plugin.apply_language(&mut event);

        assert!(traite, "le fournisseur xai doit etre pris en charge");
        let language = event.language.expect("le modele doit etre produit");
        assert_eq!(language.factory, "responses");
        assert_eq!(language.package, PACKAGE);
        // L'identifiant vient de l'API du modele, pas de l'identifiant du
        // modele, qui n'est meme pas lu par ce gestionnaire.
        assert_eq!(language.id, "grok-4");
    }

    #[test]
    fn un_evenement_d_un_autre_fournisseur_est_laisse_sans_modele() {
        let plugin = Plugin::xai();
        let mut event = LanguageEvent::new(
            Model::new("vercel", "ai-sdk/groq-llama"),
            sdk_deja_construit(),
            Options::new(),
        );

        let traite = plugin.apply_language(&mut event);

        assert!(!traite, "un autre fournisseur ne doit pas etre pris en charge");
        assert!(event.language.is_none(), "le gestionnaire ne doit rien ecrire");
    }

    #[test]
    fn un_fournisseur_vide_ou_de_casse_differente_ne_ressemble_pas_a_xai() {
        // Piege `?` contre `??` : en JavaScript une chaine vide est falsy, et
        // un ternaire l'aurait confondue avec une absence. Ici la source
        // compare avec `!==`, donc une chaine vide est une chaine vide, ni
        // vraie ni fausse : elle ne correspond simplement pas.
        let plugin = Plugin::xai();
        for provider in ["", "XAI", "Xai", "xai ", " xai"] {
            let mut event = LanguageEvent::new(
                Model::new(provider, "grok-4"),
                sdk_deja_construit(),
                Options::new(),
            );
            assert!(!plugin.apply_language(&mut event), "le fournisseur {:?} ne correspond pas", provider);
            assert!(event.language.is_none());
        }
    }

    #[test]
    fn les_options_du_evenement_de_langage_n_influencent_pas_le_modele_choisi() {
        // Le gestionnaire `language` ne lit pas `options`. Deux evenements qui
        // ne different que par leurs options doivent produire le meme modele.
        let plugin = Plugin::xai();
        let mut options = Options::new();
        options.insert("temperature".to_string(), json!(0.5));

        let mut sans_options = evenement_de_langage_xai();
        let mut avec_options = LanguageEvent::new(
            Model::new(PROVIDER_ID, "grok-4"),
            sdk_deja_construit(),
            options,
        );

        plugin.apply_language(&mut sans_options);
        plugin.apply_language(&mut avec_options);

        assert_eq!(sans_options.language, avec_options.language);
    }

    #[test]
    fn le_meme_modele_sert_pour_toutes_les_api_du_fournisseur_xai() {
        // Le gestionnaire ne depend que du fournisseur : deux API differentes
        // du meme fournisseur passent toutes les deux, chacune avec son id.
        let plugin = Plugin::xai();
        for api in ["grok-4", "grok-3-mini", ""] {
            let mut event = LanguageEvent::new(
                Model::new(PROVIDER_ID, api),
                sdk_deja_construit(),
                Options::new(),
            );
            assert!(plugin.apply_language(&mut event));
            assert_eq!(event.language.expect("modele attendu").id, api);
        }
    }

    // ------------------------------------------------- noms de champs JSON

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let plugin = Plugin::xai();
        let mut sdk_event =
            SdkEvent::new(Model::new(PROVIDER_ID, "grok-4"), PACKAGE, Options::new());
        plugin.apply_sdk(&mut sdk_event);
        let mut language_event = evenement_de_langage_xai();
        plugin.apply_language(&mut language_event);

        let sdk_json = serde_json::to_value(&sdk_event).unwrap();
        let language_json = serde_json::to_value(&language_event).unwrap();

        for (json, nom) in [
            (&sdk_json, "sdk"),
            (&language_json, "language"),
        ] {
            // Les noms communs aux deux interfaces, a l'identique.
            assert!(json.get("model").is_some(), "{} : le champ model manque", nom);
            assert!(json.get(nom).is_some(), "{} : le champ {} manque", nom, nom);
            assert!(json.get("options").is_some(), "{} : le champ options manque", nom);
            assert!(json.get("sdk").is_some(), "{} : le champ sdk manque", nom);
            assert!(json["model"].get("providerID").is_some(), "{} : providerID manquant", nom);
            assert!(json["model"].get("api").is_some(), "{} : api manquant", nom);
            assert!(json["model"]["api"].get("id").is_some(), "{} : api.id manquant", nom);
        }

        // Le piege de casse : `providerID` porte un D majuscule.
        assert_eq!(sdk_json["model"]["providerID"], "xai");
        assert_eq!(language_json["model"]["providerID"], "xai");
        assert!(sdk_json["model"].get("providerId").is_none(), "le D ne doit pas devenir minuscule");
        assert!(language_json["model"].get("providerId").is_none(), "le D ne doit pas devenir minuscule");
        assert!(sdk_json["model"].get("provider_id").is_none(), "le nom Rust ne doit pas fuir en JSON");
        assert!(sdk_json.get("model_id").is_none());
        assert!(sdk_json.get("Model").is_none());

        // `package` n'existe que dans l'evenement de SDK, comme dans l'interface
        // `SDKEvent` du TS. L'autre evenement ne le declare pas et ne doit donc
        // pas le faire apparaitre.
        assert!(sdk_json.get("package").is_some(), "le champ package manque");
        assert!(language_json.get("package").is_none(), "l'evenement de langage n'a pas de package");
        assert!(sdk_json.get("language").is_none(), "l'evenement de SDK n'a pas de language");
    }

    #[test]
    fn un_sdk_absent_ne_serialise_pas_de_cle_nulle() {
        // En TS le champ est `sdk?: SDK`, donc `JSON.stringify` omet la cle
        // quand elle vaut `undefined`. Serde doit faire pareil.
        let event = SdkEvent::new(Model::new(PROVIDER_ID, "grok-4"), PACKAGE, Options::new());

        let json = serde_json::to_value(&event).unwrap();

        assert!(json.get("sdk").is_none(), "une cle sdk: null n'existe pas en TS");
        assert_eq!(
            json,
            json!({
                "model": { "providerID": "xai", "api": { "id": "grok-4" } },
                "package": "@ai-sdk/xai",
                "options": {}
            })
        );
    }

    #[test]
    fn un_fournisseur_ecrit_provider_id_est_refuse_a_la_lecture() {
        // Le test qui protege vraiment l'echange avec le TypeScript : si le
        // `rename` disparait, cet evenement est lu au lieu d'etre refuse, et
        // rien ne le signale de l'interieur du code Rust.
        let faux = json!({
            "model": { "providerId": "xai", "api": { "id": "grok-4" } },
            "package": "@ai-sdk/xai",
            "options": {}
        });
        let bon = json!({
            "model": { "providerID": "xai", "api": { "id": "grok-4" } },
            "package": "@ai-sdk/xai",
            "options": {}
        });

        assert!(
            serde_json::from_value::<SdkEvent>(faux).is_err(),
            "providerId ne doit pas etre accepte a la place de providerID"
        );
        let event: SdkEvent = serde_json::from_value(bon).unwrap();
        assert_eq!(event.model.provider_id, "xai");
    }

    #[test]
    fn un_evenement_de_langage_sans_sdk_est_refuse() {
        // En TS `readonly sdk: SDK` n'est pas facultatif, contrairement a
        // `sdk?` dans l'autre evenement. Cette difference se voit.
        let json = json!({
            "model": { "providerID": "xai", "api": { "id": "grok-4" } },
            "options": {}
        });

        assert!(serde_json::from_value::<LanguageEvent>(json).is_err());
    }

    #[test]
    fn un_evenement_issu_du_typescript_va_et_vient_entre_json_et_rust() {
        // Forme reelle produite par `aisdk.ts` : le modele y a beaucoup plus
        // de champs que ceux que ce plugin regarde.
        let json = json!({
            "model": {
                "providerID": "xai",
                "id": "grok-4-0709",
                "api": {
                    "id": "grok-4",
                    "type": "aisdk",
                    "package": "@ai-sdk/xai",
                    "settings": { "apiKey": "secret" }
                },
                "request": { "body": {}, "headers": {}, "variant": "default" }
            },
            "package": "@ai-sdk/xai",
            "options": { "name": "xai", "apiKey": "secret" }
        });

        let event: SdkEvent = serde_json::from_value(json).unwrap();

        // Les champs en trop sont ignores, comme le fait le decodeur de Schema.
        assert_eq!(event.model.provider_id, "xai");
        assert_eq!(event.model.api.id, "grok-4");
        assert_eq!(event.package, PACKAGE);
        // Ce qui n'est pas relu par le plugin disparait a l'ecriture : le
        // struct garde les deux seuls champs de ModelV2.Info dont il se sert.
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({
                "model": { "providerID": "xai", "api": { "id": "grok-4" } },
                "package": "@ai-sdk/xai",
                "options": { "apiKey": "secret", "name": "xai" }
            })
        );
        // Et cet aller-retour est stable.
        let relu: SdkEvent =
            serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
        assert_eq!(relu, event);
    }

    #[test]
    fn un_champ_inconnu_dans_un_evenement_est_ignore_comme_en_typescript() {
        let json = json!({
            "model": { "providerID": "xai", "api": { "id": "grok-4" }, "inconnu": 1 },
            "package": "@ai-sdk/xai",
            "options": {},
            "inconnu": true
        });

        let event: SdkEvent = serde_json::from_value(json).unwrap();

        assert_eq!(event.model.provider_id, "xai");
    }

    // ------------------------------------------------------ identite du plugin

    #[test]
    fn le_plugin_s_annonce_comme_xai_et_verrouille_le_fournisseur_xai() {
        // Deux constantes distinctes dans le TS : `id: "xai"` pour le registre
        // des plugins, `ProviderV2.ID.make("xai")` pour le filtre. Elles
        // aujourd'hui la meme valeur, mais elles n'ont pas la meme raison
        // d'etre : si l'une bouge, la comparaison de l'autre doit le montrer.
        let plugin = Plugin::xai();

        assert_eq!(plugin.id, "xai");
        assert_eq!(ID, "xai");
        assert_eq!(PROVIDER_ID, "xai");
        assert_eq!(PACKAGE, "@ai-sdk/xai");
        assert_eq!(SDK_FACTORY, "createXai");
        assert_eq!(LANGUAGE_FACTORY, "responses");
        assert_eq!(serde_json::to_value(&plugin).unwrap(), json!({ "id": "xai" }));
    }

    #[test]
    fn appliquer_un_gestionnaire_ne_change_pas_l_autre_evenement() {
        // Chaque application est isolee : appliquer le gestionnaire sdk ne doit
        // rien changer du gestionnaire language, et reciproquement.
        let plugin = Plugin::xai();
        let mut sdk_event = SdkEvent::new(
            Model::new(PROVIDER_ID, "grok-4"),
            PACKAGE,
            Options::new(),
        );
        let mut language_event = evenement_de_langage_xai();

        assert!(plugin.apply_sdk(&mut sdk_event));
        assert!(language_event.language.is_none());
        assert!(plugin.apply_language(&mut language_event));
        assert_eq!(sdk_event.model.api.id, language_event.model.api.id);
    }
}
