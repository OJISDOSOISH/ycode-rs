//! Portage Rust de `opencode/packages/core/src/config/reference.ts`.
//!
//! La source tient en vingt-deux lignes et ne contient que des declarations de
//! schema, plus une ligne d'auto-alias de module :
//!
//! ```ts
//! export * as ConfigReference from "./reference"
//!
//! import { Schema } from "effect"
//!
//! export class Git extends Schema.Class<Git>("ConfigV2.Reference.Git")({
//!   repository: Schema.String,
//!   branch: Schema.String.pipe(Schema.optional),
//!   description: Schema.String.pipe(Schema.optional),
//!   hidden: Schema.Boolean.pipe(Schema.optional),
//! }) {}
//!
//! export class Local extends Schema.Class<Local>("ConfigV2.Reference.Local")({
//!   path: Schema.String,
//!   description: Schema.String.pipe(Schema.optional),
//!   hidden: Schema.Boolean.pipe(Schema.optional),
//! }) {}
//!
//! export const Entry = Schema.Union([Schema.String, Git, Local])
//! export type Entry = typeof Entry.Type
//!
//! export const Info = Schema.Record(Schema.String, Entry)
//! export type Info = typeof Info.Type
//! ```
//!
//! C'est le schema de la section `references` du fichier de configuration, celle
//! qui nomme des repertoires locaux et des depots Git utilisables comme contexte
//! externe. Le champ est branche sous le nom `references` dans
//! `packages/core/src/config.ts` (ligne 99), et sous les **deux** noms
//! `references` (ligne 45) et `reference` (ligne 48) dans
//! `packages/core/src/v1/config/config.ts`, le second marque `@deprecated`.
//! Partout il est `Schema.optional` : ici `Info` ne porte que la forme du
//! dictionnaire, et c'est l'appelant qui applique l'absence.
//!
//! Ce fichier ne contient **aucune logique**. Il n'y a ni lecture de disque, ni
//! resolution de chemin, ni appel reseau. La seule chose a porter fidelement, ce
//! sont les **formes JSON**, parce qu'elles font l'interface avec le TypeScript et
//! avec le fichier de configuration ecrit par l'utilisateur. Tout ce qui suit est
//! donc de la structure, et les tests ne verifient que la structure.
//!
//! ## La ligne 1 : `export * as ConfigReference from "./reference"`
//!
//! Le module s'exporte lui-meme sous le nom `ConfigReference`. C'est un pur alias
//! d'espace de noms : il ne porte aucune donnee, et il n'a donc pas de traduction
//! Rust. On en garde la trace ici pour que la relecture ne le cherche pas dans le
//! code, et parce que cet alias est ce qui permet aux autres modules d'ecrire
//! `import { ConfigReference } from "./config/reference"` puis `ConfigReference.Entry`.
//! En Rust, ce role est tenu par le nom du module lui-meme.
//!
//! ## Les identifiants de schema `"ConfigV2.Reference.Git"` et `"ConfigV2.Reference.Local"`
//!
//! `Schema.Class<Self>("identifiant")` enregistre l'identifiant dans l'annuaire
//! global d'Effect, ou il sert de cle de comparaison et d'entree de documentation.
//! Rust n'a pas d'annuaire equivalent : on expose les deux chaines dans des
//! constantes, ce qui est la traduction la plus honnete. Elles ne sont utilisees
//! nulle part dans le portage, parce que rien en aval ne les consulte.
//!
//! ## Le point le plus important : l'union est SANS tag
//!
//! `Entry` est `Schema.Union([Schema.String, Git, Local])`, et `Git` comme
//! `Local` sont des `Schema.Class` **simples**, pas des `Schema.TaggedClass`.
//! Aucun champ discriminant n'est donc ajoute aux objets. Un depot git serialise
//! en `{"repository": "..."}` et un repertoire local en `{"path": "..."}` : il
//! n'y a rien d'autre.
//!
//! C'est confirme par le consommateur, `packages/core/src/config/plugin/reference.ts`,
//! qui discrimine **structurellement** :
//!
//! ```ts
//! const description = typeof entry === "string" ? undefined : entry.description
//! function local(entry: ConfigReference.Entry): entry is string | ConfigReference.Local {
//!   return typeof entry === "string"
//!     ? entry.startsWith(".") || entry.startsWith("/") || entry.startsWith("~")
//!     : "path" in entry
//! }
//! ```
//!
//! Il teste `"path" in entry`, jamais `entry.type === "local"`. Le tag `type` est
//! construit plus loin et ailleurs, par `Reference.LocalSource.make({ type: "local", ... })`,
//! qui est une autre classe. **Il ne faut donc pas** mettre `#[serde(tag = "...")]`
//! sur `Entry`, meme si la table de traductions du brief associe "ADT a tag" et
//! `enum` tague : ici il n'y a pas de tag, et en ajouter un casserait le JSON.
//! D'ou `#[serde(untagged)]`.
//!
//! Consequence a verifier en relecture : avec `untagged`, serde essaie les
//! variantes dans l'ordre de declaration, donc `Git` avant `Local`, exactement
//! comme `Schema.Union([String, Git, Local])`. Pour tout objet qui porte au plus
//! l'un des deux champs requis, l'ordre n'a aucune importance, parce que
//! `repository` et `path` sont tous deux obligatoires : `{"repository": "r"}` ne
//! peut pas etre un `Local`, et `{"path": "p"}` ne peut pas etre un `Git`. Le cas
//! pathologique est le seul objet qui porte **les deux** champs : il est traite
//! par le test `un_objet_portant_les_deux_champs_obligatoires_est_lu_comme_un_depot_git`.
//!
//! ## Les noms de champs : ici, pas de piege, mais un test quand meme
//!
//! Les sept noms de la source sont `repository`, `branch`, `description`,
//! `hidden`, `path`, plus les deux noms de classe. Aucun n'est en `camelCase`, donc
//! aucun `#[serde(rename = ...)]` n'est necessaire a la correction. On en pose
//! quand meme un explicite sur chaque champ, comme dans `config_compaction.rs`,
//! `config_watcher.rs` et `config_tool_output.rs` : il ecrit ce que le nom du champ
//! Rust dit deja, et sert de verrou si le champ est renomme un jour. Un test dedie
//! verifie quand meme le JSON produit, parce que c'est l'erreur la plus frequente
//! de tout ce portage et qu'elle est invisible de l'interieur du code Rust.
//!
//! ## Le piege `?` contre `??`
//!
//! **`reference.ts` lui-meme n'emploie ni l'un ni l'autre.** C'est une
//! declaration de schema pure : il n'y a ni ternaire, ni coalescent, ni aucune
//! expression. Il n'y a donc rien a distinguer a ce niveau, et il serait honnete
//! mais inutile d'inventer deux fonctions de conversion sans point d'appel.
//!
//! En revanche, le **consommateur** en emploie les deux, sur ces champs-la, et les
//! methodes `description`, `hidden`, `branche`, `chemin`, `depot` et `est_local`
//! ci-dessous en sont la traduction. Elles viennent de
//! `packages/core/src/config/plugin/reference.ts`, pas de `reference.ts`.
//!
//! La distinction se voit sur `description`. Cote consommateur :
//!
//! ```ts
//! const description = typeof entry === "string" ? undefined : entry.description
//! ...(description === undefined ? {} : { description }),
//! ```
//!
//! Le premier ternaire ne teste pas la veracite, il teste le **type** : il ne
//! remplace que l'etat "chaine", jamais une chaine vide. Le second teste
//! explicitement `=== undefined`, donc il laisse passer la chaine vide. Concretement,
//! `description: ""` **survit** jusqu'au `Reference.LocalSource`. Une traduction par
//! veracite l'aurait supprimee, et le texte de description de la reference aurait
//! disparu du contexte sans que rien ne signale l'erreur.
//!
//! Le seul ternaire qui teste vraiment la veracite dans ce coin du code est
//! `doc.path ? path.dirname(doc.path) : location.directory`, qui porte sur le
//! chemin du *fichier de configuration* et pas sur un champ de ce schema. Il
//! n'est donc pas porte ici.
//!
//! ## Un `??` reel dans la meme famille, a titre de contre-exemple
//!
//! `packages/core/src/v1/config/migrate.ts` ligne 45 fait
//!
//! ```ts
//! references: info.references ?? info.reference,
//! ```
//!
//! C'est le seul `??` du coin, et il montre pourquoi la distinction vaut d'etre
//! ecrite quelque part. Il teste la **nullite** : un `references: {}`, un objet
//! vide mais **present**, survit et masque entierement le `reference`
//! deprecie qui porte peut-etre dix entrees. Une section `references: ""` n'est
//! meme pas representable, puisque la section est un objet. Une traduction par
//! veracite, ou un `!est_vide()` glisse quelque part, aurait fait remonter les
//! dix entrees et aurait change le contenu du fichier migre. Le test
//! `une_section_vide_presente_masque_la_section_depreciee` verrouille le cas.
//!
//! ## `Schema.String` accepte la chaine vide
//!
//! Ni `repository`, ni `branch`, ni `description`, ni `path` ne portent de `check`.
//! Ce sont des `Schema.String` nus. Donc `repository: ""` est un depot git valide
//! du point de vue du schema, et `description: ""` est une description presente
//! mais vide. Aucune validation de type ne les distingue d'une chaine pleine, et
//! on n'en ajoute pas : les ajouter serait inventer une regle que la source
//! n'a pas.
//!
//! C'est coherent avec le consommateur : `local()` teste `"".startsWith(".")`,
//! `"".startsWith("/")` et `"".startsWith("~")`, qui valent tous les trois `false`,
//! donc une chaine vide n'est **pas** locale et part dans la branche git, avec
//! `repository: ""`. C'est le comportement voulu, et il est ecrit dans un test.
//!
//! ## L'ordre des cles de `Info`
//!
//! `Info` est un `Schema.Record(Schema.String, Entry)`, donc un objet JSON. On le
//! porte par `BTreeMap<String, Entry>` comme l'impose la table de traductions, ce
//! qui a un effet de bord a signaler : `BTreeMap` re-emet les cles **triees**, alors
//! qu'un objet JavaScript conserve l'ordre d'insertion, donc l'ordre du fichier de
//! configuration. Le JSON reste equivalent comme objet, mais pas identique octet
//! pour octet si les alias n'etaient pas deja tries. C'est la seule divergence
//! connue de ce fichier, et elle est voulue : la determinisme l'emporte.
//!
//! ## Proprietes inconnues
//!
//! Comme en TypeScript, les champs en trop sont ignores et non refuses :
//! `Schema.Class` ne pose pas de controle d'exces. On ne met donc pas
//! `deny_unknown_fields`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifiant de schema enregistre par `Schema.Class` pour `Git`.
///
/// Rust n'a pas d'annuaire de schemas : cette constante est la seule trace
/// de l'enregistrement. Elle n'est lue par personne.
pub const GIT_SCHEMA_ID: &str = "ConfigV2.Reference.Git";

