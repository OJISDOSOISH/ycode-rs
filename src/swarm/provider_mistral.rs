//! Portage de `packages/core/src/plugin/provider/mistral.ts`.
//!
//! ## Ce que fait la source
//!
//! Quinze lignes, une seule chose. `MistralPlugin` est un plugin declare par
//! `define({ id: "mistral", effect })`, et son effet enregistre **un seul**
//! crochet sur `ctx.aisdk.sdk`. Ce crochet filtre sur le nom du paquet npm :
//! si `evt.package` n'est pas exactement `"@ai-sdk/mistral"`, il sort sans rien
//! faire ; sinon il importe le paquet et pose
//! `evt.sdk = mod.createMistral(evt.options)`.
//!
//! A comparer avec `vercel.ts`, qui enregistre en plus un `ctx.catalog.transform`
//! pour ajouter des en-tetes HTTP. Mistral ne touche pas au catalogue, ne pose
//! aucun en-tete, aucune `baseURL`, aucune valeur par defaut. Rien de plus.
//!
//! Les tests TypeScript `test/plugin/provider-mistral.test.ts` parlent d'un nom
//! de fournisseur `"mistral.chat"`, mais cette normalisation se fait dans
//! `core/src/aisdk.ts`, pas dans ce fichier. Elle n'est donc pas portee ici.
//!
//! ## Choix de portage
//!
//! - Le champ `effect` est une fonction : il n'a pas de representation JSON. Le
//!   plugin est donc decoupe en deux, une donnee et un comportement :
//!   `MistralPlugin` porte l'`id` enregistre, `on_sdk_event` porte l'effet.
//! - `createMistral` vient du paquet npm `@ai-sdk/mistral`, dont le code
//!   JavaScript n'a pas d'equivalent dans le depot Rust. On ne l'invente pas :
//!   la fabrique est **injectee**. `on_sdk_event` recoit donc une fonction qui
//!   prend `evt.options` et renvoie le SDK, ce qui est exactement le contrat de
//!   `createMistral` en JavaScript.
//! - `evt.sdk` vaut `any` en TypeScript. `serde_json::Value` est la traduction
//!   honnete de `any` : n'importe quoi, y compris `null`. Comme le champ est
//!   optionnel (`sdk?`), il devient `Option<Value>` et n'est pas serialise
//!   quand il est absent, ce qui reproduit l'absence de cle en JavaScript.
//! - `evt.model` est de type `ModelV2.Info`, un grand schema porte par un autre
//!   fichier du portage. Ce plugin ne le lit jamais, une seule fois, pour rien.
//!   Il est donc garde opaque en `Value` plutot que recopie.
//! - `ctx.aisdk.sdk(...)` renvoie un `Registration` que le gestionnaire de
//!   crochets conserve. Cette mecanique appartient a `registration.ts`, pas a
//!   ce fichier : elle n'est pas portee ici.
//!
//! ## Le piege `?` contre `??`
//!
//! Il ne se pose pas ici. La source ecrit `evt.package !== "@ai-sdk/mistral"`,
//! une egalite stricte, pas un ternaire. La casse compte, et la chaine vide ne
//! correspond a rien. Aucun test de veracite n'intervient, donc aucune chaine
//! vide ne disparait ou ne survit par accident.
//!
//! ## Noms de champs
//!
//! Les quatre noms de l'evenement sont des mots simples en minuscules : `model`,
//! `package`, `options`, `sdk`. Ils sont ecrits explicitement dans les
//! `#[serde(rename = ...)]` quand meme, et un test compare le JSON produit
//! champ par champ, parce que c'est le point de rupture habituel de ce portage.
//! Attention en particulier : `package` n'est pas un mot reserve de Rust, le
//! nom de champ Rust est bien `package`, exactement comme en TypeScript.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "mistral"` dans `define`, et c'est la chaine que
/// `PluginV2.ID.make(loaded.id)` utilise comme cle d'enregistrement.
pub const PLUGIN_ID: &str = "mistral";

/// Le seul nom de paquet npm auquel ce plugin repond.
pub const PACKAGE: &str = "@ai-sdk/mistral";

/// Le nom de la fabrique exportee par le paquet npm.
///
/// Sert uniquement a tracer l'appel JavaScript `mod.createMistral(...)`. Le code
/// de cette fabrique n'est pas dans le depot Rust : voir `on_sdk_event`.
pub const FACTORY: &str = "createMistral";

