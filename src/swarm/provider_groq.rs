//! Portage de `packages/core/src/plugin/provider/groq.ts`.
//!
//! ## Ce que dit la source
//!
//! ```ts
//! export const GroqPlugin = define({
//!   id: "groq",
//!   effect: Effect.fn(function* (ctx) {
//!     yield* ctx.aisdk.sdk(
//!       Effect.fn(function* (evt) {
//!         if (evt.package !== "@ai-sdk/groq") return
//!         const mod = yield* Effect.promise(() => import("@ai-sdk/groq"))
//!         evt.sdk = mod.createGroq(evt.options)
//!       }),
//!     )
//!   }),
//! })
//! ```
//!
//! Le fichier ne definit aucun schema, aucune cle d'API et aucun modele : c'est
//! un des vingt-neuf plugins de `plugin/provider`, tous identiques sur cette
//! forme. Le plugin enregistre **un seul** crochet sur l'evenement `sdk` du
//! service AISDK. Quand l'evenement annonce le paquet npm `@ai-sdk/groq`, le
//! crochet charge ce paquet et remplace `evt.sdk` par la valeur de
//! `createGroq(evt.options)`. Tout autre paquet est ignore. Aucun autre
//! crochet n'est enregistre : pas de `aisdk.language`, contrairement a
//! `xai.ts`, `openai.ts` ou `sap-ai-core.ts`.
//!
//! ## Les choix de portage
//!
//! 1. L'import dynamique `import("@ai-sdk/groq")` n'a pas d'equivalent Rust :
//!    le paquet npm est charge par l'hote, pas par le code compile. Il devient
//!    le trait [`GroqModule`], dont l'unique methode est l'appel a
//!    `createGroq`. Tout le comportement observable de la source reste dans le
//!    crochet : filtrer sur `package`, puis affecter. Seul le chargement du
//!    module est injecte, et rien d'autre.
//! 2. `Effect.promise(...)` rendait la source asynchrone a cet endroit. Le
//!    chargement etant fait par l'hote, l'affectation est synchrone ici :
//!    aucune fonction `async` n'est necessaire.
//! 3. `ctx.aisdk.sdk(crochet)` est le contrat du contexte de plugin. On n'en
//!    prend que la tranche utilisee par ce fichier, le trait [`AisdkSdkHook`] :
//!    le vrai `PluginContext` a dix domaines, tous sans rapport avec groq.
//! 4. Le `return` du crochet sort du generateur **avant** le chargement du
//!    module et **avant** l'affectation. Un evenement d'un autre paquet conserve
//!    donc le `sdk` qu'il avait deja. La comparaison est un `!==` strict : la
//!    casse compte et la chaine vide ne correspond pas. Trois tests dedies.
//! 5. `define` est un simple `return plugin` (`plugin/internal.ts:59`), et
//!    l'objet `{ id, effect }` devient une unit struct [`GroqPlugin`] avec deux
//!    constantes et une fonction. Rien de plus : pas de couche, pas de service.
//!
//! ## Les noms de champs
//!
//! Le type de l'evenement n'est pas ecrit dans `groq.ts` : il vient du hook
//! partage `packages/plugin/src/v2/effect/aisdk.ts`, duplique dans
//! `packages/core/src/aisdk.ts` sous le nom `SDKEvent`.
//!
//! | TypeScript            | Rust          | Note                        |
//! |-----------------------|---------------|-----------------------------|
//! | `model`               | `model`       | non lu par ce plugin        |
//! | `package`             | `package`     | champ de la comparaison     |
//! | `options`             | `options`     | passe a `createGroq`        |
//! | `sdk?`                | `Option<Sdk>` | seule cle optionnelle       |
//!
//! Aucun de ces quatre noms n'est en camelCase, donc aucun nom n'a besoin d'etre
//! reecrit : ils sont tous deja des mots uniques en minuscules, ce qui est le
//! cas le plus favorable. Les `#[serde(rename = "...")]` sont poses quand meme,
//! de facon explicite, pour que la comparaison avec le TypeScript ne depende
//! d'aucune convention implicite. Attention : `package` et `options` ont exactement
//! le meme nom des deux cotes, et `model` se prononce "model", pas "mode".
//!
//! Le champ `sdk` est optionnel cote TS (`sdk?: any`), donc `Option<Sdk>` avec
//! `skip_serializing_if` : un evenement sur lequel aucun crochet n'a repondu
//! ne doit pas serialiser `"sdk": null`, que le TS ne produirait pas.
//!
//! ## Ce qui n'est pas porte
//!
//! - Le type `ModelV2Info` du champ `model` n'est pas porte ici, et ce plugin ne
//!   le lit jamais : le champ reste donc une valeur JSON opaque, sans forme
//!   imposee. Un agent qui portera le modele pourra remplacer le `Value`.
//! - `type SDK = any` devient `serde_json::Value`, la seule representation
//!   d'un `any` qui survive a une compilation Rust.
//! - La `Scope` d'Effect, qui desenregistre le crochet a la fermeture du
//!   programme, n'existe pas en Rust et n'est pas simulee : [`Registration`] est
//!   une unit struct. Aucun comportement de `groq.ts` n'est perdu, car la source
//!   ignore la valeur qu'elle recoit.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Le paquet npm que ce plugin reconnait.
///
/// C'est une litterale ecrite en dur dans la comparaison de la source. Elle
/// vaut aussi pour le nom du paquet et pour celui de la constante, et le test
/// de nommage verifie qu'il n'y a pas de faute de frappe.
pub const GROQ_PACKAGE: &str = "@ai-sdk/groq";

