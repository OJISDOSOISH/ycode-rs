//! Portage de `packages/core/src/config/lsp.ts`.
//!
//! La source tient en dix-huit lignes et ne contient aucune logique : deux
//! unions de schema et un literal. Il n'y a donc rien a inventer.
//!
//! ```ts
//! export * as ConfigLSP from "./lsp"
//!
//! export const Disabled = Schema.Struct({
//!   disabled: Schema.Literal(true),
//! })
//!
//! export class Server extends Schema.Class<Server>("ConfigV2.LSP.Server")({
//!   command: Schema.String.pipe(Schema.Array),
//!   extensions: Schema.String.pipe(Schema.Array, Schema.optional),
//!   disabled: Schema.Boolean.pipe(Schema.optional),
//!   env: Schema.Record(Schema.String, Schema.String).pipe(Schema.optional),
//!   initialization: Schema.Record(Schema.String, Schema.Unknown).pipe(Schema.optional),
//! }) {}
//!
//! export const Entry = Schema.Union([Disabled, Server])
//! export const Info = Schema.Union([Schema.Boolean, Schema.Record(Schema.String, Entry)])
//! ```
//!
//! Quatre points a lire avant le code.
//!
//! 1. La premiere ligne, `export * as ConfigLSP from "./lsp"`, est un reexport
//!    du module sur lui-meme. Il n'apporte rien : en Rust le module est deja
//!    lui-meme, la ligne disparait du portage. C'est un motif recurrent dans
//!    opencode, ce n'est pas un import vers un fichier voisin.
//!
//! 2. Aucune des deux unions n'a de tag. `Entry` est un choix entre deux
//!    formes JSON sans discriminant, `Info` entre un booleen et une table. La
//!    traduction est donc `#[serde(untagged)]`, jamais `#[serde(tag = "...")]`,
//!    et l'ordre des variantes reste celui du TS : `Schema.Union` essaie ses
//!    membres dans l'order ecrit.
//!
//! 3. `Schema.optional` devient `Option<T>` avec
//!    `skip_serializing_if = "Option::is_none"`. Consequence a assumer :
//!    Serde lit `null` comme `None` la ou le TS n'accepte que l'absence de
//!    cle. Invisible pour du JSON ecrit par le TS, visible sur une saisie
//!    manuelle.
//!
//! 4. Piege `?` contre `??` : ce fichier ne contient ni l'un ni l'autre, il
//!    n'y a donc aucun test de veracite a porter. Le piege reste neanmoins
//!    present sous une autre forme, cote serialisation : `disabled` est un
//!    `Option<bool>` et `false` est *falsy* en JavaScript. Un portage
//!    equivalent a `if (b) { Some(b) } else { None }` ferait disparaitre
//!    `false` du JSON. Ici `Some(false)` est conserve, et une chaine vide dans
//!    `command`, `extensions` ou `env` est conservee aussi. Deux tests
//!    verrouillent ces deux cas.
//!
//! NOMS DE CHAMPS : les cinq champs du serveur (`command`, `extensions`,
//! `disabled`, `env`, `initialization`) sont des mots uniques en minuscules,
//! donc aucun nom camelCase et aucun piege `projectID`. Le `rename` explicite
//! est pose quand meme sur chaque champ, comme un verrou : un renommage
//! interne ulterieur ne peut pas changer le JSON produit. Un test verifie la
//! liste exacte des cles.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Identifiant de schema donne a la classe `Server` par le TS :
/// `Schema.Class<Server>("ConfigV2.LSP.Server")`.
///
/// Le TS s'en sert pour l'affichage des erreurs et pour l'introspection ; il
/// n'intervient pas dans le JSON. Il est conserve ici pour que la relecture
/// puisse le comparer a la source sans rouvrir le `.ts`.
pub const SCHEMA_IDENTIFIER: &str = "ConfigV2.LSP.Server";

