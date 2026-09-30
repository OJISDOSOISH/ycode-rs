//! Portage Rust de `opencode/packages/core/src/plugin/provider/openai-compatible.ts`.
//!
//! ## Ce que dit la source
//!
//! Dix-sept lignes, dont une seule de logique. Ce fichier est un **plugin de
//! fournisseur generique** : il ne parle a aucun fournisseur en particulier. Il
//! enregistre un seul crochet sur `ctx.aisdk.sdk`, et ce crochet fait deux
//! choses, dans cet ordre, pour tout modele dont le paquet AI SDK contient la
//! sous-chaine `@ai-sdk/openai-compatible` :
//!
//! 1. Il force `options.includeUsage = true`, sauf si la valeur deja presente
//!    est exactement le booleen `false`.
//! 2. Il construit le fournisseur avec `createOpenAICompatible(options)` et
//!    l'ecrit dans `evt.sdk`.
//!
//! Le crochet commence par `if (evt.sdk) return`. C'est une **reservation** :
//! les plugins sont enregistres dans un tableau (`provider.ts:36`) et
//! `aisdk.ts:179` les parcourt dans l'ordre, donc le premier arrive premier
//! servi. `cloudflare-workers-ai.ts:32` appelle le meme
//! `createOpenAICompatible` et est enregistre avant celui-ci : si Cloudflare a
//! deja rempli `evt.sdk`, ce plugin doit se taire. Le portage conserve ce
//! caractere "premier servi", c'est-a-dire que les gardes sont des sorties
//! normales, pas des erreurs.
//!
//! ## Choix de portage
//!
//! - **Le tiret reste dans les chaines, jamais dans les identifiants.** Le
//!   fichier source s'appelle `openai-compatible.ts`, d'ou le module Rust
//!   `provider_openai_compatible`. Mais `id: "openai-compatible"` et
//!   `"@ai-sdk/openai-compatible"` sont des **donnees** : elles sortent en JSON
//!   et se comparent a des chaines construites ailleurs. Lessoulins y
//!   casseraient tout. Trois tests verrouillent cela.
//!
//! - **Noms de champs.** Les trois champs de l'evenement sont
//!   `package`, `options`, `sdk` : deja en minuscules et en un seul mot dans
//!   le TS, donc aucun `#[serde(rename = ...)]` n'est necessaire, et un test
//!   le verifie quand meme. Le vrai nom sensible de ce fichier est la **cle**
//!   `includeUsage`, qui vit dans un dictionnaire libre : elle est portee par
//!   la constante `INCLUDE_USAGE_KEY` et testee explicitement. Ecrire
//!   `include_usage` dans ce dictionnaire serait le bug classique de ce lot.
//!
//! - **`!== false` n'est ni un ternaire ni un `??`.** C'est une inegalite
//!   stricte contre le litteral `false`. Donc `null`, `0`, `""`, `[]`, `{}` et
//!   `true` sont tous ecrases par `true`, et **seul** `false` survit. Un
//!   portage par `??` aurait laisse `null` en place, un portage par un test de
//!   veracite aurait laisse `0` et `""` en place. Deux tests le verrouillent.
//!
//! - **`evt.package.includes(...)` est un test de sous-chaine**, pas une
//!   egalite. Une chaine vide ne contient rien, donc le plugin ne s'applique
//!   pas a un nom de paquet vide : test dedie.
//!
//! - **`options` est `readonly` en TS mais le crochet le modifie.** Le
//!   `readonly` porte sur le *champ* de l'evenement, pas sur le *contenu* de
//!   l'objet : `evt.options.includeUsage = true` mute l'objet en place. D'ou
//!   ici un champ `options: BTreeMap<String, Value>` mutable, modifie sur
//!   place, et non reattribue. Une chaine vide reste une chaine vide : rien
//!   dans ce fichier ne teste une option en veracite a la JavaScript.
//!
//! - **Le `import()` dynamique n'a pas d'equivalent Rust.** Le paquet
//!   `@ai-sdk/openai-compatible` est du JavaScript ; `Cargo.toml` ne declare
//!   aucun SDK AI, et la regle du lot interdit d'y toucher. Le hook prend donc
//!   la **fabrique en parametre** : l'appelant fournit la fonction qui sait
//!   construire le SDK. Le test fournit une fabrique qui enregistre ce qu'elle
//!   recoit, ce qui permet de verifier que la fabrique voit bien les options
//!   *modifiees*.
//!
//! - **Le champ `model` n'est pas porte.** `AISDK.SDKEvent`
//!   (`aisdk.ts:12`) porte aussi `model: ModelV2.Info`, mais ce plugin ne le lit
//!   ni ne l'ecrit jamais. Le type n'est defini que dans `aisdk.ts`, qui n'est
//!   dans le lot de personne. Le definir ici serait inventer une forme. En
//!   revanche la lecture d'un evenement qui en porte un doit rester possible :
//!   Serde ignore les champs en trop, comme le decodeur de Schema, et un test
//!   le verifie.
//!
//! - **L'instance construite n'est pas serialisee.** En TS, `evt.sdk` est un
//!   objet vivant, avec des methodes (`sdk.languageModel(id)` est appele par
//!   `aisdk.ts:224`) : il ne survit pas a un aller-retour JSON. D'ou
//!   `#[serde(skip)]` sur le champ, et un `Sdk` qui n'en derive pas
//!   `Serialize`. Le `Sdk` Rust ne retient que l'identite de l'instance : le
//!   paquet et les options utilisees pour la construire. Ce sont des donnees,
//!   pas du comportement invente.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// `id` du plugin, tel que `provider.ts` l'enregistre.
///
/// Le tiret fait partie de la chaine. Il ne doit jamais devenir un soulin :
/// cette valeur est comparee a celle que le reste du code attend.
pub const PLUGIN_ID: &str = "openai-compatible";