/// Identifiant de schema enregistre par `Schema.Class` pour `Local`.
///
/// Rust n'a pas d'annuaire de schemas : cette constante est la seule trace
/// de l'enregistrement. Elle n'est lue par personne.
pub const LOCAL_SCHEMA_ID: &str = "ConfigV2.Reference.Local";

/// Depot Git nomme par la section `references` de la configuration.
///
/// Schema d'origine : `ConfigV2.Reference.Git`.
///
/// `repository` est obligatoire. Les trois autres champs sont optionnels et leur
/// absence ne doit pas se voir dans le JSON : `skip_serializing_if` retire la cle
/// plutot que d'ecrire `null`, ce que le TypeScript n'ecrit jamais.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Git {
    /// Adresse du depot. Chaine vide acceptee, aucun `check` dans la source.
    #[serde(rename = "repository")]
    pub repository: String,

    /// Branche a checkout. Absente = comportement par defaut du consommateur.
    #[serde(rename = "branch", default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,

    /// Texte de description lu par l'agent. Chaine vide acceptee et survive.
    #[serde(rename = "description", default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Masque la reference dans les listes proposees a l'utilisateur.
    #[serde(rename = "hidden", default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

/// Repertoire local nomme par la section `references` de la configuration.
///
/// Schema d'origine : `ConfigV2.Reference.Local`.
///
/// Meme forme que `Git` moins `branch` : c'est `branch` qui fait la difference
/// entre les deux cote code, et `path` qui fait la difference cote JSON, puisque
/// l'union ne porte aucun tag. Voir le test sur la forme des deux objets.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Local {
    /// Chemin du repertoire. Chaine vide acceptee, aucun `check` dans la source.
    #[serde(rename = "path")]
    pub path: String,

    /// Texte de description lu par l'agent. Chaine vide acceptee et survive.
    #[serde(rename = "description", default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Masque la reference dans les listes proposees a l'utilisateur.
    #[serde(rename = "hidden", default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