/// Traduction de `Schema.Literal(true)`.
///
/// Un litteral est une valeur unique acceptee : il **refuse** tout le reste.
/// Un `bool` nu accepterait `false`, ce qui serait moins valide que la source,
/// donc le champ n'est pas un `bool` mais ce type-marqueur.
///
/// Il n'y a qu'une seule valeur possible, donc il n'y a rien a comparer : la
/// seule information du type est "ceci vaut `true`".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiteralTrue;

impl Serialize for LiteralTrue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for LiteralTrue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Ok(LiteralTrue)
        } else {
            Err(<D::Error as serde::de::Error>::custom(
                "le champ disabled n'accepte que la valeur true",
            ))
        }
    }
}

/// Forme `Disabled` de l'union `Entry` : desactiver le serveur sans l'effacer.
///
/// En TS : `Schema.Struct({ disabled: Schema.Literal(true) })`.
///
/// Le champ est obligatoire et ne peut valoir que `true`. Il n'y a donc aucun
/// etat representable de cette forme ou le serveur soit desactive et
/// configure : il faut un `Server` avec `disabled: true`.
///
/// Effet de bord assume : `{"disabled": true, "command": ["x"]}` lit comme
/// `Disabled`, donc `command` est perdu. C'est la consequence de l'ordre des
/// variantes de `Schema.Union([Disabled, Server])`, conserve ici. Le
/// comportement du parseur d'Effect sur les proprietes en trop n'a pas pu etre
/// verifie sur ce poste, le paquet `effect` n'y est pas installe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disabled {
    /// Le litteral `true`. Refuse `false`, comme la source.
    #[serde(rename = "disabled")]
    pub disabled: LiteralTrue,
}

impl Disabled {
    /// Construit la forme desactivee. Le champ n'a qu'une valeur possible.
    pub fn new() -> Self {
        Self { disabled: LiteralTrue }
    }
}

/// Forme `Server` de l'union `Entry` : un serveur de langage lance en externe.
///
/// En TS : `Schema.Class<Server>("ConfigV2.LSP.Server")({ ... })`.
///
/// `command` est la seule valeur obligatoire : c'est la ligne de commande
/// lancee telle quelle, programme et arguments dans un seul element. Le TS
/// fait `Schema.String.pipe(Schema.Array)`, il ne decoupe donc jamais sur les
/// espaces : `["tsserver --stdio"]` reste un element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Server {
    /// Ligne de commande du serveur, args compris, sans decoupage.
    #[serde(rename = "command")]
    pub command: Vec<String>,

    /// Extensions de fichiers que ce serveur doit traiter.
    #[serde(rename = "extensions", skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,

    /// Desactivation du serveur. `Some(false)` doit rester `false` en JSON.
    #[serde(rename = "disabled", skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,

    /// Variables ajoutees a l'environnement du serveur.
    #[serde(rename = "env", skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,

    /// Charge utile `initializationOptions` envoyee au serveur.
    ///
    /// En TS : `Schema.Record(Schema.String, Schema.Unknown)`. `Schema.Unknown`
    /// n'applique aucune contrainte, donc `serde_json::Value` est la
    /// traduction exacte, y compris pour `null`.
    #[serde(rename = "initialization", skip_serializing_if = "Option::is_none")]
    pub initialization: Option<BTreeMap<String, Value>>,
}

impl Server {
    /// Construit un serveur a partir de sa seule valeur obligatoire.
    pub fn new(command: Vec<String>) -> Self {
        Self {
            command,
            extensions: None,
            disabled: None,
            env: None,
            initialization: None,
        }
    }
}

/// Une entree de la table `lsp` : serveur desactive, ou serveur configure.
///
/// En TS : `Schema.Union([Disabled, Server])`. Pas de tag, donc
/// `#[serde(untagged)]`. L'ordre est celui du TS, `Disabled` d'abord.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    /// Serveur desactive, aucune configuration conservee.
    Disabled(Disabled),

    /// Serveur configure, avec au minimum sa ligne de commande.
    Server(Server),
}

