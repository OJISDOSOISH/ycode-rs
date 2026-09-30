//! Portage Rust de `opencode/packages/core/src/plugin/provider/google.ts`.
//!
//! La source tient en quinze lignes et ne fait qu'une chose : declarer le plugin
//! Google, et lui faire enregistrer un unique crochet sur le domaine `aisdk.sdk`.
//!
//! Decomposition de la source :
//!
//! - `define({ id, effect })` vient de `../internal.ts`. C'est une fonction
//!   d'identite qui retourne l'objet qu'on lui donne sans le modifier. Il n'y a
//!   donc rien a porter la-dedans au-dela d'un identifiant et d'une fonction :
//!   voir `PLUGIN_ID` et `sdk_hook`.
//! - `effect` appelle `ctx.aisdk.sdk(handler)`. L'inscription elle-meme est faite
//!   par l'hote (`packages/plugin/src/v2/effect/registration.ts`) ; ce fichier ne
//!   fournit que le gestionnaire. Le domaine est donc une simple constante,
//!   `HOOK_DOMAIN`.
//! - Le gestionnaire teste `evt.package` contre le litteral `"@ai-sdk/google"`,
//!   charge dynamiquement le paquet `@ai-sdk/google`, puis ecrit
//!   `evt.sdk = mod.createGoogleGenerativeAI(evt.options)`.
//!
//! Ce que le portage ne peut pas faire, et qui est donc volontairement absent :
//!
//! - L'import dynamique et l'appel a la fabrique `createGoogleGenerativeAI`
//!   vivent dans du code JavaScript externe. Ce module n'a pas de systeme de
//!   modules JavaScript : il enregistre donc **quel** appel aurait lieu, dans
//!   `SdkFactoryCall`. L'execution de l'appel reste a la charge de l'appelant.
//! - `ctx` n'est pas reproduit. `PluginContext` (neuf domaines de crochets) est
//!   defini dans le paquet `@opencode-ai/plugin`, hors de ce lot. Le portage
//!   s'arrete donc a la fonction pure qui prend l'evenement.
//!
//! Pieges a ecarter explicitement :
//!
//! - **La garde est une comparaison stricte `!==`, pas une recherche de prefixe.**
//!   Le depot contient deux paquets voisins declares dans `google-vertex.ts` :
//!   `@ai-sdk/google-vertex` et `@ai-sdk/google-vertex/anthropic`. Un
//!   `starts_with`, un `contains` ou une comparaison de casse les accepteraient
//!   et casseraient Vertex. Test dedie.
//! - **Aucun ternaire `?` ni coalescent `??` dans la source.** La chaine vide
//!   n'a donc aucun traitement particulier : un `package` vide n'est simplement
//!   pas egal a `"@ai-sdk/google"`, le crochet ne fait rien. Il ne faut pas
//!   transformer la garde en test de veracite.
//! - **Noms de champs.** Les quatre cles de l'evenement (`model`, `package`,
//!   `options`, `sdk`) sont des mots uniques en minuscules : le nom Rust est deja
//!   identique au nom TypeScript, aucun `#[serde(rename)]` n'est requis, et
//!   `package` n'est pas un mot cle Rust donc pas d'identifiant brut non plus.
//!   La seule casse piegeuse du fichier est le nom de la fabrique, qui porte un
//!   `AI` en majuscules. Il est verrouille par un test.
//! - `model` est de type `ModelV2Info`, non porte par ce lot : le champ est garde
//!   en `serde_json::Value`. Ce modele contient des champs en camelCase majuscule
//!   du type `providerID` ; ils ne vivent pas ici, mais c'est le piege a
//!   surveiller le jour ou quelqu'un portera `ModelV2Info`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Champ `id` de l'objet passe a `define`.
pub const PLUGIN_ID: &str = "google";

/// Domaine sur lequel le plugin enregistre son unique crochet.
pub const HOOK_DOMAIN: &str = "aisdk.sdk";

/// Nom du paquet AI SDK reconnu par la garde, compare a `evt.package`.
pub const AI_SDK_PACKAGE: &str = "@ai-sdk/google";

/// Nom de la fabrique exportee par ce paquet, appelee avec `evt.options`.
///
/// La casse est celle du paquet JavaScript : `AI` en majuscules, jamais `Ai`.
pub const AI_SDK_FACTORY: &str = "createGoogleGenerativeAI";

