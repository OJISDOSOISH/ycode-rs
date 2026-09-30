//! Portage Rust de
//! `opencode/packages/core/src/plugin/provider/deepinfra.ts`.
//!
//! ## Ce que fait la source
//!
//! Quinze lignes, et **ce n'est pas un objet de configuration litteral** :
//! c'est un enregistrement de crochet. `define` (dans
//! `packages/core/src/plugin/internal.ts:59`) ne fait que renvoyer son
//! argument, donc `DeepInfraPlugin` est l'objet `{ id, effect }`.
//!
//! L'effet enregistre un unique crochet sur la liste `ctx.aisdk.sdk`, dont la
//! liste complete des evenements est dans `packages/core/src/aisdk.ts:12` sous
//! le nom `SDKEvent`. Le crochet fait deux choses, et seulement deux :
//!
//! 1. `deepinfra.ts:9` : `if (evt.package !== "@ai-sdk/deepinfra") return`.
//!    C'est un **test d'egalite stricte sur une chaine**, pas un test de
//!    veracite. Tout paquet qui n'est pas exactement `@ai-sdk/deepinfra` est
//!    laisse de cote, y compris la chaine vide.
//! 2. `deepinfra.ts:10-11` : chargement dynamique du paquet, puis
//!    `evt.sdk = mod.createDeepInfra(evt.options)`. Les options de l'evenement
//!    sont transmises **telles quelles**, sans tri, sans copie, sans valeur
//!    par defaut.
//!
//! ## Ce qui n'est volontairement pas porte
//!
//! - L'`import()` dynamique de `@ai-sdk/deepinfra` et l'appel a
//!   `createDeepInfra` : ce sont des liens vers un paquet JavaScript, pas une
//!   donnee. Les fabriquer en Rust serait inventer un SDK qui n'existe pas.
//!   Le nom de la fabrique est conserve dans `factory`, pour que la couche
//!   d'execution sache quoi appeler.
//! - L'objet `SDKEvent` lui-meme. Il est defini dans `aisdk.ts`, donc il
//!   appartient a un autre module du portage. Le recopier ici creerait une
//!   seconde verite pour le meme type. Ce que le plugin **lit** de l'evenement
//!   est `package` et `options`, et ce qu'il **ecrit** est `sdk` ; c'est tout,
//!   et ces trois noms sont cites ci-dessus pour que la relecture puisse les
//!   verifier contre `aisdk.ts:12`.
//! - Le type `ModelV2.Info` du champ `model`, qui n'est pas lu ici.
//!
//! ## Noms de champs
//!
//! Le seul champ que le TS declare lui-meme est `id`, en minuscules. Les deux
//! autres noms de la structure (`package`, `factory`) sont reprises telles
//! quelles, en minuscules egalement : **aucun `#[serde(rename)]` n'est
//! necessaire**, et surtout aucune difference de casse ne doit apparaitre.
//! Le test `les_noms_de_champs_json_sont_ceux_du_typescript` verifie les
//! trois cles au JSON, parce que c'est l'erreur la plus frequente de ce
//! portage et qu'elle est invisible de l'interieur du code Rust.
//!
//! ## Piege `?` contre `??`
//!
//! La source ne contient ni ternaire `?` ni coalescent `??`. Le seul test
//! present est `!==`, qui teste l'egalite et non la veracite. En JavaScript
//! `""` est falsy : un portage par veracite traiterait la chaine vide comme un
//! paquet absent et la laisserait passer. Ici la chaine vide est rejetee, donc
//! le comportement est identique a la source, et le test
//! `un_nom_de_paquet_vide_est_ignore` verrouille ce point.

use serde::{Deserialize, Serialize};

/// Identifiant du plugin, tel qu'il apparait dans la liste interne des plugins.
///
/// En TS : `id: "deepinfra"` (`deepinfra.ts:5`).
pub const ID: &str = "deepinfra";

