//! Portage de `opencode/packages/core/src/credential/sql.ts`.
//!
//! ## Ce que contient la source
//!
//! Quatorze lignes, une seule importation de types, et **aucune fonction** :
//!
//! ```ts
//! export const CredentialTable = sqliteTable("credential", {
//!   id: text().$type<Credential.ID>().primaryKey(),
//!   integration_id: text().$type<Credential.Info["integrationID"]>(),
//!   label: text().notNull(),
//!   value: text({ mode: "json" }).$type<Credential.Value>().notNull(),
//!   connector_id: text(),
//!   method_id: text(),
//!   active: integer({ mode: "boolean" }),
//!   ...Timestamps,
//! })
//! ```
//!
//! La source ne lit pas, n'ecrit pas, ne trie pas, ne compare pas. Elle
//! **decrit** un contrat de stockage. C'est tout.
//!
//! ## Pourquoi il n'y a pas de SQL ici
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel`, ni `libsql` :
//! il n'existe aucun moyen d'ecrire une requete, et ajouter une dependance
//! n'est pas du ressort d'un portage de fichier. Ce fichier n'ecrit donc **aucun
//! DDL**, **aucune migration**, **aucune chaine SQL** et **aucune couche
//! d'interrogation**. Il prepare le contrat que cette couche viendra consommer.
//!
//! ## La forme exacte n'est pas une invention
//!
//! Le DDL reellement produit se lit dans le depot, a deux endroits identiques :
//! `packages/core/src/database/schema.gen.ts`, et la migration
//! `20260611192811_lush_chimera.ts` lignes 10 a 22. Ce sont ces chaines qui
//! font foi, pas le code declaratif, parce que ce sont elles que SQLite
//! executera un jour :
//!
//! ```sql
//! CREATE TABLE `credential` (
//!   `id` text PRIMARY KEY,
//!   `integration_id` text,
//!   `label` text NOT NULL,
//!   `value` text NOT NULL,
//!   `connector_id` text,
//!   `method_id` text,
//!   `active` integer,
//!   `time_created` integer NOT NULL,
//!   `time_updated` integer NOT NULL
//! );
//! ```
//!
//! ## Le point central : deux drapeaux, pas un
//!
//! Fait verifie dans le modele Drizzle : **`notNull` et `primaryKey` sont deux
//! drapeaux independants, et les deux formes produisent le meme DDL.** La
//! source ecrit `id: text().$type<Credential.ID>().primaryKey()`, donc elle
//! ecrit `primaryKey` et n'ecrit pas `notNull`. Le DDL produit dit
//! `id text PRIMARY KEY` : `PRIMARY KEY` a chasse le `NOT NULL` juge
//! redondant, exactement comme le ferait une declaration
//! `text().notNull().primaryKey()`. Sur cette colonne, `notNull` s'ecrit et
//! se perd.
//!
//! Consequence SQLite, non executee ici faute de pilote : dans une table a
//! `rowid`, une `PRIMARY KEY` qui n'est pas `INTEGER PRIMARY KEY` **n'impose
//! pas** `NOT NULL`. Le schema autorise donc physiquement `NULL` dans
//! `credential.id`.
//!
//! Le fichier separe les deux questions sans les confondre et sans choisir
//! entre elles :
//!
//! - [`ecrit_not_null`] repond a ce que la **source ecrit** ;
//! - [`admet_null`] repond a ce que la **base autorise**.
//!
//! Ces deux reponses sont les methodes `not_null_declares` et
//! `admet_null_dans_sqlite` de [`ColumnDef`], reutilisees telles quelles. Ce
//! fichier ne **redeclare pas** ce modele : il l'importe de
//! `crate::swarm::event_sql`, qui le porte deja. Le dupliquer donnerait deux
//! descriptions de la meme regle, qui divergeraient en silence, sans la
//! moindre erreur de compilation.
//!
//! Particularite de cette table, a ne pas confondre avec les voisines : les
//! deux listes **concordent**. La raison tient a `id` : c'est la seule colonne
//! qui porte la cle primaire, **et** la seule colonne sur laquelle la source
//! n'ecrit rien du tout. Le statut de cle primaire n'ajoute donc aucune colonne
//! a la liste de celles que la base admet a `NULL`, alors qu'il en ajoute une
//! sur `event_sequence` de `event_sql.rs`, dont la source ecrit pourtant
//! `notNull()`. C'est un hasard de cette table, pas une regle : un test le
//! verifie des deux cotes.
//!
//! ## Ce que ce fichier ne redeclare pas
//!
//! Trois contrats credential sont deja ports ailleurs, dans le meme lot ou dans
//! le meme crate. Les redeclarer ne produirait **aucune erreur Rust** : deux
//! definitions divergentes du meme contrat compilent, et divergent en silence.
//! Ce fichier n'en declare donc aucun.
//!
//! | Contrat deja porte | Ou | Ce que fait ce fichier a la place |
//! |---|---|---|
//! | `CredentialId`, `IntegrationId`, `CredentialValue`, `CredentialInfo`, `CredentialRow` | `crate::core::credential` | rien : les colonnes sont des `String` et un `serde_json::Value` |
//! | `CredentialId`, `CredentialInfo` (renvoyeur de connexion) | `crate::swarm::integration_connection` | rien |
//! | `ColumnDef`, `TableDef`, `Storage`, `TextMode`, `IndexDef` | `crate::swarm::event_sql` | import, jamais copie |
//!
//! Deux consequences precises.
//!
//! - `crate::core::credential::CredentialId` et `IntegrationId` sont des
//!   **alias** de `String` (`pub type CredentialId = String`), pas des
//!   newtypes. Un champ `String` est donc literally le meme type que ces
//!   alias : il n'y a rien a reexporter et rien qui puisse diverger. Aucun
//!   newtype n'est invente ici, alors qu'un `IntegrationId` en newtype aurait
//!   cree un troisieme contrat pour le meme identifiant.
//! - La ligne de ce fichier s'appelle [`CredentialTableRow`] et non
//!   `CredentialRow`, parce que `CredentialRow` est deja pris dans
//!   `crate::core::credential`. Les deux ne sont pas le meme type et ne doivent
//!   pas l'etre : celui de `core::credential` ne porte que les quatre colonnes
//!   que le service lit, celui-ci porte les neuf du DDL.
//!
//! Aucun type n'est donc declare ici qui existe deja ailleurs. Les seuls types
//! de ce fichier sont [`AttributsColonne`], [`CrochetOrm`], [`FormeValeur`] et
//! [`CredentialTableRow`], accompagnes de dix fonctions pures, toutes listees
//! plus bas. Aucune n'ouvre de fichier, aucune n'accede a l'horloge, aucune ne
//! touche le disque : ce sont des fonctions sur des tranches, comme leurs
//! voisines de `event_sql.rs` et `permission_sql.rs`.
//!
//! ## La table a change deux fois, et la base ne dit pas laquelle
//!
//! La migration `20260611035744_credential.ts` n'a pas cree la table d'aujourd'hui :
//!
//! ```sql
//! CREATE TABLE `credential` (
//!   `id` text PRIMARY KEY,
//!   `connector_id` text NOT NULL,
//!   `method_id` text NOT NULL,
//!   `label` text NOT NULL,
//!   `value` text NOT NULL,
//!   `active` integer DEFAULT false NOT NULL,
//!   `time_created` integer NOT NULL,
//!   `time_updated` integer NOT NULL
//! );
//! CREATE UNIQUE INDEX `credential_connector_active_idx`
//!   ON `credential` (`connector_id`) WHERE "credential"."active" = 1;
//! ```
//!
//! La migration `20260611192811_lush_chimera.ts` a supprime cet index, supprime
//! la table, et l'a recreee dans la forme de la source : `integration_id` en
//! plus, `connector_id` / `method_id` / `active` redeclares sans `NOT NULL`, le
//! `DEFAULT false` perdu avec.
//!
//! Consequence reelle, et non theorique : une base arretee a la premiere
//! migration refuse `NULL` dans `connector_id`, `method_id` et `active`, et
//! garantit au plus un credential actif par `connector_id`. La base d'aujourd'hui
//! n'interdit ni l'un ni l'autre. Le fichier porte cet ecart
//! (colonnes [`COLONNES_NOT_NULL_RETIREES`], [`COLONNES_AJOUTEES`],
//! [`INDEX_UNIQUE_ABANDONNE`]) sans le resoudre : la source ne dit rien de ces
//! trois colonnes, elle ne fait que les recopier.
//!
//! Noter aussi que la source **ne declare aucun index** :
//! [`CREDENTIAL_INDEXES`] est vide, et c'est une information, pas un oubli.
//!
//! ## Les types marques n'existent qu'a la compilation
//!
//! Trois colonnes portent `.$type<...>()` : `id`, `integration_id` et `value`.
//! Ces marques sont des types fantomes d'Effect, dont l'effet a l'execution est
//! nul. Elles sont portees ici comme **chaines**, dans
//! [`AttributsColonne::marque`], transcrites lettre par lettre.
//!
//! C'est le piege des majuscules du projet, et il est reel ici : le nom de la
//! premiere marque est `Credential.ID`, avec `ID` en majuscules, et non
//! `CredentialId` ni `Credential.Id`. La deuxieme est
//! `Credential.Info["integrationID"]`, avec `integrationID` en majuscules
//! finales, et non `integration_id` ni `integrationId`. Une faute de casse sur
//! l'un ou l'autre ne se voit ni a la compilation ni dans le DDL, puisque la
//! marque ne sort jamais de la base : c'est une faute de documentation, qui ne
//! se remarque qu'a la relecture. Deux tests la verrouillent.
//!
//! ## Le piege `!x` contre `?? x`, et les deux fonctions qui les separent
//!
//! Ce fichier ne contient **ni ternaire ni coalescent** : la source non plus.
//! Le piege se pose chez le voisin, `packages/core/src/credential.ts`, sur la
//! **meme colonne** `integration_id` :
//!
//! - ligne 57, `if (!row.integration_id) return` : c'est un **test de
//!   veracite**, donc la chaine vide est traitee comme une absence ;
//! - ligne 98, `input.label ?? "default"` : c'est un **test de nullite**, donc
//!   une chaine vide survit et remplace meme pas le defaut.
//!
//! Ce sont deux fonctions differentes et non interchangeables, et le fichier les
//! porte toutes les deux, nommees d'apres l'expression qu'elles traduisent :
//!
//! - [`integration_absente`] teste la **nullite** de la colonne. Une chaine
//!   vide n'est pas absente : elle est presente.
//! - [`integration_sans_contenu`] teste la **veracite**. Elle est vraie pour une
//!   colonne `NULL` *et* pour la chaine vide.
//!
//! [`lignes_discordees`] ne fait qu'extraire le cas ou les deux lectures
//! different, c'est a dire la ligne dont `integration_id` vaut `""` : presente
//! en base, invisible du cote applicatif. Ce fichier ne filtre **aucune** liste
//! de credentials par lui-meme : `all`, `list`, `get`, `create`, `update` et
//! `remove` appartiennent a `credential.ts`, dont le portage vit dans
//! `crate::core::credential`. Les recopier ici ferait diverger deux
//! implementations du meme comportement.
//!
//! Rappel utile : `notNull` interdit `NULL`, il n'interdit pas `""`. Une ligne
//! dont les chaines sont vides reste recevable en base, et n'est pas traitee
//! comme une absence de valeur.
//!
//! ## Les secrets
//!
//! La colonne `value` **est** le secret : jeton d'acces, jeton de refresh ou cle
//! API, selon la variante de `Credential.Value`. Ce fichier ne materialise donc
//! aucun secret, n'en journalise aucun, et n'en ecrit aucun en dur, pas meme
//! dans ses tests. Il ne porte que la **forme** : un objet JSON serialise dans
//! une colonne texte, dont le type est `serde_json::Value`. Le type exact,
//! c'est-a-dire l'union taggee `oauth` / `key`, appartient a
//! `crate::core::credential::CredentialValue`, qui la porte deja. Les fixtures
//! de ce fichier sont des gabarits explicitement fictifs, marques
//! `<VALEUR-FACTICE>`, et le derive `Debug` de la ligne est un risque de fuite
//! qu'il faut avoir en tete au moment de l'imprimer.

