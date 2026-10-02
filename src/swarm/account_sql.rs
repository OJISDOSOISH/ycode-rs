//! Portage Rust de `opencode/packages/core/src/account/sql.ts`.
//!
//! ## Ce que porte reellement la source
//!
//! Trente-neuf lignes, et **trois tables Drizzle, zero fonction** :
//!
//! ```ts
//! export const AccountTable = sqliteTable("account", {
//!   id: text().$type<AccountV2.ID>().primaryKey(),
//!   email: text().notNull(),
//!   url: text().notNull(),
//!   access_token: text().$type<AccountV2.AccessToken>().notNull(),
//!   refresh_token: text().$type<AccountV2.RefreshToken>().notNull(),
//!   token_expiry: integer(),
//!   ...Timestamps,
//! })
//!
//! export const AccountStateTable = sqliteTable("account_state", {
//!   id: integer().primaryKey(),
//!   active_account_id: text()
//!     .$type<AccountV2.ID>()
//!     .references(() => AccountTable.id, { onDelete: "set null" }),
//!   active_org_id: text().$type<AccountV2.OrgID>(),
//! })
//!
//! // LEGACY
//! export const ControlAccountTable = sqliteTable(
//!   "control_account",
//!   { /* ... */ },
//!   (table) => [primaryKey({ columns: [table.email, table.url] })],
//! )
//! ```
//!
//! Il n'y a donc rien a executer. Ce qui est decrit est un **contrat de
//! stockage**, et sa seule traduction honnete sans dependance SQL est la forme
//! de la ligne, plus les contraintes que cette ligne porte, exprimees sous
//! forme de fonctions pures.
//!
//! ## Pourquoi il n'y a pas de SQL ici
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : il n'existe
//! aucun moyen d'ecrire une requete, et ajouter une dependance n'est pas du
//! ressort d'un portage de fichier. Ce fichier ne cree **aucune migration** non
//! plus. Le SQL sera branche plus tard, par decision du projet ; ce fichier
//! prepare donc le contrat que cette couche viendra consommer - noms de tables,
//! de colonnes, de cle etrangere et de contrainte de cle primaire - sans
//! pretendre executer quoi que ce soit.
//!
//! ## La forme exacte est deja connue : ce n'est pas une invention
//!
//! `packages/core/src/database/schema.gen.ts` contient le DDL genere par
//! Drizzle : lignes 27 a 32 pour `account_state`, lignes 35 a 44 pour `account`,
//! lignes 47 a 57 pour `control_account`. Ce sont ces chaines qui font foi, et non
//! le code declaratif, parce que ce sont elles que SQLite executera un jour :
//!
//! ```sql
//! CREATE TABLE `account_state` (
//!   `id` integer PRIMARY KEY,
//!   `active_account_id` text,
//!   `active_org_id` text,
//!   CONSTRAINT `fk_account_state_active_account_id_account_id_fk`
//!     FOREIGN KEY (`active_account_id`) REFERENCES `account`(`id`) ON DELETE SET NULL
//! );
//! CREATE TABLE `account` (
//!   `id` text PRIMARY KEY,
//!   `email` text NOT NULL,
//!   `url` text NOT NULL,
//!   `access_token` text NOT NULL,
//!   `refresh_token` text NOT NULL,
//!   `token_expiry` integer,
//!   `time_created` integer NOT NULL,
//!   `time_updated` integer NOT NULL
//! );
//! CREATE TABLE `control_account` (
//!   `email` text NOT NULL,
//!   `url` text NOT NULL,
//!   `access_token` text NOT NULL,
//!   `refresh_token` text NOT NULL,
//!   `token_expiry` integer,
//!   `active` integer NOT NULL,
//!   `time_created` integer NOT NULL,
//!   `time_updated` integer NOT NULL,
//!   CONSTRAINT `control_account_pk` PRIMARY KEY(`email`, `url`)
//! );
//! ```
//!
//! ## Le point central de ce fichier : deux drapeaux, pas un
//!
//! Le modele Drizzle porte **deux drapeaux independants** sur une cle
//! primaire : `notNull` et `primaryKey`. Les confondre est l'erreur qui ne se
//! voit pas a la compilation et ne se voit qu'a l'echange avec la base.
//!
//! La source ecrit `id: text().$type<AccountV2.ID>().primaryKey()`, **sans**
//! `.notNull()`. Le DDL produit dit `id text PRIMARY KEY`, sans `NOT NULL`, et
//! l'instantane de `drizzle-kit` enregistre `"notNull": false`. Ce qui est
//! notable, c'est que `id: text().notNull().primaryKey()` produirait le meme
//! DDL et le meme instantane : sur cette colonne, `notNull` est ecrit et perd.
//! SQLite ne le rattrape pas - une `TEXT PRIMARY KEY` d'une table a `rowid`
//! **accepte `NULL`**, et deux lignes nulles n'entrent donc pas en collision
//! sur la cle primaire.
//!
//! C'est pourquoi [`AccountRow::id`] est un `Option<String>` et non un
//! `String`, et pourquoi [`first_duplicate_by_id`] existe : sans elle, on
//! croirait a tort que l'unicite de `account.id` est garantie par la base.
//!
//! `src/swarm/permission_sql.rs` a choisi `String` pour `permission.id`, dans
//! une situation declarative identique. L'ecart est voulu et ne doit pas etre
//! lu comme une contradiction : la-bas rien dans le depot ne produit de `id`
//! nul, et le porter en `Option` aurait propage une possibilite inatteignable.
//! Ici la nullite est **porteuse de sens** : `ON DELETE SET NULL` travaille sur
//! la colonne voisine `active_account_id`, la cle etrangere pointe vers
//! `account.id`, et la cle primaire de `control_account` est, elle, composite
//! et porte sur deux colonnes `NOT NULL`. Le raisonnement sur `NULL` traverse
//! donc les trois tables de ce fichier, il ne peut pas rester invisible.
//!
//! Pour la meme raison, ce fichier ne dit **jamais** qu'une colonne "est nulle" :
//! il dit soit que la source **ecrit** `notNull()`, soit que SQLite
//! **autorise** `NULL`. Ces deux verites sont portees par deux listes
//! distinctes, [`ACCOUNT_NOT_NULL_DECLARES`] et [`ACCOUNT_PRIMARY_KEY`], et la
//! seule fonction qui les rapproche est
//! [`admet_null_sans_pragma`].
//!
//! ## Une derive reelle entre la base neuve et la base migree
//!
//! `database/migration/20260228203230_blue_harpoon.ts` ligne 23 a cree
//! `account_state` avec `` `id` integer PRIMARY KEY NOT NULL ``, tandis que
//! `schema.gen.ts` ligne 28 ecrit `` `id` integer PRIMARY KEY ``. Les deux
//! echappements sont donc faux pour la meme colonne, selon la base :
//!
//! - une base creee par les migrations a `account_state.id NOT NULL` ;
//! - une base creee par le DDL genere l'acceptent a `NULL`.
//!
//! Le portage ne choisit pas entre les deux : il porte `id` comme
//! [`AccountStateRow::id`] non optionnel, parce que l identifiant d'une ligne
//! d'etat est ecrit par le code qui la cree, et signale l'ecart. Une ligne
//! d'etat sans identifiant n'a pas de sens metier, contrairement a un `id` de
//! compte sans identifiant, qui est une faute de donnee et non un etat valide.
//!
//! ## Le piege de nommage, qui est reel ici
//!
//! Les huit colonnes de `account` sont toutes en `snake_case` et correspondent
//! a des noms de champ identiques. Le piege n'est donc pas la casse, il est
//! **l'absence** de piege apparente, et c'est ce qui le rend dangereux : une
//! faute de frappe sur `access_token` ou sur `token_expiry` passerait la
//! compilation et casserait au premier echange. Les `#[serde(rename = "...")]`
//! ci-dessous sont donc ecrits explicitement malgre leur redondance : ils
//! figent le contrat du DDL, invisible de l'interieur du code Rust.
//!
//! Contraste utile : `AccountV2.Info` (dans `src/account.ts` ligne 24) expose
//! `active_org_id`, mais **pas** `active_account_id`, `token_expiry` ni les
//! jetons. Le JSON public et la ligne SQL n'ont pas le meme contenu, et
//! serialiser la ligne avec les noms du JSON n'est pas le bon deplacement dans
//! ce fichier-ci. Un test le verifie.
//!
//! ## Les types marques n'existent qu'a la compilation
//!
//! `AccountV2.ID`, `AccountV2.OrgID`, `AccountV2.AccessToken` et
//! `AccountV2.RefreshToken` sont des `Schema.String` d'Effect
//! (`src/account.ts` lignes 6 a 16) : a l'execution ce sont des chaines
//! ordinaires, que `AccountV2.ID.make("account-test")` fabrique a partir d'une
//! simple chaine, comme le fait le test `httpapi-experimental.test.ts` ligne 63.
//! Aucun type du swarm ne les porte deja, on ne donc **pas** inventer de
//! marqueur Rust : ce sont des `String`.
//!
//! ## `active` : un booleen stocke en entier
//!
//! `active: integer({ mode: "boolean" }).notNull().$default(() => false)` produit
//! `active integer NOT NULL`, sans `DEFAULT` dans le DDL : le `false` par
//! defaut est un crochet de l'ORM, applique a la construction de la requete, et
//! non une contrainte de la base. Consequence concrete, identique a celle que
//! [`ControlAccountRow`] porte sur `time_updated` : la colonne doit etre
//! fournie a l'insertion, faute de quoi la base refuse la ligne.
//!
//! ## Le piege `?` contre `??`, signale sans le porter
//!
//! Ce fichier ne contient ni ternaire ni coalescent : il n'y a rien a y
//! confondre. Le piege se pose chez les voisins, sur **les memes colonnes** :
//! `active_account_id` et `active_org_id` sont des pointeurs optionnels, et un
//! appelant qui ecrit `state.active_account_id ? eq(...) : undefined` traite une
//! chaine vide comme absente, la ou `??` la traiterait comme presente.
//!
//! On ne l'invente pas ici, et on n'ecrit pas de fonction de filtrage qui
//! trancherait : ce serait introduire un comportement que la source ne contient
//! pas. Ce qui est porte a la place, c'est la distinction elle-meme :
//! [`resolve_active_account`] raisonne sur la **nullite** du pointeur, parce
//! qu'une cle etrangere `ON DELETE SET NULL` produit `NULL` et non `""`. Un
//! futur portage de l'appelant pourra s'appuyer sur ce choix, qui est explicite,
//! plutot que de le redecouvrir.
//!
//! ## Le perimetre
//!
//! Ce fichier porte :
//!
//! - les trois formes de ligne, colonnes dans l'ordre du `CREATE TABLE` ;
//! - les deux drapeaux `notNull` / `primaryKey` par table, separes ;
//! - la cle primaire composite `(email, url)` de `control_account`, sous forme
//!   de cle a deux composants, avec detection de doublon et filtrage
//!   d'insertion ;
//! - le `ON DELETE SET NULL` de `account_state.active_account_id`, vu comme une
//!   projection sur une tranche ;
//! - la resolution du compte actif, y compris le pointeur pendant ;
//! - le report de `active_org_id` de la migration `move_org_to_state`, dont on
//!   ne peut retenir que l'ensemble des lignes touchees, la colonne source
//!   ayant ete supprimee par la migration elle-meme.
//!
//! Il ne porte **pas** :
//!
//! - `ControlAccountRow::active` comme colonne de production, ni le `true`
//!   initial : la table est explicitement `// LEGACY` dans la source.
//! - Les effets de connexion, de rafraichissement de jeton, ni les classes
//!   d'erreur de `src/account.ts` : elles sont dans `account.ts`, pas ici.
//! - Les services de l'API HTTP d'authentification, qui vivent ailleurs.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Nom de la table des comptes.
pub const ACCOUNT_TABLE_NAME: &str = "account";

