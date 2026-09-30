//! Portage Rust de `opencode/packages/core/src/plugin/provider/perplexity.ts`.
//!
//! La source fait quinze lignes et n'est **pas** un objet de configuration
//! litteral : c'est un plugin. Il ne fait que deux choses.
//!
//! 1. Il s'identifie, avec `id: "perplexity"`.
//! 2. Il enregistre **un seul** point d'accroche, `ctx.aisdk.sdk`, et dans le
//!    gestionnaire de ce point d'accroche :
//!    - si `evt.package` est different de `"@ai-sdk/perplexity"`, alors sortie
//!      immediate, sans rien modifier ;
//!    - sinon, chargement du paquet npm puis ecriture de
//!      `evt.sdk = mod.createPerplexity(evt.options)`.
//!
//! Ce qui n'est volontairement **pas** porte, et pourquoi :
//!
//! - L'enregistrement `yield* ctx.aisdk.sdk(...)` et le type `Effect` qui
//!   l'accompagne. Le point d'accroche est installe par `plugin/host.ts` et la
//!   forme exacte du payload est declaree dans
//!   `packages/plugin/src/v2/effect/aisdk.ts`. Ni l'un ni l'autre n'est dans ce
//!   lot. On exporte donc le gestionnaire lui-meme, `on_aisdk_sdk`, qui est le
//!   seul code reellement ecrit dans la source, et le point d'accroche est
//!   decrit par la constante `REGISTERED_HOOKS`.
//! - `import("@ai-sdk/perplexity")` puis `createPerplexity`. Le paquet npm
//!   **n'est pas installe** sur cette machine (ni lui, ni `@ai-sdk/provider`,
//!   verifie), et il n'a aucun equivalent Rust. On ne l'invente pas. Le champ
//!   `sdk` ne contient donc pas un SDK fonctionnel, mais la **trace de l'appel**
//!   que la source fait : le nom de la fabrique et les options recues. C'est
//!   une representation honnete du seul effet de ce fichier, et elle est
//!   testable.
//! - Le point d'accroche voisin `ctx.aisdk.language` n'est **pas** enregistre
//!   ici, meme si le type `AISDKHooks` en propose un. Un test le verrouille,
//!   parce que c'est le genre de detail qui se perd a la relecture.
//!
//! Piege des noms de champs : **sans objet sur ce fichier, et c'est verifie**.
//! Les quatre champs du payload sont `model`, `package`, `options` et `sdk`.
//! Ce sont des mots uniques entierement en minuscules : le nom Rust est deja
//! identique au nom TypeScript, donc aucun `#[serde(rename)]` n'est requis.
//! Et `package`, qui ressemble a un mot reserve, n'en est pas un en Rust. Un
//! test serialise le payload et compare les cles une a une, pour que la
//! garantie survive a toute evolution future du fichier.
//!
//! Piege `?` contre `??` : **sans objet non plus**. La source ne contient ni
//! ternaire ni coalescent. Le seul test de veracite est
//! `evt.package !== "@ai-sdk/perplexity"`, qui est une comparaison de chaine
//! stricte : une chaine vide est donc **rejetee**, et ne doit surtout pas etre
//! traitee comme un paquet absent que l'on tolerate. Un test dedie.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// En TS : `export const PerplexityPlugin = define({ id: "perplexity", ... })`.
///
/// C'est l'identifiant sous lequel le plugin est enregistre dans la liste
/// `ProviderPlugins` de `plugin/provider.ts`.
pub const PLUGIN_ID: &str = "perplexity";

/// Le seul paquet que ce plugin reconnait.
///
/// En TS : `if (evt.package !== "@ai-sdk/perplexity") return`. La comparaison
/// est stricte, exacte, sensible a la casse : c'est une egalite de chaine, pas
/// une recherche de prefixe.
pub const AI_SDK_PACKAGE: &str = "@ai-sdk/perplexity";

/// Nom de la fabrique appelee quand le paquet correspond.
///
/// En TS : `mod.createPerplexity(evt.options)` apres
/// `import("@ai-sdk/perplexity")`.
pub const PERPLEXITY_FACTORY: &str = "createPerplexity";

/// Les points d'accroche que ce plugin enregistre, dans l'ordre de la source.
///
/// En TS, le gestionnaire du plugin ne fait qu'un seul
/// `yield* ctx.aisdk.sdk(...)`, et rien d'autre. Le second crochet du type
/// `AISDKHooks`, `language`, n'est pas utilise par ce plugin. Cette constante
/// est la seule maniere honnete de rendre cette absence testable.
pub const REGISTERED_HOOKS: [&str; 1] = ["sdk"];