/// L'identifiant du plugin, le `id` de l'objet passe a `define`.
///
/// C'est sous cet identifiant que `PluginV2.add` enregistre le plugin.
pub const GROQ_PLUGIN_ID: &str = "groq";

/// Les options d'un SDK AI : `Record<string, any>`.
///
/// Meme traduction que dans `config_plugin.rs` : un dictionnaire libre devient
/// `BTreeMap<String, Value>`. On gagne un ordre de cles deterministe, on perd
/// l'ordre d'insertion de l'objet JavaScript.
pub type Options = BTreeMap<String, Value>;

/// La valeur d'un SDK AI.
///
/// En TS c'est `type SDK = any` : le coeur ne connait pas la forme de cette
/// valeur, seul le paquet npm qui l'a produite la connait. Le JSON est donc la
/// seule representation honnete d'un `any` en Rust.
pub type Sdk = Value;

/// L'evenement `sdk` que le service AISDK fait circuler entre les crochets.
///
/// Les trois premiers champs sont `readonly` dans le type d'origine : seul `sdk`
/// est modifiable, et c'est exactement ce que fait le crochet groq.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// `readonly model: ModelV2Info`.
    ///
    /// Le type n'est pas porte par ce fichier et le crochet ne le lit jamais.
    /// La valeur reste donc opaque, sans forme imposee, plutot que d'inventer
    /// une forme de modele qui divergerait du type reel.
    #[serde(rename = "model")]
    pub model: Value,

    /// `readonly package: string`. C'est le champ que le crochet compare a
    /// `@ai-sdk/groq`.
    #[serde(rename = "package")]
    pub package: String,

    /// `readonly options: Record<string, any>`, transmis tel quel, sans
    /// copie ni tri, a `createGroq`.
    #[serde(rename = "options")]
    pub options: Options,

    /// `sdk?: any`. La seule cle optionnelle de l'evenement, et le seul champ
    /// que le crochet ecrit.
    ///
    /// `Option` parce que la cle est absente tant qu'aucun crochet n'a repondu.
    /// `skip_serializing_if` parce que le TS declare `sdk?: any` : un evenement
    /// sur lequel aucun crochet n'a ecrit ne doit pas produire `"sdk": null`,
    /// que le JSON d'origine ne contient pas.
    #[serde(default, rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Sdk>,
}

