//! Portage de `packages/core/src/v1/config/error.ts`.
//!
//! Ce module ne contient que des classes d'erreur. Chacune est creee par
//! `NamedError.create(Nom, champs)`, et `util/error.ts` (non porte ici)
//! construit systematiquement le schema suivant, a `error.ts:26-29` :
//!
//! ```text
//! Schema.Struct({ name: Schema.Literal(name), data })
//! ```
//!
//! Quatre points a lire avant le code.
//!
//! 1. La ligne 1 du TS, `export * as ConfigErrorV1 from "./error"`, est un
//!    reexport du module sur lui-meme : en Rust le module est deja lui-meme,
//!    la ligne disparait du portage.
//!
//! 2. **La forme JSON est `{ "name": ..., "data": { ... } }`, sur deux
//!    niveaux, et non `{ "name": ..., "path": ... }`.** C'est le point le
//!    plus piege du fichier. `toObject()` (`error.ts:55-60`) renvoie
//!    explicitement `{ name, data }`, donc `data` est bien un champ frere du
//!    tag `name`, et non un aplatissement.
//!
//!    La traduction directe est la representation **adjacente** de serde,
//!    `#[serde(tag = "name", content = "data")]` : elle ecrit le tag et le
//!    contenu dans deux champs voisins du meme objet, ce qui est litteralement
//!    le `toObject()` du TS. Les variantes sont donc des **newtype**.
//!
//!    Pourquoi pas `#[serde(tag = "name")]` tout court ? Avec un tag interne,
//!    serde considere qu'il n'y a qu'un seul niveau : le code genere par
//!    `serde_derive` route une variante newtype vers `deserialize_newtype_variant`
//!    (voir `serde_derive/de/enum_internally.rs`), c'est a dire que les
//!    champs de la charge utile se retrouvent **a cote** du tag. On
//!    obtiendrait `{"name":"ConfigJsonError","path":"..."}`, ce qui est faux.
//!    Il faudrait alors des variantes struct a un champ `data` pour
//!    recoudre le pb, ce qui est un detour fragile. La forme adjacente evite
//!    le pb par construction.
//!
//! 3. Les cinq noms de tag sont ecrits un par un en `#[serde(rename = ...)]`,
//!    jamais via `rename_all`, pour qu'une relecture les verifie d'un coup
//!    d'oeil et qu'un changement de convention de casse ne les derive pas.
//!    Attention, ils ne sont pas les noms des exports TypeScript : le TS
//!    exporte `JsonError` mais enregistre le tag `"ConfigJsonError"`. C'est
//!    la chaine passee a `NamedError.create` qui fait foi, prefixe `Config`
//!    compris.
//!
//! 4. Il n'y a **qu'un seul** `#[serde(flatten)]` dans ce fichier, sur le champ
//!    `rest` de `Issue`, et il n'est pas dans une variante d'enum : c'est un
//!    champ de struct, la forme la plus ordinaire qui soit. Il est
//!    obligatoire, voir la doc de `Issue`. Le tag interne, lui, a disparu.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Une anomalie de validation rapportee par le lecteur de configuration.
///
/// TS : `Schema.StructWithRest(Schema.Struct({ message, path }),
/// [Schema.Record(Schema.String, Schema.Unknown)])`.
///
/// `message` et `path` viennent du `Schema.Struct` interne : ils sont
/// obligatoires, donc leur type Rust n'a pas besoin d'etre `Option`. Le
/// `Schema.Record` de droite autorise en revanche n'importe quelle cle de
/// chaine et n'importe quelle valeur.
///
/// C'est le seul `flatten` du fichier, et il ne peut pas etre retire. Sans
/// lui, serde ignore silencieusement les cles inconnues a la
/// deserialization : un `{"code":42}` present dans le JSON du TS disparaitrait
/// du cote Rust. Or `StructWithRest` les conserve dans la valeur decodee.
/// `#[serde(flatten)]` sur une table les fait entrer et sortir au meme niveau
/// que `message` et `path`, ce qui est exactement la definition de
/// `StructWithRest`. Aucun `deny_unknown_fields` ici : ces cles sont
/// explicitement autorisees par le schema.
///
/// La table est un `BTreeMap` et non un `HashMap` pour que l'ordre de sortie
/// des cles inconnues soit deterministe.
///
/// Seul `PartialEq` est derive, et l'option n'est pas propagee aux types qui
/// contiennent un `Issue` : le reste y est un `serde_json::Value`, on ne veut
/// pas dependre de son implementation de `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    /// Le message humain, obligatoire.
    pub message: String,
    /// Le chemin du champ fautif, en liste de segments. Peut etre vide.
    pub path: Vec<String>,
    /// Toutes les autres cles, que le TS accepte sans les decrire.
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

