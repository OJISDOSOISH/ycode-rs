//! Portage de `packages/core/src/config/formatter.ts` (schema `ConfigV2.Formatter`).
//!
//! La source tient en douze lignes et ne contient que deux declarations : un
//! `Entry` et une union `Info`. Il n'y a rien d'autre a porter.
//!
//! ```ts
//! export class Entry extends Schema.Class<Entry>("ConfigV2.Formatter.Entry")({
//!   disabled: Schema.Boolean.pipe(Schema.optional),
//!   command: Schema.String.pipe(Schema.Array, Schema.optional),
//!   environment: Schema.Record(Schema.String, Schema.String).pipe(Schema.optional),
//!   extensions: Schema.String.pipe(Schema.Array, Schema.optional),
//! }) {}
//!
//! export const Info = Schema.Union([Schema.Boolean, Schema.Record(Schema.String, Entry)])
//! ```
//!
//! Trois points meritent d'etre lus avant le code Rust.
//!
//! 1. La premiere ligne de la source, `export * as ConfigFormatter from
//!    "./formatter"`, est un reexport du module sur lui-meme. Il n'apporte
//!    rien : en Rust le module est deja lui-meme, la ligne disparait du
//!    portage.
//!
//! 2. L'union `Info` **n'a pas de tag**. Ce n'est pas un ADT tagge comme le
//!    serait un `{ type: "..." }` : c'est un choix entre deux formes JSON qui
//!    n'ont aucun champ discriminant, `false` ou `{ "prettier": { ... } }`. La
//!    traduction est donc `#[serde(untagged)]`, pas `#[serde(tag = "...")]`.
//!    L'ordre des variantes est celui du TS, `Boolean` d'abord, parce que
//!    `Schema.Union` essaie ses membres dans l'ordre ecrit et fait pareil.
//!
//! 3. `Schema.optional` autorise la cle absente, ou presente avec la valeur
//!    `undefined`. Comme `JSON.stringify` supprime les cles `undefined`, une
//!    entree dont un champ est absent et une entree dont le champ vaut
//!    `undefined` produisent le meme JSON. C'est exactement
//!    `Option<T>` suivi de `skip_serializing_if = "Option::is_none"`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Regle de reformatage configuree pour un seul formateur.
///
/// Une cle de `Info::Overrides` nomme soit un formateur integre a opencode
/// (`prettier`, `gofmt`, `rustfmt`...), soit une commande personnalisee
/// lancee telle quelle.
///
/// Les quatre champs sont optionnels, comme dans le TS. Une entree peut donc
/// n'porter aucun champ et se serialiser en `{}`.
///
/// Deux divergences assumees par rapport au schema d'origine :
///
/// - `command` est une **liste de chaines**, pas une chaine a couper. Le TS
///   fait `Schema.String.pipe(Schema.Array)`, il ne decoupe donc jamais sur les
///   espaces. `["prettier --write"]` reste un seul element et n'est pas
///   interprete comme un programme et ses arguments. Ne pas "corriger" ca en
///   un `Vec<String>` issu d'un `split(' ')`.
/// - Serde accepte `null` pour un `Option<T>` la ou `Schema.optional` du TS
///   n'accepte que `undefined`. Un `{"disabled": null}` est donc lu par ce
///   portage alors que le TS le refuserait. Aucun JSON ecrit par le TS ne
///   contient `null` ici, la difference n'est visible que sur une saisie
///   manuelle.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Entry {
    /// Desactive ce formateur sans effacer sa configuration.
    #[serde(rename = "disabled", skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,

    /// Ligne de commande du formateur, args compris, sans le decoupage.
    #[serde(rename = "command", skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    /// Variables ajoutees a l'environnement du formateur.
    #[serde(rename = "environment", skip_serializing_if = "Option::is_none")]
    pub environment: Option<BTreeMap<String, String>>,

    /// Extensions de fichiers trailinges que ce formateur doit traiter.
    #[serde(rename = "extensions", skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
}