/// Ce que rend l'enregistrement d'un crochet.
///
/// En TS, `Registration` ne porte qu'un `dispose` que la `Scope` appelle a la
/// fermeture. La `Scope` d'Effect n'existe pas en Rust et n'est pas simulee
/// ici, donc c'est une unit struct : la source ignore de toute facon la valeur
/// qu'elle recoit, puisque c'est la portee qui la conserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration;

/// Le contrat du contexte dont la source n'utilise qu'une methode.
///
/// En TS c'est `ctx.aisdk.sdk(crochet)`, c'est-a-dire une propriete de
/// `PluginContext` dont le type est `Hooks<{ sdk: ..., language: ... }>`. Seul
/// `sdk` est pris ici, parce que c'est le seul que la source appelle.
pub trait AisdkSdkHook {
    /// Enregistre le crochet et rend son inscription.
    ///
    /// Le parametre est passe par reference mutable : le service AISDK conserve
    /// ce crochet dans sa liste `sdkHooks` pour l'appeler sur chaque evenement,
    /// puis le retire de cette liste a la fermeture de la portee. C'est le
    /// comportement de `register` dans `packages/core/src/aisdk.ts`.
    fn sdk(&mut self, crochet: &mut dyn CrochetSdk) -> Registration;
}

/// Le corps d'un crochet enregistre.
///
/// En TS c'est une fonction generee par `Effect.fn`. Ici c'est un trait, parce
/// que l'hote doit pouvoir conserver le crochet, l'appeler pour chaque
/// evenement, puis le retirer de sa liste a la fermeture de la portee.
pub trait CrochetSdk {
    /// Le corps de `Effect.fn(function* (evt) { ... })`.
    fn on_sdk(&mut self, evt: &mut SdkEvent);
}

/// Le paquet `@ai-sdk/groq`, vu du plugin.
///
/// La source fait `import("@ai-sdk/groq")` puis `mod.createGroq(evt.options)`.
/// Ce chargement dynamique n'a pas d'equivalent Rust : il est injecte par
/// l'hote, qui detient deja le paquet npm.
pub trait GroqModule {
    /// `createGroq(options)` du paquet npm.
    ///
    /// Le resultat est un `Option` parce que `evt.sdk = ...` est une affectation
    /// non conditionnelle : une fabrique qui ne renverrait rien en JavaScript
    /// laisserait le champ a `None`, exactement comme le ferait `undefined`.
    fn create_groq(&self, options: &Options) -> Option<Sdk>;
}

/// Le crochet que le plugin groq enregistre sur l'evenement `sdk`.
pub struct CrochetGroq<'a, M: GroqModule + ?Sized> {
    module: &'a M,
}

impl<'a, M: GroqModule + ?Sized> CrochetGroq<'a, M> {
    /// Un crochet qui produit ses SDK avec le module fourni.
    pub fn new(module: &'a M) -> Self {
        Self { module }
    }
}

impl<'a, M: GroqModule + ?Sized> CrochetSdk for CrochetGroq<'a, M> {
    fn on_sdk(&mut self, evt: &mut SdkEvent) {
        // `!==` strict, donc comparaison exacte : la casse compte, et la chaine
        // vide ne correspond pas. Le retour vide sort du crochet avant le
        // chargement du module, donc `sdk` garde la valeur qu'il avait.
        if evt.package != GROQ_PACKAGE {
            return;
        }
        evt.sdk = self.module.create_groq(&evt.options);
    }
}

/// Le plugin groq : `export const GroqPlugin = define({ id, effect })`.
pub struct GroqPlugin;

impl GroqPlugin {
    /// L'identifiant enregistre par `PluginV2.add` : `id: "groq"`.
    pub const ID: &'static str = GROQ_PLUGIN_ID;

    /// Le paquet npm reconnu par ce plugin.
    pub const PACKAGE: &'static str = GROQ_PACKAGE;