impl Issue {
    /// Construit une anomalie sans cle inconnue. Le TS n'a pas de
    /// constructeur ici, c'est un confort de portage.
    pub fn new(message: impl Into<String>, path: Vec<String>) -> Self {
        Self {
            message: message.into(),
            path,
            rest: BTreeMap::new(),
        }
    }

    /// Ajoute une cle inconnue, celle que `Schema.Record` autorise.
    pub fn avec(mut self, cle: impl Into<String>, valeur: impl Into<serde_json::Value>) -> Self {
        self.rest.insert(cle.into(), valeur.into());
        self
    }
}

/// Charge utile de `ConfigJsonError` : le fichier n'est pas du JSON valide.
///
/// TS : `NamedError.create("ConfigJsonError", { path, message })`, ou
/// `message` est `Schema.optional(Schema.String)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonErrorData {
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl JsonErrorData {
    pub fn new(path: impl Into<String>, message: Option<String>) -> Self {
        Self {
            path: path.into(),
            message,
        }
    }
}

/// Charge utile de `ConfigInvalidError` : le JSON est valide mais faux.
///
/// TS : `NamedError.create("ConfigInvalidError", { path, issues, message })`,
/// ou `issues` est une liste de `Issue` et `message` est optionnel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidErrorData {
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "issues", skip_serializing_if = "Option::is_none")]
    pub issues: Option<Vec<Issue>>,
    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl InvalidErrorData {
    pub fn new(path: impl Into<String>, issues: Option<Vec<Issue>>, message: Option<String>) -> Self {
        Self {
            path: path.into(),
            issues,
            message,
        }
    }
}

/// Charge utile de `ConfigFrontmatterError` : le frontmatter est invalide.
///
/// TS : `NamedError.create("ConfigFrontmatterError", { path, message })`,
/// ou les deux champs sont obligatoires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontmatterErrorData {
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "message")]
    pub message: String,
}

impl FrontmatterErrorData {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Charge utile de `ConfigDirectoryTypoError` : un dossier est mal orthographie.
///
/// TS : `NamedError.create("ConfigDirectoryTypoError",
/// { path, dir, suggestion })`, les trois champs obligatoires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectoryTypoErrorData {
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "dir")]
    pub dir: String,
    #[serde(rename = "suggestion")]
    pub suggestion: String,
}

impl DirectoryTypoErrorData {
    pub fn new(path: impl Into<String>, dir: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            dir: dir.into(),
            suggestion: suggestion.into(),
        }
    }
}

/// Charge utile de `ConfigRemoteAuthError` : l'authentification distante a
/// echoue.
///
/// TS : `NamedError.create("ConfigRemoteAuthError", { url, remote })`, les
/// deux champs obligatoires. Il n'y a pas de champ `message` ici, et il ne
/// faut pas lui en ajouter un pour uniformiser les cinq erreurs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteAuthErrorData {
    #[serde(rename = "url")]
    pub url: String,
    #[serde(rename = "remote")]
    pub remote: String,
}

impl RemoteAuthErrorData {
    pub fn new(url: impl Into<String>, remote: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            remote: remote.into(),
        }
    }
}

/// L'union des cinq erreurs de configuration.
///
/// `content = "data"` est l'element cle de ce portage : il place le tag
/// `name` et la charge utile dans deux champs freres du meme objet, ce que
/// fait le `toObject()` du TS. Les variantes sont des newtype, ce qui est
/// coherent : chacune porte exactement une charge utile. Voir le point 2 de
/// l'en-tete pour lepiege du tag interne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", content = "data")]
pub enum ConfigError {
    /// `ConfigJsonError`
    #[serde(rename = "ConfigJsonError")]
    Json(JsonErrorData),
    /// `ConfigInvalidError`
    #[serde(rename = "ConfigInvalidError")]
    Invalid(InvalidErrorData),
    /// `ConfigFrontmatterError`
    #[serde(rename = "ConfigFrontmatterError")]
    Frontmatter(FrontmatterErrorData),
    /// `ConfigDirectoryTypoError`
    #[serde(rename = "ConfigDirectoryTypoError")]
    DirectoryTypo(DirectoryTypoErrorData),
    /// `ConfigRemoteAuthError`
    #[serde(rename = "ConfigRemoteAuthError")]
    RemoteAuth(RemoteAuthErrorData),
}

