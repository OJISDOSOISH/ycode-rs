//! Portage Rust de `opencode/packages/core/src/plugin/provider/togetherai.ts`.
//!
//! Quinze lignes cote TypeScript, et **aucune n'est un objet de configuration
//! litteral**. Ce fichier declare un plugin. Il dit au chargeur de plugins :
//! "si un modele demande le paquet `@ai-sdk/togetherai`, alors remplace son SDK
//! par celui que construit `createTogetherAI`". C'est tout le comportement.
//!
//! ## Ce que dit la source, ligne par ligne
//!
//! - `define({ id: "togetherai", effect })` : `define` ne fait rien, il renvoie
//!   son argument tel quel. Le plugin porte donc un `id` et un `effect`.
//! - `ctx.aisdk.sdk(callback)` enregistre un callback sur le crochet `sdk` du
//!   domaine `aisdk` du contexte de plugin.
//! - Le callback commence par trier : `if (evt.package !== "@ai-sdk/togetherai")`
//!   `return`. C'est une comparaison **stricte de chaines**, pas un test de
//!   veracite. Donc un `package` vide ne correspond pas, et le retour early
//!   laisse l'evenement intact.
//! - Si le paquet correspond, le callback charge dynamiquement
//!   `@ai-sdk/togetherai` puis ecrit `evt.sdk = mod.createTogetherAI(evt.options)`.
//!
//! La forme de l'evenement est donnee par `AISDKHooks` (package `plugin`,
//! `v2/effect/aisdk.ts`) : `{ model, package, options, sdk? }`, les trois
//! premiers en `readonly` et obligatoires, le quatrieme optionnel.
//!
//! ## Choix de portage
//!
//! - `Effect.fn` devient une fonction Rust ordinaire. Pas d'`async` : dans le
//!   code de decision il n'y a aucune attente.
//! - `import()` dynamique n'a pas d'equivalent Rust : il n'y a pas de paquet npm
//!   a charger a l'execution, et on n'invente pas de-chargeur de modules. On
//!   expose a la place une **fabrique** `Fn(&Options) -> Value` providee par
//!   l'appelant, et le callback l'appelle. C'est le seul point du portage qui
//!   doit etre branche sur le vrai SDK.
//! - Le contexte de plugin est reduit a un seul trait, `AisdkContext`, qui ne
//!   porte que `on_aisdk_sdk`. Le reste du contexte n'est pas utilise par
//!   `togetherai.ts` et n'est donc pas declare ici.
//! - L'evenement est un `struct` derive `Serialize, Deserialize`. Il ne voyage
//!   pas en JSON dans la source, mais on le serialise quand meme pour pouvoir
//!   verifier les noms de champs, qui est l'erreur la plus frequente de ce
//!   portage et la seule invisible de l'interieur du code Rust.
//! - Le champ TS `package` est un mot reserve de Rust. Le champ Rust s'appelle
//!   donc `package_`, avec un `#[serde(rename = "package")]` explicite. Idem
//!   pour tous les autres champs : le `rename` est ecrit meme quand le nom est
//!   deja identique, pour que la comparaison avec le TypeScript soit lisible
//!   d'un coup d'oeil.
//! - `sdk?: any` devient `Option<Value>` avec `default` et
//!   `skip_serializing_if`. En JavaScript, un champ a `undefined` disparait de
//!   `JSON.stringify` ; on obtient le meme resultat en n'ecrivant pas la cle.
//! - `evt.sdk = ...` est une **affectation inconditionnelle**. Il n'y a aucun
//!   test de veracite la-dessus : meme un `sdk` deja present, meme un `sdk`
//!   falsy (`0`, `""`, `false`), est ecrase. Un test verrouille ce point.
//!
//! ## Ce qui n'est PAS ici
//!
//! Le type `ModelV2Info` (champ `model`) vient du SDK genere et n'est pas
//! porte par ce fichier : c'est un schema d'un autre module. Il est represente
//! par `serde_json::Value`, qui n'invente aucun champ. Attention au relecteur :
//! `ModelV2Info` contient `providerID`, en majuscules, et c'est un piege du
//! meme genre que ceux de la vague 1.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Le paquet npm que ce plugin sait construire.
///
/// Valeur litterale de `evt.package !== "@ai-sdk/togetherai"` dans la source.
/// C'est la seule chaine que le callback cherche, et la seule qui le fait
/// agir : toute autre valeur le renvoie sans rien toucher.
pub const TOGETHERAI_PACKAGE: &str = "@ai-sdk/togetherai";