/// Nom du paquet npm que ce plugin reconnait, et le seul.
///
/// En TS : la chaine comparee dans `if (evt.package !== "@ai-sdk/deepinfra")`
/// (`deepinfra.ts:9`) et le chemin de l'`import()` dynamique
/// (`deepinfra.ts:10`). Les deux occurrences sont identiques, ce qui est
/// verifie par le test qui impose l'egalite des deux constantes.
pub const SDK_PACKAGE: &str = "@ai-sdk/deepinfra";

/// Nom de la fabrique exportee par le paquet, appelee sur les options.
///
/// En TS : `mod.createDeepInfra(evt.options)` (`deepinfra.ts:11`). La casse
/// compte, `createDeepInfra` et non `createDeepinfra`.
pub const FACTORY: &str = "createDeepInfra";

/// Le plugin DeepInfra.
///
/// En TS : `export const DeepInfraPlugin = define({ id: "deepinfra", effect: ... })`.
///
/// La structure ne contient que ce que la source declare de fige. Le champ
/// `effect` de la source est un effet `Effect` qui enregistre un crochet dans
/// une liste interne : il n'a pas de representation en Rust et il n'est pas
/// represente ici. Ce qui reste, c'est l'identite du plugin et le predicat
/// que la source applique aux evenements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepInfraPlugin {
    /// Identifiant du plugin, identique a la cle de `ID`.
    ///
    /// Nom de champ TS : `id`. Minuscule, donc pas de `rename`.
    pub id: String,

    /// Paquet que le plugin reconnait, identique a la valeur de `SDK_PACKAGE`.
    ///
    /// Nom de champ TS : `package`, celui du crochet de l'evenement
    /// (`aisdk.ts:15`). Minuscule, donc pas de `rename`.
    pub package: String,

    /// Fabrique a appeler sur les options, identique a la valeur de `FACTORY`.
    ///
    /// Nom de champ TS : aucun, le TS appelle directement
    /// `mod.createDeepInfra`. Le nom est donc copie depuis le code, pas depuis
    /// une declaration de champ.
    pub factory: String,
}

impl DeepInfraPlugin {
    /// Construit le plugin avec les valeurs de la source, sans aucune autre
    /// possibilite : il n'y a rien a choisir.
    pub fn new() -> Self {
        Self {
            id: ID.to_string(),
            package: SDK_PACKAGE.to_string(),
            factory: FACTORY.to_string(),
        }
    }

    /// Le plugin reconnait-il cet evenement ?
    ///
    /// Equivalent direct de la garde `deepinfra.ts:9`. La reponse est `false`
    /// pour tout paquet autre que `@ai-sdk/deepinfra`, et le crochet sort alors
    /// sans rien ecrire sur l'evenement.
    ///
    /// La comparaison est une egalite de chaine, pas un test de veracite : une
    /// chaine vide, un prefixe, ou une difference de casse donnent tous `false`,
    /// exactement comme `!==` en JavaScript.
    pub fn handles(&self, package: &str) -> bool {
        package == self.package.as_str()
    }
}

impl Default for DeepInfraPlugin {
    /// `Default` rend la construction impossible a oublier, et donne le meme
    /// resultat que `new`, puisque la source n'offre aucune variante.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ------------------------------------------------------------- identite

    #[test]
    fn le_plugin_s_identifie_comme_deepinfra() {
        let plugin = DeepInfraPlugin::new();

        assert_eq!(plugin.id, "deepinfra");
        assert_eq!(plugin.package, "@ai-sdk/deepinfra");
        assert_eq!(plugin.factory, "createDeepInfra");
    }

    #[test]
    fn le_paquet_compare_et_le_paquet_importe_sont_le_meme() {
        // La source ecrit deux fois la meme chaine, une fois dans la garde et
        // une fois dans le chemin de l'import. Si les deux constantes
        // divergeaient, le plugin accepterait un paquet puis en chargerait
        // un autre.
        assert_eq!(SDK_PACKAGE, "@ai-sdk/deepinfra");
        assert_eq!(DeepInfraPlugin::new().package, SDK_PACKAGE);
    }

    // ------------------------------------------------------- noms de champs

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let json = serde_json::to_value(DeepInfraPlugin::new()).unwrap();