    /// Le `effect` de la source : enregistre le crochet `sdk` et rend son
    /// inscription.
    ///
    /// La source enchaine un `yield*` sur `ctx.aisdk.sdk(...)`. Ce seul
    /// `yield*` est le comportement du plugin : rien d'autre n'est enregistre,
    /// et le resultat est jete au sol.
    pub fn effect<C, M>(ctx: &mut C, module: &M) -> Registration
    where
        C: AisdkSdkHook,
        M: GroqModule + ?Sized,
    {
        ctx.sdk(&mut CrochetGroq::new(module))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    /// Le paquet de test : note les options qu'il recoit et renvoie toujours la
    /// meme valeur, comme une fabrique sans etat.
    struct ModuleDeTest {
        options_recues: RefCell<Vec<Options>>,
        sdk: Sdk,
    }

    impl ModuleDeTest {
        fn nouveau(sdk: Sdk) -> Self {
            ModuleDeTest { options_recues: RefCell::new(Vec::new()), sdk }
        }
    }

    impl GroqModule for ModuleDeTest {
        fn create_groq(&self, options: &Options) -> Option<Sdk> {
            self.options_recues.borrow_mut().push(options.clone());
            Some(self.sdk.clone())
        }
    }

    /// Un module qui ne produit rien, comme une fabrique JavaScript qui
    /// renverrait `undefined`.
    struct ModuleVide;

    impl GroqModule for ModuleVide {
        fn create_groq(&self, _options: &Options) -> Option<Sdk> {
            None
        }
    }

    /// Contexte de test : compte les crochets enregistres, puis les applique
    /// aux evenements qu'on lui a confies, comme le fait le service AISDK.
    struct ContexteDeTest {
        crochets: usize,
        evenements: Vec<SdkEvent>,
    }

    impl AisdkSdkHook for ContexteDeTest {
        fn sdk(&mut self, crochet: &mut dyn CrochetSdk) -> Registration {
            self.crochets += 1;
            for evt in self.evenements.iter_mut() {
                crochet.on_sdk(evt);
            }
            Registration
        }
    }

    /// Un evenement de depart, sans sdk.
    fn evenement(package: &str) -> SdkEvent {
        SdkEvent {
            model: json!({ "id": "llama-3.3-70b-versatile" }),
            package: package.to_string(),
            options: BTreeMap::new(),
            sdk: None,
        }
    }

    /// Enregistre le plugin sur un contexte qui contient deja ses evenements,
    /// puis rend le contexte pour observer le resultat.
    fn enregistrer(evenements: Vec<SdkEvent>, module: &dyn GroqModule) -> ContexteDeTest {
        let mut ctx = ContexteDeTest { crochets: 0, evenements };
        GroqPlugin::effect(&mut ctx, module);
        ctx
    }

    // --------------------------------------------------------------- identite

    #[test]
    fn le_plugin_s_appelle_groq_et_reconnait_le_paquet_groq() {
        assert_eq!(GroqPlugin::ID, "groq");
        assert_eq!(GroqPlugin::PACKAGE, "@ai-sdk/groq");
    }

    #[test]
    fn le_plugin_enregistre_un_seul_crochet_et_aucun_autre() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));

        let ctx = enregistrer(Vec::new(), &module);

        assert_eq!(ctx.crochets, 1);
    }

    // --------------------------------------------------------- filtre sur paquet

    #[test]
    fn un_evenement_d_un_autre_paquet_ne_produit_aucun_sdk() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));

        let ctx = enregistrer(vec![evenement("@ai-sdk/openai")], &module);

        assert_eq!(ctx.evenements[0].sdk, None);
        assert_eq!(module.options_recues.borrow().len(), 0);
    }

    #[test]
    fn un_nom_de_paquet_vide_ne_declenche_rien() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));

        let ctx = enregistrer(vec![evenement("")], &module);

        assert_eq!(ctx.evenements[0].sdk, None);
        assert_eq!(module.options_recues.borrow().len(), 0);
    }

    #[test]
    fn le_nom_du_paquet_est_compare_sensiblement_a_la_casse() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));

        let ctx = enregistrer(vec![evenement("@ai-sdk/Groq")], &module);

        assert_eq!(ctx.evenements[0].sdk, None);
    }

    #[test]
    fn un_sdk_deja_present_est_conserve_pour_un_autre_paquet() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));
        let mut evt = evenement("@ai-sdk/openai");
        evt.sdk = Some(json!({ "provider": "openai" }));

        let ctx = enregistrer(vec![evt], &module);

        assert_eq!(ctx.evenements[0].sdk, Some(json!({ "provider": "openai" })));
    }

    // ------------------------------------------------------- sortie du crochet

    #[test]
    fn un_evenement_groq_remplace_le_sdk_par_celui_du_module() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));

        let ctx = enregistrer(vec![evenement(GROQ_PACKAGE)], &module);

        assert_eq!(ctx.evenements[0].sdk, Some(json!({ "provider": "groq" })));
    }

    #[test]
    fn un_sdk_deja_present_est_ecrase_pour_le_paquet_groq() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));
        let mut evt = evenement(GROQ_PACKAGE);
        evt.sdk = Some(json!({ "provider": "ancien" }));

        let ctx = enregistrer(vec![evt], &module);

        assert_eq!(ctx.evenements[0].sdk, Some(json!({ "provider": "groq" })));
    }

    #[test]
    fn un_module_qui_ne_produit_rien_laisse_le_champ_sdk_vide() {
        let ctx = enregistrer(vec![evenement(GROQ_PACKAGE)], &ModuleVide);

        assert_eq!(ctx.evenements[0].sdk, None);
    }

    // --------------------------------------------------------------- options

    #[test]
    fn les_options_de_l_evenement_sont_transmises_telles_elles_au_module() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));
        let mut evt = evenement(GROQ_PACKAGE);
        evt.options.insert("apiKey".to_string(), json!("gsk_de_test"));
        evt.options.insert("baseURL".to_string(), json!("https://api.groq.com"));

        enregistrer(vec![evt.clone()], &module);

        let recues = module.options_recues.borrow();
        assert_eq!(recues.len(), 1);
        assert_eq!(recues[0], evt.options);
    }

    #[test]
    fn des_options_vides_sont_transmises_comme_un_dictionnaire_vide() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));

        enregistrer(vec![evenement(GROQ_PACKAGE)], &module);

        let recues = module.options_recues.borrow();
        assert_eq!(recues[0].len(), 0);
    }

    #[test]
    fn le_crochet_ne_touche_pas_au_modele_de_l_evenement() {
        let module = ModuleDeTest::nouveau(json!({ "provider": "groq" }));
        let evt = evenement(GROQ_PACKAGE);

        let ctx = enregistrer(vec![evt.clone()], &module);

        assert_eq!(ctx.evenements[0].model, evt.model);
    }

    // --------------------------------------------------------- nommage JSON

    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut evt = evenement(GROQ_PACKAGE);
        evt.sdk = Some(json!({ "provider": "groq" }));

        let json = serde_json::to_value(&evt).unwrap();

        // Ni `mode`, ni `packagE`, ni `Options` : les quatre noms sont ceux de
        // `SDKEvent` dans `packages/core/src/aisdk.ts`.
        assert_eq!(
            json,
            json!({
                "model": { "id": "llama-3.3-70b-versatile" },
                "package": "@ai-sdk/groq",
                "options": {},
                "sdk": { "provider": "groq" },
            })
        );
        assert_eq!(json.as_object().unwrap().len(), 4);
    }

    #[test]
    fn un_evenement_sans_sdk_ne_serialise_pas_la_cle_sdk() {
        let evt = evenement(GROQ_PACKAGE);

        let json = serde_json::to_value(&evt).unwrap();

        assert!(json.get("sdk").is_none());
        assert_eq!(json.as_object().unwrap().len(), 3);
    }

    #[test]
    fn un_evenement_serialise_puis_relu_redonne_le_meme_evenement() {
        let mut evt = evenement(GROQ_PACKAGE);
        evt.options.insert("apiKey".to_string(), json!("gsk_de_test"));
        evt.sdk = Some(json!({ "provider": "groq" }));

        let json = serde_json::to_value(&evt).unwrap();
        let relu: SdkEvent = serde_json::from_value(json).unwrap();

        assert_eq!(relu, evt);
    }
}