impl ConfigError {
    /// Tag de `ConfigJsonError`, identique a la chaine du TS.
    pub const TAG_JSON: &'static str = "ConfigJsonError";
    /// Tag de `ConfigInvalidError`, identique a la chaine du TS.
    pub const TAG_INVALID: &'static str = "ConfigInvalidError";
    /// Tag de `ConfigFrontmatterError`, identique a la chaine du TS.
    pub const TAG_FRONTMATTER: &'static str = "ConfigFrontmatterError";
    /// Tag de `ConfigDirectoryTypoError`, identique a la chaine du TS.
    pub const TAG_DIRECTORY_TYPO: &'static str = "ConfigDirectoryTypoError";
    /// Tag de `ConfigRemoteAuthError`, identique a la chaine du TS.
    pub const TAG_REMOTE_AUTH: &'static str = "ConfigRemoteAuthError";

    /// Les cinq tags, dans l'ordre de declaration du TS.
    pub const TAGS: [&'static str; 5] = [
        Self::TAG_JSON,
        Self::TAG_INVALID,
        Self::TAG_FRONTMATTER,
        Self::TAG_DIRECTORY_TYPO,
        Self::TAG_REMOTE_AUTH,
    ];

    /// Le tag de cette erreur, c'est a dire la valeur du champ `name`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Json(_) => Self::TAG_JSON,
            Self::Invalid(_) => Self::TAG_INVALID,
            Self::Frontmatter(_) => Self::TAG_FRONTMATTER,
            Self::DirectoryTypo(_) => Self::TAG_DIRECTORY_TYPO,
            Self::RemoteAuth(_) => Self::TAG_REMOTE_AUTH,
        }
    }

    /// Equivaut a `NamedError.isInstance` : le JSON porte-t-il le bon `name` ?
    ///
    /// En TS le test porte sur un `unknown` et demande seulement que `name`
    /// soit une propriete presente et egale au tag, sans regarder `data`. Sur
    /// une valeur qui n'est pas un objet, `get("name")` renvoie `None`, donc
    /// la reponse est `false` comme en TS.
    pub fn is_instance(&self, valeur: &serde_json::Value) -> bool {
        valeur.get("name") == Some(&serde_json::Value::String(self.name().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les cinq erreurs produisent exactement les cinq chaines du TypeScript,
    /// au caractere pres. Ce test verrouille la forme `{ "name", "data" }` :
    /// un `{"name":...,"path":...}` le ferait echouer.
    #[test]
    fn le_json_a_exactement_la_forme_du_typescript() {
        let cas: Vec<(ConfigError, &str)> = vec![
            (
                ConfigError::Json(JsonErrorData::new("opencode.json", None)),
                r#"{"name":"ConfigJsonError","data":{"path":"opencode.json"}}"#,
            ),
            (
                ConfigError::Invalid(InvalidErrorData::new("opencode.json", None, None)),
                r#"{"name":"ConfigInvalidError","data":{"path":"opencode.json"}}"#,
            ),
            (
                ConfigError::Frontmatter(FrontmatterErrorData::new("opencode.md", "frontmatter vide")),
                r#"{"name":"ConfigFrontmatterError","data":{"path":"opencode.md","message":"frontmatter vide"}}"#,
            ),
            (
                ConfigError::DirectoryTypo(DirectoryTypoErrorData::new(
                    "plugin",
                    "plugni",
                    "plugin",
                )),
                r#"{"name":"ConfigDirectoryTypoError","data":{"path":"plugin","dir":"plugni","suggestion":"plugin"}}"#,
            ),
            (
                ConfigError::RemoteAuth(RemoteAuthErrorData::new("https://exemple.test", "origin")),
                r#"{"name":"ConfigRemoteAuthError","data":{"url":"https://exemple.test","remote":"origin"}}"#,
            ),
        ];
        assert_eq!(cas.len(), ConfigError::TAGS.len());
        for (erreur, attendu) in cas {
            assert_eq!(
                serde_json::to_string(&erreur).unwrap(),
                *attendu,
                "forme JSON incorrecte pour {:?}",
                erreur.name()
            );
            let relu: ConfigError = serde_json::from_str(attendu).unwrap();
            assert_eq!(erreur, relu, "aller-retour JSON non fidele pour {:?}", erreur.name());
        }
    }

    /// La charge utile est bien imbriquee sous `data`, et rien n'a ete
    /// eparpille a cote du tag. Un tag interne l'aurait aplatie.
    #[test]
    fn la_charge_utile_est_imbriquee_sous_data() {
        let erreur = ConfigError::Json(JsonErrorData::new("opencode.json", None));
        let json = serde_json::to_value(&erreur).unwrap();
        let objet = json.as_object().unwrap();
        assert_eq!(objet.len(), 2, "le JSON doit avoir exactement deux cles");
        assert_eq!(objet["name"].as_str().unwrap(), "ConfigJsonError");
        assert_eq!(objet["data"].as_object().unwrap().len(), 1);
        assert!(objet["data"].get("path").is_some());
        assert!(objet.get("path").is_none(), "le payload ne doit pas etre aplati");
    }

    /// `Schema.optional` donne une cle absente, pas une cle a `null`.
    #[test]
    fn un_champ_optionnel_absent_ne_laisse_pas_de_cle_dans_le_json() {
        let erreur = ConfigError::Json(JsonErrorData::new("a.json", None));
        let json = serde_json::to_string(&erreur).unwrap();
        assert_eq!(json, r#"{"name":"ConfigJsonError","data":{"path":"a.json"}}"#);
        assert!(!json.contains("message"));

        let erreur = ConfigError::Invalid(InvalidErrorData::new("a.json", None, Some("m".to_string())));
        let json = serde_json::to_string(&erreur).unwrap();
        assert_eq!(
            json,
            r#"{"name":"ConfigInvalidError","data":{"path":"a.json","message":"m"}}"#
        );
        assert!(!json.contains("issues"));
    }

    /// Les noms de champs de la charge utile, un par un, sur les cinq erreurs.
    /// Aucun de ces noms ne comporte de majuscule interne, donc aucun `rename`
    /// n'est requis, mais le test verrouille la reponse.
    #[test]
    fn les_noms_de_champs_du_payload_sont_ceux_du_typescript() {
        let cas: Vec<(ConfigError, Vec<&str>)> = vec![
            (
                ConfigError::Json(JsonErrorData::new("a.json", Some("m".to_string()))),
                vec!["message", "path"],
            ),
            (
                ConfigError::Invalid(InvalidErrorData::new(
                    "a.json",
                    Some(Vec::new()),
                    Some("m".to_string()),
                )),
                vec!["issues", "message", "path"],
            ),
            (
                ConfigError::Frontmatter(FrontmatterErrorData::new("a.md", "m")),
                vec!["message", "path"],
            ),
            (
                ConfigError::DirectoryTypo(DirectoryTypoErrorData::new("a.json", "plugin", "plugins")),
                vec!["dir", "path", "suggestion"],
            ),
            (
                ConfigError::RemoteAuth(RemoteAuthErrorData::new("https://exemple.test", "origin")),
                vec!["remote", "url"],
            ),
        ];
        for (erreur, attendues) in cas {
            let json = serde_json::to_value(&erreur).unwrap();
            let data = json.get("data").unwrap().as_object().unwrap();
            let mut cles: Vec<&str> = data.keys().map(|c| c.as_str()).collect();
            cles.sort_unstable();
            assert_eq!(cles, attendues, "noms de champs de {:?}", erreur.name());
        }
    }

    /// `path` est une `Schema.String` obligatoire : une chaine vide est une
    /// chaine valide et doit survivre. Seuls `null` et `undefined` font
    /// disparaitre une valeur ; la tester comme si elle etait fausse ferait
    /// disparaitre `""` et divergerait du TS.
    #[test]
    fn un_chemin_vide_survit_dans_le_payload() {
        let erreur = ConfigError::DirectoryTypo(DirectoryTypoErrorData::new("", "", ""));
        assert_eq!(
            serde_json::to_string(&erreur).unwrap(),
            r#"{"name":"ConfigDirectoryTypoError","data":{"path":"","dir":"","suggestion":""}}"#
        );
        assert_eq!(
            serde_json::to_string(&Issue::new("m", Vec::new())).unwrap(),
            r#"{"message":"m","path":[]}"#
        );
    }

    /// Une liste d'anomalies vide reste une liste vide, presente dans le JSON :
    /// elle est presente a l'appel, donc `Some(vec![])`, et non `None`.
    #[test]
    fn une_liste_d_issues_vide_est_acceptee_et_reste_visible() {
        let erreur = ConfigError::Invalid(InvalidErrorData::new("a.json", Some(Vec::new()), None));
        assert_eq!(
            serde_json::to_string(&erreur).unwrap(),
            r#"{"name":"ConfigInvalidError","data":{"path":"a.json","issues":[]}}"#
        );
        let relu: ConfigError =
            serde_json::from_str(r#"{"name":"ConfigInvalidError","data":{"path":"a.json","issues":[]}}"#)
                .unwrap();
        match relu {
            ConfigError::Invalid(donnees) => assert_eq!(donnees.issues, Some(Vec::new())),
            autre => panic!("mauvaise variante : {:?}", autre.name()),
        }
    }

    /// Une seule anomalie, cas limite de la liste : elle ressort entiere.
    #[test]
    fn une_seule_anomalie_ressort_entiere() {
        let brut = concat!(
            r#"{"name":"ConfigInvalidError","data":{"path":"a.json","issues":["#,
            r#"{"message":"attendu une chaine","path":["plugins"]}"#,
            r#"]}}"#
        );
        let relu: ConfigError = serde_json::from_str(brut).unwrap();
        // La re-serialisation est verifiee avant le `match`, car le `match`
        // consomme `relu` en deplacant la charge utile.
        assert_eq!(serde_json::to_string(&relu).unwrap(), brut);
        match relu {
            ConfigError::Invalid(donnees) => {
                let issues = donnees.issues.unwrap();
                assert_eq!(issues.len(), 1);
                assert_eq!(issues[0].message, "attendu une chaine");
                assert_eq!(issues[0].path, vec!["plugins".to_string()]);
            }
            autre => panic!("mauvaise variante : {:?}", autre.name()),
        }
    }

    /// `StructWithRest` : les cles inconnues de l'Issue sont conservees, et
    /// ressortent au meme niveau que `message` et `path`.
    #[test]
    fn un_issue_conserve_les_champs_inconnus_du_typescript() {
        let brut = concat!(
            r#"{"name":"ConfigInvalidError","data":{"path":"a.json","issues":["#,
            r#"{"message":"attendu une chaine","path":["plugins"],"code":42,"expected":"number"}"#,
            r#"]}}"#
        );
        let relu: ConfigError = serde_json::from_str(brut).unwrap();
        assert_eq!(relu.name(), ConfigError::TAG_INVALID);
        assert_eq!(serde_json::to_string(&relu).unwrap(), brut, "aller-retour JSON non fidele");

        let issue = Issue::new("m", vec!["plugins".to_string()])
            .avec("code", 42)
            .avec("expected", "number");
        assert_eq!(issue.rest.len(), 2);
        assert_eq!(issue.rest.get("code").unwrap(), &serde_json::json!(42));
    }

    /// Les cinq tags, dans l'ordre du TS, lus par `name()` et par le champ
    /// `name` du JSON. Les deux lectures doivent tomber d'accord.
    #[test]
    fn les_cinq_tags_sont_lus_de_la_maniere_du_typescript() {
        let erreurs = vec![
            ConfigError::Json(JsonErrorData::new("a.json", None)),
            ConfigError::Invalid(InvalidErrorData::new("a.json", None, None)),
            ConfigError::Frontmatter(FrontmatterErrorData::new("a.md", "m")),
            ConfigError::DirectoryTypo(DirectoryTypoErrorData::new("a.json", "plugin", "plugins")),
            ConfigError::RemoteAuth(RemoteAuthErrorData::new("https://exemple.test", "origin")),
        ];
        for (erreur, attendu) in erreurs.iter().zip(ConfigError::TAGS.iter()) {
            assert_eq!(erreur.name(), *attendu);
            let json = serde_json::to_value(erreur).unwrap();
            assert_eq!(json.get("name").unwrap().as_str().unwrap(), *attendu);
            assert!(erreur.is_instance(&json));
        }
        assert_eq!(erreurs.len(), ConfigError::TAGS.len());
    }

    /// Un tag inconnu ne doit pas retomber sur une variante au hasard, et les
    /// champs obligatoires du `data` le restent.
    #[test]
    fn un_json_invalide_est_refuse() {
        for brut in [
            r#"{"name":"ConfigUnknownError","data":{}}"#,
            r#"{"name":"ConfigJsonError","data":{}}"#,
            r#"{"name":"ConfigDirectoryTypoError","data":{"path":"a.json"}}"#,
            r#"{"name":"ConfigRemoteAuthError","data":{"url":"https://exemple.test"}}"#,
            r#"{"name":"ConfigFrontmatterError","data":{"path":"a.md"}}"#,
            r#"{"name":"ConfigJsonError"}"#,
        ] {
            assert!(
                serde_json::from_str::<ConfigError>(brut).is_err(),
                "aurait du etre refuse : {}",
                brut
            );
        }
    }

    /// Une entree qui n'est pas un objet du tout n'est pas une erreur de
    /// configuration, et ne passe pas non plus le test d'instance.
    #[test]
    fn une_entree_non_objet_est_refusee() {
        for brut in [r#""ConfigJsonError""#, "[]", "null", "42"] {
            assert!(serde_json::from_str::<ConfigError>(brut).is_err());
            assert!(!ConfigError::Json(JsonErrorData::new("a.json", None))
                .is_instance(&serde_json::from_str::<serde_json::Value>(brut).unwrap()));
        }
    }
}