/// Une entree de la section `references`.
///
/// Schema d'origine : `Schema.Union([Schema.String, Git, Local])`.
///
/// **L'union n'a pas de tag.** `Chaine` est le raccourci le plus court, `Git` et
/// `Local` sont deux formes d'objet distinctes, et le JSON ne les distingue que
/// par la presence de `repository` ou de `path`. D'ou `untagged` : mettre un tag
/// ici produirait un champ que le TypeScript n'emet jamais.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    /// Le raccourci `"~/notes"` ou `"https://github.com/x/y"`, ecrit en clair.
    Chaine(String),

    /// Un depot git donne en forme objet, seule facon de fixer `branch`.
    Git(Git),

    /// Un repertoire local donne en forme objet.
    Local(Local),
}

impl Entry {
    /// La description, ou `None` quand l'entree est une chaine nue.
    ///
    /// Traduit `typeof entry === "string" ? undefined : entry.description`.
    ///
    /// Le ternaire teste le **type**, pas la veracite. Une description vide
    /// donne donc `Some("")` et non `None`, et elle remonte jusqu'a la source de
    /// reference. C'est le point a ne pas simplifier en `.filter(|d| !d.is_empty())`.
    pub fn description(&self) -> Option<&str> {
        match self {
            Entry::Chaine(_) => None,
            Entry::Git(git) => git.description.as_deref(),
            Entry::Local(local) => local.description.as_deref(),
        }
    }

    /// Le drapeau `hidden`, ou `None` quand l'entree est une chaine nue.
    ///
    /// Traduit `typeof entry === "string" ? undefined : entry.hidden`.
    /// Meme distinction que pour `description` : le test porte sur le type.
    pub fn hidden(&self) -> Option<bool> {
        match self {
            Entry::Chaine(_) => None,
            Entry::Git(git) => git.hidden,
            Entry::Local(local) => local.hidden,
        }
    }