/// Contenu du champ `formatter` de la configuration.
///
/// L'union du TS : `Schema.Union([Schema.Boolean, Schema.Record(String, Entry)])`.
///
/// - `false` (ou l'absence du champ) desactive les formateurs.
/// - `true` active les formateurs integres sans surcharge.
/// - un objet active les formateurs integres et en surcharge certains, chaque
///   cle etant le nom d'un formateur.
///
/// `#[serde(untagged)]` reproduit l'union : il essaie les variantes dans l'ordre
/// declare, comme `Schema.Union`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Info {
    /// Formateurs actifs ou desactives, sans surcharge.
    Toggle(bool),

    /// Surcharges par nom de formateur. La cle de la table est le nom.
    Overrides(BTreeMap<String, Entry>),
}

#[cfg(test)]
mod tests {
    use super::{Entry, Info};
    use std::collections::BTreeMap;

    fn entrees() -> BTreeMap<String, Entry> {
        let mut table = BTreeMap::new();
        table.insert("prettier".to_string(), Entry { disabled: Some(false), ..Entry::default() });
        table
    }

    #[test]
    fn une_entree_sans_choix_ne_serialise_que_des_champs_absents() {
        let vide = Entry::default();

        assert_eq!(vide.disabled, None);
        assert_eq!(vide.command, None);
        assert_eq!(vide.environment, None);
        assert_eq!(vide.extensions, None);

        let json = serde_json::to_value(&vide).unwrap();
        assert_eq!(json, serde_json::json!({}), "une entree vide vaut un objet vide");
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Les quatre noms du TS sont des mots bascules, deja identiques a Rust.
        // Le `rename` explicite sert de verrou : si quelqu'un renomme un champ
        // en interne plus tard, le JSON produit ne bouge pas.
        let entry = Entry {
            disabled: Some(true),
            command: Some(vec!["prettier --write".to_string()]),
            environment: Some(BTreeMap::from([("NO_COLOR".to_string(), "1".to_string())])),
            extensions: Some(vec![".ts".to_string()]),
        };

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["disabled"], true);
        assert_eq!(json["command"][0], "prettier --write");
        assert_eq!(json["environment"]["NO_COLOR"], "1");
        assert_eq!(json["extensions"][0], ".ts");