/// Nom de la table d'etat : au plus une ligne logique, designee par son `id`.
pub const ACCOUNT_STATE_TABLE_NAME: &str = "account_state";

/// Nom de la table heritee du plan de controle.
///
/// La source la marque `// LEGACY` (ligne 24). Elle est portee ici parce que
/// son DDL fait partie du schema genere et qu'une base migree la contient
/// encore, pas parce qu'elle est recommandee.
pub const CONTROL_ACCOUNT_TABLE_NAME: &str = "control_account";

/// Nom de la cle etrangere de `account_state.active_account_id`.
///
/// SQLite ne respecte cette contrainte que si le mode `PRAGMA foreign_keys` est
/// actif. L'activer n'est pas du ressort de ce fichier, et la consequence est
/// signalee sur [`rows_kept_after_account_delete`] et
/// [`resolve_active_account`].
pub const ACTIVE_ACCOUNT_FOREIGN_KEY_NAME: &str = "fk_account_state_active_account_id_account_id_fk";

/// Nom de la contrainte de cle primaire composite de `control_account`.
///
/// C'est le nom que Drizzle donne a la sortie de `primaryKey({ columns: [...] })`
/// en troisieme argument de `sqliteTable`.
pub const CONTROL_ACCOUNT_PRIMARY_KEY_NAME: &str = "control_account_pk";

// ---------------------------------------------------------------------------
// Les deux drapeaux, separes
// ---------------------------------------------------------------------------

/// Colonnes de `account` sur lesquelles **la source ecrit** `notNull()`.
///
/// Cette liste ne dit rien de ce que SQLite autorise. Elle est la transcription
/// litterale du code, et c'est la seule question a laquelle la source permet
/// de repondre. `id` n'y figure pas : la source y ecrit `primaryKey()` sans
/// `notNull()`. `token_expiry` n'y figure pas non plus, et la encore sans
/// ambiguite.
pub const ACCOUNT_NOT_NULL_DECLARES: [&str; 6] = [
    "email",
    "url",
    "access_token",
    "refresh_token",
    "time_created",
    "time_updated",
];

/// Colonnes de `account` portant la cle primaire, au sens du DDL genere.
pub const ACCOUNT_PRIMARY_KEY: [&str; 1] = ["id"];

/// Colonnes de `account_state` sur lesquelles la source ecrit `notNull()`.
///
/// Elle n'en ecrit sur aucune : les trois colonnes de la table sont declarees
/// sans le predicat. Le tableau est donc vide, et il est **conserve** malgre
/// cela : c'est l'absence de `notNull()` qui est une information, et un tableau
/// vide la dit aussi clairement qu'un tableau plein.
pub const ACCOUNT_STATE_NOT_NULL_DECLARES: [&str; 0] = [];

/// Colonnes de `account_state` portant la cle primaire.
pub const ACCOUNT_STATE_PRIMARY_KEY: [&str; 1] = ["id"];

/// Colonnes de `control_account` sur lesquelles la source ecrit `notNull()`.
pub const CONTROL_ACCOUNT_NOT_NULL_DECLARES: [&str; 7] = [
    "email",
    "url",
    "access_token",
    "refresh_token",
    "active",
    "time_created",
    "time_updated",
];

/// Colonnes de `control_account` composant la cle primaire, dans l'ordre de
/// la contrainte.
///
/// L'ordre fait partie de la contrainte : `PRIMARY KEY(email, url)` n'est pas
/// la meme chose que `PRIMARY KEY(url, email)` du point de vue du tri que
/// SQLite garantit, et il est aussi l'ordre du premier argument
/// de [`ControlAccountKey`].
pub const CONTROL_ACCOUNT_PRIMARY_KEY: [&str; 2] = ["email", "url"];

/// Le DDL autorise-t-il `NULL` dans cette colonne ?
///
/// La fonction qui rapproche les deux drapeaux, et **seule** fonction du
/// fichier a le faire. Elle prend deux booleans et non des noms de colonnes,
/// volontairement : le schema declaratif generique avec ses drapeaux par
/// colonne est deja porte par `src/swarm/event_sql.rs`, et le dupliquer ici
/// ferait diverger deux descriptions du meme modele.
///
/// La reponse est `!not_null_declare || est_primaire` :
///
/// - la source ecrit `notNull()` : `NULL` est refuse ;
/// - la source n'ecrit rien : `NULL` est admis ;
/// - la colonne porte la cle primaire : `NULL` est admis **malgre tout**, car le
///   DDL produit par Drizzle n'y met pas de `NOT NULL` et que SQLite, dans une
///   table a `rowid`, ne traite pas `PRIMARY KEY` comme `NOT NULL`.
///
/// Le troisieme cas est le raison d'etre de cette fonction. C'est lui qui
/// explique que `account.id` soit un `Option<String>` alors que la colonne est
/// une cle primaire.
pub fn admet_null_sans_pragma(not_null_declare: bool, est_primaire: bool) -> bool {
    !not_null_declare || est_primaire
}

// ---------------------------------------------------------------------------
// Colonnes
// ---------------------------------------------------------------------------

/// Les huit colonnes de `account`, dans l'ordre du `CREATE TABLE`.
///
/// L'ordre n'est pas cosmetique : c'est lui qu'un futur
/// `INSERT INTO account (...) VALUES (...)` devra suivre. Il est aussi l'ordre
/// des champs de [`AccountRow`], ce qui permet de verifier d'un coup d'oeil
/// que la ligne et la table ne derivent pas l'une de l'autre.
pub const ACCOUNT_COLUMNS: [&str; 8] = [
    "id",
    "email",
    "url",
    "access_token",
    "refresh_token",
    "token_expiry",
    "time_created",
    "time_updated",
];

/// Les trois colonnes de `account_state`, dans l'ordre du `CREATE TABLE`.
pub const ACCOUNT_STATE_COLUMNS: [&str; 3] = ["id", "active_account_id", "active_org_id"];

/// Les huit colonnes de `control_account`, dans l'ordre du `CREATE TABLE`.
pub const CONTROL_ACCOUNT_COLUMNS: [&str; 8] = [
    "email",
    "url",
    "access_token",
    "refresh_token",
    "token_expiry",
    "active",
    "time_created",
    "time_updated",
];

// ---------------------------------------------------------------------------
// account
// ---------------------------------------------------------------------------