    /// La branche, presente seulement sur la forme objet `Git`.
    ///
    /// Traduit `entry.branch === undefined ? {} : { branch: entry.branch }`. Le
    /// consommateur ne lit ce champ que dans sa branche git, et une chaine nue
    /// n'a pas de propriete `branch`, donc seules les entrees `Git` en portent
    /// une. Le schema `Local` ne declare d'ailleurs pas `branch` du tout.
    pub fn branche(&self) -> Option<&str> {
        match self {
            Entry::Git(git) => git.branch.as_deref(),
            Entry::Chaine(_) | Entry::Local(_) => None,
        }
    }

    /// Le chemin a resoudre, ou `None` pour une entree git.
    ///
    /// Traduit `typeof entry === "string" ? entry : entry.path`, qui n'est lu que
    /// dans la branche locale de `est_local`. Une entree `Git` n'a donc pas de
    /// chemin, et renvoyer son `repository` ici serait faux : on renvoie `None`.
    ///
    /// **Piege d'appel.** Pour une chaine nue, cette methode rend la chaine
    /// elle-meme, y compris quand la chaine est une adresse de depot et pas un
    /// chemin : `"github.com/x/y".chemin()` rend `Some("github.com/x/y")`. C'est
    /// faithful au TypeScript, qui fait de meme, et c'est `est_local()` qui
    /// departage les deux lectures. Il faut donc toujours passer par
    /// `est_local()` avant d'appeler `chemin()` ou `depot()`.
    pub fn chemin(&self) -> Option<&str> {
        match self {
            Entry::Chaine(valeur) => Some(valeur.as_str()),
            Entry::Local(local) => Some(local.path.as_str()),
            Entry::Git(_) => None,
        }
    }

    /// Le depot a cloner, ou `None` pour une entree locale.
    ///
    /// Traduit `typeof entry === "string" ? entry : entry.repository`. Pour une
    /// chaine nue, c'est la chaine elle-meme qui sert d'adresse de depot, meme si
    /// elle ressemble a un chemin.
    pub fn depot(&self) -> Option<&str> {
        match self {
            Entry::Chaine(valeur) => Some(valeur.as_str()),
            Entry::Git(git) => Some(git.repository.as_str()),
            Entry::Local(_) => None,
        }
    }

    /// L'entree se resout-elle comme un repertoire local ?
    ///
    /// Traduit la fonction `local` de `config/plugin/reference.ts` :
    ///
    /// ```ts
    /// function local(entry: ConfigReference.Entry): entry is string | ConfigReference.Local {
    ///   return typeof entry === "string"
    ///     ? entry.startsWith(".") || entry.startsWith("/") || entry.startsWith("~")
    ///     : "path" in entry
    /// }
    /// ```
    ///
    /// Le test porte sur les **trois premiers caracteres**, jamais sur le fait que
    /// la chaine soit vide ou non. Une chaine vide ne commence par rien de tout
    /// cela, elle est donc **pas** locale, et part dans la branche git. Une
    /// traduction par veracite aurait donne le resultat inverse, ce qui est
    /// exactement la divergence que le brief signale.
    pub fn est_local(&self) -> bool {
        match self {
            Entry::Chaine(valeur) => {
                valeur.starts_with('.') || valeur.starts_with('/') || valeur.starts_with('~')
            }
            Entry::Local(_) => true,
            Entry::Git(_) => false,
        }
    }
}

/// Contenu du champ `references` de la configuration : alias vers un chemin.
///
/// Schema d'origine : `Schema.Record(Schema.String, Entry)`.
///
/// L'alias de la cle est le nom donne a la reference. cote consommateur il
/// passe par `validAlias`, qui refuse la chaine vide et les alias contenant un
/// `/`, un espace, un accent grave ou une virgule. Ce filtre n'est pas porte ici :
/// c'est une regle du plugin, pas du schema, et `reference.ts` ne le contient pas.
///
/// L'ordre des cles n'est pas celui du fichier de configuration : voir la note sur
/// `BTreeMap` dans la documentation du module, et le test
/// `la_section_reemet_ses_cles_triees_et_plus_dans_l_ordre_du_fichier`.
///
/// La **presence** de la section n'est pas portee ici : `Schema.optional` est
/// applique par l'appelant, qui embarque donc ce type dans un `Option<Info>`.
/// Une section vide reste une section presente, et les deux ne se confondent
/// pas : c'est ce que exige le `??` de `v1/config/migrate.ts`, dont le
/// contre-exemple est developpe dans la documentation du module.
pub type Info = BTreeMap<String, Entry>;

#[cfg(test)]
mod tests {
    use super::{Entry, Git, Info, Local};
    use serde_json::json;

