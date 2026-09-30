//! Portage Rust de `opencode/packages/core/src/config/plugin.ts`.
//!
//! Fichier minuscule cote TypeScript : treize lignes, dont dix de schema pur.
//! Il ne contient ni fonction ni effet, seulement deux formes de donnees et
//! l'alias de tableau qui les regroupe. On ne rajoute donc pas de mecanique :
//! le comportement tient dans la forme des donnees et dans la maniere dont le
//! JSON entre et sort.
//!
//! ## Ce que dit la source
//!
//! Une entree de plugin est un objet `{ package, options? }` ou `options` est
//! un dictionnaire de valeurs libres. Un plugin est soit cette entree, soit une
//! simple chaine de caracteres (forme courte, la plus utilisee en pratique :
//! `"plugin: [\"opencode-plugin-foo\"]"`). `Plugins` est le tableau de ces
//! deux formes, place dans la configuration sous la cle `plugins`.
//!
//! ## Choix de portage
//!
//! - L'union du TS est **non taggee** : elle est decoupee par la forme du JSON,
//!   pas par un discriminant. `#[serde(untagged)]` fait exactement cela. L'ordre
//!   des variantes suit l'ordre du TS : une chaine d'abord, une entree ensuite.
//! - Les noms de champs sont deja identiques entre le TS et Rust
//!   (`package`, `options`), donc aucun `#[serde(rename = ...)]` n'est necessaire
//!   ici. Un test verifie quand meme les noms au JSON, parce que c'est
//!   l'erreur la plus frequente de ce portage.
//! - `Schema.Record(Schema.String, Schema.Unknown)` devient un
//!   `BTreeMap<String, Value>`. On perd l'ordre d'insertion des cles de
//!   l'objet JavaScript, on gagne un ordre deterministe, ce qui est le choix
//!   retenu partout dans ce portage. Les valeurs, elles, sont conservees telles
//!   quelles : `Value` accepte nombre, booleen, null, tableau, objet imbrique.
//! - `Schema.optional` sur `options` devient `Option<...>` avec
//!   `skip_serializing_if = "Option::is_none"`, pour qu'une entree sans options
//!   ne serialise pas une cle `options: null` que le TS ne produirait pas.
//! - Une chaine vide reste une chaine vide. Rien ici n'est teste en veracite
//!   comme le ferait un ternaire JavaScript : `""` est un nom de plugin
//!   parfaitement valide, et il doit survivre a l'aller-retour JSON.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Options d'un plugin : dictionnaire libre, cle en chaine, valeur quelconque.
///
/// Le TS ecrit `Schema.Record(Schema.String, Schema.Unknown)`. `Unknown` n'a
/// aucune contrainte, donc `serde_json::Value` est la traduction exacte, sans
/// perte et sans inventer de type.
pub type Options = BTreeMap<String, Value>;

/// Entree complete de plugin : un nom de paquet, et des options libres.
///
/// En TS : `export class Entry extends Schema.Class<Entry>("ConfigV2.Plugin.Entry")`.
/// Le nom de schema `"ConfigV2.Plugin.Entry"` n'a pas d'equivalent utile en
/// Rust : il sert au rapport d'erreur du decodeur TS, que Serde ne produit pas.
/// Il est donc perdu, comme dans tous les autres `Schema.Class` du portage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Nom du paquet a charger. Champ obligatoire cote TS, donc ici aussi.
    pub package: String,
    /// Options passees au plugin. Absent si rien n'est configure.
    ///
    /// `Schema.optional` en TS autorise l'absence de la cle. Serde resout cela
    /// par `default` plus `skip_serializing_if`, les deux etant necessaires :
    /// le premier pour lire une entree sans la cle, le second pour ne pas ecrire
    /// une cle `null` absente du format d'origine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Options>,
}

impl Entry {
    /// Entree sans options, la forme la plus courte.
    pub fn new(package: impl Into<String>) -> Self {
        Self { package: package.into(), options: None }
    }

    /// Entree avec options.
    pub fn with_options(package: impl Into<String>, options: Options) -> Self {
        Self { package: package.into(), options: Some(options) }
    }

    /// Les options, ou un dictionnaire vide si la cle est absente.
    ///
    /// Pratique pour lire sans faire de test a chaque acces. La distinction
    /// entre "pas d'options" et "options vides" reste perdue ici, comme elle
    /// l'est en TS : `Schema.optional` ne distingue pas `undefined` de `{}`.
    pub fn options_or_empty(&self) -> Options {
        self.options.clone().unwrap_or_default()
    }
}