/// Une ligne de la table `account`.
///
/// C'est le type que rend un `select()` sur `AccountTable`. Les huit champs
/// correspondent, dans l'ordre, aux huit colonnes du DDL genere.
///
/// Les `rename` ci-dessous sont identiques aux noms de champ. Ils sont ecrits
/// explicitement malgre cela : ils figent le contrat du DDL, qui est invisible
/// de l'interieur du code Rust, et c'est exactement ce que la relecture doit
/// pouvoir verifier d'un coup d'oeil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRow {
    /// Cle primaire, `text PRIMARY KEY` - **sans `NOT NULL` dans le DDL**.
    ///
    /// Le type est marque `AccountV2.ID` dans le TypeScript, marque qui
    /// n'existe qu'a la compilation : a l'execution c'est une chaine, d'ou le
    /// `String` interne.
    ///
    /// L'option n'est pas une commodite de style. La source ecrit
    /// `primaryKey()` sans `notNull()`, le DDL produit est `id text PRIMARY
    /// KEY` et SQLite y accepte `NULL` : une ligne sans identifiant est
    /// **physiquement stockable**, et deux telles lignes n'entrent pas en
    /// collision sur la cle primaire. Voir [`first_duplicate_by_id`], qui est
    /// la traduction pure de ce comportement.
    #[serde(rename = "id")]
    pub id: Option<String>,
    /// `text NOT NULL`. Adresse du compte chez le fournisseur.
    #[serde(rename = "email")]
    pub email: String,
    /// `text NOT NULL`. URL du deploiement de l'API qui porte ce compte.
    #[serde(rename = "url")]
    pub url: String,
    /// `text NOT NULL`. Marque `AccountV2.AccessToken` a la compilation.
    ///
    /// La colonne ne porte aucune contrainte de contenu : une chaine vide
    /// traverse le contrat, et c'est le contrat SQL qui l'autorise, pas une
    /// decision de ce fichier.
    #[serde(rename = "access_token")]
    pub access_token: String,
    /// `text NOT NULL`. Marque `AccountV2.RefreshToken` a la compilation.
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
    /// `integer`, nullable, et ce pour une raison differente de `id`.
    ///
    /// `id` est nullable parce que la cle primaire n'impose pas `NOT NULL` en
    /// SQLite : c'est une faiblesse du schema. `token_expiry` est nullable parce
    /// que la source **l'a voulu** : ne pas avoir d'expiration enregistree est un
    /// etat normal d'un compte, pas une faute de donnee.
    ///
    /// `None` signifie "aucune expiration enregistree", ce qui est une donnee
    /// legitime et non une absence de ligne.
    ///
    /// **L'unite n'est pas determinee par ce fichier.** Elle ne l'est ni par la
    /// declaration `integer()` ni par l'instantane de `drizzle-kit`. Toutes les
    /// fonctions qui manipulent cette colonne prennent donc un `now` exprime
    /// dans la meme unite, et c'est a l'appelant de la choisir. Comparer une
    /// expiration en secondes a un `Date.now()` en millisecondes donnerait un
    /// resultat faux sans aucun signal.
    #[serde(rename = "token_expiry")]
    pub token_expiry: Option<i64>,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_created")]
    pub time_created: i64,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_updated")]
    pub time_updated: i64,
}

impl AccountRow {
    /// Construit une ligne a l'insertion, en positionnant les deux
    /// horodatages sur le meme `now`.
    ///
    /// Le `now` est fourni par l'appelant et non lu dans l'horloge : cette
    /// fonction reste pure et testable.
    pub fn new(
        id: Option<String>,
        email: &str,
        url: &str,
        access_token: &str,
        refresh_token: &str,
        token_expiry: Option<i64>,
        now: i64,
    ) -> Self {
        Self {
            id,
            email: email.to_string(),
            url: url.to_string(),
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            token_expiry,
            time_created: now,
            time_updated: now,
        }
    }

    /// Applique une mise a jour : seul `time_updated` bouge.
    ///
    /// C'est la traduction de `$onUpdate(() => Date.now())`. Le `id`,
    /// l'adresse, l'URL, les deux jetons, l'expiration et `time_created`
    /// restent inchanges.
    pub fn touch(&mut self, now: i64) {
        self.time_updated = now;
    }

    /// L'expiration enregistree, lue comme une valeur ou comme une absence.
    ///
    /// Cette fonction ne fait qu'ouvrir l'option ; elle existe pour que les
    /// lectures de `token_expiry` dans le reste du module passent par un nom
    /// unique, et pour que le cas "aucune expiration enregistree" ne soit
    /// jamais traite en silence comme une expiration a zero.
    pub fn expiration(&self) -> Expiration {
        match self.token_expiry {
            None => Expiration::Inconnue,
            Some(valeur) => Expiration::At(valeur),
        }
    }

    /// Cette ligne est-elle perimee a l'instant `now` ?
    ///
    /// `now` doit etre exprime dans la **meme unite** que `token_expiry` : voir
    /// [`AccountRow::token_expiry`].
    ///
    /// Une ligne sans expiration enregistree n'est **pas** perimee. C'est un
    /// choix de lecture, pas une deduction : `None` signifie que la base ne sait
    /// rien de la duree de vie du jeton, et traiter cette ignorance comme une
    /// expiration deja passee forcerait un rafraichissement a chaque lecture.
    /// Le choix contraire - `None` vaut perimee - est tout aussi defendable et
    /// produirait le comportement opposite ; il est signale ici parce que c'est
    /// le point le plus faible de ce fichier, et parce qu'il n'est tranche que
    /// par le code appelant, pas par le schema.
    pub fn est_perimee_a(&self, now: i64) -> bool {
        match self.token_expiry {
            None => false,
            Some(expiration) => expiration <= now,
        }
    }
}

/// L'etat d'expiration d'un jeton, distingue de sa valeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expiration {
    /// Aucune expiration enregistree : la colonne vaut `NULL`.
    ///
    /// Ce n'est pas "expiree" et ce n'est pas "valide" non plus, c'est
    /// "inconnu". La distinction existe parce que la colonne est nullable et
    /// que confondre les trois etats rend la panne invisible.
    Inconnue,
    /// Expiration enregistree, dans l'unite de la colonne.
    At(i64),
}

/// Premiere collision d'identifiant dans une tranche de lignes de compte.
///
/// C'est la violation de la cle primaire de `account`, vue sur les donnees - et
/// c'est aussi la fonction qui rend visible le piege du fichier.
///
/// Le detail qui compte est qu'une ligne **sans** identifiant n'entre jamais en
/// collision, y compris avec une autre ligne sans identifiant. La cle primaire
/// de `account` est `id text PRIMARY KEY` sans `NOT NULL`, SQLite y accepte
/// `NULL`, et le moteur ne considere pas deux `NULL` comme egaux pour une
/// contrainte d'unicite. La cle primaire n'y garantit donc strictement rien.
///
/// Le type de retour garde malgre tout son `Option` interieur : seule la
/// variante `Some(Some(id))` est atteignable, mais la distinguer de `None`
/// reste ce qui permet au appelant de dire *quelle* ligne est en double.
///
/// "Premiere" signifie premiere dans l'ordre de la tranche, ce qui rend le
/// resultat stable et dependant uniquement de l'ordre de lecture.
pub fn first_duplicate_by_id(rows: &[AccountRow]) -> Option<Option<String>> {
    let mut vues: BTreeSet<&str> = BTreeSet::new();
    for row in rows {
        // Une ligne sans identifiant saute le jeu : c'est ce `continue` qui
        // reproduit le comportement de SQLite sur les `NULL`.
        let Some(id) = row.id.as_deref() else {
            continue;
        };
        if !vues.insert(id) {
            return Some(Some(id.to_string()));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// account_state
// ---------------------------------------------------------------------------

/// Une ligne de la table `account_state`.
///
/// C'est une table d'etat et non d'entites : elle porte un pointeur vers le
/// compte actif, pas le compte. Les trois colonnes correspondent, dans l'ordre,
/// aux trois colonnes du DDL genere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountStateRow {
    /// Cle primaire, `integer PRIMARY KEY`.
    ///
    /// Le type reste non optionnel, contrairement a [`AccountRow::id`], et
    /// l'ecart est delibere. Le DDL genere ecrit `id integer PRIMARY KEY` sans
    /// `NOT NULL`, mais la migration `blue_harpoon` ligne 23 a cree la colonne
    /// avec `NOT NULL` : les deux verites ne convergent pas. On ne resout pas
    /// cette derive ici, on la signale.
    ///
    /// Cote metier, un pointeur d'etat sans identifiant n'a pas de sens : c'est
    /// la ligne *courante*, pas une collection. Un `id` de compte manquant, lui,
    /// est une faute de donnee qu'il faut pouvoir observer.
    #[serde(rename = "id")]
    pub id: i64,
    /// `text`, et cle etrangere vers `account.id` avec `ON DELETE SET NULL`.
    ///
    /// Nullable pour deux raisons qui se cumulent : la source n'ecrit pas
    /// `notNull()`, et surtout la suppression d'un compte **met cette colonne a
    /// `NULL`** sans supprimer la ligne d'etat. Une ligne d'etat sans compte
    /// actif est donc un etat normal, pas une corruption.
    ///
    /// Le type est marque `AccountV2.ID` dans le TypeScript, sans effet a
    /// l'execution.
    #[serde(rename = "active_account_id")]
    pub active_account_id: Option<String>,
    /// `text`, nullable, marque `AccountV2.OrgID` a la compilation.
    ///
    /// La colonne a ete ajoutee apres coup par la migration `move_org_to_state`
    /// et n'a jamais porte de contrainte. Elle vit dans l'etat et plus dans
    /// `account` : voir [`rows_backfilled_by_move_org_to_state`].
    #[serde(rename = "active_org_id")]
    pub active_org_id: Option<String>,
}

impl AccountStateRow {
    /// Construit une ligne d'etat.
    pub fn new(id: i64, active_account_id: Option<String>, active_org_id: Option<String>) -> Self {
        Self { id, active_account_id, active_org_id }
    }
}

/// L'etat d'etat tel qu'il reste apres la suppression d'un compte.
///
/// C'est le `ON DELETE SET NULL` de la cle etrangere, vu comme une projection
/// sur une tranche. Deux consequences, toutes deux testables :
///
/// - **la ligne d'etat survit** a la suppression du compte qu'elle designait ;
/// - **seule** `active_account_id` passe a `NULL`. `active_org_id` n'est pas
///   touche : la contrainte ne porte que sur la colonne qui reference `account`.
///
/// L'ordre des lignes restantes est preserve, comme apres un `DELETE` SQL, qui
/// ne reordonne rien.
///
/// Sans `PRAGMA foreign_keys`, SQLite ignore silencieusement cette cascade et
/// laisse un pointeur pendant : la fonction decrit donc l'intention du schema,
/// pas un comportement garanti du moteur. Le meme pragma conditionne la table
/// `session`, `event` et `project_directory` du meme depot, si bien qu'un defaut
/// d'activation affecterait toutes les tables, pas seulement celle-ci.
pub fn rows_kept_after_account_delete(rows: &[AccountStateRow], account_id: &str) -> Vec<AccountStateRow> {
    rows.iter()
        .map(|row| {
            let mut copie = row.clone();
            if copie.active_account_id.as_deref() == Some(account_id) {
                copie.active_account_id = None;
            }
            copie
        })
        .collect()
}

/// Le compte designe par l'etat, s'il existe vraiment.
///
/// C'est la resolution du pointeur `active_account_id`. Le pointeur peut etre
/// dans trois etats, et le retour n'en montre que deux :
///
/// - le pointeur est `Some` et correspond a une ligne : ce compte ;
/// - le pointeur est `NULL` : aucun compte actif ;
/// - le pointeur est `Some` et **ne correspond a aucune ligne** : `None` aussi.
///
/// Le troisieme etat est un pointeur pendant, produit par l'absence de
/// `PRAGMA foreign_keys` lors d'une suppression, ou par une insertion d'etat
/// ecrite a la main. Il n'est pas distinguable du second par le retour : qui a
/// besoin de la difference compare [`resolve_active_account`] a
/// [`AccountStateRow::active_account_id`]. Une fonction qui confondrait les
/// deux afficherait "aucun compte actif" la ou il y a en realite un pointeur
/// casse, ce qui est le genre de faute qu'on ne voit qu'a l'usage.
///
/// La ligne d'etat retenue est celle dont `id` est le plus petit : la table ne
/// contient normalement qu'une ligne, et le tri rend le choix deterministe
/// plutot que dependant de l'ordre de lecture.
pub fn resolve_active_account<'a>(
    accounts: &'a [AccountRow],
    etats: &[AccountStateRow],
) -> Option<&'a AccountRow> {
    let mut etats_ordonnes: Vec<&AccountStateRow> = etats.iter().collect();
    etats_ordonnes.sort_by_key(|etat| etat.id);
    for etat in etats_ordonnes {
        let cible = match etat.active_account_id.as_deref() {
            None => continue,
            Some(valeur) => valeur,
        };
        if let Some(trouve) = accounts.iter().find(|compte| compte.id.as_deref() == Some(cible)) {
            return Some(trouve);
        }
    }
    None
}