/// Evenement recu par le crochet `aisdk.sdk`.
///
/// En TypeScript : la charge utile de `AISDKHooks["sdk"]`, declaree dans
/// `packages/plugin/src/v2/effect/aisdk.ts`, soit
/// `{ model, package, options, sdk? }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// Modele demande. Ce plugin ne le lit pas ; le champ est garde pour que
    /// l'evenement reste fidele a sa source.
    pub model: Value,

    /// Nom du paquet a charger. C'est ce champ que la garde compare.
    pub package: String,

    /// Options destinees a la fabrique, recues telles quelles.
    pub options: BTreeMap<String, Value>,

    /// Ce qu'un plugin a ecrit dans l'evenement. Absent a l'arrivee, present
    /// apres le passage d'un crochet qui reconnait le paquet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkFactoryCall>,
}

impl SdkEvent {
    /// Construit un evenement d'arrivee : aucun SDK encore construit.
    pub fn new(model: Value, package: impl Into<String>, options: BTreeMap<String, Value>) -> Self {
        Self {
            model,
            package: package.into(),
            options,
            sdk: None,
        }
    }
}

/// Ce qu'un crochet ecrit dans le champ `sdk` de l'evenement.
///
/// En TypeScript le champ est `sdk?: any` : une valeur vivante, produite par
/// `createGoogleGenerativeAI(evt.options)`, qui ne sort jamais en JSON. On garde
/// ici la trace de l'appel pour que le crochet soit testable sans le paquet
/// JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkFactoryCall {
    /// Nom de la fabrique appelee, tel qu'il est exporte par le paquet.
    pub factory: String,

    /// Options que la fabrique a recues, copiees depuis l'evenement.
    pub options: BTreeMap<String, Value>,
}