/// Un plugin de configuration, sous ses deux formes.
///
/// En TS : `Schema.Union([Schema.String, Entry])`. Union **non taggee** : les
/// deux formes se distinguent par leur type JSON, une chaine d'un objet. Il
/// n'existe aucun champ discriminant a preserver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Plugin {
    /// Forme courte : le nom du paquet, ecrit directement comme chaine.
    Name(String),
    /// Forme longue : un objet `{ package, options? }`.
    Entry(Entry),
}

impl Plugin {
    /// Plugin en forme courte.
    pub fn name(package: impl Into<String>) -> Self {
        Plugin::Name(package.into())
    }

    /// Plugin en forme longue, sans options.
    pub fn entry(package: impl Into<String>) -> Self {
        Plugin::Entry(Entry::new(package))
    }

    /// Le nom du paquet, quelle que soit la forme.
    ///
    /// Les deux variantes portent la meme information : c'est le seul moyen de
    /// charger le plugin, les options n'en sont que l'argument. Cette lecture
    /// commune evite au reste du code de faire un `match` a chaque usage.
    pub fn package_name(&self) -> &str {
        match self {
            Plugin::Name(name) => name,
            Plugin::Entry(entry) => &entry.package,
        }
    }

    /// Les options, ou `None` pour la forme courte et pour une entree sans
    /// options. La forme courte ne peut pas en avoir : elle n'a pas de place
    /// pour les ecrire.
    pub fn options(&self) -> Option<&Options> {
        match self {
            Plugin::Name(_) => None,
            Plugin::Entry(entry) => entry.options.as_ref(),
        }
    }
}