/// Les lignes d'etat dont la migration `move_org_to_state` a reecrit
/// l'organisation.
///
/// La migration `20260309230000_move_org_to_state` ligne 9 a 11 a execute :
///
/// ```sql
/// UPDATE `account_state` SET `active_org_id` =
///   (SELECT `selected_org_id` FROM `account`
///    WHERE `account`.`id` = `account_state`.`active_account_id`);
/// ```
///
/// puis a supprime `selected_org_id` de `account`.
///
/// L'`UPDATE` reecrit une ligne dans deux cas sur trois, et le troisieme se
/// distingue par une ligne qu'il **ne touche pas** :
///
/// - pointeur `NULL` : la sous-requete ne rend rien, l'`UPDATE` ne touche pas
///   a la ligne, et `active_org_id` garde sa valeur existante ;
/// - pointeur pendant : la sous-requete rend `NULL`, la ligne est reecrite et la
///   colonne passe a `NULL` ;
/// - pointeur qui designe un compte : la ligne est reecrite et la colonne recoit
///   l'organisation de ce compte.
///
/// C'est la difference entre une valeur **nulle** et une valeur **absente** que
/// l'ecriture condense en un `Some` et un `None` sans qu'on les voie. Cette
/// fonction retient les lignes des deux derniers cas, dans l'ordre d'entree.
///
/// Elle ne dit **pas** si le report a deja eu lieu, ni ce qu'il a ecrit. Ce
/// n'est pas un oubli : la colonne source `selected_org_id` a ete supprimee par
/// la migration, et `AccountV2.OrganizationID` n'a pas d'equivalent dans une
/// ligne `account` (elle vit sur `AccountV2.Info`, qui n'est pas une ligne SQL).
/// Inventer une colonne d'organisation pour rendre la fonction complete serait
/// exactement le piege du piege : une fabrication invisible a la compilation qui
/// ne correspond a rien en base. Ce qui reste vrai, c'est que l'information
/// n'existe plus que dans `account_state.active_org_id` - et qu'une ligne d'etat
/// dont le pointeur est pendant l'a perdue, ce que confirme
/// [`rows_kept_after_account_delete`].
pub fn rows_backfilled_by_move_org_to_state<'a>(
    etats: &'a [AccountStateRow],
    comptes: &[AccountRow],
) -> Vec<&'a AccountStateRow> {
    etats
        .iter()
        .filter(|etat| {
            let cible = match etat.active_account_id.as_deref() {
                None => return false,
                Some(valeur) => valeur,
            };
            comptes.iter().any(|compte| compte.id.as_deref() == Some(cible))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// control_account (LEGACY)
// ---------------------------------------------------------------------------

/// Une ligne de la table heritee `control_account`.
///
/// Cette table n'existe plus dans le code de l'API et la source la marque
/// `// LEGACY`. Elle est portee parce que son DDL fait partie du schema genere
/// et qu'une base migree la contient encore : lire une base exige de savoir
/// quoi lire.
///
/// La difference avec [`AccountRow`] qui compte est la cle primaire : ici elle
/// est **composite**, `(email, url)`, et les deux colonnes sont `NOT NULL`.
/// Aucun des deux drapeaux n'est donc portfolios par le type seul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlAccountRow {
    /// `text NOT NULL`, et **premiere** composante de la cle primaire.
    #[serde(rename = "email")]
    pub email: String,
    /// `text NOT NULL`, et **seconde** composante de la cle primaire.
    ///
    /// A elle seule, cette colonne n'est pas unique : deux comptes de meme
    /// adresse sur deux deploiements distincts sont deux lignes legitimes. La
    /// cle n'est ni l'une ni l'autre colonne, c'est leur couple.
    #[serde(rename = "url")]
    pub url: String,
    /// `text NOT NULL`. Marque `AccountV2.AccessToken` a la compilation.
    #[serde(rename = "access_token")]
    pub access_token: String,
    /// `text NOT NULL`. Marque `AccountV2.RefreshToken` a la compilation.
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
    /// `integer`, nullable, comme sur [`AccountRow::token_expiry`] et pour la
    /// meme raison : la source n'y ecrit pas `notNull()`.
    #[serde(rename = "token_expiry")]
    pub token_expiry: Option<i64>,
    /// `integer NOT NULL`, **stocke sous forme d'entier** par
    /// `integer({ mode: "boolean" })`.
    ///
    /// Le type est `boolean` cote TypeScript et `integer` dans la base. Le
    /// champ est donc declare `bool` ici, et [`active_as_sql_integer`] porte
    /// l'encodage, plutot que de laisser un `i64` se faire passer pour un
    /// drapeau.
    ///
    /// Le `$default(() => false)` est un crochet de l'ORM : il n'y a pas de
    /// `DEFAULT` dans le DDL, donc la colonne doit etre fournie a l'insertion.
    #[serde(rename = "active")]
    pub active: bool,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_created")]
    pub time_created: i64,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    ///
    /// Comme sur [`AccountRow::time_updated`], aucun `DEFAULT` SQL : la colonne
    /// doit etre fournie a l'insertion.
    #[serde(rename = "time_updated")]
    pub time_updated: i64,
}

impl ControlAccountRow {
    /// Construit une ligne a l'insertion, en positionnant les deux
    /// horodatages sur le meme `now`.
    pub fn new(
        email: &str,
        url: &str,
        access_token: &str,
        refresh_token: &str,
        token_expiry: Option<i64>,
        active: bool,
        now: i64,
    ) -> Self {
        Self {
            email: email.to_string(),
            url: url.to_string(),
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            token_expiry,
            active,
            time_created: now,
            time_updated: now,
        }
    }

    /// Applique une mise a jour : seul `time_updated` bouge.
    pub fn touch(&mut self, now: i64) {
        self.time_updated = now;
    }

    /// Bascule le drapeau, en marquant la ligne comme modifiee.
    ///
    /// La colonne `active` est le seul contenu semantique de cette table qui
    /// soit modifiable : tout le reste est une identite ou un jeton.
    pub fn set_active(&mut self, active: bool, now: i64) {
        self.active = active;
        self.touch(now);
    }

    /// La cle primaire composite que cette ligne occupe.
    pub fn key(&self) -> ControlAccountKey {
        ControlAccountKey::new(self.email.clone(), self.url.clone())
    }
}

/// L'encodage entier du drapeau `active`.
///
/// `0` et `1` sont les deux valeurs que ce contrat produit, et les seules
/// garanties par la colonne : `0` pour `false`, `1` pour `true`. La colonne est
/// un `integer` sans `CHECK`, donc la base en accepte d'autres ; les valeurs
/// hors de ce couple sont hors contrat et [`active_from_sql_integer`] les
/// refuse plutot que de les transformer silencieusement en `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveFlag {
    /// Le drapeau est present a `false`.
    Inactif,
    /// Le drapeau est present a `true`.
    Actif,
}

