//! Portage Rust de `opencode/packages/core/src/plugin/provider/gateway.ts`.
//!
//! La source fait quinze lignes et tient en un seul export :
//!
//! ```ts
//! export const GatewayPlugin = define({ id: "gateway", effect: Effect.fn(...) })
//! ```
//!
//! `define` vient de `plugin/internal.ts` et se resume a `return plugin` : c'est
//! une fonction d'identite, rien a porter. Le seul comportement du fichier est
//! donc l'enregistrement d'un rappel sur le crochet `aisdk.sdk`, dont le corps
//! tient en trois instructions :
//!
//! 1. `if (evt.package !== "@ai-sdk/gateway") return` : filtre sur le paquet.
//! 2. `const mod = yield* Effect.promise(() => import("@ai-sdk/gateway"))` :
//!    import dynamique du module npm.
//! 3. `evt.sdk = mod.createGateway(evt.options)` : ecriture dans l'evenement.
//!
//! Ce qui a ete decide, et pourquoi :
//!
//! - **L'import dynamique n'est pas traduisible tel quel.** Le paquet
//!   `@ai-sdk/gateway` est une dependance JavaScript (declaree en
//!   `packages/core/package.json` ligne 71, version `3.0.191`) et n'est pas
//!   installe sur cette machine. Le module est donc represente par le trait
//!   [`GatewayModule`], dont l'unique methode est la traduction exacte de
//!   `mod.createGateway(evt.options)`. C'est la couture honnete : on n'invente
//!   pas le contenu d'un paquet qu'on ne peut pas lire.
//! - **`Effect.fn` devient une fonction Rust pure**, conformement a la table
//!   des conversions. Aucune `async` : il n'y a pas d'attente reelle, l'import
//!   dynamique etant le seul effet de bord et il est represente par le trait.
//! - **`Record<string, any>` devient `BTreeMap<String, Value>`**, ce qui garde
//!   un ordre deterministe, contrairement a l'objet JavaScript.
//! - **`ModelV2.Info` n'est pas materialise en struct.** Il n'est pas defini
//!   dans ce fichier et le gestionnaire ne le lit jamais ; le champ `model` est
//!   donc transporte comme une valeur JSON opaque.
//!
//! Pieges a ecarter explicitement :
//!
//! - **Comparaison stricte, pas test de veracite.** Le TypeScript ecrit
//!   `evt.package !== "@ai-sdk/gateway"`. C'est une comparaison de chaines, pas
//!   un ternaire : la chaine vide `""` n'est donc PAS traitee comme absente,
//!   elle ne correspond simplement pas et le rappel sort sans rien ecrire. Un
//!   portage par `Option::is_none` ou par test de veracite se comporterait
//!   autrement.
//! - **Ecriture inconditionnelle.** La troisieme instruction ecrase `evt.sdk`
//!   meme si le champ valait deja quelque chose. Un `get_or_insert_with` serait
//!   faux.
//! - **Le garde porte sur `package`, pas sur `options`.** Des options vides
//!   donnent quand meme un SDK ; il n'y a aucun court-circuit de ce cote.
//! - **Le `return` ne sort que du rappel.** La chaine de crochets est une
//!   boucle (`aisdk.ts` ligne 178) qui passe le MEME evenement a chaque
//!   rappel : un rappel qui ignore le paquet n'interrompt personne d'autre.
//! - **Noms de champs : aucun renommage n'est requis, et c'est verifie.** Les
//!   quatre champs de l'evenement sont `model`, `package`, `options` et `sdk`,
//!   quatre mots uniques en minuscules, deja identiques en Rust. Aucun nom
//!   camelCase n'apparait dans ce fichier, donc aucun `#[serde(rename)]` n'est
//!   pose. Le test `les_noms_de_champs_serialises_sont_ceux_du_typescript`
//!   verrouille la sortie JSON pour que la correspondance reste verifiable a
//!   l'exchange avec le TypeScript.
//! - **`sdk` est facultatif** (`sdk?: any` en TypeScript), donc `Option<Value>`
//!   avec `skip_serializing_if = "Option::is_none"` : la cle disparait du JSON
//!   quand aucun gestionnaire n'a produit de SDK, exactement comme en
//!   TypeScript.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant du plugin, champ `id` de l'objet passe a `define`.
///
/// En TS : `define({ id: "gateway", ... })`.
pub const PLUGIN_ID: &str = "gateway";