/// Le paquet AI SDK que ce plugin reconnait, et qu'il importe pour construire.
pub const PACKAGE: &str = "@ai-sdk/openai-compatible";

/// La cle d'option forcee, avec sa casse d'origine : `includeUsage`.
///
/// C'est le nom de champ le plus sensible de ce fichier. Il n'est ni en
/// snake_case ni en PascalCase cote TS, et il vit dans un dictionnaire libre
/// dont les cles ne sont donc pas verifiees par le compilateur Rust.
pub const INCLUDE_USAGE_KEY: &str = "includeUsage";

/// Options du SDK AI : `Record<string, any>` en TS.
///
/// Les valeurs sont libres, donc `serde_json::Value` est la traduction exacte.
/// `BTreeMap` et non `HashMap` : on perd l'ordre d'insertion de l'objet
/// JavaScript, on gagne un ordre deterministe, ce qui est le choix retenu
/// partout dans ce portage.
pub type Options = BTreeMap<String, Value>;

/// L'instance de fournisseur construite par la fabrique.
///
/// En TS, `evt.sdk` recoit le retour de `createOpenAICompatible`, c'est-a-dire
/// un objet du SDK AI pourvu de methodes. Ce crate n'a pas le SDK AI, donc
/// seule l'identite de l'instance est portee : d'ou elle vient, et avec
/// quelles options. Aucune methode n'est inventee.
#[derive(Debug, Clone, PartialEq)]
pub struct Sdk {
    package: String,
    options: Options,
}

impl Sdk {
    /// Instance construite pour un paquet et un jeu d'options donnes.
    pub fn new(package: impl Into<String>, options: Options) -> Self {
        Self { package: package.into(), options }
    }

    /// Le paquet pour lequel l'instance a ete construite.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Les options utilisees pour la construire.
    pub fn options(&self) -> &Options {
        &self.options
    }
}