/// Gestionnaire enregistre par le plugin sur le domaine `aisdk.sdk`.
///
/// La source, traduite ligne a ligne :
///
/// ```text
/// Effect.fn(function* (evt) {
///   if (evt.package !== "@ai-sdk/google") return
///   const mod = yield* Effect.promise(() => import("@ai-sdk/google"))
///   evt.sdk = mod.createGoogleGenerativeAI(evt.options)
/// })
/// ```
///
/// Renvoie `true` si le SDK a ete construit, `false` si la garde a arrete le
/// gestionnaire. Aucune erreur n'est possible : la source ne leve rien et ne
/// renvoie pas d'erreur non plus.
pub fn sdk_hook(evt: &mut SdkEvent) -> bool {
    if evt.package != AI_SDK_PACKAGE {
        return false;
    }
    evt.sdk = Some(SdkFactoryCall {
        factory: AI_SDK_FACTORY.to_string(),
        options: evt.options.clone(),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modele de test. Sa forme reelle (`ModelV2Info`) n'est pas portee par ce
    /// lot, donc le champ reste une valeur JSON opaque. On y met un
    /// `providerID` en majuscules, volontairement : c'est la casse que le modele
    /// utilise, et ce module n'a pas a la renommer.
    fn model_de_test() -> Value {
        let mut model = serde_json::Map::new();
        model.insert("id".to_string(), Value::String(String::from("gemini-2.5-pro")));
        model.insert("providerID".to_string(), Value::String(String::from("google")));
        Value::Object(model)
    }

    fn options_de_test() -> BTreeMap<String, Value> {
        let mut options = BTreeMap::new();
        options.insert("apiKey".to_string(), Value::String(String::from("k")));
        options.insert(
            "baseURL".to_string(),
            Value::String(String::from("https://generativelanguage.googleapis.com")),
        );
        options
    }

    /// Le paquet exact est reconnu, la fabrique est appelee avec les options de
    /// l'evenement, et le resultat est bien ecrit dans le champ `sdk`.
    #[test]
    fn un_paquet_exact_construit_le_sdk_avec_les_options_recues() {
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE, options_de_test());

        let construit = sdk_hook(&mut evt);

        assert!(construit);
        let appel = evt.sdk.as_ref().expect("le sdk doit etre construit");
        assert_eq!(appel.factory, "createGoogleGenerativeAI");
        assert_eq!(appel.options, options_de_test());
    }

    /// Les paquets voisins declares dans `google-vertex.ts` ne doivent pas
    /// declencher la fabrique Google. C'est le piege principal du fichier : la
    /// garde est une egalite stricte, pas un prefixe, pas une casse insensible.
    #[test]
    fn un_paquet_voisin_au_prefixe_laisse_le_crochet_inerte() {
        for paquet in [
            "@ai-sdk/google-vertex",
            "@ai-sdk/google-vertex/anthropic",
            "@ai-sdk/google-beta",
            "@ai-sdk/google ",
            "@ai-sdk/Google",
            "google",
            "ai-sdk/google",
        ] {
            let mut evt = SdkEvent::new(model_de_test(), paquet, options_de_test());

            let construit = sdk_hook(&mut evt);

            assert!(!construit, "paquet accepte a tort : {paquet}");
            assert!(evt.sdk.is_none(), "sdk construit a tort : {paquet}");
        }
    }

    /// La garde compare des chaines, elle ne teste pas la veracite : un nom de
    /// paquet vide n'est ni un paquet manquant ni un paquet valide, il est
    /// simplement different. Le champ reste donc present et vide.
    #[test]
    fn un_paquet_vide_laisse_le_crochet_inerte_et_survit() {
        let mut evt = SdkEvent::new(model_de_test(), "", BTreeMap::new());

        let construit = sdk_hook(&mut evt);

        assert!(!construit);
        assert!(evt.sdk.is_none());
        assert_eq!(evt.package, "");

        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json.get("package").and_then(|v| v.as_str()), Some(""));
    }

    /// Les quatre cles de l'evenement doivent s'echanger sous exactement les
    /// noms du TypeScript, et `sdk` doit disparaitre tant qu'il est absent.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE, options_de_test());

        let json = serde_json::to_value(&evt).unwrap();
        let objet = json.as_object().unwrap();

        for nom in ["model", "package", "options"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert_eq!(objet.len(), 3);
        assert!(objet.get("sdk").is_none(), "sdk absent doit etre omis");
        assert_eq!(objet.get("package").and_then(|v| v.as_str()), Some("@ai-sdk/google"));

        let mut construit = evt;
        assert!(sdk_hook(&mut construit));
        let objet = serde_json::to_value(&construit).unwrap();
        let objet = objet.as_object().unwrap();

        assert_eq!(objet.len(), 4);
        let appel = objet.get("sdk").unwrap();
        let appel = appel.as_object().unwrap();
        assert_eq!(appel.get("factory").and_then(|v| v.as_str()), Some("createGoogleGenerativeAI"));
    }

    /// Un evenement venue du TypeScript se deserialise, puis le crochet le
    /// complete. Le champ `sdk` absent ne doit pas confondre le deserialiseur.
    #[test]
    fn un_evenement_recu_du_typescript_se_complete_par_le_crochet() {
        let mut evt: SdkEvent = serde_json::from_str(
            r#"{"model":{"id":"gemini-2.5-pro","providerID":"google"},"package":"@ai-sdk/google","options":{"apiKey":"k"}}"#,
        )
        .unwrap();

        assert!(evt.sdk.is_none());
        assert_eq!(evt.options.get("apiKey").and_then(|v| v.as_str()), Some("k"));
        assert_eq!(
            evt.model.get("providerID").and_then(|v| v.as_str()),
            Some("google")
        );

        assert!(sdk_hook(&mut evt));
        let appel = evt.sdk.as_ref().unwrap();
        assert_eq!(appel.options.get("apiKey").and_then(|v| v.as_str()), Some("k"));
    }

    /// Les options sont copiees au moment de la construction du SDK, donc
    /// modifier l'evenement apres coup ne touche pas l'appel deja enregistre.
    /// C'est une copie explicite de la reference JavaScript, qui elle est
    /// partagee.
    #[test]
    fn les_options_du_sdk_construit_sont_une_copie_independante() {
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE, options_de_test());

        assert!(sdk_hook(&mut evt));
        evt.options.insert(String::from("apiKey"), Value::String(String::from("z")));

        let appel = evt.sdk.as_ref().unwrap();
        assert_eq!(appel.options.get("apiKey").and_then(|v| v.as_str()), Some("k"));
        assert_eq!(evt.options.get("apiKey").and_then(|v| v.as_str()), Some("z"));
    }

    /// Un objet d'options JavaScript garde l'ordre d'insertion de ses cles. Le
    /// portage utilise `BTreeMap`, donc l'ordre devient alphabetique. Les
    /// donnees sont les memes, seule l'ordre des cles JSON differe : c'est
    /// arbitraire mais deterministe, contrairement a l'objet JavaScript dont
    /// l'ordre depend de l'ecriture.
    #[test]
    fn les_options_serialisees_sont_classees_par_ordre_alphabetique() {
        let mut options = BTreeMap::new();
        options.insert(String::from("zeta"), Value::Bool(true));
        options.insert(String::from("alpha"), Value::Bool(true));
        options.insert(String::from("mu"), Value::Bool(true));

        let json = serde_json::to_string(&options).unwrap();

        assert_eq!(json, r#"{"alpha":true,"mu":true,"zeta":true}"#);
    }
}