    fn git_complet() -> Git {
        Git {
            repository: "https://github.com/opencode/opencode".to_string(),
            branch: Some("dev".to_string()),
            description: Some("le code".to_string()),
            hidden: Some(true),
        }
    }

    fn local_complet() -> Local {
        Local {
            path: "~/notes".to_string(),
            description: Some("mes notes".to_string()),
            hidden: Some(false),
        }
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Le piege numero un du portage. Les sept noms de `reference.ts` sont des
        // mots bascules, donc deja identiques en Rust, mais le test le verifie
        // quand meme : c'est l'erreur la plus frequente de tout ce portage et elle
        // est invisible de l'interieur du code Rust.
        let git = serde_json::to_value(git_complet()).unwrap();
        let objet_git = git.as_object().unwrap();
        assert_eq!(objet_git.len(), 4, "le depot git a exactement quatre champs");
        assert_eq!(objet_git["repository"], "https://github.com/opencode/opencode");
        assert_eq!(objet_git["branch"], "dev");
        assert_eq!(objet_git["description"], "le code");
        assert_eq!(objet_git["hidden"], true);

        let local = serde_json::to_value(local_complet()).unwrap();
        let objet_local = local.as_object().unwrap();
        assert_eq!(objet_local.len(), 3, "le repertoire local a exactement trois champs");
        assert_eq!(objet_local["path"], "~/notes");
        assert_eq!(objet_local["description"], "mes notes");
        assert_eq!(objet_local["hidden"], false);

        // Aucune majuscule interne : c'est le controle qui attraperait un
        // `repositoryID` ou un `descriptionID` mal recopie.
        for nom in objet_git.keys().chain(objet_local.keys()) {
            assert!(
                nom.chars().all(|c| c.is_ascii_lowercase()),
                "le nom de champ {} contient autre chose que des minuscules",
                nom
            );
        }
    }

    #[test]
    fn un_nom_de_champ_mal_casse_ou_ajoute_un_souligne_est_refuse_ou_ignore() {
        // Le piege numero un du portage. Les cinq noms de `reference.ts` sont des
        // mots simples, sans majuscule interne ni trait d'union, donc la forme
        // `snake_case` leur est identique et il n'y a rien a distinguer sur ce
        // fichier. Le controle equivalent porte donc sur la casse et sur les
        // decorateurs, et il doit etre strict : une cle mal ecrite n'est ni
        // acceptee, ni corrigee silencieusement.
        //
        // `repository` et `path` sont obligatoires, donc la faute se voit tout
        // de suite.
        assert!(serde_json::from_value::<Entry>(json!({ "Repository": "github.com/x/y" })).is_err());
        assert!(serde_json::from_value::<Entry>(json!({ "Path": "./docs" })).is_err());
        // La forme correcte est bien acceptee, pour que le test ne passe pas
        // par un refus general de l'union.
        let correct: Entry = serde_json::from_value(json!({ "repository": "github.com/x/y" })).unwrap();
        assert!(matches!(correct, Entry::Git(_)));

        // Sur un champ optionnel, la faute est invisible : `Branch` n'est pas
        // `branch` et `branch_name` non plus, donc la branche est lue comme
        // absente. C'est le symptome typique d'une faute de casse, celui que
        // rien ne signale dans la production, et le test le rend visible.
        let entree: Entry =
            serde_json::from_value(json!({ "repository": "github.com/x/y", "Branch": "dev" })).unwrap();
        assert_eq!(entree.branche(), None, "un nom mal casse ne vaut pas le champ vrai");
        assert!(serde_json::to_value(&entree).unwrap().get("Branch").is_none());

        let souligne: Entry = serde_json::from_value(
            json!({ "repository": "github.com/x/y", "branch_name": "dev", "hidden_flag": true }),
        )
        .unwrap();
        assert_eq!(souligne.branche(), None, "branch_name ne vaut pas branch");
        assert_eq!(souligne.hidden(), None, "hidden_flag ne vaut pas hidden");
    }