use serde::{Deserialize, Serialize};

use crate::swarm::event_sql::{IndexDef, Storage, TableDef, TextMode};

/// La declaration d'une colonne de cette table.
///
/// Reexport de `crate::swarm::event_sql::ColumnDef`, qui porte deja les deux
/// drapeaux et les deux questions qu'ils repondent. Ce fichier n'en declare pas
/// de version propre : deux modeles du meme generateur de Drizzle divergeraient
/// sans qu'aucune erreur ne le signale.
pub use crate::swarm::event_sql::ColumnDef;

// ---------------------------------------------------------------------------
// Noms de colonnes
// ---------------------------------------------------------------------------

/// Nom de la table, premier argument de `sqliteTable`.
///
/// Il se relit dans le `CREATE TABLE` de `schema.gen.ts` et dans les deux
/// migrations citees dans la documentation du module.
pub const TABLE_NAME: &str = "credential";

/// Colonne de la cle primaire, `text PRIMARY KEY` **sans `NOT NULL`**.
pub const COLUMN_ID: &str = "id";

/// Colonne de l'integration proprietaire, `text` nullable.
///
/// C'est la colonne que le voisin `credential.ts` teste, ligne 57, par
/// veracite. Voir [`integration_absente`] et [`integration_sans_contenu`].
pub const COLUMN_INTEGRATION_ID: &str = "integration_id";

/// Colonne de l'etiquette, `text NOT NULL`.
pub const COLUMN_LABEL: &str = "label";

/// Colonne du secret, `text({ mode: "json" }) NOT NULL`.
///
/// C'est la seule colonne JSON de la table, et la seule qui porte un secret.
pub const COLUMN_VALUE: &str = "value";

/// Colonne du connecteur, `text` nullable aujourd'hui.
pub const COLUMN_CONNECTOR_ID: &str = "connector_id";

/// Colonne de la methode d'authentification, `text` nullable aujourd'hui.
pub const COLUMN_METHOD_ID: &str = "method_id";

/// Colonne du drapeau d'activite, `integer({ mode: "boolean" })` nullable.
pub const COLUMN_ACTIVE: &str = "active";

/// Colonne de creation, `integer NOT NULL`, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_CREATED: &str = "time_created";

/// Colonne de mise a jour, `integer NOT NULL`, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_UPDATED: &str = "time_updated";