/// Le plugin Mistral, partie donnee.
///
/// En TypeScript, `define({ id: "mistral", effect })`. Le champ `effect` est une
/// fonction et n'a pas de place dans une structure serialisable : il est porte
/// par `on_sdk_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MistralPlugin {
    /// La valeur de `id`, telle qu'elle circule dans l'enregistrement.
    #[serde(rename = "id")]
    pub id: String,
}

impl MistralPlugin {
    /// Construit le plugin avec son identifiant officiel.
    pub fn new() -> Self {
        MistralPlugin {
            id: PLUGIN_ID.to_string(),
        }
    }
}

impl Default for MistralPlugin {
    fn default() -> Self {
        MistralPlugin::new()
    }
}

/// L'evenement recu par le crochet `sdk`.
///
/// En TypeScript, `SDKEvent` dans `core/src/aisdk.ts`, recopie dans
/// `AISDKHooks` de `packages/plugin/src/v2/effect/aisdk.ts`. Les trois premiers
/// champs sont `readonly` et le quatrieme est modifiable : ici c'est `&mut` sur
/// l'evenement entier, ce qui dit la meme chose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkHookEvent {
    /// Le modele demande. Jamais lu par ce plugin, donc garde opaque.
    #[serde(rename = "model")]
    pub model: Value,
    /// Le nom du paquet npm cherche. C'est le seul champ que le filtre lit.
    #[serde(rename = "package")]
    pub package: String,
    /// Les options a transmettre a la fabrique, sans reinterpretation.
    #[serde(rename = "options")]
    pub options: Value,
    /// Le SDK construit. Absent tant qu'aucun plugin n'en a pose un.
    #[serde(rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

/// Dit si ce plugin repond a ce nom de paquet.
///
/// La source ecrit `evt.package !== "@ai-sdk/mistral"` puis `return`. C'est une
/// egalite stricte : sensible a la casse, et la chaine vide ne correspond pas.
pub fn applies_to(package: &str) -> bool {
    package == PACKAGE
}