    #[test]
    fn un_champ_optionnel_absent_disparait_du_json_et_ne_devient_pas_null() {
        // `Schema.optional` retire la cle. Ecrire `null` produirait un JSON
        // que le TypeScript n'emet jamais et que son schema relit differemment.
        let git = Git {
            repository: "github.com/x/y".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_value(&git).unwrap();
        assert_eq!(json, json!({ "repository": "github.com/x/y" }));
        assert!(json.get("branch").is_none());
        assert!(json.get("description").is_none());
        assert!(json.get("hidden").is_none());

        let local = Local {
            path: "./docs".to_string(),
            ..Default::default()
        };
        assert_eq!(serde_json::to_value(&local).unwrap(), json!({ "path": "./docs" }));
    }

    #[test]
    fn un_champ_optionnel_valant_null_vaut_une_absence() {
        // `Schema.optional` n'est pas `exact` par defaut : `null` est traite
        // comme `undefined`. Serde fait pareil pour un `Option`.
        let git: Git = serde_json::from_value(json!({
            "repository": "github.com/x/y",
            "branch": null,
            "description": null,
            "hidden": null
        }))
        .unwrap();
        assert_eq!(git, Git { repository: "github.com/x/y".to_string(), ..Default::default() });
    }

    #[test]
    fn l_entree_raccourci_reste_une_chaine_et_les_deux_formes_d_objet_restent_des_objets() {
        // L'union ne porte AUCUN tag. Un depot git est `{"repository": ...}`, un
        // repertoire local est `{"path": ...}`, et rien d'autre. Si un tag
        // discriminait les variantes, le JSON ne serait plus compatible.
        let entrees = vec![
            (Entry::Chaine("~/notes".to_string()), json!("~/notes")),
            (Entry::Git(git_complet()), git_complet_json()),
            (Entry::Local(local_complet()), local_complet_json()),
        ];
        for (entree, attendu) in entrees {
            assert_eq!(serde_json::to_value(&entree).unwrap(), attendu);
        }
    }

    #[test]
    fn chaque_forme_d_objet_se_retrouve_dans_la_bonne_variante() {
        let depuis_git: Entry =
            serde_json::from_value(json!({ "repository": "github.com/x/y" })).unwrap();
        assert_eq!(depuis_git, Entry::Git(Git { repository: "github.com/x/y".to_string(), ..Default::default() }));

        let depuis_local: Entry = serde_json::from_value(json!({ "path": "./docs" })).unwrap();
        assert_eq!(depuis_local, Entry::Local(Local { path: "./docs".to_string(), ..Default::default() }));

        let depuis_chaine: Entry = serde_json::from_value(json!("github.com/x/y")).unwrap();
        assert_eq!(depuis_chaine, Entry::Chaine("github.com/x/y".to_string()));

        // La chaine vide reste une chaine vide : `Schema.String` est nu et le
        // schema n'impose aucune longueur.
        let vide: Entry = serde_json::from_value(json!("")).unwrap();
        assert_eq!(vide, Entry::Chaine(String::new()));
    }

    #[test]
    fn une_entree_sans_Champ_obligatoire_est_refusee_a_la_lecture() {
        // Ni `repository` ni `path` : aucune variante ne convient, l'union echoue.
        // C'est bien le comportement du TypeScript, qui refuse aussi `{}`.
        assert!(serde_json::from_value::<Entry>(json!({})).is_err());
        assert!(serde_json::from_value::<Entry>(json!({ "description": "rien" })).is_err());
        // Un type incorrect est refuse de meme.
        assert!(serde_json::from_value::<Entry>(json!({ "repository": 42 })).is_err());
        assert!(serde_json::from_value::<Entry>(json!({ "path": "./docs", "hidden": "oui" })).is_err());
        assert!(serde_json::from_value::<Entry>(json!(42)).is_err());
    }

    #[test]
    fn un_objet_portant_les_deux_champs_obligatoires_est_lu_comme_un_depot_git() {
        // Le seul objet que les deux formes d'objet peuvent accepter, et le cas
        // pathologique annonce dans la documentation du module.
        //
        // `#[serde(untagged)]` essaie les variantes dans l'ordre de declaration,
        // donc `Git` avant `Local`, comme `Schema.Union([String, Git, Local])`.
        // `Git` gagne parce que `repository` est present, et `path` devient alors
        // un champ en trop, donc un champ ignore. Le TypeScript prend la meme
        // branche et perd `path` lui aussi a la reecriture : le comportement
        // diverge sur le papier mais pas en pratique.
        let entree: Entry =
            serde_json::from_value(json!({ "repository": "github.com/x/y", "path": "./docs" })).unwrap();
        assert_eq!(
            entree,
            Entry::Git(Git { repository: "github.com/x/y".to_string(), ..Default::default() })
        );

        // Consequence : `est_local()` est faux, donc le consommateur part en
        // git et `path` est perdu. C'est bien ce que fait `"path" in entry` sur
        // une entree decodee en `Git`, qui n'a pas de propriete `path`.
        assert!(!entree.est_local());
        assert_eq!(entree.depot(), Some("github.com/x/y"));
        assert_eq!(entree.chemin(), None);
        assert_eq!(serde_json::to_value(&entree).unwrap(), json!({ "repository": "github.com/x/y" }));
    }

    #[test]
    fn une_chaine_vide_est_un_depot_git_et_pas_un_repertoire_local() {
        // Le piege de veracite. `local()` teste `"".startsWith(".")` et ses deux
        // soeurs, qui valent tous `false`. La chaine vide n'est donc pas locale,
        // et part dans la branche git avec `repository: ""`. Une traduction par
        // veracite aurait dit l'inverse.
        let vide = Entry::Chaine(String::new());
        assert!(!vide.est_local());
        assert_eq!(vide.depot(), Some(""));

        // Et la table entiere du predicat, sur les trois prefixes lus par la source.
        assert!(Entry::Chaine(".".to_string()).est_local());
        assert!(Entry::Chaine("./docs".to_string()).est_local());
        assert!(Entry::Chaine("/etc/notes".to_string()).est_local());
        assert!(Entry::Chaine("~/notes".to_string()).est_local());
        assert!(Entry::Chaine("~notes".to_string()).est_local());
        assert!(!Entry::Chaine("github.com/x/y".to_string()).est_local());
        assert!(!Entry::Chaine("HTTPS://github.com/x/y".to_string()).est_local());
    }

    #[test]
    fn la_forme_objet_est_locale_seulement_si_c_est_un_repertoire() {
        // `local()` teste `"path" in entry` : la forme git n'a pas de `path`,
        // donc elle part en git, meme si sa chaine commence par un slash.
        assert!(Entry::Local(local_complet()).est_local());
        assert!(!Entry::Git(Git { repository: "/srv/git".to_string(), ..Default::default() }).est_local());
    }

    #[test]
    fn une_description_vide_survit_la_lecture_de_l_entree() {
        // Le piege `?` contre `??`. Le ternaire du consommateur teste le TYPE de
        // l'entree, pas sa veracite, et le test suivant teste `=== undefined`.
        // Donc `description: ""` est une description presente et vide, et elle
        // doit remonter. Un filtre par veracite l'aurait fait disparaitre.
        let entree: Entry = serde_json::from_value(json!({ "path": "./docs", "description": "" })).unwrap();
        assert_eq!(entree.description(), Some(""));
        assert_eq!(
            serde_json::to_value(&entree).unwrap(),
            json!({ "path": "./docs", "description": "" })
        );

        // En revanche une entree chaine n'a pas de description, meme si la chaine
        // est vide : la, c'est bien `undefined`.
        assert_eq!(Entry::Chaine(String::new()).description(), None);
    }

    #[test]
    fn un_flag_hidden_a_false_est_different_d_une_absence_de_flag() {
        // `hidden: false` est une decision de l'utilisateur, pas une absence.
        // Confondre les deux afficherait une reference que l'on voulait cacher.
        let visible: Entry = serde_json::from_value(json!({ "path": "./docs", "hidden": false })).unwrap();
        assert_eq!(visible.hidden(), Some(false));
        let neutre: Entry = serde_json::from_value(json!({ "path": "./docs" })).unwrap();
        assert_eq!(neutre.hidden(), None);
    }

    #[test]
    fn branche_et_chemin_ne_sont_jamais_presents_tous_les_deux() {
        // Le consommateur lit `entry.branch` seulement dans sa branche git, et
        // `entry.path` seulement dans sa branche locale. Aucun `match` ne peut
        // donc renvoyer les deux a la fois : c'est le schema `Local` qui ne
        // declare pas `branch`.
        let git = Entry::Git(git_complet());
        assert_eq!(git.branche(), Some("dev"));
        assert_eq!(git.chemin(), None);

        let local = Entry::Local(local_complet());
        assert_eq!(local.chemin(), Some("~/notes"));
        assert_eq!(local.branche(), None);

        // Une chaine nue fournit la meme valeur des deux cotes, parce que le
        // consommateur fait `typeof entry === "string" ? entry : ...`.
        let courte = Entry::Chaine("github.com/x/y".to_string());
        assert_eq!(courte.depot(), Some("github.com/x/y"));
        assert_eq!(courte.chemin(), Some("github.com/x/y"));
        assert_eq!(courte.branche(), None);
    }

    #[test]
    fn chemin_et_depot_ne_se_lisent_qu_apres_est_local() {
        // `chemin()` et `depot()` sont les deux lectures du ternaire du
        // consommateur, et c'est `est_local()` qui departage, pas la forme de
        // l'entree. Une chaine nue rend donc les DEUX valeurs a la fois, y
        // compris quand elle est une adresse de depot et pas un chemin. C'est
        // faithful au TypeScript, qui fait exactement de meme, et c'est la
        // raison pour laquelle les deux lectures ne doivent jamais etre appelees
        // l'une sans l'autre.
        let adresse = Entry::Chaine("github.com/x/y".to_string());
        assert!(!adresse.est_local());
        assert_eq!(adresse.depot(), Some("github.com/x/y"));
        assert_eq!(adresse.chemin(), Some("github.com/x/y"));
        assert_eq!(adresse.chemin(), adresse.depot(), "les deux lectures rendent la meme chaine");

        let chemin = Entry::Chaine("./docs".to_string());
        assert!(chemin.est_local());
        assert_eq!(chemin.chemin(), Some("./docs"));
        // `depot()` rend quand meme la chaine, mais le consommateur ne le lit
        // pas dans la branche locale : il n'y a donc rien a cloner.
        assert_eq!(chemin.depot(), Some("./docs"));
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // `Schema.Class` ne refuse pas les champs en trop : on ne pose pas
        // `deny_unknown_fields`.
        let entree: Entry =
            serde_json::from_value(json!({ "repository": "github.com/x/y", "depth": 1 })).unwrap();
        assert_eq!(entree, Entry::Git(Git { repository: "github.com/x/y".to_string(), ..Default::default() }));
    }

    #[test]
    fn une_section_vide_donne_un_objet_json_vide() {
        // `Schema.Record(String, Entry)` vide : c'est un objet vide, pas `null`
        // et pas une liste. Cote consommateur, c'est ce que donne
        // `doc.info.references ?? {}` quand `references` est absent.
        let info: Info = Info::new();
        assert_eq!(serde_json::to_string(&info).unwrap(), "{}");
        assert!(serde_json::from_value::<Info>(json!({})).unwrap().is_empty());
    }

    #[test]
    fn un_alias_unique_avec_les_trois_formes_survit_a_un_allers_retour_json() {
        // Le cas reel : `references` du fichier de configuration, relu puis
        // reecrit. Le contenu doit rester identique.
        let json = json!({
            "notes": "~/notes",
            "code": { "repository": "https://github.com/opencode/opencode", "branch": "dev" },
            "docs": { "path": "./docs", "description": "la doc", "hidden": true }
        });
        let info: Info = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(info.len(), 3);
        assert_eq!(info["notes"], Entry::Chaine("~/notes".to_string()));
        assert_eq!(info["docs"].hidden(), Some(true));
        assert_eq!(info["code"].branche(), Some("dev"));

        // `BTreeMap` re-emet les cles triees, donc l'objet ressort dans un autre
        // ordre que le fichier. Attention : l'egalite de `serde_json::Value` ne
        // tient pas compte de l'ordre des cles, donc cette assertion ne voit pas
        // la difference. C'est le test suivant, qui compare la chaine
        // serialisee, qui la verifie reellement.
        assert_eq!(serde_json::to_value(&info).unwrap(), json);
    }

    #[test]
    fn la_section_reemet_ses_cles_triees_et_plus_dans_l_ordre_du_fichier() {
        // Divergence connue et voulue de ce fichier. Un objet JavaScript garde
        // l'ordre d'insertion, donc l'ordre du fichier de configuration ;
        // `BTreeMap` trie. Le JSON reste equivalent comme objet, mais pas
        // identique octet pour octet si les alias n'etaient pas deja tries.
        let mut info = Info::new();
        info.insert("zeta".to_string(), Entry::Chaine("~/z".to_string()));
        info.insert("milieu".to_string(), Entry::Chaine("~/m".to_string()));
        info.insert("alpha".to_string(), Entry::Chaine("~/a".to_string()));

        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"alpha":"~/a","milieu":"~/m","zeta":"~/z"}"#
        );