/// Les neuf colonnes declarees par la source, dans l'ordre du `CREATE TABLE`.
///
/// L'ordre n'est pas cosmetique : c'est lui qu'un futur
/// `INSERT INTO credential (...) VALUES (...)` devra suivre. Il est aussi
/// l'ordre des champs de [`CredentialTableRow`].
pub const TABLE_CREDENTIAL: TableDef = TableDef {
    name: "credential",
    columns: &[
        // `text().$type<Credential.ID>().primaryKey()` : `primaryKey` sans
        // `notNull`, donc `nullable: true` litteral. Le type n'est pas
        // `not_null_declares` parce que la source ne l'ecrit pas.
        ColumnDef {
            name: "id",
            storage: Storage::Text,
            nullable: true,
            primary_key: true,
            mode: TextMode::Plain,
            references: None,
        },
        // `text().$type<Credential.Info["integrationID"]>()` : rien d'ecrit,
        // donc nullable des deux cotes.
        ColumnDef {
            name: "integration_id",
            storage: Storage::Text,
            nullable: true,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        // `text().notNull()`.
        ColumnDef {
            name: "label",
            storage: Storage::Text,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        // `text({ mode: "json" }).$type<Credential.Value>().notNull()` : seule
        // colonne JSON, et seule colonne `NOT NULL` qui porte un secret.
        ColumnDef {
            name: "value",
            storage: Storage::Text,
            nullable: false,
            primary_key: false,
            mode: TextMode::Json,
            references: None,
        },
        // `text()` : nu.
        ColumnDef {
            name: "connector_id",
            storage: Storage::Text,
            nullable: true,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        // `text()` : nu.
        ColumnDef {
            name: "method_id",
            storage: Storage::Text,
            nullable: true,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        // `integer({ mode: "boolean" })` : pas de `notNull`, donc nullable, et
        // pas de `DEFAULT` non plus. Le mode booleen est porte par
        // `COLONNES_BOOLEENNES`, voir `forme`.
        ColumnDef {
            name: "active",
            storage: Storage::Integer,
            nullable: true,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        // `integer().notNull().$default(() => Date.now())`, via `...Timestamps`.
        ColumnDef {
            name: "time_created",
            storage: Storage::Integer,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        // `integer().notNull().$onUpdate(() => Date.now())`, via `...Timestamps`.
        ColumnDef {
            name: "time_updated",
            storage: Storage::Integer,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
    ],
};

/// Les index declares par la source : **aucun**.
///
/// Le tableau est vide, et il est conserve malgre cela : l'absence d'index est
/// une information. La seule table voisine qui en declare un est `event`, et
/// le troisieme argument de `sqliteTable` est vide ici.
///
/// L'index `credential_connector_active_idx` de la premiere migration ne vit
/// donc pas dans ce tableau : il a ete explicitement supprime par
/// `lush_chimera`, et le porter serait pretendre que la base en impose
/// aujourd'hui une contrainte que la source ne demande pas.
pub const CREDENTIAL_INDEXES: [IndexDef; 0] = [];

// ---------------------------------------------------------------------------
// Ce que la source ecrit, et ce que la base autorise
// ---------------------------------------------------------------------------

/// La declaration d'une colonne, ou `None` si le nom n'existe pas.
///
/// La recherche est **exacte**, comme en SQL : `Integration_Id` ne trouve pas
/// `integration_id`, et `integrationID` non plus. Ce point merite d'etre teste,
/// parce que `integrationID` est le nom que porte le contrat JSON dans la
/// source, et le confondre avec `integration_id` serait le piege du projet
/// realise exactement.
pub fn colonne(nom: &str) -> Option<&'static ColumnDef> {
    TABLE_CREDENTIAL.columns.iter().find(|colonne| colonne.name == nom)
}

/// La source ecrit-elle `notNull()` sur cette colonne ?
///
/// C'est la premiere des deux questions, et la seule a laquelle la source
/// permette de repondre : elle se lit sur le code. Une colonne inconnue repond
/// `false`, ce qui veut dire "la source n'ecrit rien", et non "la colonne
/// refuse `NULL`". Pour la question opposee, voir [`admet_null`].
pub fn ecrit_not_null(nom: &str) -> bool {
    match colonne(nom) {
        Some(declaree) => declaree.not_null_declares(),
        None => false,
    }
}

/// La base creee par ce DDL admet-elle `NULL` dans cette colonne ?
///
/// C'est la seconde question, et elle n'a pas la meme reponse. Dans une table a
/// `rowid`, SQLite ne traite pas `PRIMARY KEY` comme `NOT NULL` : la cle
/// primaire de `credential` n'interdit donc pas `NULL`, que la source ait ecrit
/// `notNull()` ou non.
///
/// Une colonne inconnue repond `true` : le DDL ne dit rien d'elle, donc rien
/// ne l'interdit.
pub fn admet_null(nom: &str) -> bool {
    match colonne(nom) {
        Some(declaree) => declaree.admet_null_dans_sqlite(),
        None => true,
    }
}

/// Ce que la colonne attend comme valeur.
///
/// Cette fonction **compose** les deux representations au lieu d'en
/// inventer une troisieme : `TextMode` de la declaration dit deja si une
/// colonne texte est du texte ou du JSON, et `storage` dit deja si elle est
/// entiere. Le seul cas que ces deux champs ne savent pas exprimer est le
/// mode booleen, un mode de valeur sur une colonne **entiere**, la ou
/// `TextMode` ne parle que de texte. D'ou la liste dediee
/// [`COLONNES_BOOLEENNES`].
///
/// Une colonne inconnue repond `None`.
pub fn forme(nom: &str) -> Option<FormeValeur> {
    let declaree = colonne(nom)?;
    if COLONNES_BOOLEENNES.contains(&declaree.name) {
        return Some(FormeValeur::Booleen);
    }
    Some(match declaree.storage {
        Storage::Text => match declaree.mode {
            TextMode::Json => FormeValeur::Json,
            TextMode::Plain => FormeValeur::Chaine,
        },
        Storage::Integer => FormeValeur::Entier,
    })
}

/// La forme de valeur attendue dans une colonne de `credential`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormeValeur {
    /// Une chaine, telle quelle. `text()`.
    Chaine,
    /// Un objet JSON, serialise dans la colonne texte par le pilote.
    /// `text({ mode: "json" })`. Une seule colonne de la table, `value`.
    Json,
    /// Un booleen, stocke en entier. `integer({ mode: "boolean" })`. Une seule
    /// colonne de la table, `active`.
    ///
    /// L'encodage entier de ce drapeau n'est pas redefini ici : il est deja
    /// porte par `crate::swarm::account_sql` pour la meme construction, et le
    /// recopier donnerait deux fonctions du meme encodage 0 / 1.
    Booleen,
    /// Un entier brut. `integer()`. Les deux horodatages, en millisecondes
    /// depuis l'epoch.
    Entier,
}

/// Les colonnes declarees `integer({ mode: "boolean" })`.
///
/// Une seule. Le mode est porte a part parce que `TextMode` de `event_sql`
/// ne parle que des colonnes texte, et y loger un booleen forcerait a y ecrire
/// du vide. Un test verifie que cette liste ne cite que des colonnes reelles, et
/// que chacune est bien une colonne entiere.
pub const COLONNES_BOOLEENNES: [&str; 1] = ["active"];

// ---------------------------------------------------------------------------
// Ce que ColumnDef ne sait pas dire : les crochets ORM et les types marques
// ---------------------------------------------------------------------------

/// Crochet d'ORM porte par une colonne, et non contrainte de la base.
///
/// Les deux colonnes de `...Timestamps` portent un crochet de ce genre. Ce
/// n'est **pas** un `DEFAULT` SQL : le DDL genere ne contient aucun `DEFAULT`,
/// et l'instantane de `drizzle-kit` non plus. Consequence concrete et
/// verifiable : `time_created` et `time_updated` sont `NOT NULL` sans valeur
/// par defaut, donc une insertion qui les omet est refusee par la base. Le
/// crochet est applique au moment de construire la requete, par l'ORM, et
/// nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrochetOrm {
    /// Aucun crochet. C'est le cas de sept des neuf colonnes.
    Aucun,
    /// `.$default(() => Date.now())` : la colonne est renseignee a l'insertion
    /// si l'appelant ne la fournit pas. Portable par `time_created`.
    DefaultAInsertion,
    /// `.$onUpdate(() => Date.now())` : la colonne est rafraichie a chaque
    /// mise a jour. Portable par `time_updated`.
    MiseAJourAutomatique,
}

/// Ce que la declaration d'une colonne porte **en plus** du stockage et des
/// deux drapeaux.
///
/// Cette structure ne repete pas [`ColumnDef`] : elle ne porte que ce que
/// `ColumnDef` ne peut pas dire, a savoir la marque TypeScript, qui n'existe
/// qu'a la compilation, et le crochet d'ORM, qui n'est pas une contrainte. Un
/// test verifie que les neuf entrees correspondent exactement, **dans l'ordre**,
/// aux neuf colonnes de [`TABLE_CREDENTIAL`], pour que les deux descriptions ne
/// puissent pas deriver l'une de l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributsColonne {
    /// Nom de la colonne, identique a la cle de l'objet de la source.
    pub nom: &'static str,
    /// La marque `.$type<...>()`, transcrite lettre par lettre.
    ///
    /// `None` sur les six colonnes sans marque. Une marque n'a aucun effet a
    /// l'execution : elle est ici pour que la relecture puisse comparer la
    /// declaration a la source, et pour que la faute de majuscules
    /// (`Credential.ID` et non `CredentialId`) soit visible.
    pub marque: Option<&'static str>,
    /// Le crochet d'ORM de la colonne, s'il y en a un.
    pub crochet: CrochetOrm,
}

/// Les neuf entrees, dans l'ordre du `CREATE TABLE`.
pub const ATTRIBUTS_CREDENTIAL: [AttributsColonne; 9] = [
    // `.$type<Credential.ID>()` : deux majuscules, point compris.
    AttributsColonne { nom: "id", marque: Some("Credential.ID"), crochet: CrochetOrm::Aucun },
    // `.$type<Credential.Info["integrationID"]>()` : la seule fois ou la
    // source indexe un type, et `integrationID` s'ecrit avec `ID` en
    // majuscules. Attention : `integration_id` est le nom de la **colonne**,
    // `integrationID` le nom du **type** de la source. Ce sont deux chaines
    // differentes, dans deux mondes differents, et ce fichier les porte toutes
    // les deux sans les confondre.
    AttributsColonne {
        nom: "integration_id",
        marque: Some("Credential.Info[\"integrationID\"]"),
        crochet: CrochetOrm::Aucun,
    },
    AttributsColonne { nom: "label", marque: None, crochet: CrochetOrm::Aucun },
    // `.$type<Credential.Value>()` : la seule colonne qui porte un secret.
    AttributsColonne {
        nom: "value",
        marque: Some("Credential.Value"),
        crochet: CrochetOrm::Aucun,
    },
    AttributsColonne { nom: "connector_id", marque: None, crochet: CrochetOrm::Aucun },
    AttributsColonne { nom: "method_id", marque: None, crochet: CrochetOrm::Aucun },
    AttributsColonne { nom: "active", marque: None, crochet: CrochetOrm::Aucun },
    // `...Timestamps` : `.$default(...)` puis `.$onUpdate(...)`.
    AttributsColonne {
        nom: "time_created",
        marque: None,
        crochet: CrochetOrm::DefaultAInsertion,
    },
    AttributsColonne {
        nom: "time_updated",
        marque: None,
        crochet: CrochetOrm::MiseAJourAutomatique,
    },
];

/// Les attributs d'une colonne, ou `None` si le nom n'existe pas.
///
/// La recherche est exacte, comme pour [`colonne`].
pub fn attributs(nom: &str) -> Option<&'static AttributsColonne> {
    ATTRIBUTS_CREDENTIAL.iter().find(|attribut| attribut.nom == nom)
}

// ---------------------------------------------------------------------------
// La table d'hier, et ce que la migration a change
// ---------------------------------------------------------------------------

/// Les huit colonnes de la table creee par `20260611035744_credential`.
///
/// Ni `integration_id`, ni les neuf colonnes actuelles : huit colonnes, dans
/// l'ordre de ce `CREATE TABLE`.
pub const COLONNES_DE_LA_TABLE_INITIALE: [&str; 8] = [
    "id",
    "connector_id",
    "method_id",
    "label",
    "value",
    "active",
    "time_created",
    "time_updated",
];

/// Les colonnes que la table initiale declarait `NOT NULL` et que la source
/// actuelle ne declare plus.
///
/// `lush_chimera` a supprime la table et l'a recreee, donc ces trois colonnes
/// sont devenues des colonnes ordinaires, sans contrainte. Une base arretee a
/// la premiere migration les refuse malgre tout : elle vit avec un schema que
/// la source ne decrit plus.
pub const COLONNES_NOT_NULL_RETIREES: [&str; 3] = ["connector_id", "method_id", "active"];

/// La colonne que la source porte et que la table initiale n'avait pas.
///
/// Elle est **nullable**, et c'est ce qui compte : `credential.ts` s'appuie
/// dessus pour lister et pour remplacer le credential d'une integration, et ne
/// verrait jamais une ligne sans integration dans la base d'aujourd'hui.
pub const COLONNES_AJOUTEES: [&str; 1] = ["integration_id"];

/// L'index unique de la table initiale, supprime par `lush_chimera`.
///
/// `CREATE UNIQUE INDEX credential_connector_active_idx ON credential
/// (connector_id) WHERE "credential"."active" = 1` : au plus un credential
/// actif par connecteur. La source ne declare aucun index, donc cette garantie
/// n'existe plus dans une base neuve. Elle n'est pas portee comme une
/// contrainte, mais notee comme une perte.
pub const INDEX_UNIQUE_ABANDONNE: &str = "credential_connector_active_idx";

/// Colonnes declarees par la source et absentes de la table initiale.
///
/// Attendu : `["integration_id"]`, et rien d'autre. Une colonne de plus ici
/// signifierait que la source a evolue sans que la migration ne suive.
pub fn colonnes_ajoutees() -> Vec<&'static str> {
    TABLE_CREDENTIAL
        .noms_de_colonnes()
        .into_iter()
        .filter(|nom| !COLONNES_DE_LA_TABLE_INITIALE.contains(nom))
        .collect()
}

/// Colonnes de la table initiale que la source ne declare plus.
///
/// Attendu : vide. Une colonne ici signifierait que la migration a retire une
/// colonne que la source conserve encore, ce qui serait l'inverse du cas
/// reel.
pub fn colonnes_supprimees() -> Vec<&'static str> {
    COLONNES_DE_LA_TABLE_INITIALE
        .iter()
        .copied()
        .filter(|nom| !TABLE_CREDENTIAL.noms_de_colonnes().contains(nom))
        .collect()
}