/// L'evenement que le service AISDK fait circuler dans ses crochets.
///
/// En TS : `AISDK.SDKEvent` (`aisdk.ts:12`), dont la forme cote plugin est
/// reprise dans `packages/plugin/src/v2/effect/aisdk.ts`.
///
/// Le champ `model` de l'interface d'origine n'est pas porte ici : ce plugin ne
/// le touche pas, et son type (`ModelV2.Info`) n'est porte par personne dans ce
/// lot. Le champ reste tolere a la lecture, voir le test correspondant.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// `readonly package: string`. Le paquet AI SDK du modele.
    ///
    /// Meme casse qu'en TS : deja en minuscules, un seul mot, donc pas de
    /// `#[serde(rename = ...)]`.
    pub package: String,
    /// `readonly options: Record<string, any>`.
    ///
    /// Le `readonly` du TS porte sur le champ, pas sur le contenu de l'objet :
    /// le crochet mute l'objet, il ne le reattribue pas. D'ou un champ
    /// modifiable sur place.
    #[serde(default)]
    pub options: Options,
    /// `sdk?: any`, la sortie du crochet.
    ///
    /// Ignore a la serialisation : en TS c'est un objet vivant, avec des
    /// methodes, il ne peut pas survivre a un aller-retour JSON.
    #[serde(default, skip)]
    pub sdk: Option<Sdk>,
}

impl SdkEvent {
    /// Evenement sans options et sans SDK, la forme de depart.
    pub fn new(package: impl Into<String>) -> Self {
        Self { package: package.into(), options: Options::new(), sdk: None }
    }

    /// Evenement avec des options deja preparees.
    pub fn with_options(package: impl Into<String>, options: Options) -> Self {
        Self { package: package.into(), options, sdk: None }
    }
}

/// Le plugin, equivalent de `export const OpenAICompatiblePlugin`.
///
/// Le nom TS est `OpenAICompatiblePlugin`. Il est ecrit ici
/// `OpenAiCompatiblePlugin` : c'est la forme Rust usuelle pour un sigle, et
/// aucun identifiant ne doit contenir de tiret.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenAiCompatiblePlugin;

impl OpenAiCompatiblePlugin {
    /// Le plugin, comme l'export TS le fournit.
    pub fn new() -> Self {
        Self
    }

    /// La valeur de `id`, tiret compris.
    pub fn id(&self) -> &'static str {
        PLUGIN_ID
    }

    /// Le paquet que ce plugin reconnait, tiret compris.
    pub fn package(&self) -> &'static str {
        PACKAGE
    }

    /// Le corps du hook enregistre dans `ctx.aisdk.sdk`.
    ///
    /// `create` est la fabrique : en TS c'est
    /// `mod.createOpenAICompatible(evt.options)`, obtenu par un `import()`
    /// dynamique d'un paquet JavaScript qui n'a pas d'equivalent Rust ici.
    ///
    /// La fabrique recoit les options **par valeur**, donc une copie de celles
    /// que l'evenement porte. En TS la fabrique est une fermeture qui garde
    /// une reference sur le meme objet que `evt.options`. Le comportement
    /// observable est le meme tant que personne ne modifie `evt.options` apres
    /// le retour du crochet, ce que le code source ne fait jamais.
    ///
    /// Renvoie `true` si le crochet a construit le SDK, `false` s'il s'est
    /// retire. Un retour `false` ne signifie pas une erreur : les deux gardes
    /// du TS sont des sorties normales.
    pub fn on_sdk_event<F>(&self, event: &mut SdkEvent, create: F) -> bool
    where
        F: FnOnce(Options) -> Sdk,
    {
        if event.sdk.is_some() {
            return false;
        }
        if !matches_package(&event.package) {
            return false;
        }
        force_include_usage(&mut event.options);
        let options = event.options.clone();
        event.sdk = Some(create(options));
        true
    }
}

/// L'export TS, sous forme de fonction.
///
/// En TS c'est une `const` qui tient un objet `{ id, effect }`. Ici l'objet
/// n'a plus rien a contenir : le crochet est une methode, il ne reste que
/// l'identite du plugin.
pub fn open_ai_compatible_plugin() -> OpenAiCompatiblePlugin {
    OpenAiCompatiblePlugin
}