/// Contenu du champ `lsp` de la configuration.
///
/// En TS : `Schema.Union([Schema.Boolean, Schema.Record(Schema.String, Entry)])`.
///
/// - `false` desactive tous les serveurs de langage.
/// - `true` les active sans surcharge.
/// - un objet active les serveurs integres et en surcharge certains, chaque
///   cle etant l'identifiant du serveur.
///
/// `#[serde(untagged)]` reproduit l'union : les variantes sont essaiees dans
/// l'ordre declare, comme `Schema.Union`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Info {
    /// Serveurs actifs ou desactives, sans surcharge.
    Toggle(bool),

    /// Surcharges par identifiant de serveur. La cle de la table est le nom.
    Overrides(BTreeMap<String, Entry>),
}

#[cfg(test)]
mod tests {
    use super::{Disabled, Entry, Info, LiteralTrue, Server, SCHEMA_IDENTIFIER};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    fn serveur_complet() -> Server {
        Server {
            command: vec!["tsserver".to_string(), "--stdio".to_string()],
            extensions: Some(vec![".ts".to_string(), ".tsx".to_string()]),
            disabled: Some(false),
            env: Some(BTreeMap::from([("NODE_ENV".to_string(), "test".to_string())])),
            initialization: Some(BTreeMap::from([(
                "preferences".to_string(),
                json!({ "provideFormatter": true }),
            )])),
        }
    }

    #[test]
    fn une_entree_desactivee_ne_serie_que_disabled_true() {
        let desactive = Disabled::new();

        assert_eq!(serde_json::to_value(&desactive).unwrap(), json!({ "disabled": true }));
        assert_eq!(desactive.disabled, LiteralTrue);
        assert_eq!(SCHEMA_IDENTIFIER, "ConfigV2.LSP.Server");
    }

    #[test]
    fn desactive_faux_est_refuse_car_le_champ_est_un_litteral() {
        // `Schema.Literal(true)` refuse `false`. Un `bool` nu l'accepterait.
        assert!(serde_json::from_value::<Disabled>(json!({ "disabled": false })).is_err());
        assert!(serde_json::from_value::<Disabled>(json!({ "disabled": null })).is_err());
        assert!(serde_json::from_value::<Disabled>(json!({})).is_err());
        assert!(serde_json::from_value::<Disabled>(json!({ "disabled": "true" })).is_err());

        // L'union ne rattrape pas non plus : `Server` exige `command`.
        assert!(serde_json::from_value::<Entry>(json!({ "disabled": false })).is_err());
    }

    #[test]
    fn un_serveur_minimal_ne_serie_que_its_commande() {
        let minimal = Server::new(vec!["gopls".to_string()]);

        assert_eq!(minimal.command, vec!["gopls".to_string()]);
        assert_eq!(minimal.extensions, None);
        assert_eq!(minimal.disabled, None);
        assert_eq!(minimal.env, None);
        assert_eq!(minimal.initialization, None);

        assert_eq!(serde_json::to_value(&minimal).unwrap(), json!({ "command": ["gopls"] }));
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Les cinq noms du TS sont des mots uniques en minuscules, donc aucun
        // camelCase a rattraper. Le test verifie quand meme la liste exacte des
        // cles, parce qu'une faute de `rename` serait invisible du reste.
        let json = serde_json::to_value(&serveur_complet()).unwrap();

        let mut cles: Vec<&str> = json.as_object().unwrap().keys().map(|c| c.as_str()).collect();
        cles.sort();
        assert_eq!(
            cles,
            vec!["command", "disabled", "env", "extensions", "initialization"]
        );

        assert_eq!(json["command"], json!([ "tsserver", "--stdio" ]));
        assert_eq!(json["extensions"], json!([ ".ts", ".tsx" ]));
        assert_eq!(json["disabled"], false);
        assert_eq!(json["env"]["NODE_ENV"], "test");
        assert_eq!(json["initialization"]["preferences"]["provideFormatter"], true);
    }