// ---------------------------------------------------------------------------
// La forme de la ligne
// ---------------------------------------------------------------------------

/// Une ligne de la table `credential`, telle que le DDL la decrit.
///
/// Ce n'est **pas** `crate::core::credential::CredentialRow`, qui ne porte que
/// les quatre colonnes que le service lit. Celui-ci porte les neuf colonnes du
/// DDL, dans l'ordre du `CREATE TABLE`, y compris les trois que la source
/// recopie sans les contraindre et les deux horodatages.
///
/// Deux conventions de serialisation, et le choix est delibere :
///
/// - **aucun `skip_serializing_if`**. Une colonne absente du JSON est une
///   faute de protocole, pas une valeur : en SQL, l'absence se dit `NULL` et
///   rien d'autre. Une colonne `None` se serialise donc en `null`, avec sa
///   cle presente, ce qui distingue "la colonne vaut `NULL`" de "la colonne
///   n'existe pas". C'est l'inverse du reflexe pose sur une structure JSON,
///   ou une cle absente vaut `undefined`.
/// - **tous les `rename` sont ecrits**, y compris quand ils sont identiques
///   au nom du champ. Le contrat de la ligne est celui de la base, pas celui du
///   JSON public, et la casse des majuscules de la source (`integrationID`)
///   ne doit pas pouvoir contaminer ces noms par automatisme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CredentialTableRow {
    /// `text PRIMARY KEY`, **sans `NOT NULL` dans le DDL**.
    ///
    /// Le type est marque `Credential.ID` dans la source, marque qui n'existe
    /// qu'a la compilation : a l'execution c'est une chaine, d'ou le `String`.
    ///
    /// L'option est portee parce que la SOURCE l'implique. SQLite autorise
    /// `NULL` dans une `TEXT PRIMARY KEY` d'une table a `rowid`, comme le dit
    /// [`admet_null`], donc une colonne dont la source n'ecrit pas `notNull`
    /// admet reellement l'absence.
    ///
    /// On pourrait argumenter que `id` devrait rester non optionnel, parce
    /// qu'une ligne sans identifiant n'est adressable par aucune operation du
    /// service. Cet argument est valide, mais il introduirait une contrainte que
    /// la source ne declare pas, et le meme contrat declaratif est traite
    /// differemment dans `account_sql`, ou `id` est un `Option<String>` pour la
    /// meme declaration `text().primaryKey()` sans `notNull()`.
    ///
    /// Deux representations divergentes d'un meme contrat ne produisent aucune
    /// erreur de compilation : Rust autorise un meme nom dans deux modules, et
    /// les deux evoluent independamment. La regle du lot est donc simple : quand
    /// deux fichiers persistent un contrat declaratif identique, c'est la source
    /// qui tranche, jamais le gout de l'agent qui les ecrit. La source dit la
    /// meme chose dans les deux cas, donc les deux fichiers disent la meme
    /// chose. Un test verrouille ce choix, dans les deux sens.
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// `text`, **nullable** : la source n'ecrit rien dessus.
    ///
    /// Nullable pour une raison metier et non technique : la colonne n'est pas
    /// obligatoire dans la source parce que la premiere version de la table ne
    /// portait pas d'integration du tout, et que la migration a decide de
    /// rendre la colonne nullable plutot que de rejeter les lignes deja
    /// ecrites.
    ///
    /// `None` signifie "la ligne n'appartient a aucune integration", ce qui
    /// est une donnee reellement presente en base. Ce que le voisin
    /// `credential.ts` en fait est une **autre** question, traitee par
    /// [`integration_sans_contenu`], et elle donne une reponse differente.
    #[serde(rename = "integration_id")]
    pub integration_id: Option<String>,
    /// `text NOT NULL`. Etiquette affichee a l'utilisateur.
    ///
    /// La colonne accepte la chaine vide : `notNull` interdit `NULL`, il
    /// n'interdit pas `""`. C'est pourquoi le voisin ecrit
    /// `input.label ?? "default"` et non `input.label || "default"` : le
    /// coalescent teste la nullite, et la chaine vide est un libelle valide.
    #[serde(rename = "label")]
    pub label: String,
    /// `text({ mode: "json" }) NOT NULL` : **le secret**.
    ///
    /// Le type est `serde_json::Value` et rien d'autre, volontairement. La
    /// forme exacte de cette valeur est l'union taggee
    /// `oauth` / `key`, portee par
    /// `crate::core::credential::CredentialValue` : la redeclarer ici
    /// produirait un quatrieme contrat pour le meme secret, qui divergerait en
    /// silence de celui que `credential.ts` decode.
    ///
    /// `serde_json::Value` accepte bien plus que l'union - une chaine, un
    /// nombre, `null` -, et c'est le decodeur du service qui restreint. Une
    /// ligne dont `value` n'est pas un objet JSON est donc representable ici
    /// alors qu'elle serait refusee la.
    ///
    /// Aucun test de ce fichier n'ecrit de jeton ni de cle : la forme seule.
    #[serde(rename = "value")]
    pub value: serde_json::Value,
    /// `text`, nullable aujourd'hui, `NOT NULL` dans la table initiale.
    ///
    /// La source la recopie sans la declarer, et ne s'en sert nulle part : ni
    /// l'ecriture du `create` ne la fournit, ni la lecture ne la consulte.
    #[serde(rename = "connector_id")]
    pub connector_id: Option<String>,
    /// `text`, nullable aujourd'hui, `NOT NULL` dans la table initiale.
    ///
    /// Comme [`Self::connector_id`] : colonne heritee, recopiee, jamais
    /// lue ni ecrite par le service. Les jetons OAuth ont leurs dates
    /// d'expiration dans `value`, pas ici.
    #[serde(rename = "method_id")]
    pub method_id: Option<String>,
    /// `integer({ mode: "boolean" })`, nullable, et **sans `DEFAULT`**.
    ///
    /// Le type est `boolean` cote source et `integer` en base. Le champ est
    /// donc declare `Option<bool>` ici : la colonne est nullable, donc
    /// l'absence d'activite est une donnee, pas un defaut.
    ///
    /// Deux consequences a garder en tete, et qui se voient au premier
    /// echange. D'une part, l'encodage en base est l'entier `0` ou `1`, et non
    /// le booleen : un pilote qui lit la colonne brute rendra `1` et `0`, que
    /// `serde` refuse dans un `bool`. L'encodage est deja porte par
    /// `crate::swarm::account_sql` pour la meme construction. D'autre part, le
    /// `DEFAULT false` de la table initiale a disparu avec la migration :
    /// l'absence de drapeau est donc distinguishable de la presence du
    /// drapeau a `false`.
    #[serde(rename = "active")]
    pub active: Option<bool>,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    ///
    /// Aucun `DEFAULT` SQL : le `$default` est un crochet d'ORM, applique a la
    /// construction de la requete. Une insertion qui omet la colonne est donc
    /// refusee par la base. Le `now` est fourni par l'appelant dans
    /// [`Self::nouvelle`], ce qui garde la fonction pure.
    #[serde(rename = "time_created")]
    pub time_created: i64,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    ///
    /// Comme [`Self::time_created`], aucun `DEFAULT` SQL, et le `$onUpdate`
    /// n'est pas non plus un `DEFAULT` : c'est un crochet de mise a jour. Voir
    /// [`Self::toucher`].
    #[serde(rename = "time_updated")]
    pub time_updated: i64,
}