/// `!evt.package.includes("@ai-sdk/openai-compatible")`, en positif.
///
/// Recherche de **sous-chaine**, comme `.includes` en JavaScript, et non
/// egalite. C'est volontaire cote TS : d'autres plugins font le meme test par
/// sous-chaine sur la meme chaine (`google-vertex.ts:89`).
///
/// Une chaine vide ne contient aucune sous-chaine non vide : le plugin ne
/// s'applique donc pas a un nom de paquet vide.
pub fn matches_package(package: &str) -> bool {
    package.contains(PACKAGE)
}

/// `evt.options.includeUsage !== false`, en positif.
///
/// Ce n'est **pas** un test de veracite, et ce n'est **pas** un `??`. C'est une
/// inegalite stricte contre le litteral `false`. Le seul cas ou elle est fausse
/// est donc : la cle vaut le booleen `false`.
pub fn include_usage_is_forced(options: &Options) -> bool {
    !matches!(options.get(INCLUDE_USAGE_KEY), Some(Value::Bool(false)))
}

/// `if (evt.options.includeUsage !== false) evt.options.includeUsage = true`.
///
/// Ecrit `true` dans les options sauf si la cle vaut deja le booleen `false`,
/// auquel cas elle est laissee telle quelle. Les autres options ne sont pas
/// touchees.
pub fn force_include_usage(options: &mut Options) {
    if include_usage_is_forced(options) {
        options.insert(INCLUDE_USAGE_KEY.to_string(), Value::Bool(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Construit un dictionnaire d'options a partir de paires littegrales.
    fn map(paires: &[(&str, Value)]) -> Options {
        let mut options = Options::new();
        for (cle, valeur) in paires {
            options.insert((*cle).to_string(), valeur.clone());
        }
        options
    }

    // --------------------------------------------------- identifiants et tirets

    #[test]
    fn l_identifiant_du_plugin_garde_son_tiret() {
        let plugin = open_ai_compatible_plugin();

        assert_eq!(plugin.id(), "openai-compatible");
        assert_eq!(plugin.id(), PLUGIN_ID);
        assert!(plugin.id().contains('-'), "le tiret fait partie de la donnee");
        assert!(!plugin.id().contains('_'), "le tiret ne doit pas devenir un soulin");
    }

    #[test]
    fn le_paquet_reconnu_garde_son_tiret_et_son_prefixe() {
        assert_eq!(PACKAGE, "@ai-sdk/openai-compatible");
        assert!(PACKAGE.contains("-compatible"), "le tiret fait partie de la donnee");
        assert!(!PACKAGE.contains('_'), "le tiret ne doit pas devenir un soulin");
    }

    #[test]
    fn la_detection_du_paquet_cherche_une_sous_chaine() {
        // `.includes` n'est pas une egalite : le test est deliberement plus
        // large que le paquet seul.
        assert!(matches_package(PACKAGE));
        assert!(matches_package("@ai-sdk/openai-compatible-extra"));
        assert!(matches_package("prefixe-@ai-sdk/openai-compatible"));
        assert!(!matches_package(""), "une chaine vide ne contient rien");
        assert!(!matches_package("@ai-sdk/openai"), "le paquet voisin ne convient pas");
        assert!(!matches_package("@ai-sdk/azure"));
        assert!(!matches_package("openai-compatible"), "le prefixe @ai-sdk/ compte");
    }

    // ------------------------------------------------ les deux gardes du crochet

    #[test]
    fn un_paquet_non_reconnu_laisse_l_evenement_intact() {
        let plugin = open_ai_compatible_plugin();
        let mut event = SdkEvent::new("@ai-sdk/openai");
        let mut fabrique_appelee = false;

        let touche = plugin.on_sdk_event(&mut event, |options| {
            fabrique_appelee = true;
            Sdk::new(PACKAGE, options)
        });

        assert!(!touche, "le crochet doit signaler qu'il n'a rien fait");
        assert!(!fabrique_appelee, "la fabrique ne doit pas etre appelee");
        assert!(event.sdk.is_none());
        assert!(event.options.is_empty(), "les options ne doivent pas etre touchees");
    }

    #[test]
    fn un_sdk_deja_presente_laisse_l_evenement_intact() {
        // C'est tout l'objet du `if (evt.sdk) return` : premier arrive, premier
        // servi. `cloudflare-workers-ai.ts` construit le meme SDK et passe
        // avant ce plugin dans `provider.ts`.
        let plugin = open_ai_compatible_plugin();
        let premier = Sdk::new("autre-plugin", Options::new());
        let mut event = SdkEvent::with_options(PACKAGE, map(&[("name", json!("groq"))]));
        event.sdk = Some(premier.clone());
        let mut fabrique_appelee = false;

        let touche = plugin.on_sdk_event(&mut event, |options| {
            fabrique_appelee = true;
            Sdk::new(PACKAGE, options)
        });

        assert!(!touche);
        assert!(!fabrique_appelee, "un autre plugin a deja construit le SDK");
        assert_eq!(event.sdk, Some(premier), "le SDK existant doit rester intact");
        assert!(
            event.options.get(INCLUDE_USAGE_KEY).is_none(),
            "un evenement deja servi ne doit plus etre modifie"
        );
    }

    // -------------------------------------------- includeUsage, le piege central

    #[test]
    fn une_option_absente_est_forcee_a_vrai() {
        let mut options = Options::new();

        force_include_usage(&mut options);

        assert_eq!(options.get(INCLUDE_USAGE_KEY), Some(&Value::Bool(true)));
        assert_eq!(options.len(), 1);
    }

    #[test]
    fn la_valeur_false_est_la_seule_qui_survit() {
        // `!== false` est une inegalite stricte contre le litteral `false` :
        // c'est le seul cas ou la condition est fausse.
        let mut options = map(&[("includeUsage", Value::Bool(false))]);

        assert!(!include_usage_is_forced(&options));
        force_include_usage(&mut options);
        assert_eq!(options.get(INCLUDE_USAGE_KEY), Some(&Value::Bool(false)));
    }

    #[test]
    fn une_valeur_absorbante_vaut_mieux_que_faux() {
        // Piege `?` contre `??` : un portage par `??` aurait laisse `null` en
        // place, un portage par un test de veracite aurait laisse `0` et `""`.
        // Ici tout ce qui n'est pas le booleen `false` est ecrase par `true`.
        for valeur in [json!(null), json!(true), json!(0), json!(""), json!([]), json!({}), json!(42)] {
            let mut options = map(&[("includeUsage", valeur.clone())]);

            assert!(include_usage_is_forced(&options), "{} devrait etre force a vrai", valeur);
            force_include_usage(&mut options);
            assert_eq!(
                options.get(INCLUDE_USAGE_KEY),
                Some(&Value::Bool(true)),
                "valeur initiale : {}",
                valeur
            );
        }
    }

    #[test]
    fn les_autres_options_sont_conservees_et_classees() {
        let mut options = map(&[
            ("name", json!("groq")),
            ("baseURL", json!("https://exemple.test")),
        ]);

        force_include_usage(&mut options);

        assert_eq!(options.len(), 3);
        assert_eq!(options["name"], json!("groq"));
        assert_eq!(options["baseURL"], json!("https://exemple.test"));
        // Un objet JavaScript garde l'ordre d'insertion, un `BTreeMap` impose
        // l'ordre lexicographique. C'est le choix retenu partout dans ce lot.
        let cles: Vec<&str> = options.keys().map(|cle| cle.as_str()).collect();
        assert_eq!(cles, vec!["baseURL", "includeUsage", "name"]);
    }

    #[test]
    fn l_ordre_des_options_ne_depend_pas_de_l_ordre_d_entree() {
        let mut avant = map(&[("name", json!("a")), ("apiKey", json!("b")), ("baseURL", json!("c"))]);
        let mut apres = map(&[("baseURL", json!("c")), ("apiKey", json!("b")), ("name", json!("a"))]);

        force_include_usage(&mut avant);
        force_include_usage(&mut apres);

        assert_eq!(avant, apres);
        assert_eq!(
            serde_json::to_string(&avant).unwrap(),
            serde_json::to_string(&apres).unwrap()
        );
    }

    // ------------------------------------------- noms de champs a l'echange JSON

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let event = SdkEvent::with_options(PACKAGE, map(&[("includeUsage", Value::Bool(false))]));

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["package"], json!(PACKAGE), "le champ doit s'appeler package");
        assert!(json.get("Package").is_none(), "la casse ne doit pas changer");
        assert_eq!(json["options"]["includeUsage"], json!(false), "la cle doit garder sa casse");
        assert!(json["options"].get("IncludeUsage").is_none(), "la casse ne doit pas changer");
        assert!(json["options"].get("include_usage").is_none(), "le soulin n'existe pas en TS");
        // L'instance construite est un objet vivant cote TS, avec des methodes :
        // elle ne fait pas partie du JSON.
        assert!(json.get("sdk").is_none(), "le SDK vivant n'est pas une donnee JSON");
        assert!(json.get("Sdk").is_none());
    }

    #[test]
    fn un_evenement_vide_a_les_valeurs_par_defaut() {
        let evenement = SdkEvent::default();

        assert_eq!(evenement.package, "");
        assert!(evenement.options.is_empty());
        assert!(evenement.sdk.is_none());
        assert_eq!(evenement, SdkEvent::new(""));
    }

    #[test]
    fn un_evenement_sans_nom_de_paquet_est_refuse() {
        // `readonly package: string` est obligatoire en TS, donc aussi ici. Ce
        // champ n'est donc pas marque `default`, contrairement a `options` et a
        // `sdk` qui sont optionnels.
        assert!(serde_json::from_value::<SdkEvent>(json!({ "options": {} })).is_err());
        assert!(serde_json::from_value::<SdkEvent>(json!({ "package": "" })).is_ok());
    }

    #[test]
    fn un_evenement_qui_porte_un_modele_se_lit_quand_meme() {
        // `AISDK.SDKEvent` porte aussi `model: ModelV2.Info`, que ce plugin ne
        // lit ni n'ecrit jamais, et dont le type n'est porte par personne dans
        // ce lot. Le champ n'est donc pas dans la struct, mais sa presence ne
        // doit pas faire echouer la lecture.
        let brut = json!({
            "model": { "id": "llama-3", "providerID": "groq" },
            "package": PACKAGE,
            "options": { "name": "groq" },
        });

        let event: SdkEvent = serde_json::from_value(brut).unwrap();

        assert_eq!(event.package, PACKAGE);
        assert!(event.sdk.is_none());
        assert_eq!(event.options["name"], json!("groq"));
    }

    // ------------------------------------------------------- le crochet complet

    #[test]
    fn la_fabrique_recoit_les_options_modifiees() {
        let plugin = open_ai_compatible_plugin();
        let mut event = SdkEvent::with_options(PACKAGE, map(&[("name", json!("groq"))]));

        let touche = plugin.on_sdk_event(&mut event, |options| Sdk::new(PACKAGE, options));

        assert!(touche, "le paquet correspond et aucun SDK n'etait present");
        let sdk = event.sdk.expect("le SDK doit avoir ete construit");
        assert_eq!(sdk.package(), PACKAGE);
        // La fabrique voit les options APRES la modification de includeUsage,
        // comme en TS, ou c'est le meme objet qui est passe a la fabrique.
        assert_eq!(sdk.options()[INCLUDE_USAGE_KEY], Value::Bool(true));
        assert_eq!(sdk.options().len(), 2);
        assert_eq!(event.options[INCLUDE_USAGE_KEY], Value::Bool(true));
    }

    #[test]
    fn un_nom_de_paquet_vide_ne_declenche_pas_le_plugin() {
        // Piege `?` contre `??` : le test du TS est une recherche de
        // sous-chaine, pas une question de nullite. Une chaine vide ne
        // contient rien, le plugin s retire.
        let plugin = open_ai_compatible_plugin();
        let mut event = SdkEvent::new("");

        let touche = plugin.on_sdk_event(&mut event, |options| Sdk::new(PACKAGE, options));

        assert!(!touche);
        assert!(event.sdk.is_none());
        assert!(event.options.is_empty(), "rien ne doit etre ajoute a des options vides");
    }
}