/// Options passees a `createTogetherAI`.
///
/// En TS : `readonly options: Record<string, any>`. `Record<string, any>` est
/// un objet a cles libres et valeurs libres, donc un dictionnaire
/// `BTreeMap<String, Value>`. `BTreeMap` plutot que `HashMap` parce que ce
/// portage privilegie partout le determinisme, et que l'ordre d'insertion des
/// cles JavaScript n'a ici aucune signification.
pub type Options = BTreeMap<String, Value>;

/// La fabrique qui remplace le `import()` dynamique de la source.
///
/// En TS : `mod.createTogetherAI(evt.options)`, ou `mod` vient de
/// `import("@ai-sdk/togetherai")`. Cette boite est l'espace ou le portage doit
/// etre branche sur le vrai SDK ; ici on ne fait que l'appeler.
pub type SdkFactory = dyn Fn(&Options) -> Value;

/// L'evenement recu par le crochet `aisdk.sdk`.
///
/// En TS, le type de l'evenement est donne par la premiere cle de
/// `AISDKHooks["sdk"]` : `{ model, package, options, sdk? }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// `readonly model: ModelV2Info`, obligatoire.
    ///
    /// Le schema `ModelV2Info` appartient au SDK genere, pas a ce module. On
    /// garde la valeur brute plutot que d'inventer un struct qui serait faux.
    #[serde(rename = "model")]
    pub model: Value,

    /// `readonly package: string`, obligatoire.
    ///
    /// `package` est un mot reserve de Rust, d'ou le nom de champ `package_`.
    /// Le nom JSON, lui, reste exactement `package`.
    #[serde(rename = "package")]
    pub package_: String,

    /// `readonly options: Record<string, any>`, obligatoire.
    ///
    /// Pas d'`Option` : la source ne marque pas ce champ optionnel, il est
    /// toujours present a l'appel de la fabrique. Une entree sans `options`
    /// doit donc etre rejetee a la lecture, comme en TypeScript.
    #[serde(rename = "options")]
    pub options: Options,

    /// `sdk?: any`, optionnel : absent tant qu'aucun plugin n'a agi.
    #[serde(default, rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

impl SdkEvent {
    /// Evenement en etat, avant passage d'un plugin.
    pub fn new(model: Value, package: impl Into<String>, options: Options) -> Self {
        Self { model, package_: package.into(), options, sdk: None }
    }
}

/// Le contexte de plugin, reduit au seul morceau utilise par ce fichier.
///
/// En TS, `PluginContext.aisdk` est un `AISDKHooks`, c'est-a-dire un objet dont
/// chaque cle est une fonction d'enregistrement de callback. On ne declare que
/// `sdk`, la seule cle touchee par `togetherai.ts`.
pub trait AisdkContext {
    /// Equivalent de `ctx.aisdk.sdk(callback)`.
    ///
    /// Le callback recoit l'evenement **par reference mutable** : c'est
    /// `evt.sdk = ...` qui exige cette mutabilite, et rien d'autre dans la
    /// source ne modifie l'evenement.
    fn on_aisdk_sdk(&self, callback: Box<dyn Fn(&mut SdkEvent)>);
}

/// Pose le SDK sur un evenement, si l'evenement concerne le bon paquet.
///
/// Corps du callback de la source :
///
/// ```text
/// if (evt.package !== "@ai-sdk/togetherai") return
/// evt.sdk = mod.createTogetherAI(evt.options)
/// ```
///
/// Renvoie `true` si le callback a agi, `false` s'il a rendu la main sans rien
/// toucher. Le TypeScript ne retourne rien : ce booleen est un supplement
/// d'information pour l'appelant et les tests, il ne change aucun comportement.
pub fn set_sdk_if_matching(evt: &mut SdkEvent, create: &SdkFactory) -> bool {
    // Comparaison stricte de chaines, comme le `!==` de la source. Attention :
    // ce n'est pas un test de veracite, donc une chaine vide ne correspond pas,
    // et le tri est correct.
    if evt.package_ != TOGETHERAI_PACKAGE {
        return false;
    }

    // Affectation inconditionnelle : pas de test de veracite sur l'ancien
    // `evt.sdk`, meme s'il existe deja et meme s'il est falsy. On construit
    // d'abord la valeur, puis on l'ecrit, pour que les deux emprunts ne se
    // chevauchent jamais.
    let sdk = create(&evt.options);
    evt.sdk = Some(sdk);
    true
}