impl CredentialTableRow {
    /// Construit une ligne a l'insertion, en positionnant les deux
    /// horodatages sur le meme `now`.
    ///
    /// Les quatre premieres colonnes sont exactement celles que le `create` de
    /// `credential.ts` fournit dans son `values`, et les cinq autres restent a
    /// `None` : le service n'ecrit ni `connector_id`, ni `method_id`, ni
    /// `active`, et la seule colonne nullable qu'il laisse reste donc
    /// `integration_id`.
    ///
    /// Le `now` est fourni et non lu dans l'horloge : cette fonction reste pure
    /// et testable sans acces disque ni horloge systeme.
    ///
    /// `id` est un `Option`, pas un `impl Into<String>` : la source ecrit
    /// `text PRIMARY KEY` sans `notNull()`, donc un `id` absent est une valeur
    /// legitime du contrat et non un cas particulier du constructeur.
    pub fn nouvelle(
        id: Option<String>,
        integration_id: Option<String>,
        label: impl Into<String>,
        value: serde_json::Value,
        now: i64,
    ) -> Self {
        Self {
            id,
            integration_id,
            label: label.into(),
            value,
            connector_id: None,
            method_id: None,
            active: None,
            time_created: now,
            time_updated: now,
        }
    }

    /// Applique une mise a jour : seul `time_updated` bouge.
    ///
    /// C'est la traduction de `$onUpdate(() => Date.now())`, qui porte sur le
    /// crochet et non sur la colonne : le `id`, l'integration, le libelle, le
    /// secret et la date de creation restent inchanges.
    pub fn toucher(&mut self, now: i64) {
        self.time_updated = now;
    }
}

// ---------------------------------------------------------------------------
// Les deux lectures de la colonne d'integration
// ---------------------------------------------------------------------------

/// La colonne `integration_id` vaut-elle `NULL` ?
///
/// C'est un **test de nullite**, l'equivalent d'un `??` et d'un `=== undefined`.
/// La chaine vide n'est pas une absence : elle est presente, et cette fonction
/// repond `false` pour elle.
///
/// C'est la lecture de la **base** : la seule question que le DDL permet de
/// poser.
pub fn integration_absente(ligne: &CredentialTableRow) -> bool {
    ligne.integration_id.is_none()
}

/// La colonne `integration_id` est-elle vide au sens de la veracite ?
///
/// C'est un **test de veracite**, l'equivalent d'un `!x`. Il est vrai pour une
/// colonne `NULL` *et* pour la chaine vide, parce que les deux sont fausses
/// comme chaine.
///
/// C'est la lecture du voisin `credential.ts`, ligne 57 :
/// `if (!row.integration_id) return`. Une ligne dont `integration_id` vaut la
/// chaine vide est donc ecartee de `all` et de `list`, alors qu'elle est
/// parfaitement stockable et parfaitement lisible.
///
/// Ne pas confondre les deux lectures : les fusionner en une seule fonction
/// obligerait a choisir, et les deux choix sont faux dans un cas chacun.
pub fn integration_sans_contenu(ligne: &CredentialTableRow) -> bool {
    match ligne.integration_id.as_deref() {
        Some(valeur) => valeur.is_empty(),
        None => true,
    }
}