/// Le nom de paquet que ce plugin reconnait, et le seul.
///
/// En TS : `if (evt.package !== "@ai-sdk/gateway") return`, puis
/// `import("@ai-sdk/gateway")`. Les deux occurrences sont le meme litteral,
/// d'ou une seule constante.
pub const PACKAGE: &str = "@ai-sdk/gateway";

/// L'evenement recu par le crochet `aisdk.sdk`.
///
/// En TS : `SDKEvent` dans `packages/core/src/aisdk.ts`, reecrit a l'identique
/// dans `packages/plugin/src/v2/effect/aisdk.ts` sous le nom `sdk`.
///
/// `model` est un `ModelV2.Info` cote TypeScript. Il n'est pas defini dans ce
/// fichier et le gestionnaire ne le lit jamais, d'ou une valeur JSON opaque
/// plutot qu'un struct invente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// `readonly model: ModelV2.Info`. Non lu par ce plugin, transporte tel
    /// quel.
    pub model: Value,

    /// `readonly package: string`. C'est ce champ que le gestionnaire compare
    /// a [`PACKAGE`]. Nom identique en TypeScript et en Rust.
    pub package: String,

    /// `readonly options: Record<string, any>`. C'est cet objet qui est passe
    /// tel quel a `createGateway`.
    pub options: BTreeMap<String, Value>,

    /// `sdk?: any`, ecrit par le gestionnaire. Absent du JSON tant qu'aucun
    /// gestionnaire n'a rien produit, comme en TypeScript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

impl SdkEvent {
    /// Construit un evenement avec des options vides et aucun SDK, l'etat dans
    /// lequel `AISDK.runSDK` le construit avant de faire tourner les crochets.
    pub fn new(model: Value, package: impl Into<String>) -> Self {
        Self {
            model,
            package: package.into(),
            options: BTreeMap::new(),
            sdk: None,
        }
    }
}

/// Le module npm `@ai-sdk/gateway`, vu par le seul appel que la source en fait.
///
/// Cette couture remplace le `import()` dynamique, qui n'a pas d'equivalent
/// Rust : le paquet est du JavaScript. L'implementateur rejoue le role de
/// `mod.createGateway`.
pub trait GatewayModule {
    /// En TS : `mod.createGateway(evt.options)`.
    ///
    /// L'argument est passe par reference, comme l'objet `evt.options` en
    /// JavaScript : le module ne doit pas le modifier.
    fn create_gateway(&self, options: &BTreeMap<String, Value>) -> Value;
}

/// Un rappel du crochet `aisdk.sdk`, c'est-a-dire le `Effect.fn` interne.
///
/// Le boolien renvoie dit si le rappel a ecrit le champ `sdk`. Il n'est pas
/// utilise par la chaine : il sert uniquement a observer le comportement dans
/// les tests.
pub type SdkHook<'a> = Box<dyn FnMut(&mut SdkEvent) -> bool + 'a>;

/// Les crochets `aisdk`, version concrete du `ctx.aisdk` de la source.
///
/// Seule la famille `sdk` est representee : c'est la seule utilisee par ce
/// fichier. La famille `language` existe dans `packages/plugin/src/v2/effect/
/// aisdk.ts` mais aucun plugin fournisseur ne s'y inscrit, donc elle n'est pas
/// portee ici.
#[derive(Default)]
pub struct AisdkHooks<'a> {
    /// Les rappels enregistres, dans l'ordre d'enregistrement.
    pub sdk: Vec<SdkHook<'a>>,
}