impl ActiveFlag {
    /// La valeur entiere stockee dans la colonne.
    pub fn as_sql_integer(self) -> i64 {
        match self {
            ActiveFlag::Inactif => 0,
            ActiveFlag::Actif => 1,
        }
    }
}

/// L'encodage d'un `bool` en entier de colonne.
pub fn active_as_sql_integer(active: bool) -> i64 {
    if active {
        1
    } else {
        0
    }
}

/// La lecture d'un entier de colonne en drapeau.
///
/// Seuls `0` et `1` sont acceptes. Une autre valeur rend `None` : c'est une
/// donnee hors contrat, et la faire passer pour `false` ferait croire a un
/// compte inactif la ou la base contient autre chose.
pub fn active_from_sql_integer(valeur: i64) -> Option<ActiveFlag> {
    match valeur {
        0 => Some(ActiveFlag::Inactif),
        1 => Some(ActiveFlag::Actif),
        _ => None,
    }
}

/// La cle primaire composite de `control_account` : `(email, url)`.
///
/// L'ordre des champs suit l'ordre des colonnes de la contrainte. Il n'est pas
/// cosmetique : `Ord`, utilise par [`BTreeSet`], compare d'abord `email`, puis
/// `url`, et c'est l'ordre de tri que SQLite garantit pour un `PRIMARY KEY`
/// composite.
///
/// Une cle est un **couple**. Deux lignes qui partagent l'adresse mais pas l'URL
/// sont deux cles distinctes, et c'est bien le comportement voulu : un compte
/// deploie sur plusieurs serveurs a la fois.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControlAccountKey {
    /// Colonne `email`, premiere composante de la contrainte et premier critere
    /// de tri.
    pub email: String,
    /// Colonne `url`, seconde composante.
    pub url: String,
}

impl ControlAccountKey {
    /// Construit une cle a partir de ses deux colonnes.
    pub fn new(email: String, url: String) -> Self {
        Self { email, url }
    }
}

/// Premiere cle en double dans une tranche de lignes heritees.
///
/// C'est la violation de `PRIMARY KEY(email, url)`, vue sur les donnees. Une
/// tranche valide n'en contient aucune : le retour `None` prouve que les lignes
/// sont incoherentes avec le DDL.
///
/// "Premiere" signifie premiere dans l'ordre de la tranche, ce qui rend le
/// resultat stable et dependant uniquement de l'ordre de lecture.
pub fn first_duplicate_control_account(rows: &[ControlAccountRow]) -> Option<ControlAccountKey> {
    let mut vues: BTreeSet<ControlAccountKey> = BTreeSet::new();
    for row in rows {
        if !vues.insert(row.key()) {
            return Some(row.key());
        }
    }
    None
}