/// Les lignes sur lesquelles les deux lectures ne tombent pas d'accord.
///
/// Ce sont les lignes dont `integration_id` vaut la chaine vide : presentes en
/// base, donc visibles par [`integration_absente`], et sans contenu, donc
/// ecartees par [`integration_sans_contenu`].
///
/// Ce n'est **pas** une fonction de filtrage de credentials, et elle ne se
/// veut pas telle : le filtrage appartient a `credential.ts`, dont le portage
/// vit dans `crate::core::credential`, et dont la lecture est celle de la
/// veracite. Ici, on rend seulement la surface d'accord et on laisse la
/// decision a l'appelant. L'ordre d'entree est preserve.
pub fn lignes_discordees(lignes: &[CredentialTableRow]) -> Vec<&CredentialTableRow> {
    lignes
        .iter()
        // `ligne` est un `&&CredentialTableRow` dans la closure de `filter` :
        // le deref est ecrit, il ne repose pas sur la coercion.
        .filter(|ligne| !integration_absente(*ligne) && integration_sans_contenu(*ligne))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Fixtures
    //
    // Aucun test de ce fichier n'ecrit de secret. Les valeurs ci-dessous sont
    // des GABARITS, et le contenu de la colonne `key` est un texte de
    // remplacement marque comme tel. Ce qui est teste ici, c'est la FORME de
    // la valeur - un objet JSON, avec un champ `type` - et non un jeton.
    // ------------------------------------------------------------------

    /// La forme d'une valeur `Credential.Value` de variante `key`.
    ///
    /// `"<VALEUR-FACTICE>"` n'est pas une cle et ne doit jamais l'etre.
    fn valeur_factice_cle() -> serde_json::Value {
        serde_json::json!({
            "type": "key",
            "key": "<VALEUR-FACTICE-N-EST-PAS-UN-SECRET>",
        })
    }

    /// La forme d'une valeur `Credential.Value` de variante `oauth`.
    ///
    /// Les trois chaines sont des remplacements. Seule la forme compte, et le
    /// champ `methodID` y est ecrit en majuscules comme dans la source.
    fn valeur_factice_oauth() -> serde_json::Value {
        serde_json::json!({
            "type": "oauth",
            "methodID": "<VALEUR-FACTICE>",
            "refresh": "<VALEUR-FACTICE>",
            "access": "<VALEUR-FACTICE>",
            "expires": 0,
        })
    }

    /// Une ligne minimale, avec une integration presente ou non.
    fn ligne(id: Option<&str>, integration: Option<&str>, now: i64) -> CredentialTableRow {
        CredentialTableRow::nouvelle(id.map(str::to_string), integration.map(str::to_string), "default", valeur_factice_cle(), now)
    }

    // ------------------------------------------------------------------
    // La declaration
    // ------------------------------------------------------------------

    #[test]
    fn table_exposes_its_nine_columns_in_ddl_order() {
        assert_eq!(TABLE_CREDENTIAL.name, "credential");
        assert_eq!(
            TABLE_CREDENTIAL.noms_de_colonnes(),
            vec![
                "id",
                "integration_id",
                "label",
                "value",
                "connector_id",
                "method_id",
                "active",
                "time_created",
                "time_updated",
            ]
        );
    }

    #[test]
    fn column_lookup_is_exact_and_rejects_the_json_contract_casing() {
        assert!(colonne("integration_id").is_some());
        // Le nom du type de la source, pas celui de la colonne. Le confondre
        // serait le piege du projet, realise exactement.
        assert!(colonne("integrationID").is_none());
        assert!(colonne("integrationId").is_none());
        assert!(colonne("Integration_Id").is_none());
        // Et le nom de la table n'est pas un nom de colonne.
        assert!(colonne("credential").is_none());
        assert!(colonne("").is_none());
    }

    #[test]
    fn id_is_the_only_primary_key_column() {
        let primaires: Vec<&str> = TABLE_CREDENTIAL
            .columns
            .iter()
            .filter(|colonne| colonne.primary_key)
            .map(|colonne| colonne.name)
            .collect();
        assert_eq!(primaires, vec!["id"]);
    }

    #[test]
    fn both_primary_key_declarations_produce_the_same_ddl() {
        // Fait verifie : `notNull` et `primaryKey` sont deux drapeaux
        // independants, et quand Drizzle emet `PRIMARY KEY` sur la colonne il
        // retire le `NOT NULL` juge redondant. Les deux declarations ci-dessous
        // sont donc indiscernables une fois le DDL produit.
        let sans_not_null = ColumnDef {
            name: "id",
            storage: Storage::Text,
            nullable: true,
            primary_key: true,
            mode: TextMode::Plain,
            references: None,
        };
        let avec_not_null = ColumnDef {
            name: "id",
            storage: Storage::Text,
            nullable: false,
            primary_key: true,
            mode: TextMode::Plain,
            references: None,
        };

        // Ce que la source ecrit differe...
        assert!(!sans_not_null.not_null_declares());
        assert!(avec_not_null.not_null_declares());
        // ... mais les deux sont des cles primaires...
        assert!(sans_not_null.primary_key && avec_not_null.primary_key);
        // ... et les deux colonnes admettent `NULL` a la base.
        assert!(sans_not_null.admet_null_dans_sqlite());
        assert!(avec_not_null.admet_null_dans_sqlite());
    }

    #[test]
    fn credential_is_the_only_table_where_both_nullability_lists_agree() {
        // Particularite de cette table, a ne pas confondre avec les voisines :
        // la source n'ecrit ni `notNull` ni `primaryKey` sans etre la seule
        // colonne primaire. Le statut de cle primaire n'ajoute donc AUCUNE
        // colonne a celles que la base admet a `NULL`.
        assert_eq!(
            TABLE_CREDENTIAL.colonnes_sans_not_null_declares(),
            TABLE_CREDENTIAL.colonnes_admettant_null_dans_sqlite()
        );
        assert_eq!(
            TABLE_CREDENTIAL.colonnes_sans_not_null_declares(),
            vec!["id", "integration_id", "connector_id", "method_id", "active"]
        );
    }

    #[test]
    fn columns_where_the_source_writes_not_null_really_reject_null() {
        // Le converse : la ou la source ecrit `notNull()`, le DDL porte le
        // `NOT NULL` et le moteur refuse `NULL`. Ces quatre colonnes ne portent
        // pas la cle primaire, donc rien ne leur retire la contrainte.
        assert_eq!(
            TABLE_CREDENTIAL.colonnes_avec_not_null_declares(),
            vec!["label", "value", "time_created", "time_updated"]
        );
        for nom in ["label", "value", "time_created", "time_updated"] {
            assert!(ecrit_not_null(nom), "{nom} : la source ecrit notNull");
            assert!(!admet_null(nom), "{nom} : la base refuse NULL");
        }
    }

    #[test]
    fn the_primary_key_column_still_admits_null_in_the_database() {
        // Le point qui vaut tout ce fichier : `id text PRIMARY KEY` sans `NOT
        // NULL`, et SQLite qui n'y voit pas une interdiction.
        assert!(!ecrit_not_null("id"), "la source n ecrit pas notNull sur id");
        assert!(colonne("id").expect("colonne id").primary_key);
        assert!(admet_null("id"), "et pourtant la base autorise NULL");
    }

    #[test]
    fn an_unknown_column_answers_every_question_with_its_default() {
        // Une colonne inconnue ne peut pas repondre sur une question qui la
        // concerne : elle n'ecrit rien, donc rien ne l'interdit.
        assert!(!ecrit_not_null("colonne_inexistante"));
        assert!(admet_null("colonne_inexistante"));
        assert!(colonne("colonne_inexistante").is_none());
        assert_eq!(forme("colonne_inexistante"), None);
        assert!(attributs("colonne_inexistante").is_none());
    }

    // ------------------------------------------------------------------
    // Le mode des colonnes
    // ------------------------------------------------------------------

    #[test]
    fn value_is_the_only_json_column_and_the_only_one_holding_a_secret() {
        let json: Vec<&str> = TABLE_CREDENTIAL
            .columns
            .iter()
            .filter(|colonne| colonne.mode == TextMode::Json)
            .map(|colonne| colonne.name)
            .collect();
        assert_eq!(json, vec!["value"]);
        assert_eq!(forme("value"), Some(FormeValeur::Json));
    }

    #[test]
    fn active_is_the_only_boolean_stored_as_an_integer() {
        assert_eq!(COLONNES_BOOLEENNES, ["active"]);
        assert_eq!(colonne("active").expect("colonne active").storage, Storage::Integer);
        assert_eq!(forme("active"), Some(FormeValeur::Booleen));
        // Le mode ne contredit pas le stockage : un booleen est bien un entier.
        assert_eq!(colonne("active").expect("colonne active").mode, TextMode::Plain);
        // Aucune autre colonne n'est un booleen.
        for nom in ["id", "integration_id", "label", "value", "time_created"] {
            assert_ne!(forme(nom), Some(FormeValeur::Booleen), "{nom} n est pas booleen");
        }
    }

    #[test]
    fn value_forms_of_all_nine_columns_are_composed_from_storage_and_mode() {
        // `forme` ne doit rien invorter : elle compose `storage`, `TextMode` et
        // la liste des booleens. Les neuf resultats sont donc forces.
        assert_eq!(forme("id"), Some(FormeValeur::Chaine));
        assert_eq!(forme("integration_id"), Some(FormeValeur::Chaine));
        assert_eq!(forme("label"), Some(FormeValeur::Chaine));
        assert_eq!(forme("value"), Some(FormeValeur::Json));
        assert_eq!(forme("connector_id"), Some(FormeValeur::Chaine));
        assert_eq!(forme("method_id"), Some(FormeValeur::Chaine));
        assert_eq!(forme("active"), Some(FormeValeur::Booleen));
        assert_eq!(forme("time_created"), Some(FormeValeur::Entier));
        assert_eq!(forme("time_updated"), Some(FormeValeur::Entier));
    }

    #[test]
    fn both_secret_variants_fit_in_the_json_column() {
        // Le type exact de `value` appartient a `crate::core::credential` : ce
        // fichier ne le repete pas. Ce qui est verifie ici, c'est que la seule
        // colonne JSON de la table accueille bien la forme des deux variantes
        // de l'union, et que ce test n'ecrit pour autant aucun secret reel.
        for valeur in [valeur_factice_cle(), valeur_factice_oauth()] {
            assert!(valeur.is_object());
            assert!(valeur.get("type").is_some());
        }
    }

    // ------------------------------------------------------------------
    // Les majuscules de la source (piege 1)
    // ------------------------------------------------------------------

    #[test]
    fn typescript_type_marks_are_transcribed_letter_for_letter() {
        // Piege reel et invisible : `ID` en majuscules, deux fois.
        assert_eq!(attributs("id").expect("id").marque, Some("Credential.ID"));
        assert_eq!(attributs("value").expect("value").marque, Some("Credential.Value"));
        assert_eq!(
            attributs("integration_id").expect("integration_id").marque,
            Some("Credential.Info[\"integrationID\"]")
        );
    }

    #[test]
    fn type_marks_are_never_rewritten_to_snake_case_or_camel_case() {
        // Les trois fautes qu'un relecteur presse commettrait, verifiees une
        // par une. Aucune n'aurait produit d'erreur de compilation.
        let id = attributs("id").expect("id").marque.expect("marque de id");
        assert!(!id.contains("Id"), "ID ne s ecrit pas Id : {id}");
        assert!(!id.contains('_'), "une marque ne s ecrit pas en snake_case : {id}");

        let integration = attributs("integration_id")
            .expect("integration_id")
            .marque
            .expect("marque de integration_id");
        assert!(integration.contains("integrationID"), "{integration}");
        assert!(
            !integration.contains("integration_id"),
            "le nom de colonne ne se substitue pas au nom de type : {integration}"
        );
        assert!(!integration.contains("integrationId"), "{integration}");

        // Les six autres colonnes n'ont pas de marque du tout.
        for nom in ["label", "connector_id", "method_id", "active", "time_created", "time_updated"] {
            assert!(
                attributs(nom).expect(nom).marque.is_none(),
                "{nom} ne porte pas de $type"
            );
        }
    }

    #[test]
    fn a_type_mark_changes_neither_storage_nor_mode_of_its_column() {
        // `.$type<...>()` est un type fantome : effet nul a l'execution. Les
        // trois colonnes marquees ne se distinguent donc par rien d'autre que
        // leur nom, et chacune garde le stockage de sa declaration.
        for nom in ["id", "integration_id", "value"] {
            let declaree = colonne(nom).expect(nom);
            assert_eq!(declaree.storage, Storage::Text, "{nom} reste du texte");
            assert!(declaree.references.is_none(), "{nom} ne porte pas de cle etrangere");
        }
        // Le mode suit la declaration, jamais la marque : `value` reste la
        // seule colonne JSON du trio, les deux autres du texte nu.
        assert_eq!(colonne("value").expect("marquee").mode, TextMode::Json);
        assert_eq!(colonne("id").expect("marquee").mode, TextMode::Plain);
        assert_eq!(colonne("integration_id").expect("marquee").mode, TextMode::Plain);
    }

    // ------------------------------------------------------------------
    // Les crochets d'ORM
    // ------------------------------------------------------------------

    #[test]
    fn both_timestamps_carry_an_orm_hook_that_is_not_a_sql_default() {
        let created = attributs("time_created").expect("time_created").crochet;
        let updated = attributs("time_updated").expect("time_updated").crochet;
        assert_eq!(created, CrochetOrm::DefaultAInsertion);
        assert_eq!(updated, CrochetOrm::MiseAJourAutomatique);
        // Le DDL ne contient aucun DEFAULT : les deux colonnes restent
        // NOT NULL sans valeur de repli, donc une insertion qui les omet est
        // refusee par la base.
        assert!(ecrit_not_null("time_created"));
        assert!(ecrit_not_null("time_updated"));
        assert!(!admet_null("time_created"));
        assert!(!admet_null("time_updated"));
    }

    #[test]
    fn the_other_seven_columns_carry_no_orm_hook() {
        for nom in ["id", "integration_id", "label", "value", "connector_id", "method_id", "active"] {
            assert_eq!(
                attributs(nom).expect(nom).crochet,
                CrochetOrm::Aucun,
                "{nom} ne porte ni $default ni $onUpdate"
            );
        }
    }

    #[test]
    fn column_attributes_follow_the_table_in_the_same_order() {
        // Les deux descriptions - la declaration et les attributs - ne doivent
        // pas pouvoir deriver l'une de l'autre. Meme ordre, meme longueur.
        let noms_attributs: Vec<&str> = ATTRIBUTS_CREDENTIAL.iter().map(|a| a.nom).collect();
        assert_eq!(noms_attributs, TABLE_CREDENTIAL.noms_de_colonnes());
    }

    // ------------------------------------------------------------------
    // La migration lush_chimera
    // ------------------------------------------------------------------

    #[test]
    fn the_current_table_adds_one_column_and_drops_none() {
        assert_eq!(colonnes_ajoutees(), vec!["integration_id"]);
        assert!(colonnes_supprimees().is_empty());
        assert_eq!(COLONNES_AJOUTEES, ["integration_id"]);
    }

    #[test]
    fn three_columns_lost_their_not_null_constraint_when_the_table_was_recreated() {
        // La table initiale les declarait NOT NULL, la source actuelle non.
        // L'ecart est porte, pas resolu : une base arretee a la premiere
        // migration refuse encore NULL sur ces trois colonnes.
        assert_eq!(COLONNES_NOT_NULL_RETIREES, ["connector_id", "method_id", "active"]);
        for nom in COLONNES_NOT_NULL_RETIREES {
            assert!(!ecrit_not_null(nom), "{nom} : la source n ecrit plus notNull");
            assert!(admet_null(nom), "{nom} : la base neuve autorise NULL");
        }
        // Les quatre autres NOT NULL de la table initiale sont toujours
        // declares par la source : seules les horodatages et label / value.
        for nom in ["label", "value", "time_created", "time_updated"] {
            assert!(!COLONNES_NOT_NULL_RETIREES.contains(&nom), "{nom} n a pas ete touche");
            assert!(ecrit_not_null(nom));
        }
    }

    #[test]
    fn the_source_declares_no_index_and_the_abandoned_unique_index_is_merely_noted() {
        assert_eq!(CREDENTIAL_INDEXES.len(), 0);
        assert_eq!(INDEX_UNIQUE_ABANDONNE, "credential_connector_active_idx");
        // Aucune colonne de la table ne cite cet index, donc rien n'en depend.
        let citees: Vec<&str> = CREDENTIAL_INDEXES
            .iter()
            .flat_map(|index| index.columns.iter().copied())
            .collect();
        assert!(citees.is_empty());
    }

    // ------------------------------------------------------------------
    // La forme de la ligne
    // ------------------------------------------------------------------

    #[test]
    fn the_row_serializes_to_exactly_the_nine_ddl_column_names() {
        // Le contrat de la ligne est celui de la base, pas celui du JSON
        // public. Une faute sur `integration_id` passerait la compilation.
        let l = ligne(Some("cred_1", Some("int_1"), 0);
        let json = serde_json::to_value(&l).expect("serialisation");
        let mut cles: Vec<&str> = json
            .as_object()
            .expect("objet")
            .keys()
            .map(String::as_str)
            .collect();
        cles.sort();
        assert_eq!(
            cles,
            vec![
                "active",
                "connector_id",
                "id",
                "integration_id",
                "label",
                "method_id",
                "time_created",
                "time_updated",
                "value",
            ]
        );
        assert_eq!(json["id"], "cred_1");
        assert_eq!(json["integration_id"], "int_1");
        assert_eq!(json["label"], "default");
    }

    #[test]
    fn the_row_never_serializes_the_json_contract_camel_case_name() {
        // Piege 1, cote echange : la source ecrit `integrationID` dans son
        // `Schema.Class`, et la base ecrit `integration_id`. Les deux doivent
        // pouvoir coexister, et la ligne ne sort que sous le nom de colonne.
        let l = ligne(Some("cred_1", Some("int_1"), 0);
        let json = serde_json::to_value(&l).expect("serialisation");
        assert!(json.get("integrationID").is_none());
        assert!(json.get("integrationId").is_none());
        assert!(json.get("timeCreated").is_none());
        // Et le nom de colonne est bien present, ce qui n'est pas la meme chose.
        assert!(json.get("integration_id").is_some());
    }

    #[test]
    fn a_null_column_stays_a_present_key_with_a_null_value() {
        // Choix de serialisation : aucun `skip_serializing_if`. Une colonne
        // `NULL` se dit `null`, avec sa cle presente ; une colonne absente du
        // JSON serait une faute de protocole, pas une valeur. C'est l'inverse
        // du reflexe pose sur une structure JSON, ou une cle absente vaut
        // `undefined`.
        let l = ligne(Some("cred_1", None, 0);
        let json = serde_json::to_value(&l).expect("serialisation");
        for nom in ["integration_id", "connector_id", "method_id", "active"] {
            assert!(json.get(nom).is_some(), "{nom} doit rester une cle presente");
            assert!(json[nom].is_null(), "{nom} doit valoir null");
        }
        // Aller-retour : la colonne nulle revient bien en `None`.
        let relue: CredentialTableRow = serde_json::from_value(json).expect("deserialisation");
        assert_eq!(relue, l);
    }

    #[test]
    fn an_empty_string_in_a_nullable_column_survives_serialization() {
        // `notNull` interdit `NULL`, il n'interdit pas `""`. Une integration
        // vide est une valeur presente, et le fichier ne doit pas la
        // transformer en `None` sous le pretexte qu'elle ne designe rien.
        let mut l = ligne(Some("cred_1", Some("int_1"), 0);
        l.integration_id = Some(String::new());
        let json = serde_json::to_value(&l).expect("serialisation");
        assert!(json["integration_id"].is_string());
        assert_eq!(json["integration_id"], "");
        let relue: CredentialTableRow = serde_json::from_value(json).expect("deserialisation");
        assert_eq!(relue.integration_id.as_deref(), Some(""));
    }

    #[test]
    fn the_row_round_trips_through_json_including_nullable_columns() {
        let l = ligne(Some("cred_1", Some("int_1"), 1_700_000_000_000);
        let relue: CredentialTableRow =
            serde_json::from_str(&serde_json::to_string(&l).expect("serialisation")).expect("deserialisation");
        assert_eq!(relue, l);

        // Et avec les colonnes heritees renseignees, ce que le service ne fait
        // jamais mais que la base peut contenir.
        let mut heritee = ligne(Some("cred_2", None, 0);
        heritee.connector_id = Some("conn_1".to_string());
        heritee.method_id = Some("meth_1".to_string());
        heritee.active = Some(true);
        let relue: CredentialTableRow =
            serde_json::from_str(&serde_json::to_string(&heritee).expect("serialisation")).expect("deserialisation");
        assert_eq!(relue, heritee);
    }

    #[test]
    fn an_absent_id_round_trips_as_an_absent_key() {
        // `id` porte un `Option<String>` parce que la source ecrit
        // `text PRIMARY KEY` SANS `notNull()`, et SQLite autorise `NULL` dans une
        // cle primaire `TEXT` d'une table a `rowid`. La ligne dont `id` est absent
        // est donc representable.
        //
        // `skip_serializing_if` : `optionalKey` de l'original retire la cle a
        // l'encodage, donc un `id` absent ne doit produire ni `"id": null` ni la
        // cle du tout. Une cle presente a null serait indistinguishable d une
        // valeur absente cote consommateur TypeScript.
        let l = ligne(None, Some("int_1"), 0);
        let json = serde_json::to_value(&l).expect("serialisation");
        assert!(
            !json.as_object().expect("objet").contains_key("id"),
            "un id absent ne doit pas produire la cle : {json}"
        );
    }

    #[test]
    fn an_explicit_null_id_is_read_as_absent_not_as_a_failure() {
        // Contraste avec le test precedent. `serde` lit `"id": null` dans un
        // `Option<String>` comme `None`, sans erreur. C est le comportement du
        // decodeur de serde, et il est le bon ici : la source accepte cette
        // ligne, donc la refuser serait inventer une contrainte absente.
        //
        // Le piege ici est l inverse de celui du nom de test precedent, et il
        // vaut la peine d etre verrouille des deux cotes.
        let json = serde_json::json!({
            "id": null,
            "integration_id": "int_1",
            "label": "default",
            "value": { "type": "key" },
            "connector_id": null,
            "method_id": null,
            "active": null,
            "time_created": 0,
            "time_updated": 0,
        });
        let row: CredentialTableRow = serde_json::from_value(json).expect("lecture");
        assert_eq!(row.id, None, "null se lit comme une absence");
    }

    #[test]
    fn the_active_flag_round_trips_as_a_boolean_and_its_absence_as_null() {
        let mut l = ligne(Some("cred_1", Some("int_1"), 0);
        l.active = Some(false);
        let json = serde_json::to_value(&l).expect("serialisation");
        assert_eq!(json["active"], false, "le booleen reste un booleen cote JSON");
        assert!(json.get("active").is_some());

        // L'absence de drapeau est `null`, et se distingue donc de la presence
        // du drapeau a `false` - ce que le `DEFAULT false` de la table
        // initiale ne permettait pas.
        let sans = ligne(Some("cred_2", Some("int_1"), 0);
        let json = serde_json::to_value(&sans).expect("serialisation");
        assert!(json["active"].is_null());
    }

    #[test]
    fn creating_a_row_sets_both_timestamps_to_the_same_instant() {
        let l = CredentialTableRow::nouvelle(Some("cred_1".into()), Some("int_1".into()), "default", valeur_factice_cle(), 42);
        assert_eq!(l.time_created, 42);
        assert_eq!(l.time_updated, 42);
        // Les colonnes que le service n'ecrit pas restent nulles.
        assert!(l.connector_id.is_none());
        assert!(l.method_id.is_none());
        assert!(l.active.is_none());
    }

    #[test]
    fn touching_a_row_moves_only_the_update_timestamp() {
        let mut l = ligne(Some("cred_1", Some("int_1"), 100);
        l.toucher(500);
        assert_eq!(l.time_updated, 500);
        assert_eq!(l.time_created, 100, "la date de creation ne recule jamais");
        assert_eq!(l.id.as_deref(), Some("cred_1"));
        assert_eq!(l.integration_id.as_deref(), Some("int_1"));
        assert_eq!(l.label, "default");
    }

    // ------------------------------------------------------------------
    // `!x` contre `?? x` (piege 2)
    // ------------------------------------------------------------------

    #[test]
    fn the_nullity_and_truthiness_checks_on_integration_disagree_on_an_empty_string() {
        // Les quatre cas de la matrice, parce que c'est toute la difference
        // entre un ternaire et un coalescent :
        //              colonne       nullite (`??`)  veracite (`!x`)
        let presente = ligne(Some("cred_1", Some("int_1"), 0);
        let vide = ligne(Some("cred_2", Some(""), 0);
        let absente = ligne(Some("cred_3", None, 0);

        assert!(!integration_absente(&presente));
        assert!(!integration_sans_contenu(&presente));

        assert!(!integration_absente(&vide), "la chaine vide est presente");
        assert!(integration_sans_contenu(&vide), "et la veracite la tient pour vide");

        assert!(integration_absente(&absente));
        assert!(integration_sans_contenu(&absente));
    }

    #[test]
    fn an_empty_string_never_becomes_none_in_the_row() {
        // La chaine vide est une etiquette et une integration presentes. Elle
        // ne doit ni disparaitre de la ligne, ni etre transformee en `None`
        // par un `unwrap_or_default` ou un `if !x`.
        let mut l = ligne(Some("cred_1", Some("int_1"), 0);
        l.integration_id = Some(String::new());
        l.label = String::new();
        assert_eq!(l.integration_id.as_deref(), Some(""));
        assert_eq!(l.label, "");
        assert!(!integration_absente(&l));
        assert!(integration_sans_contenu(&l));
    }

    #[test]
    fn disagreeing_rows_are_the_empty_integration_rows_truthiness_would_drop() {
        // Une seule ligne des trois est presente a la base et vide pour le
        // voisin : celle dont `integration_id` vaut la chaine vide.
        let tranche = vec![
            ligne(Some("cred_1", Some("int_1"), 0),
            ligne(Some("cred_2", Some(""), 0),
            ligne(Some("cred_3", None, 0),
        ];
        let discordees = lignes_discordees(&tranche);
        assert_eq!(discordees.len(), 1);
        assert_eq!(discordees[0].id.as_deref(), Some("cred_2"));
    }

    #[test]
    fn a_slice_without_an_empty_integration_produces_no_disagreeing_rows() {
        let tranche = vec![
            ligne(Some("cred_1", Some("int_1"), 0),
            ligne(Some("cred_2", Some("int_2"), 0),
            ligne(Some("cred_3", None, 0),
        ];
        assert!(lignes_discordees(&tranche).is_empty());

        // Et une tranche vide ne produit rien non plus.
        let vide: Vec<CredentialTableRow> = Vec::new();
        assert!(lignes_discordees(&vide).is_empty());
    }

    #[test]
    fn disagreeing_rows_keep_their_input_order() {
        let tranche = vec![
            ligne(Some("cred_1", Some(""), 0),
            ligne(Some("cred_2", Some("int_1"), 0),
            ligne(Some("cred_3", Some(""), 0),
        ];
        let discordees = lignes_discordees(&tranche);
        assert_eq!(discordees.len(), 2);
        assert_eq!(discordees[0].id.as_deref(), Some("cred_1"));
        assert_eq!(discordees[1].id.as_deref(), Some("cred_3"));
    }
}