/// L'instance de SDK tel que la source le produit, sans la bibliotheque
/// sous-jacente.
///
/// En TS, `evt.sdk` recoit le retour de `createPerplexity(evt.options)`, donc
/// un objet vivant du paquet `@ai-sdk/perplexity`, que le TypeScript type
/// `any`. Ce paquet n'existe pas en Rust. plutot que d'inventer un SDK, on
/// conserve ce que la source fait reellement et qui est observable : le nom de
/// la fabrique et les options qui lui ont ete passees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkInstance {
    /// Nom de la fabrique qui a ete appelee, `"createPerplexity"`.
    pub factory: String,

    /// Les options recues, transmises telles quelles, sans triage ni filtrage.
    ///
    /// Cote source, `options` est construit par `prepareOptions` dans
    /// `core/src/aisdk.ts` et contient notamment la cle `name`, qui vaut
    /// l'identifiant du fournisseur du modele. Le plugin n'y touche pas.
    pub options: BTreeMap<String, Value>,
}

impl SdkInstance {
    /// Reproduit l'appel `mod.createPerplexity(options)`.
    ///
    /// La fabrique de la source ne transforme pas les options, elle les
    /// recoit. On les recopie donc telles quelles, et on retient le nom de la
    /// fabrique pour que l'appel reste identifiable de l'exterieur.
    pub fn create_perplexity(options: BTreeMap<String, Value>) -> Self {
        Self {
            factory: PERPLEXITY_FACTORY.to_string(),
            options,
        }
    }
}

/// Le payload de point d'accroche `aisdk.sdk`.
///
/// En TS : `AISDKHooks["sdk"]`, c'est a dire `{ model, package, options, sdk }`.
/// Le type est declare dans `packages/plugin/src/v2/effect/aisdk.ts`, qui n'est
/// pas dans ce lot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AisdkSdkEvent {
    /// Le modele pour lequel on cherche un SDK. Le type exact
    /// `ModelV2Info` vient du paquet `@opencode-ai/sdk`, absent de ce lot, donc
    /// la charge utile est laissee opaque.
    pub model: Value,

    /// Le paquet demande, par exemple `"@ai-sdk/perplexity"`.
    pub package: String,

    /// Les options a transmettre a la fabrique. Non modifiees par le plugin.
    pub options: BTreeMap<String, Value>,

    /// Le SDK cree, absent tant qu'aucun plugin ne l'a rempli.
    ///
    /// En TS : `sdk?: any`. Le champ est donc facultatif des deux cotes : il
    /// disparait du JSON tant qu'il est absent, et il se lit comme absent quand
    /// la cle manque a la deserialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkInstance>,
}

impl AisdkSdkEvent {
    /// Vrai si le paquet demande est exactement celui de ce plugin.
    ///
    /// Egalite stricte, comme dans la source. `"@ai-sdk/perplexity-compatible"`,
    /// `"perplexity"`, `"@ai-sdk/Perplexity"` et la chaine vide sont donc tous
    /// rejetes.
    pub fn is_perplexity_package(&self) -> bool {
        self.package == AI_SDK_PACKAGE
    }
}