impl<'a> AisdkHooks<'a> {
    /// Fait tourner tous les rappels sur le meme evenement, qui est modifie en
    /// place.
    ///
    /// C'est la fonction `run` de `packages/core/src/aisdk.ts` ligne 174,
    /// specialisee sur la liste `sdk`. Le point important est la boucle : tous
    /// les rappels sont appeles, et un rappel qui sort tot ne stoppe pas les
    /// suivants, parce qu'ils partagent tous le meme objet `event`.
    ///
    /// La source retourne l'evenement ; ici il est modifie par reference, donc
    /// l'appelant le lit apres l'appel, ce qui evite d'ecrire une signature a
    /// deux durees de vie.
    pub fn run_sdk(&mut self, event: &mut SdkEvent) {
        for hook in self.sdk.iter_mut() {
            hook(event);
        }
    }
}

/// Le descripteur exporte par la source.
///
/// En TS : `export const GatewayPlugin = define({ id, effect })`. Comme
/// `define` est l'identite, l'objet reste tel quel. Le champ `effect` etant une
/// fonction, il devient la methode [`GatewayPlugin::effect`] plutot qu'un champ.
pub struct GatewayPlugin<M> {
    /// Champ `id` de l'objet litteral. Toujours [`PLUGIN_ID`].
    pub id: &'static str,

    /// Le module npm qui remplace l'`import()` dynamique.
    module: M,
}

impl<M> GatewayPlugin<M> {
    /// Construit le descripteur avec son module.
    pub fn new(module: M) -> Self {
        Self {
            id: PLUGIN_ID,
            module,
        }
    }
}

/// La borne `M: GatewayModule` n'apparait que sur [`GatewayPlugin::effect`],
/// parce que c'est la seule methode qui appelle le paquet npm. `new` se contente
/// de le stocker.
impl<M: GatewayModule> GatewayPlugin<M> {
    /// Le `effect` de la source : enregistre le rappel sur le crochet `sdk`.
    ///
    /// En TS :
    /// `yield* ctx.aisdk.sdk(Effect.fn(function* (evt) { ... }))`.
    pub fn effect<'a>(&'a self, ctx: &mut AisdkHooks<'a>) {
        let module = &self.module;
        ctx.sdk.push(Box::new(move |event| on_sdk(module, event)));
    }
}