/// Liste de plugins, placee dans la configuration sous la cle `plugins`.
///
/// En TS : `export const Plugins = Plugin.pipe(Schema.Array)`. Le tableau est
/// ordonne et l'ordre est significant : les plugins sont charges dans l'ordre
/// d'ecriture du fichier de configuration. Un simple `Vec` preserve cet ordre.
pub type Plugins = Vec<Plugin>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ------------------------------------------------------------ formes de base

    #[test]
    fn une_chaine_de_plugin_se_lit_comme_un_nom_de_paquet() {
        let json = json!("opencode-plugin-foo");

        let plugin: Plugin = serde_json::from_value(json).unwrap();

        assert_eq!(plugin, Plugin::Name("opencode-plugin-foo".to_string()));
        assert_eq!(plugin.package_name(), "opencode-plugin-foo");
    }

    #[test]
    fn un_objet_avec_package_se_lit_comme_une_entree() {
        let json = json!({ "package": "opencode-plugin-foo", "options": { "verbose": true } });

        let plugin: Plugin = serde_json::from_value(json).unwrap();

        match &plugin {
            Plugin::Entry(entry) => {
                assert_eq!(entry.package, "opencode-plugin-foo");
                assert_eq!(entry.options_or_empty().get("verbose"), Some(&Value::Bool(true)));
            }
            autre => panic!("l'objet aurait du donner une entree, pas : {:?}", autre),
        }
    }

    #[test]
    fn une_entree_sans_options_a_la_forme_la_plus_courte() {
        let plugin = Plugin::entry("opencode-plugin-foo");

        let json = serde_json::to_value(&plugin).unwrap();

        // L'absence d'options ne doit produire aucune cle, pas meme `null` :
        // le TS n'ecrit jamais `options: null`.
        assert_eq!(json, json!({ "package": "opencode-plugin-foo" }));
        assert!(json.get("options").is_none());
    }

    // ------------------------------------------------------------------ listes

    #[test]
    fn une_liste_vide_de_plugins_donne_une_liste_vide() {
        let plugins: Plugins = serde_json::from_value(json!([])).unwrap();

        assert!(plugins.is_empty());
        assert_eq!(serde_json::to_value(&plugins).unwrap(), json!([]));
    }

    #[test]
    fn une_liste_ordonnee_conserve_l_ordre_du_fichier_de_configuration() {
        let json = json!(["a", { "package": "b" }, "c"]);

        let plugins: Plugins = serde_json::from_value(json.clone()).unwrap();

        // L'ordre de chargement est significant, il ne doit pas etre trie.
        assert_eq!(plugins[0].package_name(), "a");
        assert_eq!(plugins[1].package_name(), "b");
        assert_eq!(plugins[2].package_name(), "c");
        assert_eq!(serde_json::to_value(&plugins).unwrap(), json);
    }

    #[test]
    fn une_liste_inversee_ne_se_retourne_pas() {
        let json = json!(["c", "b", "a"]);

        let plugins: Plugins = serde_json::from_value(json.clone()).unwrap();
        let noms: Vec<&str> = plugins.iter().map(|p| p.package_name()).collect();

        assert_eq!(noms, vec!["c", "b", "a"]);
        assert_eq!(serde_json::to_value(&plugins).unwrap(), json);
    }

    // ------------------------------------------------- noms de champs et cas limites

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let mut options = Options::new();
        options.insert("flag".to_string(), Value::Bool(true));
        let plugin = Plugin::Entry(Entry::with_options("pkg", options));

        let json = serde_json::to_value(&plugin).unwrap();

        assert_eq!(json["package"], "pkg", "le champ doit s'appeler package");
        assert!(json.get("Package").is_none(), "la casse ne doit pas changer");
        assert_eq!(json["options"]["flag"], true, "le champ doit s'appeler options");
        assert!(json.get("Options").is_none(), "la casse ne doit pas changer");
    }

    #[test]
    fn les_options_acceptent_toute_forme_de_valeur() {
        let json = json!({
            "package": "pkg",
            "options": { "n": 1, "f": 1.5, "b": false, "z": null, "t": [1, 2], "o": { "k": "v" } }
        });

        let plugin: Plugin = serde_json::from_value(json).unwrap();
        let options = plugin.options().expect("les options doivent etre lues");

        // `Schema.Unknown` n'exclut rien : rien ne doit etre rejete ni converti.
        assert_eq!(options["n"], json!(1));
        assert_eq!(options["f"], json!(1.5));
        assert_eq!(options["b"], json!(false));
        assert_eq!(options["z"], Value::Null);
        assert_eq!(options["t"], json!([1, 2]));
        assert_eq!(options["o"], json!({ "k": "v" }));
    }

    #[test]
    fn un_champ_inconnu_dans_une_entree_est_ignore_comme_en_typescript() {
        // Le decodeur de Schema ne rejette pas les proprietes en trop, il les
        // ignore. Serde fait pareil par defaut, et c'est ce qu'on veut garder.
        let json = json!({ "package": "pkg", "inconnu": 42 });

        let plugin: Plugin = serde_json::from_value(json).unwrap();

        assert_eq!(plugin.package_name(), "pkg");
    }

    #[test]
    fn les_deux_forme_de_plugin_donnent_le_meme_nom_de_paquet() {
        // C'est le seul point commun entre les deux formes, et le seul champ
        // qui identifie le plugin a charger.
        let court = Plugin::name("opencode-plugin-foo");
        let long = Plugin::Entry(Entry::with_options("opencode-plugin-foo", Options::new()));

        assert_eq!(court.package_name(), "opencode-plugin-foo");
        assert_eq!(long.package_name(), "opencode-plugin-foo");
        assert!(court.options().is_none(), "la forme courte n'a pas d'options");
        assert_eq!(long.options().map(|o| o.len()), Some(0));
    }

    #[test]
    fn un_nom_de_plugin_vide_reste_un_nom_valide() {
        // Piege `?` contre `??` : en JavaScript une chaine vide est falsy. Ici
        // rien n'est teste en veracite, la chaine vide traverse le JSON intacte.
        let plugin: Plugin = serde_json::from_value(json!("")).unwrap();

        assert_eq!(plugin, Plugin::Name(String::new()));
        assert_eq!(serde_json::to_value(&plugin).unwrap(), json!(""));
    }

    #[test]
    fn une_valeur_qui_n_est_ni_chaine_ni_entree_est_refusee() {
        // Ni une chaine, ni un objet portant `package` : l'union non taggee ne
        // peut pas decider, donc le decoding echoue plutot que d'inventer.
        assert!(serde_json::from_value::<Plugin>(json!(42)).is_err());
        assert!(serde_json::from_value::<Plugin>(json!(null)).is_err());
        assert!(serde_json::from_value::<Plugin>(json!({ "autre": 1 })).is_err());
        assert!(serde_json::from_value::<Plugin>(json!({})).is_err());
    }
}