    #[test]
    fn un_champ_optionnel_a_false_reste_present_dans_le_json() {
        // Le piege `?` contre `??`, transporte sous une autre forme : `false` est
        // *falsy* en JavaScript. Un portage qui ecrit `disabled` seulement si
        // il est vrai perdrait cette information, et le TS la conserve.
        let serveur = Server {
            disabled: Some(false),
            ..Server::new(vec!["rust-analyzer".to_string()])
        };

        let json = serde_json::to_value(&serveur).unwrap();
        assert_eq!(json["disabled"], false);
        assert!(json.get("disabled").is_some(), "la cle disabled ne doit pas disparaitre");

        let relu: Server = serde_json::from_value(json).unwrap();
        assert_eq!(relu.disabled, Some(false));
    }

    #[test]
    fn une_chaine_vide_survit_partout_ou_le_ts_l_accepte() {
        // Une chaine vide est *falsy* mais elle est presente. Ni `command`, ni
        // `extensions`, ni une valeur de `env` ne doivent disparaitre.
        let serveur: Server = serde_json::from_value(json!({
            "command": [""],
            "extensions": [""],
            "env": { "": "" }
        }))
        .unwrap();

        assert_eq!(serveur.command, vec![String::new()]);
        assert_eq!(serveur.extensions, Some(vec![String::new()]));
        assert_eq!(
            serveur.env.as_ref().unwrap().get("").map(String::as_str),
            Some("")
        );

        assert_eq!(
            serde_json::to_value(&serveur).unwrap(),
            json!({ "command": [""], "extensions": [""], "env": { "": "" } })
        );
    }

    #[test]
    fn une_liste_vide_est_un_choix_et_non_une_absence_de_choix() {
        let explicite: Server =
            serde_json::from_value(json!({ "command": ["x"], "extensions": [] })).unwrap();
        assert_eq!(explicite.extensions, Some(Vec::<String>::new()));
        assert_eq!(serde_json::to_value(&explicite).unwrap()["extensions"], json!([]));

        let absent = Server::new(vec!["x".to_string()]);
        assert!(serde_json::to_value(&absent).unwrap().get("extensions").is_none());
    }

    #[test]
    fn une_table_vide_donne_un_objet_vide_pas_un_booleen() {
        let vide = Info::Overrides(BTreeMap::new());
        let json = serde_json::to_value(&vide).unwrap();

        assert_eq!(json, json!({}));
        assert_eq!(serde_json::from_value::<Info>(json).unwrap(), Info::Overrides(BTreeMap::new()));
    }

    #[test]
    fn un_booleen_donne_le_bouton_et_un_objet_donne_les_surcharges() {
        assert_eq!(serde_json::to_value(&Info::Toggle(false)).unwrap(), json!(false));
        assert_eq!(serde_json::to_value(&Info::Toggle(true)).unwrap(), json!(true));

        let mut table = BTreeMap::new();
        table.insert("rust".to_string(), Entry::Server(Server::new(vec!["rust-analyzer".to_string()])));
        assert_eq!(
            serde_json::to_value(&Info::Overrides(table.clone())).unwrap(),
            json!({ "rust": { "command": ["rust-analyzer"] } })
        );

        match serde_json::from_value::<Info>(json!({ "rust": { "command": ["rust-analyzer"] } }))
            .unwrap()
        {
            Info::Overrides(lu) => assert_eq!(lu, table),
            Info::Toggle(b) => panic!("un objet ne doit pas devenir un booleen : {}", b),
        }
    }

    #[test]
    fn une_surcharge_desactivee_et_une_surcharge_serveur_se_lisent_bien() {
        let ecrit_par_le_ts = json!({
            "typescript": { "command": ["tsserver", "--stdio"], "extensions": [".ts"] },
            "gopls": { "disabled": true }
        });

        let relu: Info = serde_json::from_value(ecrit_par_le_ts.clone()).unwrap();

        match &relu {
            Info::Overrides(table) => {
                assert_eq!(table.len(), 2);
                assert_eq!(
                    table["typescript"],
                    Entry::Server(Server {
                        command: vec!["tsserver".to_string(), "--stdio".to_string()],
                        extensions: Some(vec![".ts".to_string()]),
                        ..Server::new(Vec::new())
                    })
                );
                assert_eq!(table["gopls"], Entry::Disabled(Disabled::new()));
            }
            Info::Toggle(b) => panic!("un objet ne doit pas devenir un booleen : {}", b),
        }

        assert_eq!(serde_json::to_value(&relu).unwrap(), ecrit_par_le_ts);
    }