        // Et exactement ces quatre cles, ni plus ni moins.
        let mut cles: Vec<&str> =
            json.as_object().unwrap().keys().map(|cle| cle.as_str()).collect();
        cles.sort();
        assert_eq!(cles, vec!["command", "disabled", "environment", "extensions"]);
    }

    #[test]
    fn un_champ_absent_et_un_champ_a_null_vautent_pareil() {
        // Le TS accepte la cle absente comme la cle a `undefined` (que
        // `JSON.stringify` supprime). Serde fait la meme chose des deux.
        let sans_champ: Entry = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(sans_champ, Entry::default());

        let avec_null: Entry = serde_json::from_value(serde_json::json!({ "disabled": null })).unwrap();
        assert_eq!(avec_null, Entry::default());
    }

    #[test]
    fn une_liste_vide_et_une_liste_absente_ne_sont_pas_la_meme_chose() {
        // `command: []` est un choix explicite, pas une absence de choix. Le
        // JSON doit donc garder la cle vide, sinon on perd la distinction.
        let vide: Entry = serde_json::from_value(serde_json::json!({ "command": [] })).unwrap();
        assert_eq!(vide.command, Some(Vec::<String>::new()));
        assert_eq!(serde_json::to_value(&vide).unwrap(), serde_json::json!({ "command": [] }));

        let absent = Entry::default();
        assert_eq!(absent.command, None);
        assert!(serde_json::to_value(&absent).unwrap().get("command").is_none());
    }

    #[test]
    fn un_environnement_vide_et_un_environnement_absent_ne_sont_pas_la_meme_chose() {
        let vide: Entry = serde_json::from_value(serde_json::json!({ "environment": {} })).unwrap();
        assert_eq!(vide.environment, Some(BTreeMap::<String, String>::new()));
        assert_eq!(serde_json::to_value(&vide).unwrap(), serde_json::json!({ "environment": {} }));

        assert_eq!(Entry::default().environment, None);
    }

    #[test]
    fn une_commande_avec_des_espaces_reste_un_seul_element() {
        // Le TS fait `Schema.String.pipe(Schema.Array)` : il ne decoupe rien.
        // La commande et ses arguments sont portes tels quels.
        let entry: Entry = serde_json::from_value(serde_json::json!({
            "command": ["gofmt -w -s ."]
        }))
        .unwrap();

        match &entry.command {
            Some(parts) => {
                assert_eq!(parts.len(), 1);
                assert_eq!(parts[0], "gofmt -w -s .");
            }
            None => panic!("la commande ne doit pas disparaitre"),
        }
    }

    #[test]
    fn un_boleen_donne_un_boleen_et_une_table_donne_un_objet() {
        let desactive = Info::Toggle(false);
        assert_eq!(serde_json::to_value(&desactive).unwrap(), serde_json::json!(false));

        let actif = Info::Toggle(true);
        assert_eq!(serde_json::to_value(&actif).unwrap(), serde_json::json!(true));

        let table = Info::Overrides(entrees());
        assert_eq!(
            serde_json::to_value(&table).unwrap(),
            serde_json::json!({ "prettier": { "disabled": false } })
        );
    }

    #[test]
    fn une_table_vide_donne_un_objet_vide_pas_un_boleen() {
        let table = Info::Overrides(BTreeMap::new());
        let json = serde_json::to_value(&table).unwrap();
        assert_eq!(json, serde_json::json!({}));

        let relu: Info = serde_json::from_value(json).unwrap();
        assert_eq!(relu, Info::Overrides(BTreeMap::new()));
    }

    #[test]
    fn le_json_du_typescript_est_relu_dans_la_bonne_variante() {
        // Le vrai test de compatibilite : un objet ecrit comme le TS l'ecrit.
        let json = serde_json::json!({
            "prettier": { "command": ["prettier --write"], "extensions": [".ts", ".tsx"] },
            "gofmt": { "disabled": true }
        });

        match serde_json::from_value::<Info>(json).unwrap() {
            Info::Overrides(table) => {
                assert_eq!(table.len(), 2);
                assert_eq!(table["gofmt"].disabled, Some(true));
                assert_eq!(
                    table["prettier"].command,
                    Some(vec!["prettier --write".to_string()])
                );
                assert_eq!(table["prettier"].extensions, Some(vec![".ts".to_string(), ".tsx".to_string()]));
            }
            Info::Toggle(b) => panic!("un objet ne doit pas devenir un booleen : {}", b),
        }
    }

    #[test]
    fn un_booleen_du_typescript_est_relu_comme_un_booleen() {
        match serde_json::from_value::<Info>(serde_json::json!(false)).unwrap() {
            Info::Toggle(b) => assert!(!b),
            Info::Overrides(_) => panic!("un booleen ne doit pas devenir une table"),
        }

        match serde_json::from_value::<Info>(serde_json::json!(true)).unwrap() {
            Info::Toggle(b) => assert!(b),
            Info::Overrides(_) => panic!("un booleen ne doit pas devenir une table"),
        }
    }

    #[test]
    fn l_ordre_d_insertion_des_formateurs_ne_compte_pas() {
        // La table du TS est un `Record`, dont l'ordre des cles n'a pas de
        // sens. Le `BTreeMap` impose l'ordre alphabetique, donc deux tables
        // construites dans des ordres differents donnent le meme JSON.
        let mut avant = BTreeMap::new();
        avant.insert("zofmt".to_string(), Entry::default());
        avant.insert("astyle".to_string(), Entry::default());
        avant.insert("prettier".to_string(), Entry::default());

        let mut apres = BTreeMap::new();
        apres.insert("prettier".to_string(), Entry::default());
        apres.insert("astyle".to_string(), Entry::default());
        apres.insert("zofmt".to_string(), Entry::default());

        assert_eq!(
            serde_json::to_string(&Info::Overrides(avant)).unwrap(),
            serde_json::to_string(&Info::Overrides(apres.clone())).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&Info::Overrides(apres)).unwrap(),
            "{\"astyle\":{},\"prettier\":{},\"zofmt\":{}}"
        );
    }

    #[test]
    fn une_valeur_hors_schema_est_refusee() {
        // Ni booleen ni table de `Entry` : l'union doit echouer, pas deviner.
        for invalide in [
            serde_json::json!("desactive"),
            serde_json::json!(1),
            serde_json::json!(null),
            serde_json::json!([{ "disabled": true }]),
            serde_json::json!({ "prettier": 3 }),
        ] {
            assert!(
                serde_json::from_value::<Info>(invalide.clone()).is_err(),
                "{} aurait du etre refuse",
                invalide
            );
        }
    }
}