/// Le corps du generateur interne, une fois l'`Effect` retire.
///
/// En TS :
/// ```ts
/// if (evt.package !== "@ai-sdk/gateway") return
/// const mod = yield* Effect.promise(() => import("@ai-sdk/gateway"))
/// evt.sdk = mod.createGateway(evt.options)
/// ```
///
/// Le `return` de la premiere ligne devient `return false`, l'import devient
/// l'appel de la methode du [`GatewayModule`], et l'ecriture dans `evt` est un
/// ecrasement inconditionnel de `sdk`.
pub fn on_sdk<M: GatewayModule + ?Sized>(module: &M, event: &mut SdkEvent) -> bool {
    if event.package != PACKAGE {
        return false;
    }
    event.sdk = Some(module.create_gateway(&event.options));
    true
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;

    /// Faux paquet npm : note les options recues et renvoie un SDK repere.
    #[derive(Default)]
    struct FauxModule {
        appels: RefCell<Vec<BTreeMap<String, Value>>>,
    }

    impl FauxModule {
        /// Nombre d'appels a `createGateway`.
        fn nb_appels(&self) -> usize {
            self.appels.borrow().len()
        }

        /// Options du nieme appel, indexation depuis zero.
        fn options_du(&self, index: usize) -> BTreeMap<String, Value> {
            self.appels.borrow()[index].clone()
        }
    }

    impl GatewayModule for FauxModule {
        fn create_gateway(&self, options: &BTreeMap<String, Value>) -> Value {
            self.appels.borrow_mut().push(options.clone());
            serde_json::json!({ "provider": "gateway" })
        }
    }

    fn options(paires: &[(&str, &str)]) -> BTreeMap<String, Value> {
        paires
            .iter()
            .map(|(cle, valeur)| (cle.to_string(), Value::from(*valeur)))
            .collect()
    }

    /// Un evenement portant le paquet attendu passe dans le gestionnaire, qui
    /// ecrit le SDK renvoye par `createGateway`.
    #[test]
    fn le_paquet_attendu_donne_un_sdk_au_gestionnaire() {
        let module = FauxModule::default();
        let mut event = SdkEvent::new(serde_json::json!({ "id": "auto" }), PACKAGE);

        let ecrit = on_sdk(&module, &mut event);

        assert!(ecrit);
        assert_eq!(event.sdk, Some(serde_json::json!({ "provider": "gateway" })));
        assert_eq!(module.nb_appels(), 1);
    }

    /// Un paquet different laisse le champ `sdk` intact et n'appelle jamais le
    /// module. C'est le cas le plus courant : plusieurs plugins partagent la
    /// meme chaine, et un seul doit reagir a son paquet.
    #[test]
    fn un_paquet_different_laisse_le_champ_sdk_absent() {
        let module = FauxModule::default();
        let mut event = SdkEvent::new(serde_json::json!({}), "@ai-sdk/openai-compatible");

        let ecrit = on_sdk(&module, &mut event);

        assert!(!ecrit);
        assert_eq!(event.sdk, None);
        assert_eq!(module.nb_appels(), 0);
    }

    /// La comparaison est stricte : un paquet qui contient le nom attendu, qui
    /// le prolonge, ou qui differe seulement par la casse n'est pas accepte.
    /// Un portage par "contient" ou par comparaison insensible a la casse
    /// reussirait ces cas a tort.
    #[test]
    fn un_paquet_ressemblant_nest_pas_confondu_avec_le_paquet_attendu() {
        for paquet in [
            "@ai-sdk/gateway-extra",
            "prefixe-@ai-sdk/gateway",
            "@ai-sdk/Gateway",
            "@ai-sdk/gateway ",
            "gateway",
            "",
        ] {
            let module = FauxModule::default();
            let mut event = SdkEvent::new(serde_json::json!({}), paquet);

            let ecrit = on_sdk(&module, &mut event);

            assert!(!ecrit, "paquet accepte a tort : {paquet:?}");
            assert_eq!(event.sdk, None, "ecriture a tort pour {paquet:?}");
            assert_eq!(module.nb_appels(), 0, "module appele a tort pour {paquet:?}");
        }
    }

    /// Les options sont transmises au module a l'identique, sans ajout, sans
    /// renommage et sans perte. C'est la seule chose que le gestionnaire fasse
    /// de `evt.options`.
    #[test]
    fn les_options_sont_transmises_telles_quelle_au_module() {
        let module = FauxModule::default();
        let mut event = SdkEvent::new(serde_json::json!({}), PACKAGE);
        event.options = options(&[("baseURL", "https://gateway.example"), ("apiKey", "secret")]);

        on_sdk(&module, &mut event);

        let recues = module.options_du(0);
        assert_eq!(recues.len(), 2);
        assert_eq!(recues.get("baseURL"), Some(&Value::from("https://gateway.example")));
        assert_eq!(recues.get("apiKey"), Some(&Value::from("secret")));
        // L'objet d'origine n'est pas consomme : il reste lisible apres l'appel.
        assert_eq!(event.options.get("apiKey"), Some(&Value::from("secret")));
    }

    /// Des options vides donnent quand meme un SDK : le garde porte sur
    /// `package`, jamais sur `options`. Un portage par test de veracite qui
    /// court-circuiterait sur une table vide laisserait passer ce cas a tort.
    #[test]
    fn des_options_vides_donnent_toutefois_un_sdk() {
        let module = FauxModule::default();
        let mut event = SdkEvent::new(serde_json::json!({}), PACKAGE);
        event.options = BTreeMap::new();

        let ecrit = on_sdk(&module, &mut event);

        assert!(ecrit);
        assert!(event.sdk.is_some());
        assert!(module.options_du(0).is_empty());
    }

    /// L'ecriture est inconditionnelle : un `sdk` deja present est ecrase.
    /// Le TypeScript fait une affectation, pas un remplissage conditionnel.
    #[test]
    fn un_sdk_deja_present_est_ecrase() {
        let module = FauxModule::default();
        let mut event = SdkEvent::new(serde_json::json!({}), PACKAGE);
        event.sdk = Some(serde_json::json!("ancien"));

        on_sdk(&module, &mut event);

        assert_eq!(event.sdk, Some(serde_json::json!({ "provider": "gateway" })));
    }

    /// Noms de champs a l'echange : le JSON produit porte exactement `model`,
    /// `package` et `options`, plus `sdk` seulement quand il est present. Aucun
    /// `#[serde(rename)]` n'etant pose dans ce fichier, ces noms sont la
    /// reference meme en cas de renommage ulterieur du struct Rust.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut event = SdkEvent::new(serde_json::json!({ "id": "auto" }), PACKAGE);
        event.options = options(&[("baseURL", "https://gateway.example")]);

        let sans_sdk = serde_json::to_value(&event).unwrap();
        let objet = sans_sdk.as_object().unwrap();
        assert_eq!(objet.len(), 3);
        for nom in ["model", "package", "options"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert!(!objet.contains_key("sdk"), "sdk ne doit pas sortir quand il est absent");
        assert_eq!(objet.get("package").and_then(|v| v.as_str()), Some("@ai-sdk/gateway"));

        event.sdk = Some(serde_json::json!({ "provider": "gateway" }));
        let avec_sdk = serde_json::to_value(&event).unwrap();
        let objet = avec_sdk.as_object().unwrap();
        assert_eq!(objet.len(), 4);
        assert!(objet.contains_key("sdk"));

        // Aller-retour : ce que le TypeScript ecrit doit se relire identiquement.
        let relu: SdkEvent = serde_json::from_value(avec_sdk).unwrap();
        assert_eq!(relu, event);
    }

    /// Le `return` du gestionnaire ne sort que de son propre rappel : la boucle
    /// de `aisdk.ts` passe le meme evenement au suivant, qui s'execute meme si
    /// le paquet ne correspondait pas.
    #[test]
    fn la_chaine_de_crochets_continue_apres_un_rappel_qui_ignore_le_paquet() {
        // Le module est partage par reference comptee pour que le premier
        // rappel puisse le posseder, donc aucun emprunt de variable locale ne
        // reste vivant dans la liste de crochets.
        let module = Rc::new(FauxModule::default());
        let pour_le_rappel = Rc::clone(&module);
        let passe = Cell::new(false);
        let temoin = passe.clone();

        let mut ctx = AisdkHooks {
            sdk: Vec::new(),
        };
        ctx.sdk
            .push(Box::new(move |event| on_sdk(pour_le_rappel.as_ref(), event)));
        ctx.sdk.push(Box::new(move |event| {
            temoin.set(true);
            event.sdk = Some(serde_json::json!("pose par le second rappel"));
            false
        }));

        let mut event = SdkEvent::new(serde_json::json!({}), "@ai-sdk/anthropic");
        ctx.run_sdk(&mut event);

        assert!(passe.get(), "le second rappel n'a pas ete appele");
        assert_eq!(module.nb_appels(), 0);
        assert_eq!(event.sdk, Some(serde_json::json!("pose par le second rappel")));
    }

    /// Le descripteur expose le bon identifiant, et son `effect` enregistre un
    /// unique rappel qui produit le SDK comme le ferait la source.
    #[test]
    fn le_plugin_enregistre_son_crochet_avec_son_identifiant() {
        let module = FauxModule::default();
        let plugin = GatewayPlugin::new(module);
        assert_eq!(plugin.id, "gateway");
        assert_eq!(PLUGIN_ID, "gateway");

        let mut ctx = AisdkHooks {
            sdk: Vec::new(),
        };
        plugin.effect(&mut ctx);
        assert_eq!(ctx.sdk.len(), 1);

        let mut event = SdkEvent::new(serde_json::json!({}), PACKAGE);
        ctx.run_sdk(&mut event);

        assert_eq!(event.sdk, Some(serde_json::json!({ "provider": "gateway" })));
    }
}