/// Les lignes candidates que la cle primaire accepterait reellement.
///
/// C'est la forme pure de la contrainte declaree par ce fichier. Un candidat
/// dont le couple `(email, url)` existe deja est ecarte, quel que soit le reste
/// de son contenu : seuls les deux colonnes de la contrainte comptent.
///
/// L'ordre de sortie est l'ordre d'entree, et deux candidats identiques ne
/// laissent passer que le premier.
pub fn insertable_control_accounts<'a>(
    existing: &[ControlAccountRow],
    candidates: &'a [ControlAccountRow],
) -> Vec<&'a ControlAccountRow> {
    let mut connues: BTreeSet<ControlAccountKey> =
        existing.iter().map(ControlAccountRow::key).collect();
    let mut retenues: Vec<&ControlAccountRow> = Vec::new();
    for candidat in candidates {
        if connues.insert(candidat.key()) {
            retenues.push(candidat);
        }
    }
    retenues
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ------------------------------------------------------------

    fn compte(id: Option<&str>, email: &str, url: &str, now: i64) -> AccountRow {
        AccountRow::new(id.map(str::to_string), email, url, "acc", "ref", None, now)
    }

    fn compte_avec_expiration(id: &str, expiration: Option<i64>) -> AccountRow {
        AccountRow::new(Some(id.to_string()), "a@b.c", "https://ex", "acc", "ref", expiration, 0)
    }

    fn heritee(email: &str, url: &str, active: bool) -> ControlAccountRow {
        ControlAccountRow::new(email, url, "acc", "ref", None, active, 0)
    }

    // -- les deux drapeaux ---------------------------------------------------

    #[test]
    fn les_deux_drapneaux_sont_independants_et_leur_reunion_decide_de_null() {
        // Le tableau complet des quatre cas. C'est la table de verite du
        // modele, et elle tient en quatre lignes.
        //
        //               notNull ecrit | cle primaire | NULL admis ?
        //                       oui      |      non     |     non
        //                       oui      |      oui     |     oui
        //                       non      |      non     |     oui
        //                       non      |      oui     |     oui
        assert!(!admet_null_sans_pragma(true, false), "notNull ecrit refuse NULL");
        assert!(!admet_null_sans_pragma(true, true), "NOT NULL l emporte sur PRIMARY KEY");
        assert!(admet_null_sans_pragma(false, false), "rien n est ecrit, rien n interdit");
        assert!(admet_null_sans_pragma(false, true), "PRIMARY KEY n emporte PAS NOT NULL");
    }

    #[test]
    fn la_cle_primaire_de_account_ninterdit_pas_null() {
        // Le point central du fichier. La source ecrit `primaryKey()` sans
        // `notNull()`, le DDL dit `id text PRIMARY KEY`, et SQLite y accepte
        // `NULL` : le type est donc un `Option`.
        assert!(!ACCOUNT_NOT_NULL_DECLARES.contains(&"id"), "la source n ecrit pas notNull sur id");
        assert_eq!(ACCOUNT_PRIMARY_KEY, ["id"]);
        assert!(admet_null_sans_pragma(false, true), "et pourtant SQLite autorise NULL");
    }

    #[test]
    fn le_ddl_genere_refuse_null_la_la_ou_la_source_l_ecrit() {
        // Le converse sur `email` : la source ecrit `notNull()`, donc le moteur
        // refuse `NULL`, et c'est la seule autorite qui compte ici.
        assert!(ACCOUNT_NOT_NULL_DECLARES.contains(&"email"));
        assert!(!admet_null_sans_pragma(true, false));
    }

    #[test]
    fn les_listes_de_drapaux_ne_nominent_que_des_colonnes_reelles() {
        // Chaque liste ne cite que des colonnes de sa table. Ce test echoue si
        // quelqu'un invente un nom de colonne.
        //
        // Il ne dit rien du recouvrement des deux drapeaux : voir le test
        // suivant, car le recouvrement existe et la source l'ecrit.
        for (colonnes, not_null, primaires) in [
            (
                ACCOUNT_COLUMNS.as_slice(),
                ACCOUNT_NOT_NULL_DECLARES.as_slice(),
                ACCOUNT_PRIMARY_KEY.as_slice(),
            ),
            (
                ACCOUNT_STATE_COLUMNS.as_slice(),
                ACCOUNT_STATE_NOT_NULL_DECLARES.as_slice(),
                ACCOUNT_STATE_PRIMARY_KEY.as_slice(),
            ),
            (
                CONTROL_ACCOUNT_COLUMNS.as_slice(),
                CONTROL_ACCOUNT_NOT_NULL_DECLARES.as_slice(),
                CONTROL_ACCOUNT_PRIMARY_KEY.as_slice(),
            ),
        ] {
            for nom in not_null.iter().chain(primaires.iter()) {
                assert!(colonnes.contains(nom), "{nom} doit etre une colonne de la table");
            }
        }
    }

    #[test]
    fn le_recouvrement_des_deux_drapaux_est_exactement_la_cle_primaire_composite() {
        // La source ecrit `notNull()` sur `email` et `url`, puis pose
        // `primaryKey({ columns: [table.email, table.url] })`. Les deux colonnes
        // portent donc les deux drapeaux, et le DDL garde les deux predicats :
        //
        //   `email` text NOT NULL,
        //   `url` text NOT NULL,
        //   CONSTRAINT `control_account_pk` PRIMARY KEY(`email`, `url`)
        //
        // Inversement, `account` et `account_state` ont une cle primaire
        // simple, ecrite sans `notNull()`, et ne se recouvrent donc pas. C'est
        // la seule difference entre les deux formes de cle du fichier.
        let mut recouvrement: Vec<&str> = CONTROL_ACCOUNT_NOT_NULL_DECLARES
            .iter()
            .copied()
            .filter(|nom| CONTROL_ACCOUNT_PRIMARY_KEY.contains(nom))
            .collect();
        recouvrement.sort_unstable();
        assert_eq!(recouvrement, ["email", "url"]);

        for (not_null, primaires) in [
            (
                ACCOUNT_NOT_NULL_DECLARES.as_slice(),
                ACCOUNT_PRIMARY_KEY.as_slice(),
            ),
            (
                ACCOUNT_STATE_NOT_NULL_DECLARES.as_slice(),
                ACCOUNT_STATE_PRIMARY_KEY.as_slice(),
            ),
        ] {
            for nom in not_null {
                assert!(!primaires.contains(nom), "{nom} ne peut pas porter les deux drapeaux");
            }
        }
    }

    // -- formes de ligne et noms de colonnes --------------------------------

    #[test]
    fn la_ligne_de_compte_serialise_huit_noms_de_colonnes_du_ddl() {
        // Le contrat de la ligne est celui de la base. Une faute sur
        // `access_token` ou sur `token_expiry` passerait la compilation et
        // casserait au premier echange.
        let l = compte(Some("acc_1"), "a@b.c", "https://ex", 1_700_000_000_000);
        let json = serde_json::to_value(&l).unwrap();
        for nom in ACCOUNT_COLUMNS {
            assert!(json.get(nom).is_some(), "la colonne {nom} doit exister dans la ligne serialisee");
        }
        assert_eq!(json.as_object().unwrap().len(), 8, "la table n a que huit colonnes");
        assert_eq!(json["access_token"], "acc");
        assert_eq!(json["time_created"], 1_700_000_000_000i64);
    }

    #[test]
    fn la_ligne_ne_serialise_jamais_le_camelcase_du_json_public() {
        // Toutes les colonnes sont deja en snake_case : le piege ici est
        // l absence de piege apparente, pas une faute de casse connue. Ce test
        // verrouille le contrat quand meme.
        let json = serde_json::to_value(compte(Some("acc_1"), "a@b.c", "https://ex", 0)).unwrap();
        assert!(json.get("accessToken").is_none());
        assert!(json.get("timeCreated").is_none());
    }

    #[test]
    fn la_ligne_de_compte_ne_porte_pas_les_colonnes_du_json_public_qui_sont_absentes_du_ddl() {
        // `AccountV2.Info` expose `active_org_id`, mais pas `active_account_id`,
        // ni `token_expiry`, ni les jetons. Le JSON public et la ligne SQL n'ont
        // pas le meme contenu.
        let json = serde_json::to_value(compte(Some("acc_1"), "a@b.c", "https://ex", 0)).unwrap();
        assert!(json.get("active_org_id").is_none(), "l organisation vit dans account_state");
        assert!(json.get("active_account_id").is_none(), "le pointeur vit dans account_state");
        assert!(json.get("tokenExpiry").is_none());
    }

    #[test]
    fn la_ligne_d_etat_serialise_ses_trois_colonnes_dans_lordre_du_ddl() {
        let json = serde_json::to_value(AccountStateRow::new(1, Some("acc_1".into()), None)).unwrap();
        for nom in ACCOUNT_STATE_COLUMNS {
            assert!(json.get(nom).is_some(), "la colonne {nom} doit exister");
        }
        assert_eq!(json.as_object().unwrap().len(), 3);
        assert_eq!(json["active_account_id"], "acc_1");
        assert!(json["active_org_id"].is_null(), "l organisation inconnue reste nulle");
    }

    #[test]
    fn la_ligne_heritee_serialise_ses_huit_colonnes_dans_lordre_du_ddl() {
        let json = serde_json::to_value(heritee("a@b.c", "https://ex", true)).unwrap();
        for nom in CONTROL_ACCOUNT_COLUMNS {
            assert!(json.get(nom).is_some(), "la colonne {nom} doit exister");
        }
        assert_eq!(json.as_object().unwrap().len(), 8);
        assert_eq!(json["active"], true, "le booleen reste un booleen cote JSON");
    }

    #[test]
    fn une_ligne_relu_de_json_reprend_les_memes_valeurs() {
        // Aller-retour sur les trois tables, colonne nullable comprise.
        let l = compte_avec_expiration("acc_1", Some(1_800_000_000));
        let relue: AccountRow = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(relue, l);

        let e = AccountStateRow::new(7, Some("acc_1".into()), Some("org_1".into()));
        let relue: AccountStateRow = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(relue, e);

        let h = heritee("a@b.c", "https://ex", true);
        let relue: ControlAccountRow = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(relue, h);
    }

    #[test]
    fn un_id_nul_sur_la_ligne_porte_du_json_null_et_non_absent() {
        // `Some` et `None` doivent rester distinguables a l'echange : c'est la
        // difference entre une ligne sans identifiant et une colonne manquante.
        let l = compte(None, "a@b.c", "https://ex", 0);
        let json = serde_json::to_value(&l).unwrap();
        assert!(json.get("id").is_some(), "la colonne existe");
        assert!(json["id"].is_null(), "mais sa valeur est nulle");
        let relue: AccountRow = serde_json::from_value(json).unwrap();
        assert_eq!(relue.id, None);
    }

    // -- horodatages --------------------------------------------------------

    #[test]
    fn la_creation_pose_les_deux_horodatages_sur_le_meme_instant() {
        let l = compte(Some("acc_1"), "a@b.c", "https://ex", 42);
        assert_eq!(l.time_created, 42);
        assert_eq!(l.time_updated, 42);
    }

    #[test]
    fn une_mise_a_jour_ne_bouge_que_l_horodatage_de_maj() {
        let mut l = compte(Some("acc_1"), "a@b.c", "https://ex", 100);
        l.touch(500);
        assert_eq!(l.time_updated, 500);
        assert_eq!(l.time_created, 100, "la date de creation ne recule jamais");
        assert_eq!(l.access_token, "acc");
        assert_eq!(l.id.as_deref(), Some("acc_1"));
    }

    // -- expiration ---------------------------------------------------------

    #[test]
    fn une_expiration_absente_n_est_pas_confondue_avec_une_expiration_a_zero() {
        // Le piege de la colonne nullable : `NULL` et `0` sont deux donnees,
        // et les confondre ferait expire tous les comptes sans expiration.
        let inconnue = compte_avec_expiration("acc_1", None);
        let zero = compte_avec_expiration("acc_2", Some(0));
        assert_eq!(inconnue.expiration(), Expiration::Inconnue);
        assert_eq!(zero.expiration(), Expiration::At(0));
        assert_ne!(inconnue.expiration(), zero.expiration());
        assert_eq!(serde_json::to_value(&inconnue).unwrap()["token_expiry"].is_null(), true);
        assert_eq!(serde_json::to_value(&zero).unwrap()["token_expiry"], 0i64);
    }

    #[test]
    fn l_absence_d_expiration_ne_signifie_pas_expire() {
        // Choix de lecture, pas deduction : voir la documentation de
        // `est_perimee_a`. Le cas inverse donnerait le comportement oppose,
        // et c'est le point le plus faible du fichier.
        let l = compte_avec_expiration("acc_1", None);
        assert!(!l.est_perimee_a(0));
        assert!(!l.est_perimee_a(1_800_000_000));
    }

    #[test]
    fn une_expiration_echue_est_perimee_et_une_expiration_future_ne_l_est_pas() {
        // Le now est exprime dans la MEME unite que la colonne.
        let l = compte_avec_expiration("acc_1", Some(1_000));
        assert!(!l.est_perimee_a(999));
        assert!(l.est_perimee_a(1_000), "la borne est inclusive, comme un now()");
        assert!(l.est_perimee_a(1_001));
    }

    // -- cle primaire non garantie sur account -------------------------------

    #[test]
    fn deux_comptes_de_meme_identifiant_sont_une_collision_reelle() {
        let tranche = vec![
            compte(Some("acc_1"), "a@b.c", "https://ex", 0),
            compte(Some("acc_2"), "d@e.f", "https://ex", 0),
            compte(Some("acc_1"), "g@h.i", "https://autre", 0),
        ];
        assert_eq!(first_duplicate_by_id(&tranche), Some(Some("acc_1".to_string())));
    }

    #[test]
    fn deux_comptes_sans_identifiant_ne_sont_pas_une_collision() {
        // Le point qui vaut tout ce fichier : `id text PRIMARY KEY` sans
        // `NOT NULL` laisse passer NULL, et SQLite ne voit pas deux NULL
        // comme egaux. La cle primaire de `account` ne garantit donc rien.
        let tranche = vec![
            compte(None, "a@b.c", "https://ex", 0),
            compte(None, "d@e.f", "https://ex", 0),
            compte(None, "g@h.i", "https://ex", 0),
        ];
        assert_eq!(first_duplicate_by_id(&tranche), None);
    }

    #[test]
    fn un_id_manquant_et_un_id_present_ne_se_confondent_pas_dans_la_detection() {
        // Une ligne sans identifiant n entre pas en collision avec une ligne qui
        // en a un : les deux valeurs sont differentes, y compris pour l'index.
        let tranche = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0), compte(None, "d@e.f", "https://ex", 0)];
        assert_eq!(first_duplicate_by_id(&tranche), None);
    }

    #[test]
    fn une_tranche_vide_ou_sans_doublon_ne_signale_rien() {
        let vide: Vec<AccountRow> = Vec::new();
        assert_eq!(first_duplicate_by_id(&vide), None);
        assert_eq!(first_duplicate_by_id(&[compte(Some("acc_1"), "a@b.c", "https://ex", 0)]), None);
    }

    // -- ON DELETE SET NULL --------------------------------------------------

    #[test]
    fn la_suppression_d_un_compte_met_le_pointeur_a_null_sans_supprimer_la_ligne_d_etat() {
        // La ligne d etat survit : c est tout l interet du SET NULL.
        let tranche = vec![AccountStateRow::new(1, Some("acc_1".into()), Some("org_1".into()))];
        let restantes = rows_kept_after_account_delete(&tranche, "acc_1");
        assert_eq!(restantes.len(), 1, "la ligne d etat n est pas supprimee");
        assert_eq!(restantes[0].active_account_id, None, "seul le pointeur passe a null");
        assert_eq!(restantes[0].active_org_id.as_deref(), Some("org_1"), "l organisation n est pas touchee");
        assert_eq!(restantes[0].id, 1, "l identifiant de la ligne survit");
    }

    #[test]
    fn la_suppression_ne_touche_que_la_ligne_qui_designe_le_compte_supprime() {
        let tranche = vec![
            AccountStateRow::new(1, Some("acc_1".into()), None),
            AccountStateRow::new(2, Some("acc_2".into()), None),
        ];
        let restantes = rows_kept_after_account_delete(&tranche, "acc_1");
        assert_eq!(restantes.len(), 2, "aucune ligne n est supprimee");
        assert_eq!(restantes[0].active_account_id, None);
        assert_eq!(restantes[1].active_account_id.as_deref(), Some("acc_2"), "l autre pointeur est intact");
    }

    #[test]
    fn une_ligne_d_etat_deja_sans_pointeur_ne_change_pas() {
        // Un compte inconnu ne fait rien, et une ligne deja nulle reste nulle :
        // le SET NULL n a pas de quoi agir.
        let tranche = vec![AccountStateRow::new(1, None, None)];
        let restantes = rows_kept_after_account_delete(&tranche, "acc_1");
        assert_eq!(restantes, tranche);
        assert!(rows_kept_after_account_delete(&[], "acc_1").is_empty());
    }

    // -- resolution du compte actif -----------------------------------------

    #[test]
    fn un_pointeur_valide_resout_le_compte_designe() {
        let comptes = vec![
            compte(Some("acc_1"), "a@b.c", "https://ex", 0),
            compte(Some("acc_2"), "d@e.f", "https://ex", 0),
        ];
        let etats = vec![AccountStateRow::new(1, Some("acc_2".into()), None)];
        assert_eq!(resolve_active_account(&comptes, &etats).unwrap().id.as_deref(), Some("acc_2"));
    }

    #[test]
    fn un_pointeur_pendant_ne_resout_aucun_compte() {
        // Cas produit par l absence de PRAGMA foreign_keys, ou par une insertion
        // d etat ecrite a la main. Le retour est identique a celui d un pointeur
        // nul : c est la comparaison avec `active_account_id` qui les distingue.
        let comptes = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, Some("acc_9".into()), None)];
        assert!(resolve_active_account(&comptes, &etats).is_none());
        assert_eq!(etats[0].active_account_id.as_deref(), Some("acc_9"), "mais le pointeur, lui, est la");
    }

    #[test]
    fn un_pointeur_nul_ne_resout_aucun_compte() {
        let comptes = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, None, None)];
        assert!(resolve_active_account(&comptes, &etats).is_none());
    }

    #[test]
    fn un_compte_sans_identifiant_ne_peut_pas_etre_atteint_par_un_pointeur() {
        // Le pointeur est non nul, mais aucune ligne ne porte cet identifiant :
        // `None == Some("acc_1")` est faux, la ligne reste hors d'atteinte.
        let comptes = vec![compte(None, "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, Some("acc_1".into()), None)];
        assert!(resolve_active_account(&comptes, &etats).is_none());
    }

    #[test]
    fn la_resolution_retient_la_ligne_d_etat_de_plus_petit_identifiant() {
        // La table ne contient normalement qu une ligne ; le tri rend le choix
        // deterministe plutot que dependant de l ordre de lecture.
        let comptes = vec![
            compte(Some("acc_1"), "a@b.c", "https://ex", 0),
            compte(Some("acc_2"), "d@e.f", "https://ex", 0),
        ];
        let etats = vec![
            AccountStateRow::new(9, Some("acc_2".into()), None),
            AccountStateRow::new(2, Some("acc_1".into()), None),
        ];
        assert_eq!(resolve_active_account(&comptes, &etats).unwrap().id.as_deref(), Some("acc_1"));
    }

    #[test]
    fn la_resolution_saut_par_dessus_une_ligne_d_etat_sans_pointeur() {
        let comptes = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, None, None), AccountStateRow::new(2, Some("acc_1".into()), None)];
        assert_eq!(resolve_active_account(&comptes, &etats).unwrap().id.as_deref(), Some("acc_1"));
    }

    // -- report de move_org_to_state ---------------------------------------

    #[test]
    fn le_report_touche_la_ligne_dont_le_pointeur_designe_un_compte() {
        let comptes = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, Some("acc_1".into()), None)];
        let touchees = rows_backfilled_by_move_org_to_state(&etats, &comptes);
        assert_eq!(touchees.len(), 1);
        assert_eq!(touchees[0].id, 1);
    }

    #[test]
    fn le_report_ne_touche_pas_une_ligne_d_etat_sans_pointeur() {
        // La sous-requete ne rend rien, donc l UPDATE n ecrit rien : la valeur
        // existante de la colonne est conservee, meme si elle est nulle. C est
        // la difference entre une valeur conservee et une valeur ecrite.
        let comptes = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, None, Some("org_garde".into()))];
        assert!(rows_backfilled_by_move_org_to_state(&etats, &comptes).is_empty());
        assert_eq!(etats[0].active_org_id.as_deref(), Some("org_garde"), "l organisation reste en place");
    }

    #[test]
    fn le_report_ignore_un_pointeur_pendant() {
        // La sous-requete rendrait NULL, donc la ligne serait reecrite a vide.
        // La fonction ne pretend pas ecrire : elle ne dit que ce que l etat
        // courant permet d'affirmer, et la valeur source a disparu du schema.
        let comptes: Vec<AccountRow> = Vec::new();
        let etats = vec![AccountStateRow::new(1, Some("acc_9".into()), Some("org_avant".into()))];
        assert!(rows_backfilled_by_move_org_to_state(&etats, &comptes).is_empty());
        assert_eq!(etats[0].active_org_id.as_deref(), Some("org_avant"), "la fonction ne modifie rien");
    }

    #[test]
    fn le_report_ignore_un_compte_sans_identifiant() {
        // Une ligne de compte sans `id` ne peut pas etre atteinte par un pointeur,
        // donc elle ne fait pas sortir sa ligne d etat de la selection.
        let comptes = vec![compte(None, "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, Some("acc_1".into()), None)];
        assert!(rows_backfilled_by_move_org_to_state(&etats, &comptes).is_empty());
    }

    #[test]
    fn le_report_conserve_l_ordre_des_lignes_touchees() {
        let comptes = vec![
            compte(Some("acc_1"), "a@b.c", "https://ex", 0),
            compte(Some("acc_2"), "d@e.f", "https://ex", 0),
        ];
        let etats = vec![
            AccountStateRow::new(1, None, None),
            AccountStateRow::new(2, Some("acc_2".into()), None),
            AccountStateRow::new(3, Some("acc_1".into()), None),
        ];
        let touchees = rows_backfilled_by_move_org_to_state(&etats, &comptes);
        assert_eq!(touchees.len(), 2);
        assert_eq!(touchees[0].id, 2, "l ordre d entree est conserve, comme un UPDATE");
        assert_eq!(touchees[1].id, 3);
    }

    #[test]
    fn le_report_ne_touche_rien_dans_une_tranche_vide() {
        let comptes = vec![compte(Some("acc_1"), "a@b.c", "https://ex", 0)];
        let vide: Vec<AccountStateRow> = Vec::new();
        assert!(rows_backfilled_by_move_org_to_state(&vide, &comptes).is_empty());
    }

    // -- drapeau active -----------------------------------------------------

    #[test]
    fn le_drapeau_active_s_encode_en_zero_ou_un() {
        assert_eq!(active_as_sql_integer(false), 0);
        assert_eq!(active_as_sql_integer(true), 1);
        assert_eq!(ActiveFlag::Inactif.as_sql_integer(), 0);
        assert_eq!(ActiveFlag::Actif.as_sql_integer(), 1);
    }

    #[test]
    fn le_drapeau_active_ne_se_lit_que_sur_zero_et_un() {
        assert_eq!(active_from_sql_integer(0), Some(ActiveFlag::Inactif));
        assert_eq!(active_from_sql_integer(1), Some(ActiveFlag::Actif));
        // Hors contrat : la colonne est un integer sans CHECK, la base peut
        // contenir autre chose, et le refus vaut mieux qu un faux silencieux.
        assert_eq!(active_from_sql_integer(2), None);
        assert_eq!(active_from_sql_integer(-1), None);
    }

    #[test]
    fn le_drapeau_active_s_encode_et_se_relit_sans_perte() {
        for valeur in [true, false] {
            let encode = active_as_sql_integer(valeur);
            let relu = active_from_sql_integer(encode);
            assert_eq!(relu, Some(if valeur { ActiveFlag::Actif } else { ActiveFlag::Inactif }));
        }
    }

    #[test]
    fn basculer_le_drapeau_marque_la_ligne_comme_modifiee() {
        let mut l = heritee("a@b.c", "https://ex", false);
        l.time_updated = 100;
        l.set_active(true, 500);
        assert!(l.active);
        assert_eq!(l.time_updated, 500, "l ecriture passe par touch");
        assert_eq!(l.time_created, 0, "la date de creation ne bouge pas");
    }

    // -- cle primaire composite de control_account -------------------------

    #[test]
    fn la_cle_primaire_heritee_est_bien_le_couple_adresse_et_url() {
        // Si la contrainte ne portait que `email`, ce cas serait signale a tort.
        let tranche = vec![
            heritee("a@b.c", "https://un", false),
            heritee("a@b.c", "https://deux", false),
        ];
        assert_eq!(first_duplicate_control_account(&tranche), None);
        assert_eq!(insertable_control_accounts(&[], &tranche).len(), 2);
    }

    #[test]
    fn deux_heritees_de_meme_couple_sont_une_collision() {
        let tranche = vec![
            heritee("a@b.c", "https://un", false),
            heritee("d@e.f", "https://un", true),
            heritee("a@b.c", "https://un", true),
        ];
        let attendue = ControlAccountKey::new("a@b.c".to_string(), "https://un".to_string());
        assert_eq!(first_duplicate_control_account(&tranche), Some(attendue.clone()));
        assert_eq!(tranche[2].key(), attendue);
    }

    #[test]
    fn la_detection_de_doublon_ignore_le_contenu_hors_cle() {
        // Les jetons, l expiration et le drapeau ne font pas partie de la
        // contrainte : deux lignes de meme couple se confondent meme si tout le
        // reste differe.
        let tranche = vec![
            heritee("a@b.c", "https://un", false),
            heritee("a@b.c", "https://un", true),
        ];
        assert!(first_duplicate_control_account(&tranche).is_some());
        assert_eq!(insertable_control_accounts(&[], &tranche).len(), 1);
        assert!(!insertable_control_accounts(&[], &tranche)[0].active, "seule la premiere passe");
    }

    #[test]
    fn une_heritee_deja_en_base_nest_pas_reinseree() {
        let existant = vec![heritee("a@b.c", "https://un", false)];
        let candidats = vec![
            heritee("a@b.c", "https://un", true),
            heritee("a@b.c", "https://deux", true),
        ];
        let retenues = insertable_control_accounts(&existant, &candidats);
        assert_eq!(retenues.len(), 1);
        assert_eq!(retenues[0].url, "https://deux", "seule l URL inconnue passe, et l ordre d entree est conserve");
    }

    #[test]
    fn deux_candidats_identiques_ne_laissent_passer_que_le_premier() {
        let candidats = vec![heritee("a@b.c", "https://un", false), heritee("a@b.c", "https://un", true)];
        let retenues = insertable_control_accounts(&[], &candidats);
        assert_eq!(retenues.len(), 1);
        assert!(!retenues[0].active);
    }

    #[test]
    fn une_tranche_vide_na_pas_de_doublon_et_n_accepte_rien_de_nouveau() {
        let vide: Vec<ControlAccountRow> = Vec::new();
        assert!(first_duplicate_control_account(&vide).is_none());
        assert!(insertable_control_accounts(&vide, &vide).is_empty());
    }

    #[test]
    fn la_cle_heritee_se_trie_dabord_par_adresse_puis_par_url() {
        // L ordre de tri suit l ordre des colonnes de la contrainte, il doit
        // rester previsible pour le jour ou la contrainte sera parcouree.
        let mut cles = vec![
            ControlAccountKey::new("b@c.d".to_string(), "https://a".to_string()),
            ControlAccountKey::new("a@b.c".to_string(), "https://z".to_string()),
            ControlAccountKey::new("a@b.c".to_string(), "https://a".to_string()),
        ];
        cles.sort();
        let attendu = vec![
            ControlAccountKey::new("a@b.c".to_string(), "https://a".to_string()),
            ControlAccountKey::new("a@b.c".to_string(), "https://z".to_string()),
            ControlAccountKey::new("b@c.d".to_string(), "https://a".to_string()),
        ];
        assert_eq!(cles, attendu);
    }

    // -- migration et derivation --------------------------------------------

    #[test]
    fn une_adresse_vide_compte_comme_une_adresse_distincte() {
        // Les colonnes de la contrainte sont des TEXT sans contrainte de
        // contenu : la chaine vide est une valeur comme une autre.
        let tranche = vec![heritee("", "https://un", false), heritee("a@b.c", "https://un", false)];
        assert!(first_duplicate_control_account(&tranche).is_none());
        assert_eq!(insertable_control_accounts(&[], &tranche).len(), 2);
        assert_eq!(serde_json::to_value(&tranche[0]).unwrap()["email"], "");
    }

    #[test]
    fn la_derive_de_nullite_de_account_state_est_signalee_telle_quelle() {
        // Le DDL genere ecrit `id integer PRIMARY KEY` sans NOT NULL, la
        // migration blue_harpoon a cree la colonne avec NOT NULL. Les deux
        // echappements sont faux selon la base ; le type ne peut pas suivre les
        // deux, il suit l identifiant d'etat et le dit.
        assert!(!ACCOUNT_STATE_NOT_NULL_DECLARES.contains(&"id"), "le DDL genere n ecrit pas NOT NULL");
        assert_eq!(ACCOUNT_STATE_PRIMARY_KEY, ["id"]);
        assert!(admet_null_sans_pragma(false, true), "et donc une base neuve l'accepterait a NULL");
        // Le type retenu reste non optionnel : une ligne d etat sans identifiant
        // n'a pas de sens metier.
        let etat = AccountStateRow::new(0, None, None);
        assert_eq!(etat.id, 0);
    }

    #[test]
    fn les_noms_du_ddl_sont_bien_ceux_du_fichier_genere() {
        // Ces chaines ne viennent pas de la lecture du code declaratif mais du
        // DDL genere. Si elles sont justes, le branchement SQL futur n aura pas
        // a les deviner.
        assert_eq!(ACCOUNT_TABLE_NAME, "account");
        assert_eq!(ACCOUNT_STATE_TABLE_NAME, "account_state");
        assert_eq!(CONTROL_ACCOUNT_TABLE_NAME, "control_account");
        assert_eq!(ACTIVE_ACCOUNT_FOREIGN_KEY_NAME, "fk_account_state_active_account_id_account_id_fk");
        assert_eq!(CONTROL_ACCOUNT_PRIMARY_KEY_NAME, "control_account_pk");
    }

    #[test]
    fn les_colonnes_sont_dans_lordre_du_create_table() {
        // L ordre du DDL est l ordre des colonnes du INSERT a venir. Le test
        // echoue des qu une colonne est ajoutee, renommee ou deplacee, ce qui est
        // exactement le moment ou la table doit etre reconsideree.
        assert_eq!(
            ACCOUNT_COLUMNS,
            ["id", "email", "url", "access_token", "refresh_token", "token_expiry", "time_created", "time_updated"]
        );
        assert_eq!(ACCOUNT_STATE_COLUMNS, ["id", "active_account_id", "active_org_id"]);
        assert_eq!(
            CONTROL_ACCOUNT_COLUMNS,
            ["email", "url", "access_token", "refresh_token", "token_expiry", "active", "time_created", "time_updated"]
        );
    }

    #[test]
    fn la_ligne_expose_exactement_les_colonnes_declarees() {
        // Le contrat de la ligne et celui de la table doivent rester le meme
        // ensemble de noms, pour les trois tables. Sans ce test, un champ ajoute
        // au struct et oublie dans le tableau passerait la compilation.
        // Les trois tableaux n'ont pas la meme longueur : les passer en tranches
        // les rend homogenes, sinon le litteral de tableau n'a pas de type.
        let paires: [(serde_json::Value, &[&str]); 3] = [
            (serde_json::to_value(compte(Some("acc_1"), "a@b.c", "https://ex", 0)).unwrap(), &ACCOUNT_COLUMNS),
            (serde_json::to_value(AccountStateRow::new(1, None, None)).unwrap(), &ACCOUNT_STATE_COLUMNS),
            (serde_json::to_value(heritee("a@b.c", "https://ex", false)).unwrap(), &CONTROL_ACCOUNT_COLUMNS),
        ];
        for (ligne, colonnes) in paires {
            let objet = ligne.as_object().unwrap();
            for nom in colonnes {
                assert!(objet.contains_key(*nom), "la colonne {nom} doit exister dans la ligne");
            }
            assert_eq!(objet.len(), colonnes.len(), "aucune colonne en trop ni en trop peu");
        }
    }

    #[test]
    fn la_ligne_heritee_peut_avoir_des_jetons_vides() {
        // Les colonnes sont NOT NULL mais sans contrainte de contenu : une
        // chaine vide traverse le contrat de la ligne et se relit a l identique.
        let l = ControlAccountRow::new("a@b.c", "", "", "", None, false, 0);
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains(r#""url":"""#), "l url vide doit se relire : {json}");
        assert_eq!(serde_json::from_str::<ControlAccountRow>(&json).unwrap(), l);
    }

    #[test]
    fn aucune_lecture_de_pointeur_ne_depend_de_la_veracite_de_la_chaine() {
        // Le piege `?` contre `??` ne se pose pas ici : toutes les lectures de
        // `active_account_id` et `active_org_id` passent par une nullite. Un
        // pointeur vide ne doit donc pas etre traite comme un pointeur absent,
        // et il ne doit pas non plus resoudre un compte.
        let comptes = vec![compte(Some(""), "a@b.c", "https://ex", 0)];
        let etats = vec![AccountStateRow::new(1, Some(String::new()), None)];
        assert_eq!(
            resolve_active_account(&comptes, &etats).unwrap().id.as_deref(),
            Some(""),
            "une chaine vide est un identifiant, pas une absence"
        );
        // Et la suppression d un compte reellement nomme ne touche pas cette
        // ligne, la comparaison portant sur la valeur et non sur sa veracite.
        assert_eq!(rows_kept_after_account_delete(&etats, "acc_1")[0].active_account_id, Some(String::new()));
    }
}