/// Le corps du crochet enregistre par le plugin.
///
/// `factory` tient lieu de `createMistral` du paquet npm `@ai-sdk/mistral`,
/// dont le code JavaScript n'a pas d'equivalent Rust. Elle recoit
/// `evt.options` et renvoie le SDK, comme la fabrique d'origine.
///
/// Si le nom de paquet ne correspond pas, la fabrique n'est pas appelee et
/// l'evenement reste intact. Si elle correspondait, un SDK deja present serait
/// remplace, comme le fait une affectation en JavaScript.
pub fn on_sdk_event<F>(event: &mut SdkHookEvent, factory: F)
where
    F: FnOnce(&Value) -> Value,
{
    if !applies_to(&event.package) {
        return;
    }
    let sdk = factory(&event.options);
    event.sdk = Some(sdk);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Un evenement minimal, avec le paquet demande et aucune instance SDK.
    fn evenement(package: &str) -> SdkHookEvent {
        SdkHookEvent {
            model: Value::Null,
            package: package.to_string(),
            options: serde_json::json!({ "name": "mistral" }),
            sdk: None,
        }
    }

    #[test]
    fn le_plugin_s_enregistre_sous_le_nom_mistral() {
        assert_eq!(MistralPlugin::new().id, "mistral");
        assert_eq!(MistralPlugin::default().id, "mistral");
        assert_eq!(serde_json::to_string(&MistralPlugin::new()).unwrap(), r#"{"id":"mistral"}"#);
    }

    #[test]
    fn le_seul_paquet_reconnu_est_celui_du_mistral() {
        assert!(applies_to("@ai-sdk/mistral"));
        assert!(applies_to(PACKAGE));
        assert!(!applies_to("@ai-sdk/openai"));
        assert!(!applies_to("@ai-sdk/openai-compatible"));
        assert!(!applies_to("@ai-sdk/mistral-provider"));
    }

    #[test]
    fn un_paquet_mistral_construit_le_sdk_avec_les_options_de_l_evenement() {
        let mut event = evenement("@ai-sdk/mistral");
        event.options = serde_json::json!({ "name": "mistral", "apiKey": "secret" });

        let mut recues: Option<Value> = None;
        on_sdk_event(&mut event, |options: &Value| {
            recues = Some(options.clone());
            options.clone()
        });

        assert_eq!(recues, Some(event.options.clone()));
        assert_eq!(event.sdk, Some(event.options.clone()));
    }

    #[test]
    fn un_paquet_qui_n_est_pas_mistral_laisse_le_sdk_absent() {
        let mut event = evenement("@ai-sdk/openai-compatible");
        on_sdk_event(&mut event, |_options: &Value| serde_json::json!({ "fabrique": "appelee" }));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn la_fabrique_n_est_jamais_appelee_pour_un_autre_paquet() {
        let mut event = evenement("@ai-sdk/anthropic");
        let appelee = Cell::new(false);
        on_sdk_event(&mut event, |_options: &Value| {
            appelee.set(true);
            Value::Null
        });
        assert!(!appelee.get());
    }

    #[test]
    fn la_casse_du_nom_de_paquet_compte() {
        let mut event = evenement("@AI-SDK/Mistral");
        on_sdk_event(&mut event, |_options: &Value| serde_json::json!({ "fabrique": "appelee" }));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn un_nom_de_paquet_vide_ne_declenche_pas_le_crochet() {
        let mut event = evenement("");
        on_sdk_event(&mut event, |_options: &Value| serde_json::json!({ "fabrique": "appelee" }));
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn un_sdk_deja_presente_est_remplace() {
        let mut event = evenement("@ai-sdk/mistral");
        event.sdk = Some(serde_json::json!("ancien"));
        on_sdk_event(&mut event, |_options: &Value| serde_json::json!("nouveau"));
        assert_eq!(event.sdk, Some(serde_json::json!("nouveau")));
    }

    #[test]
    fn les_options_vides_sont_transmises_telles_quelles() {
        let mut event = evenement("@ai-sdk/mistral");
        event.options = serde_json::json!({});
        let mut recues: Option<Value> = None;
        on_sdk_event(&mut event, |options: &Value| {
            recues = Some(options.clone());
            serde_json::json!("sdk")
        });
        assert_eq!(recues, Some(serde_json::json!({})));
    }

    #[test]
    fn les_noms_de_champs_de_l_evenement_sont_exacts() {
        let json = serde_json::to_string(&evenement("@ai-sdk/mistral")).unwrap();
        // `sdk` est `sdk?` en TypeScript et `Option<Value>` avec
        // `skip_serializing_if` en Rust : la cle est absente tant qu'aucun SDK
        // n'est construit. Les trois autres cles sortent dans l'ordre de la
        // structure, `serde_json::to_string` n'etant pas concerne par
        // `preserve_order` (qui ne joue que sur `Value`).
        assert_eq!(
            json,
            r#"{"model":null,"package":"@ai-sdk/mistral","options":{"name":"mistral"}}"#
        );

        // Des qu'un SDK existe, la cle porte bien le nom `sdk`, verbatim.
        let mut event = evenement("@ai-sdk/mistral");
        on_sdk_event(&mut event, |_options: &Value| serde_json::json!("fabrique"));
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""sdk":"fabrique""#), "cle sdk absente : {json}");
    }

    #[test]
    fn la_cle_sdk_est_absente_tant_qu_aucun_sdk_n_est_construit() {
        let json = serde_json::to_string(&evenement("@ai-sdk/openai")).unwrap();
        assert_eq!(json, r#"{"model":null,"package":"@ai-sdk/openai","options":{"name":"mistral"}}"#);
    }

    #[test]
    fn un_evenement_se_relit_depuis_le_json_du_typescript() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":{"id":"mistral-large"},"package":"@ai-sdk/mistral","options":{"apiKey":"secret"},"sdk":"marqueur"}"#,
        )
        .unwrap();
        assert_eq!(event.package, "@ai-sdk/mistral");
        assert_eq!(event.model, serde_json::json!({ "id": "mistral-large" }));
        assert_eq!(event.options, serde_json::json!({ "apiKey": "secret" }));
        assert_eq!(event.sdk, Some(serde_json::json!("marqueur")));
    }

    #[test]
    fn un_evenement_sans_sdk_se_relit_avec_un_sdk_absent() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":null,"package":"@ai-sdk/mistral","options":{}}"#,
        )
        .unwrap();
        assert_eq!(event.sdk, None);
        assert!(applies_to(&event.package));
    }
}