        assert_eq!(json["id"], "deepinfra", "le champ doit s'appeler id");
        assert_eq!(json["package"], "@ai-sdk/deepinfra", "le champ doit s'appeler package");
        assert_eq!(json["factory"], "createDeepInfra", "le champ doit s'appeler factory");

        // Aucune variante capitalisee ne doit apparaitre : le portage a deja
        // casse deux fois sur ce genre de difference, invisible de l'interieur.
        for interdit in ["Id", "ID", "Package", "Factory"] {
            assert!(json.get(interdit).is_none(), "{} ne doit pas apparaitre", interdit);
        }

        // Aucune cle parasite non plus : trois champs, trois cles.
        assert_eq!(json.as_object().unwrap().len(), 3);
    }

    #[test]
    fn un_plugin_relu_depuis_json_garde_exactement_les_memes_chaines() {
        let attendu = json!({
            "id": "deepinfra",
            "package": "@ai-sdk/deepinfra",
            "factory": "createDeepInfra"
        });

        let plugin: DeepInfraPlugin = serde_json::from_value(attendu.clone()).unwrap();

        assert_eq!(plugin, DeepInfraPlugin::new());
        assert_eq!(serde_json::to_value(&plugin).unwrap(), attendu);
    }

    // ------------------------------------------------- reconnaissance d'un paquet

    #[test]
    fn un_evenement_du_paquet_deepinfra_est_reconnu() {
        let plugin = DeepInfraPlugin::new();

        assert!(plugin.handles("@ai-sdk/deepinfra"));
        // Et par le chemin de la constante, qui est le vrai chemin du code.
        assert!(plugin.handles(SDK_PACKAGE));
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_est_ignore() {
        let plugin = DeepInfraPlugin::new();

        // Tous les autres plugins AICore comparent sur le leur, donc un evenement
        // qui les concerne ne doit pas etre touche ici.
        assert!(!plugin.handles("@ai-sdk/openai"));
        assert!(!plugin.handles("@ai-sdk/openai-compatible"));
        assert!(!plugin.handles("@ai-sdk/anthropic"));
        assert!(!plugin.handles("@ai-sdk/deepinfra-compatible"));
    }

    #[test]
    fn un_prefixe_commun_ne_suffit_pas_a_reconnaitre_le_paquet() {
        let plugin = DeepInfraPlugin::new();

        // `!==` compare toute la chaine, pas un debut de chaine.
        assert!(!plugin.handles("@ai-sdk/deepinfra-extra"));
        assert!(!plugin.handles("@ai-sdk/deepinfra-v2"));
        // La casse compte aussi : le nom du paquet est en minuscules.
        assert!(!plugin.handles("@ai-sdk/DeepInfra"));
        assert!(!plugin.handles("@ai-sdk/DEEPINFRA"));
    }

    #[test]
    fn un_nom_de_paquet_vide_est_ignore() {
        let plugin = DeepInfraPlugin::new();

        // Piege `?` contre `??` : en JavaScript `""` est falsy. Un portage par
        // veracite traiterait la chaine vide comme un paquet absent et la
        // laisserait passer vers la fabrique. La source utilise `!==`, donc la
        // chaine vide est rejetee, et c'est ce que fait `handles`.
        assert!(!plugin.handles(""));
    }

    // ------------------------------------------------------------ construction

    #[test]
    fn deux_plugins_construits_separement_sont_identiques() {
        let a = DeepInfraPlugin::new();
        let b = DeepInfraPlugin::default();

        assert_eq!(a, b);
        assert_eq!(serde_json::to_value(&a).unwrap(), serde_json::to_value(&b).unwrap());
    }

    #[test]
    fn la_source_ne_propose_qu_un_seul_plugin() {
        // La source n'expose pas de fabrique de plugin : `DeepInfraPlugin` est
        // une constante. Le choix des valeurs est donc ferme, et une copie
        // modifiee est la seule facon de s'en ecarter.
        let mut copie = DeepInfraPlugin::new();
        copie.package = "@ai-sdk/autre".to_string();

        assert_ne!(copie, DeepInfraPlugin::new());
        assert!(copie.handles("@ai-sdk/autre"));
        assert!(!copie.handles("@ai-sdk/deepinfra"));
    }
}