        // Le meme aller-retour sur une seule entree, pour verifier au passage
        // que l'union sans tag re-emet bien la forme qu'elle a lue.
        let melange: Info = serde_json::from_value(json!({ "b": "./b", "a": "./a" })).unwrap();
        assert_eq!(serde_json::to_string(&melange).unwrap(), r#"{"a":"./a","b":"./b"}"#);
    }

    #[test]
    fn une_section_vide_presente_masque_la_section_depreciee() {
        // Le `??` de `v1/config/migrate.ts` : `info.references ?? info.reference`.
        // Il teste la nullite, donc une section vide mais presente gagne contre
        // une section depreciee pleine. Une traduction par veracite aurait
        // rendu l'inverse, et la migration aurait change le contenu du fichier.
        //
        // `Info` ne porte pas la presence de la section, `Schema.optional` etant
        // applique par l'appelant : la distinction porte donc sur le `Option`
        // qui l'enveloppe, et c'est ce `Option` que le test manipule.
        fn fusionne(a: Option<Info>, b: Option<Info>) -> Option<Info> {
            a.or(b)
        }

        let mut plein = Info::new();
        plein.insert("notes".to_string(), Entry::Chaine("~/notes".to_string()));
        let vide = Info::new();

        // `references` absent : la section depreciee prend le relais.
        assert_eq!(fusionne(None, Some(plein.clone())).map(|i| i.len()), Some(1));
        // `references` present et vide : il gagne, et la section depreciee est
        // ignoree meme si elle porte des entrees.
        let gagne = fusionne(Some(vide), Some(plein.clone())).unwrap();
        assert!(gagne.is_empty(), "une section vide presente masque la section depreciee");
        // Les deux absents : rien du tout, pas une section vide. Le `{}` du
        // plugin (`doc.info.references ?? {}`) est applique plus loin, pas ici.
        assert!(fusionne(None, None).is_none());
    }

    fn git_complet_json() -> serde_json::Value {
        json!({
            "repository": "https://github.com/opencode/opencode",
            "branch": "dev",
            "description": "le code",
            "hidden": true
        })
    }

    fn local_complet_json() -> serde_json::Value {
        json!({
            "path": "~/notes",
            "description": "mes notes",
            "hidden": false
        })
    }
}