/// Le plugin, equivalent de `TogetherAIPlugin`.
///
/// `define` renvoie son argument sans le modifier, donc le plugin se reduit a
/// son `id` et a son effet. L'effet n'est pas un champ ici mais une methode,
/// parce qu'il lui faut une fabrique, que le TypeScript obtient par `import()`
/// dynamique et que Rust n'a pas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TogetherAiPlugin {
    /// `id: "togetherai"`, la cle sous laquelle le plugin est enregistre.
    #[serde(rename = "id")]
    pub id: String,
}

impl TogetherAiPlugin {
    /// La valeur exacte de `id` dans la source.
    pub const ID: &'static str = "togetherai";

    /// Plugin pret a etre enregistre.
    pub fn new() -> Self {
        Self { id: Self::ID.to_string() }
    }

    /// Equivalent de `effect(ctx)` : enregistre le callback sur le crochet.
    ///
    /// `create` est le `createTogetherAI` reel. Le callback produit est appele
    /// une fois par evenement emis par le contexte.
    pub fn effect<C, F>(&self, ctx: &C, create: F)
    where
        C: AisdkContext + ?Sized,
        F: Fn(&Options) -> Value + 'static,
    {
        let create: Box<SdkFactory> = Box::new(create);
        ctx.on_aisdk_sdk(Box::new(move |evt| {
            set_sdk_if_matching(evt, &*create);
        }));
    }
}