    #[test]
    fn une_entree_complete_ne_peut_etre_que_desactivee_ou_serveur() {
        // Ordre des variantes de `Schema.Union([Disabled, Server])` :
        // `Disabled` est essaiee en premier, donc `{"disabled": true,
        // "command": [...]}` devient `Disabled` et perd `command`.
        // Comportement documente, pas deduit d'une lecture du code de `effect`.
        let lu = serde_json::from_value::<Entry>(json!({
            "disabled": true,
            "command": ["gopls"]
        }))
        .unwrap();

        assert_eq!(lu, Entry::Disabled(Disabled::new()));
    }

    #[test]
    fn une_valeur_hors_schema_est_refusee() {
        for invalide in [
            json!("desactive"),
            json!(1),
            json!(null),
            json!([{ "disabled": true }]),
            json!({ "rust": 3 }),
            json!({ "rust": {} }),
            json!({ "rust": { "command": 3 } }),
            json!({ "rust": { "command": [3] } }),
            json!({ "rust": { "command": ["x"], "env": { "A": 3 } } }),
        ] {
            assert!(
                serde_json::from_value::<Info>(invalide.clone()).is_err(),
                "{} aurait du etre refuse",
                invalide
            );
        }
    }

    #[test]
    fn aller_retour_conserve_la_configuration_entiere() {
        let depart = Info::Overrides(BTreeMap::from([
            ("typescript".to_string(), Entry::Server(serveur_complet())),
            ("gopls".to_string(), Entry::Disabled(Disabled::new())),
        ]));

        let texte = serde_json::to_string(&depart).unwrap();
        let relu: Info = serde_json::from_str(&texte).unwrap();

        assert_eq!(relu, depart);
    }

    #[test]
    fn lordre_des_cles_ne_compte_pas_dans_les_tables() {
        // Le `Record` du TS n'a pas d'ordre. Le `BTreeMap` impose l'ordre
        // alphabetique, donc deux tables construites dans des ordres
        // differents donnent le meme JSON.
        let mut avant = BTreeMap::new();
        avant.insert("zls".to_string(), Entry::Server(Server::new(vec!["zls".to_string()])));
        avant.insert("gopls".to_string(), Entry::Server(Server::new(vec!["gopls".to_string()])));

        let mut apres = BTreeMap::new();
        apres.insert("gopls".to_string(), Entry::Server(Server::new(vec!["gopls".to_string()])));
        apres.insert("zls".to_string(), Entry::Server(Server::new(vec!["zls".to_string()])));

        assert_eq!(
            serde_json::to_string(&Info::Overrides(avant.clone())).unwrap(),
            "{\"gopls\":{\"command\":[\"gopls\"]},\"zls\":{\"command\":[\"zls\"]}}"
        );
        assert_eq!(
            serde_json::to_string(&Info::Overrides(avant)).unwrap(),
            serde_json::to_string(&Info::Overrides(apres)).unwrap()
        );
    }

    #[test]
    fn une_charge_d_initialisation_libre_accepte_n_importe_quelle_valeur_json() {
        // `Schema.Unknown` ne valide rien : objet, tableau, chaine, nombre,
        // booleen, et meme `null` passent.
        let mut initialisation = BTreeMap::new();
        initialisation.insert("objet".to_string(), json!({ "a": [1, null] }));
        initialisation.insert("tableau".to_string(), json!([1, 2]));
        initialisation.insert("chaine".to_string(), json!("texte"));
        initialisation.insert("nombre".to_string(), json!(1.5));
        initialisation.insert("rien".to_string(), Value::Null);

        let serveur = Server { initialization: Some(initialisation), ..Server::new(vec!["x".to_string()]) };
        let relu: Server =
            serde_json::from_value(serde_json::to_value(&serveur).unwrap()).unwrap();

        assert_eq!(relu.initialization, serveur.initialization);
        assert_eq!(relu.initialization.unwrap().get("rien"), Some(&Value::Null));
    }
}