/// Le gestionnaire enregistre sur le point d'accroche `aisdk.sdk`.
///
/// En TS :
///
/// ```text
/// yield* ctx.aisdk.sdk(Effect.fn(function* (evt) {
///   if (evt.package !== "@ai-sdk/perplexity") return
///   const mod = yield* Effect.promise(() => import("@ai-sdk/perplexity"))
///   evt.sdk = mod.createPerplexity(evt.options)
/// }))
/// ```
///
/// Le gestionnaire **modifie l'evenement sur place**, ce que la source fait
/// aussi : `evt.sdk` est une ecriture, pas un retour. Un evenement dont le
/// paquet ne correspond pas est rendu intact, et un evenement qui portait deja
/// un `sdk` voit ce `sdk` remplace, parce que la source ecrit sans condition.
pub fn on_aisdk_sdk(event: &mut AisdkSdkEvent) {
    if !event.is_perplexity_package() {
        return;
    }
    let options = event.options.clone();
    event.sdk = Some(SdkInstance::create_perplexity(options));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un evenement de test avec les options demandees.
    fn evenement(package: &str, options: &[(&str, &str)]) -> AisdkSdkEvent {
        let mut table = BTreeMap::new();
        for (cle, valeur) in options {
            table.insert(cle.to_string(), Value::String(valeur.to_string()));
        }
        AisdkSdkEvent {
            model: Value::String("perplexity/sonar".to_string()),
            package: package.to_string(),
            options: table,
            sdk: None,
        }
    }

    /// Un evenement demandant le paquet `@ai-sdk/perplexity` recoit un SDK
    /// construit par la bonne fabrique.
    #[test]
    fn un_evenement_du_paquet_perplexity_recoit_un_sdk() {
        let mut event = evenement(AI_SDK_PACKAGE, &[("name", "perplexity")]);

        on_aisdk_sdk(&mut event);

        let sdk = event.sdk.as_ref().expect("le SDK devrait etre cree");
        assert_eq!(sdk.factory, "createPerplexity");
        assert_eq!(
            sdk.options.get("name").and_then(|v| v.as_str()),
            Some("perplexity")
        );
    }

    /// Un paquet qui n'est pas exactement `@ai-sdk/perplexity` laisse
    /// l'evenement intact. Le second cas vient du test TypeScript
    /// `provider-perplexity.test.ts`, qui verifie que
    /// `"@ai-sdk/perplexity-compatible"` est bien ignore.
    #[test]
    fn un_paquet_different_du_paquet_perplexity_est_ignore() {
        for package in [
            "@ai-sdk/perplexity-compatible",
            "test-provider",
            "@ai-sdk/openai",
            "perplexity",
            "@ai-sdk/Perplexity",
            " @ai-sdk/perplexity",
            "@ai-sdk/perplexity ",
        ] {
            let mut event = evenement(package, &[("name", "perplexity")]);
            let avant = event.clone();

            on_aisdk_sdk(&mut event);

            assert!(
                event.sdk.is_none(),
                "un SDK ne doit pas etre cree pour le paquet {package}"
            );
            assert_eq!(event, avant, "l'evenement ne doit pas etre modifie");
        }
    }

    /// Le paquet demande est compare de maniere stricte : la chaine vide est
    /// rejettee, elle ne doit pas etre confuse avec un paquet absent que l'on
    /// laisserait passer.
    #[test]
    fn un_paquet_vide_est_rejette_comme_les_autres() {
        let mut event = evenement("", &[]);

        on_aisdk_sdk(&mut event);

        assert!(event.sdk.is_none());
        assert!(!event.is_perplexity_package());
    }

    /// Les options sont transmises a la fabrique telles quelles, y compris une
    /// chaine vide. Une chaine vide est une valeur presente, pas une absence.
    #[test]
    fn les_options_sont_transmises_telles_quelles_a_la_fabrique() {
        let mut event = evenement(
            AI_SDK_PACKAGE,
            &[("name", ""), ("baseURL", "https://api.perplexity.ai")],
        );
        let attendu = event.options.clone();

        on_aisdk_sdk(&mut event);

        let sdk = event.sdk.as_ref().expect("le SDK devrait etre cree");
        assert_eq!(sdk.options, attendu);
        assert_eq!(sdk.options.get("name").and_then(|v| v.as_str()), Some(""));
        assert_eq!(sdk.options.len(), 2);
    }

    /// Un evenement dont le `sdk` est deja rempli voit son `sdk` remplace : la
    /// source ecrit `evt.sdk` sans condition, elle ne complete pas.
    #[test]
    fn un_sdk_deja_present_est_remplace() {
        let mut event = evenement(AI_SDK_PACKAGE, &[]);
        event.sdk = Some(SdkInstance {
            factory: "createOpenAI".to_string(),
            options: BTreeMap::new(),
        });

        on_aisdk_sdk(&mut event);

        let sdk = event.sdk.as_ref().expect("le SDK devrait etre cree");
        assert_eq!(sdk.factory, "createPerplexity");
    }

    /// Les noms de champs serialises sont exactement ceux du TypeScript :
    /// `model`, `package`, `options`, `sdk`. Aucun `nPm`, aucun `api_key`.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut event = evenement(AI_SDK_PACKAGE, &[("name", "perplexity")]);
        let objet = serde_json::to_value(&event).unwrap();
        let champs = objet.as_object().unwrap();

        // `sdk` est facultatif : absent tant qu'aucun plugin ne l'a rempli.
        assert_eq!(champs.len(), 3);
        assert!(champs.contains_key("model"), "champ absent : model");
        assert!(champs.contains_key("package"), "champ absent : package");
        assert!(champs.contains_key("options"), "champ absent : options");
        assert!(!champs.contains_key("sdk"), "sdk ne doit pas apparaitre");

        on_aisdk_sdk(&mut event);
        let objet = serde_json::to_value(&event).unwrap();
        let champs = objet.as_object().unwrap();

        assert_eq!(champs.len(), 4);
        let sdk = champs.get("sdk").expect("sdk doit apparaitre une fois rempli");
        assert_eq!(sdk.get("factory").and_then(|v| v.as_str()), Some("createPerplexity"));
        assert!(sdk.get("options").is_some());

        // Et la lecture fait l'inverse, y compris quand la cle manque.
        let relu: AisdkSdkEvent =
            serde_json::from_value(serde_json::json!({"model": null, "package": "x", "options": {}}))
                .unwrap();
        assert!(relu.sdk.is_none());
    }

    /// Le plugin n'enregistre que le point d'accroche `sdk`. Le crochet
    /// `language` du type `AISDKHooks` n'est pas utilise par ce plugin.
    #[test]
    fn le_plugin_n_enregistre_que_le_point_accroche_sdk() {
        assert_eq!(REGISTERED_HOOKS.len(), 1);
        assert_eq!(REGISTERED_HOOKS[0], "sdk");
        assert!(!REGISTERED_HOOKS.contains(&"language"));
    }
}