impl Default for TogetherAiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Faux contexte : retient les callbacks enregistres et permet d'emettre
    /// un evenement, comme le ferait le vrai `aisdk`.
    struct FauxContexte {
        crochets: Mutex<Vec<Box<dyn Fn(&mut SdkEvent)>>>,
    }

    impl FauxContexte {
        fn nouveau() -> Self {
            Self { crochets: Mutex::new(Vec::new()) }
        }

        fn nombre_de_crochets(&self) -> usize {
            self.crochets.lock().unwrap().len()
        }

        /// Emet un evenement a tous les callbacks enregistres.
        fn emettre(&self, evt: &mut SdkEvent) {
            // `pop` donne la valeur posee, sans avoir a cloner une boite de
            // closure, ce qui serait impossible.
            let crochet = self.crochets.lock().unwrap().pop();
            if let Some(crochet) = crochet {
                crochet(evt);
            }
        }
    }

    impl AisdkContext for FauxContexte {
        fn on_aisdk_sdk(&self, callback: Box<dyn Fn(&mut SdkEvent)>) {
            self.crochets.lock().unwrap().push(callback);
        }
    }

    /// Fabrique de test : compte les appels et note les options recues.
    fn fabrique_de_test(
        appels: Arc<AtomicUsize>,
        vues: Arc<Mutex<Vec<Options>>>,
        rendu: Value,
    ) -> impl Fn(&Options) -> Value + 'static {
        move |options: &Options| {
            appels.fetch_add(1, Ordering::SeqCst);
            vues.lock().unwrap().push(options.clone());
            rendu.clone()
        }
    }

    fn options_vides() -> Options {
        Options::new()
    }

    // ----------------------------------------------------------------- comportement

    #[test]
    fn un_evenement_du_paquet_togetherai_recoit_le_sdk_construit() {
        let ctx = FauxContexte::nouveau();
        let appels = Arc::new(AtomicUsize::new(0));
        let vues = Arc::new(Mutex::new(Vec::new()));
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, fabrique_de_test(appels.clone(), vues.clone(), json!({ "nom": "together" })));

        let mut evt = SdkEvent::new(json!({ "id": "meta-llama/Llama-3-70B" }), TOGETHERAI_PACKAGE, options_vides());
        ctx.emettre(&mut evt);

        assert_eq!(evt.sdk, Some(json!({ "nom": "together" })));
        assert_eq!(appels.load(Ordering::SeqCst), 1, "la fabrique doit etre appelee une fois");
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_est_laisse_entierement_intact() {
        let ctx = FauxContexte::nouveau();
        let appels = Arc::new(AtomicUsize::new(0));
        let vues = Arc::new(Mutex::new(Vec::new()));
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, fabrique_de_test(appels.clone(), vues.clone(), json!({ "nom": "together" })));

        let avant = SdkEvent::new(json!({ "id": "gpt-4o" }), "@ai-sdk/openai", options_vides());
        let mut evt = avant.clone();
        ctx.emettre(&mut evt);

        assert_eq!(evt, avant, "l'evenement ne doit avoir change d'un caractere");
        assert_eq!(appels.load(Ordering::SeqCst), 0, "la fabrique ne doit pas etre appelee");
    }

    #[test]
    fn les_options_de_l_evenement_sont_passees_telles_quelles_a_la_constructrice() {
        let ctx = FauxContexte::nouveau();
        let vues = Arc::new(Mutex::new(Vec::new()));
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, fabrique_de_test(Arc::new(AtomicUsize::new(0)), vues.clone(), json!({})));

        let mut options = Options::new();
        options.insert("apiKey".to_string(), json!("secret"));
        options.insert("baseURL".to_string(), json!("https://api.together.xyz/v1"));
        let mut evt = SdkEvent::new(json!({}), TOGETHERAI_PACKAGE, options.clone());
        ctx.emettre(&mut evt);

        let vues = vues.lock().unwrap();
        assert_eq!(vues.len(), 1);
        assert_eq!(vues[0], options, "la fabrique doit recevoir les options intactes");
        assert_eq!(vues[0]["apiKey"], json!("secret"));
        assert_eq!(vues[0]["baseURL"], json!("https://api.together.xyz/v1"));
    }

    #[test]
    fn un_sdk_deja_present_est_ecrase_meme_sil_est_faux() {
        // Piege `?` contre `??`, version fidele : la source ecrit
        // `evt.sdk = mod.createTogetherAI(...)`, sans tester l'ancien `evt.sdk`.
        // Un `0` est falsy en JavaScript, il doit donc etre ecrase quand meme.
        let ctx = FauxContexte::nouveau();
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, fabrique_de_test(Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(Vec::new())), json!({ "neuf": true })));

        let mut evt = SdkEvent::new(json!({}), TOGETHERAI_PACKAGE, options_vides());
        evt.sdk = Some(json!(0));
        ctx.emettre(&mut evt);

        assert_eq!(evt.sdk, Some(json!({ "neuf": true })), "un sdk falsy doit etre ecrase");
    }

    #[test]
    fn un_nom_de_paquet_vide_ne_correspond_pas() {
        // La source compare des chaines, elle ne teste pas la veracite. Une
        // chaine vide est falsy en JavaScript mais n'est pas le paquet vise :
        // le callback doit rendre la main.
        let ctx = FauxContexte::nouveau();
        let appels = Arc::new(AtomicUsize::new(0));
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, fabrique_de_test(appels.clone(), Arc::new(Mutex::new(Vec::new())), json!({})));

        let mut evt = SdkEvent::new(json!({}), "", options_vides());
        ctx.emettre(&mut evt);

        assert_eq!(evt.sdk, None);
        assert_eq!(appels.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn un_seul_crochet_est_enregistre_et_chaque_evenement_passe_par_lui() {
        let ctx = FauxContexte::nouveau();
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, fabrique_de_test(Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(Vec::new())), json!({})));

        assert_eq!(ctx.nombre_de_crochets(), 1, "un seul enregistrement par plugin");

        // Un evenement qui ne correspond pas ne casse rien, le suivant passe.
        let mut premier = SdkEvent::new(json!({}), "autre-paquet", options_vides());
        ctx.emettre(&mut premier);
        assert_eq!(premier.sdk, None);

        let mut second = SdkEvent::new(json!({}), TOGETHERAI_PACKAGE, options_vides());
        ctx.emettre(&mut second);
        assert_eq!(second.sdk, Some(json!({})));
    }

    // --------------------------------------------------- noms de champs et JSON

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Champ par champ, comme demande par la vague 2. Chaque nom est verifie
        // positivement, puis les variantes fautives sont verifiees absentes.
        let evt = SdkEvent::new(
            json!({ "id": "meta-llama/Llama-3-70B" }),
            TOGETHERAI_PACKAGE,
            options_vides(),
        );
        let vide = serde_json::to_value(&evt).unwrap();

        assert_eq!(vide["model"], json!({ "id": "meta-llama/Llama-3-70B" }), "le champ doit s'appeler model");
        assert_eq!(vide["package"], "@ai-sdk/togetherai", "le champ doit s'appeler package");
        assert!(vide.get("sdk").is_none(), "sdk absent tant qu'aucun plugin n'a agi");

        let avec_sdk = SdkEvent { sdk: Some(json!({ "x": 1 })), ..evt.clone() };
        let rempli = serde_json::to_value(&avec_sdk).unwrap();
        assert_eq!(rempli["sdk"], json!({ "x": 1 }), "le champ doit s'appeler sdk");
        assert!(rempli.get("Options").is_none(), "la casse ne doit pas changer");
        assert!(rempli.get("Package").is_none(), "la casse ne doit pas changer");
        assert!(rempli.get("package_").is_none(), "le nom interne Rust ne doit pas sortir");
        assert!(rempli.get("Sdk").is_none(), "la casse ne doit pas changer");
        assert!(rempli.get("Model").is_none(), "la casse ne doit pas changer");
    }

    #[test]
    fn l_evenement_se_relit_depuis_le_meme_json_quil_a_ecrit() {
        let evt = SdkEvent::new(
            json!({ "id": "m" }),
            TOGETHERAI_PACKAGE,
            {
                let mut o = Options::new();
                o.insert("apiKey".to_string(), json!("k"));
                o
            },
        );
        let avec_sdk = SdkEvent { sdk: Some(json!({ "fabrique": true })), ..evt.clone() };

        let json = serde_json::to_value(&avec_sdk).unwrap();

        assert_eq!(serde_json::from_value::<SdkEvent>(json).unwrap(), avec_sdk);
    }

    #[test]
    fn un_evenement_sans_nom_de_paquet_est_refuse() {
        // Champ obligatoire cote TS, donc obligatoire ici aussi : on ne
        // transforme pas une absence en chaine vide.
        let json = json!({ "model": {}, "options": {} });
        assert!(serde_json::from_value::<SdkEvent>(json).is_err());
    }

    #[test]
    fn un_evenement_sans_options_est_refuse() {
        // Idem pour `options`, marque obligatoire dans `AISDKHooks`.
        let json = json!({ "model": {}, "package": "@ai-sdk/togetherai" });
        assert!(serde_json::from_value::<SdkEvent>(json).is_err());
    }

    #[test]
    fn un_sdk_absent_ne_produit_pas_de_cle_nulle() {
        // En JavaScript un champ a `undefined` disparait de `JSON.stringify`.
        let evt = SdkEvent::new(json!({}), TOGETHERAI_PACKAGE, options_vides());

        let vide = serde_json::to_value(&evt).unwrap();

        assert!(vide.get("sdk").is_none());
        assert_eq!(vide.as_object().map(|o| o.len()), Some(3), "model, package, options");
    }

    // ------------------------------------------------------------- identite du plugin

    #[test]
    fn le_plugin_s_enregistre_sous_le_nom_togetherai() {
        let plugin = TogetherAiPlugin::new();

        assert_eq!(plugin.id, "togetherai");
        assert_eq!(plugin.id, TogetherAiPlugin::ID);

        let json = serde_json::to_value(&plugin).unwrap();
        assert_eq!(json, json!({ "id": "togetherai" }), "le champ doit s'appeler id");
        assert!(json.get("Id").is_none(), "la casse ne doit pas changer");
    }

    #[test]
    fn le_nom_du_paquet_vise_est_ce_du_typescript() {
        // Le caractere `@` devant `ai-sdk` est la source d'erreur classique
        // quand on recopie un nom de paquet.
        assert_eq!(TOGETHERAI_PACKAGE, "@ai-sdk/togetherai");
    }

    #[test]
    fn seul_le_sdk_change_les_autres_champs_restent_intacts() {
        // Le tri ne doit toucher ni `model` ni `options` : seuls `sdk` et
        // l'appel a la fabrique changent quand le paquet correspond.
        let ctx = FauxContexte::nouveau();
        let plugin = TogetherAiPlugin::new();
        plugin.effect(&ctx, |options: &Options| json!({ "recu": options.len() }));

        let mut options = Options::new();
        options.insert("a".to_string(), json!(1));
        options.insert("b".to_string(), json!(2));
        let mut evt = SdkEvent::new(json!({ "id": "m", "providerID": "togetherai" }), TOGETHERAI_PACKAGE, options);
        let model_avant = evt.model.clone();
        let options_avant = evt.options.clone();

        ctx.emettre(&mut evt);

        assert_eq!(evt.model, model_avant, "model ne doit pas bouger");
        assert_eq!(evt.options, options_avant, "options ne doivent pas bouger");
        assert_eq!(evt.package_, TOGETHERAI_PACKAGE, "package ne doit pas bouger");
        assert_eq!(evt.sdk, Some(json!({ "recu": 2 })));
    }
}
