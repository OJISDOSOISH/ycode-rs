//! Portage Rust de `opencode/packages/core/src/control-plane/workspace.sql.ts`.
//!
//! ## Ce que porte reellement la source
//!
//! Le fichier d'origine fait 20 lignes et ne contient **aucune fonction** : un
//! seul appel a `sqliteTable`, pour la table `workspace`, avec huit colonnes.
//! Il n'y a ni lecture, ni ecriture, ni requete, ni migration. Ce qui y decrit
//! est un **contrat de stockage**, et sa seule traduction honnete sans
//! dependance SQL est la forme de ces huit lignes, plus les contraintes
//! qu'elles portent, exprimees sous forme de fonctions pures.
//!
//! ## Pourquoi il n'y a pas de SQL ici
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : il n'existe
//! aucun moyen d'ecrire une couche de requete, et ajouter une dependance n'est
//! pas du ressort d'un portage de fichier. Ce fichier prepare donc le contrat
//! que cette couche viendra consommer, et rien de plus. Il n'ecrit aucune
//! migration et n'en doit jamais ecrire une.
//!
//! ## La forme exacte n'est pas une devinette : elle est deja ecrite
//!
//! `packages/core/src/database/schema.gen.ts` contient le DDL que Drizzle
//! genere pour cette table. Ce sont ces chaines qui font foi, parce que ce sont
//! elles que SQLite executera un jour :
//!
//! ```sql
//! CREATE TABLE `workspace` (
//!   `id` text PRIMARY KEY,
//!   `type` text NOT NULL,
//!   `name` text DEFAULT '' NOT NULL,
//!   `branch` text,
//!   `directory` text,
//!   `extra` text,
//!   `project_id` text NOT NULL,
//!   `time_used` integer NOT NULL,
//!   CONSTRAINT `fk_workspace_project_id_project_id_fk` FOREIGN KEY
//!     (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
//! );
//! ```
//!
//! Un test compare ces chaines, ligne a ligne. Le vocabulaire SQL (`text`,
//! `integer`) et le nom de la table referencee viennent de
//! `crate::swarm::project_sql`, ou ils sont deja declares : ils ne sont pas
//! redonnes ici. En revanche les noms des colonnes de **cette** table sont
//! declares ici, parce que ce sont eux le contrat du fichier.
//!
//! ## Le point central : `notNull` et `primaryKey` sont deux drapeaux
//!
//! `id` porte `.primaryKey()` et **rien d'autre** : le DDL dit
//! `id text PRIMARY KEY`, sans `NOT NULL`. C'est exactement le meme piege que
//! sur la table `project`, et il est ici plus tentant encore, parce que
//! `WorkspaceV2.ID` a l'air d'etre un type-value : il ne l'est pas. Le drapeau
//! `not_null` de `id` est donc **faux**, et `ColumnFlags` (repris de
//! `crate::swarm::project_sql`) garde les deux drapeaux separes plutot que de
//! les fusionner. Les deux tests qui les verifient sont ici aussi.
//!
//! ## Ce que la source ecrit, ce que la base autorise
//!
//! - `id` : la source y ecrit toujours une chaine `WorkspaceV2.ID`, marque qui
//!   n'existe qu'a la compilation ; le type reste donc non nullable, d'ou
//!   [`WorkspaceId`]. Mais le DDL ne dit pas `NOT NULL`, et SQLite autorise la
//!   valeur nulle dans une colonne `TEXT PRIMARY KEY` d'une table a `rowid`,
//!   exception historique levee uniquement par `WITHOUT ROWID` ou par un
//!   `INTEGER PRIMARY KEY`. La base autorise donc davantage que ce que la
//!   source ecrit, et porter cette impossibilite serait inventer une
//!   contrainte.
//! - `type` : la source ecrit le nom d'un adaptateur. Un seul est livre en
//!   dur par le depot (`worktree`, voir `control-plane/adapters/index.ts`), les
//!   autres viennent de `registerAdapter`, c'est-a-dire des greffons. La colonne
//!   n'a donc **aucun** `enum` : un `enum` refuserait des lignes que la base
//!   accepte vraiment. [`is_builtin_workspace_type`] **signale** la divergence
//!   au lieu de la **refuser**, comme le fait `is_known_directory_type` pour
//!   `project_directory`.
//! - `extra` : `text({ mode: "json" })` **sans** `$type`. C'est le contraste
//!   avec `project.commands`, ou le `$type<{ start?: string }>()` est porteur
//!   parce que le codec le verifie. Ici le contrat de l'application est
//!   `Schema.Unknown` : n'importe quelle valeur JSON est valide, donc
//!   [`decode_extra`] ne verifie **aucune** forme et ne refuse qu'une erreur de
//!   syntaxe. Un `struct` ici serait une contrainte inventee.
//! - `name` : `text DEFAULT '' NOT NULL`. La source ecrit pourtant
//!   `name: config.name ?? null` dans `create` (voir
//!   `opencode/src/control-plane/workspace.ts`), donc un `NULL` dans une
//!   colonne que la base refuse. L'absence y est donc orthographee `''`, jamais
//!   `NULL` : c'est le sens de [`name_or_default`], et un test le fixe.
//!
//! ## Le piege `?` contre `??`, sur la meme colonne
//!
//! Les deux operateurs de la source ne sont pas le meme test, et ils ne
//! s'appliquent pas au meme endroit. Sur `branch`, la preuve est nette :
//!
//! - `branch: config.branch ?? null` (`workspace.ts`, `create`) est un
//!   **coalescent de nullite** : `""` est ecrit tel quel, en `Some("")`.
//! - `...(config.branch ? { branch: config.branch } : {})` (adaptateur
//!   `worktree.ts` ligne 54) est un **test de veracite** : `""` fait tomber la
//!   branche du lot, la cle n'est meme pas presente.
//!
//! Un `Option<&str>` unique qui "traiterait la chaine vide comme absente"
//! casserait le premier cas, et l'inverse casserait le second. D'ou deux
//!fonctions distinctes, [`branch_de_colonne`] et [`branch_transmis`], et un
//! test qui les oppose sur les memes entrees. Elles ne different que sur la
//! chaine vide, et c'est exactement ce que le test verifie.
//!
//! Un troisieme cas, sur `directory` : `space.directory ?? ""`
//! (`workspace.ts` ligne 675) est encore un coalescent de nullite, et donne
//! `""` a une colonne absente sans jamais toucher a une chaine vide
//! presente. D'ou [`WorkspaceRow::directory_ou_vide`].
//!
//! ## Piege voisin : le nom de colonne n'est pas le nom du contrat
//!
//! `fromRow` (`workspace.ts` lignes 49-60) rebatit la ligne en camelCase :
//! `row.project_id` devient `projectID`, `row.time_used` devient `timeUsed`.
//! L'erreur serait invisible a la compilation et invisible aux tests de logique ;
//! elle ne se verrait qu'a l'echange avec le TypeScript. D'ou les `rename`
//! explicites de [`WorkspaceRow`] et un test qui compare le jeu de cles JSON
//! de la ligne a celui des colonnes du DDL, puis refute les cles camelCase.
//!
//! ## Les quatre etapes de la table, et pourquoi le DDL n'est pas celui de la
//! premiere migration
//!
//! La table a ete creee puis remaniee quatre fois, et le DDL actuel ne
//! correspond qu'a la derniere :
//!
//! 1. `20260225215848_workspace` : `id`, `branch`, `project_id`,
//!    `config text NOT NULL` ;
//! 2. `20260303231226_add_workspace_fields` : ajout de `type NOT NULL`, de
//!    `name` **nullable**, de `directory`, de `extra`, suppression de
//!    `config` ;
//! 3. `20260410174513_workspace-name` : reconstruction de la table pour passer
//!    `name` a `DEFAULT '' NOT NULL`, avec un `INSERT ... SELECT` qui remplace
//!    le nom absent par `''` ;
//! 4. `20260507164347_add_workspace_time` : `ALTER TABLE ... ADD time_used
//!    integer NOT NULL DEFAULT 0`.
//!
//! Consequence a ne pas perdre de vue : cette derniere migration a bien pose
//! un `DEFAULT 0` **sur les bases existantes**, mais le DDL regenere n'en
//! porte aucun. Comme pour `time_created` et `time_updated` de `project`, le
//! `$default(() => Date.now())` de la source est un crochet **cote ORM**, pas
//! un `DEFAULT` SQL : `time_used` doit donc etre fourni a l'insertion, faute de
//! quoi la base refuse la ligne. C'est le sens de
//! [`WorkspaceRow::nouvelle`], qui recoit l'instant en parametre pour rester
//! pur, et d'un test qui compte les `DEFAULT` du DDL.
//!
//! ## Ce qui n'est pas porte
//!
//! - `WorkspaceAdapterRuntime`, `WorktreeAdapter` et `registerAdapter` : ce sont
//!   des fichiers de leur portage. Le seul comportement d'adaptateur repris ici
//!   est le test de veracite de `branch`, cite parce qu'il s'applique
//!   directement a une colonne de cette table.
//! - `WorkspaceV2.ID` lui-meme, et son regle `startsWith("wrk")` : c'est le
//!   ressort de `crate::swarm::core_workspace`, qui en fait un type qui
//!   **valide**. Ce fichier ne l'importe pas et ne le redefinit pas : la
//!   colonne porte une marque qui n'existe qu'a la compilation, donc la ligne
//!   doit pouvoir relire ce que la base a reellement stocke. Si un appelant
//!   veut la forme validee, il construit un
//!   `crate::swarm::core_workspace::WorkspaceId` puis en tire la chaine.
//! - `Slug.create()` et le reste de `create`, qui ecrivent la table.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::swarm::project_sql::{ColumnFlags, PROJECT_TABLE, SQL_TYPE_INTEGER, SQL_TYPE_TEXT};

/// Reprise de l'identifiant de projet deja declare par `session/schema.rs`.
///
/// Le contrat `$type<ProjectV2.ID>()` de la source est une **marque de
/// compilation** : a l'execution c'est une chaine. Le type est donc repris tel
/// quel, non redeclare ici, comme dans `crate::swarm::project_sql`.
pub use crate::core::session::schema::ProjectId;

/// Reprise de l'identifiant d'espace de travail deja declare par
/// `session/schema.rs`.
///
/// Memoire dans le sens inverse : `crate::swarm::core_workspace::WorkspaceId`
/// est un type qui **valide** le prefixe `wrk` a la deserialisation, et il n'est
/// volontairement pas employe ici. Une ligne relue de la base ne doit pas
/// pouvoir echouer sur une question de prefixe, et la colonne `id` ne
/// l'impose pas : le DDL dit `id text PRIMARY KEY`, sans `NOT NULL`.
pub use crate::core::session::schema::WorkspaceId;

// ---------------------------------------------------------------------------
// Noms de table et de contrainte
// ---------------------------------------------------------------------------

/// Nom de la table des espaces de travail, tel que declare dans le DDL genere.
pub const WORKSPACE_TABLE: &str = "workspace";

/// Nom de la cle etrangere de `workspace.project_id`.
///
/// Le nom n'a de sens que si le mode `PRAGMA foreign_keys` est actif. C'est la
/// seule contrainte nommee que ce fichier declare : `workspace` n'a pas de cle
/// primaire composite, donc rien d'autre a nommer ici.
pub const WORKSPACE_FOREIGN_KEY_NAME: &str = "fk_workspace_project_id_project_id_fk";

/// Le seul `DEFAULT` SQL du DDL, applique a `name`.
///
/// C'est une chaine vide, et non `NULL` : l'absence de nom se lit donc comme un
/// nom vide. La colonne etant par ailleurs `NOT NULL`, les deux lectures ne
/// peuvent pas etre confondues.
pub const NAME_DEFAULT_SQL: &str = "''";

// ---------------------------------------------------------------------------
// Noms de colonnes
// ---------------------------------------------------------------------------

/// Cle primaire de l'espace de travail.
pub const COLUMN_ID: &str = "id";

/// Nom de l'adaptateur qui realise l'espace de travail. `type` est un mot cle
/// reserve en Rust, d'ou `r#type` comme nom de champ et cette constante pour le
/// nom de colonne.
pub const COLUMN_TYPE: &str = "type";

/// Nom d'affichage, chaine vide par defaut.
pub const COLUMN_NAME: &str = "name";

/// Branche de travail, absente tant que l'espace de travail n'en a pas.
pub const COLUMN_BRANCH: &str = "branch";

/// Repertoire local de l'espace de travail, absent pour un adaptateur distant.
pub const COLUMN_DIRECTORY: &str = "directory";

/// Configuration specifique a l'adaptateur, en JSON.
pub const COLUMN_EXTRA: &str = "extra";

/// Projet proprietaire, cle etrangere vers `project.id` en suppression
/// cascade.
pub const COLUMN_PROJECT_ID: &str = "project_id";

/// Dernier instant d'utilisation, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_USED: &str = "time_used";

// ---------------------------------------------------------------------------
// Les colonnes
// ---------------------------------------------------------------------------

/// Une colonne de `workspace`, telle que le DDL genere la decrit.
///
/// C'est le `ColumnSpec` de `crate::swarm::project_sql` etendu d'un seul
/// champ : `default_sql`. Cette table a un `DEFAULT` (`name`) et l'autre n'en a
/// aucun, donc rendre la colonne capable de porter les deux evite d'avoir une
/// table de `DEFAULT` a cote, qui pourrait deriver de celle des colonnes.
///
/// Le rendu suit l'ordre exact du fichier genere : `text DEFAULT '' NOT NULL`,
/// c'est-a-dire le `DEFAULT` **avant** le `NOT NULL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceColumn {
    /// Nom de colonne, avec la casse exacte du DDL.
    pub name: &'static str,
    /// Type SQL, `text` ou `integer` dans ce fichier.
    pub sql_type: &'static str,
    /// Les deux drapeaux de Drizzle, **independants**, repris sans modification
    /// du portage de `project/sql.ts`.
    pub flags: ColumnFlags,
    /// Rendu SQL du `DEFAULT`, `None` si la colonne n'en porte pas.
    ///
    /// Attention : `name.default_sql` est `Some("''")` et
    /// `time_used.default_sql` est `None` alors que les deux ont un `$default`
    /// ou un `default()` cote source. La distinction ORM / SQL est le sujet du
    /// module `time_used` de l'en-tete.
    pub default_sql: Option<&'static str>,
}

impl WorkspaceColumn {
    /// La ligne de colonne du `CREATE TABLE`, exactement comme le fichier
    /// genere l'ecrit.
    ///
    /// Ce n'est **pas** une instruction : c'est une description, destinee a etre
    /// comparee au DDL de reference dans les tests. Aucune migration n'est
    /// ecrite par ce fichier.
    pub fn ddl_line(&self) -> String {
        let mut ligne = format!("`{}` {}", self.name, self.sql_type);
        if let Some(default) = self.default_sql {
            ligne.push_str(" DEFAULT ");
            ligne.push_str(default);
        }
        if self.flags.primary_key {
            ligne.push_str(" PRIMARY KEY");
        }
        if self.flags.not_null {
            ligne.push_str(" NOT NULL");
        }
        ligne
    }
}

/// Les huit colonnes de `workspace`, dans l'ordre du `CREATE TABLE`.
///
/// Cet ordre n'est pas indifferent : c'est celui qu'un futur
/// `INSERT INTO workspace (...) VALUES (...)` devra suivre. Il est aussi l'ordre
/// des champs de [`WorkspaceRow`], ce qui permet de verifier d'un coup d'oeil
/// que la ligne et la table ne derivent pas l'une de l'autre.
pub const WORKSPACE_COLUMNS: [WorkspaceColumn; 8] = [
    WorkspaceColumn {
        name: COLUMN_ID,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::PRIMARY_KEY,
        default_sql: None,
    },
    WorkspaceColumn {
        name: COLUMN_TYPE,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::NOT_NULL,
        default_sql: None,
    },
    WorkspaceColumn {
        name: COLUMN_NAME,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::NOT_NULL,
        default_sql: Some(NAME_DEFAULT_SQL),
    },
    WorkspaceColumn {
        name: COLUMN_BRANCH,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::PLAIN,
        default_sql: None,
    },
    WorkspaceColumn {
        name: COLUMN_DIRECTORY,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::PLAIN,
        default_sql: None,
    },
    WorkspaceColumn {
        name: COLUMN_EXTRA,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::PLAIN,
        default_sql: None,
    },
    WorkspaceColumn {
        name: COLUMN_PROJECT_ID,
        sql_type: SQL_TYPE_TEXT,
        flags: ColumnFlags::NOT_NULL,
        default_sql: None,
    },
    WorkspaceColumn {
        name: COLUMN_TIME_USED,
        sql_type: SQL_TYPE_INTEGER,
        flags: ColumnFlags::NOT_NULL,
        default_sql: None,
    },
];

/// La contrainte de cle etrangere, telle que le fichier genere l'ecrit.
pub fn workspace_foreign_key_clause() -> String {
    format!(
        "CONSTRAINT `{}` FOREIGN KEY (`{}`) REFERENCES `{}`(`{}`) ON DELETE CASCADE",
        WORKSPACE_FOREIGN_KEY_NAME, COLUMN_PROJECT_ID, PROJECT_TABLE, COLUMN_ID
    )
}

// ---------------------------------------------------------------------------
// La ligne
// ---------------------------------------------------------------------------

/// Une ligne de la table `workspace`, telle que lue ou ecrite en base.
///
/// Les huit champs correspondent, dans l'ordre, aux huit colonnes du DDL. Les
/// `rename` ci-dessous sont identiques aux noms de champ : ils sont ecrits
/// explicitement malgre cela, parce qu'ils figent le contrat du DDL, invisible
/// de l'interieur du code Rust. Le renommage camelCase, lui, est **l'affaire
/// du lecteur** et vit dans `fromRow` : voir l'en-tete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRow {
    /// `text PRIMARY KEY`, sans `NOT NULL`. La seule colonne de la table dans ce
    /// cas.
    #[serde(rename = "id")]
    pub id: WorkspaceId,
    /// `text NOT NULL`, nom de l'adaptateur. Texte libre : les greffons
    /// enregistrent le leur, donc aucun `enum` ici. `type` est un mot cle
    /// reserve en Rust, d'ou l'identifiant brut.
    #[serde(rename = "type")]
    pub r#type: String,
    /// `text DEFAULT '' NOT NULL`, nom d'affichage. **Jamais** `None` : la
    /// colonne l'interdit, et l'absence se lit `""`.
    #[serde(rename = "name")]
    pub name: String,
    /// `text`, branche de travail. Absente tant que l'espace de travail n'en a
    /// pas. Une chaine vide est une valeur, pas une absence : voir
    /// [`branch_de_colonne`] et [`branch_transmis`].
    #[serde(rename = "branch")]
    pub branch: Option<String>,
    /// `text`, repertoire local. Absent pour un adaptateur distant.
    #[serde(rename = "directory")]
    pub directory: Option<String>,
    /// `text`, configuration de l'adaptateur, en JSON. `None` vaut `NULL` SQL,
    /// ce qui n'est pas la meme chose que le texte `"null"` : voir
    /// [`decode_extra`].
    #[serde(rename = "extra")]
    pub extra: Option<String>,
    /// `text NOT NULL`, cle etrangere vers `project.id` en suppression cascade.
    #[serde(rename = "project_id")]
    pub project_id: ProjectId,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch. Aucun `DEFAULT`
    /// SQL : la valeur doit etre fournie, faute de quoi la base refuse la
    /// ligne.
    #[serde(rename = "time_used")]
    pub time_used: i64,
}

impl WorkspaceRow {
    /// Construit une ligne a l'insertion, avec l'instant fourni.
    ///
    /// Les trois colonnes optionnelles partent a `None`, `name` a son `DEFAULT`
    /// et `time_used` a l'instant donne. C'est ce que Drizzle ecrit quand
    /// l'insertion omet `name` et `time_used` : le premier grace au
    /// `.default("")`, le second grace au `$default(() => Date.now())`. Le
    /// `now` est fourni par l'appelant et non lu dans l'horloge, pour que cette
    /// fonction reste pure.
    pub fn nouvelle(id: WorkspaceId, kind: String, project_id: ProjectId, now: i64) -> Self {
        WorkspaceRow {
            id,
            r#type: kind,
            name: name_or_default(None),
            branch: None,
            directory: None,
            extra: None,
            project_id,
            time_used: now,
        }
    }

    /// Le repertoire, ou la chaine vide s'il n'y en a pas.
    ///
    /// Traduit `space.directory ?? ""` (`workspace.ts` ligne 675). C'est un
    /// test de **nullite** : un repertoire present mais vide donne `""`, comme
    /// une absence. Le comportement de veracite n'a donc **pas** sa place ici,
    /// et c'est [`branch_transmis`] qui le porte, sur une autre colonne et avec
    /// une raison de s : la cle d'un objet a transmettre, pas une chaine a
    /// assembler.
    pub fn directory_ou_vide(&self) -> &str {
        match &self.directory {
            Some(repertoire) => repertoire.as_str(),
            None => "",
        }
    }

    /// Applique une utilisation : seul `time_used` bouge.
    ///
    /// Traduit le `UPDATE workspace SET time_used = Date.now() WHERE id = ?` de
    /// `core/session/projector.ts`. `id`, `name` et `time_used`-precedente ne
    /// bougent pas.
    pub fn touch(&mut self, now: i64) {
        self.time_used = now;
    }
}

// ---------------------------------------------------------------------------
// `name` : l'absence se lit `""`
// ---------------------------------------------------------------------------

/// Le nom a ecrire, l'absence valant la chaine vide.
///
/// La source ecrit `name: config.name ?? null` dans `create`, donc un `NULL`
/// dans une colonne `NOT NULL` : **SQLite refuse la ligne**. Ce que la base
/// autorise, et donc ce que la ligne peut contenir, c'est l'absence ecrite
/// `''`, la valeur du `DEFAULT`. La fonction est donc un coalescent de
/// **nullite** : `Some("")` donne `""`, et se distingue de `None` uniquement par
/// le fait qu'il rend deja la meme chaine.
pub fn name_or_default(name: Option<&str>) -> String {
    match name {
        Some(present) => present.to_string(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Le piege `?` contre `??`, sur la colonne `branch`
// ---------------------------------------------------------------------------

/// La branche a ecrire dans la colonne, l'absence valant `NULL`.
///
/// Traduit `branch: config.branch ?? null` (`workspace.ts`, `create`). C'est un
/// coalescent de **nullite** : `Some("")` donne `Some("")`, la chaine vide est
/// une valeur parfaitement legitimate et part en base.
///
/// A ne pas confondre avec [`branch_transmis`], qui teste la veracite.
pub fn branch_de_colonne(config_branch: Option<&str>) -> Option<String> {
    config_branch.map(str::to_string)
}

/// La branche a transmettre a l'adaptateur, la chaine vide valant absence.
///
/// Traduit `...(config.branch ? { branch: config.branch } : {})`
/// (`control-plane/adapters/worktree.ts`, ligne 54). C'est un test de
/// **veracite** : `Some("")` donne `None`, la cle `branch` n'est meme pas
/// presente dans l'objet passe a l'adaptateur, et celui-ci sees un espace de
/// travail sans branche.
///
/// Les deux fonctions ne different que sur la chaine vide, et c'est tout ce qui
/// les distingue. Les fusionner en une seule, dans un sens ou dans l'autre,
/// casserait un des deux appelants du depot.
pub fn branch_transmis(config_branch: Option<&str>) -> Option<String> {
    config_branch.filter(|branche| !branche.is_empty()).map(str::to_string)
}

// ---------------------------------------------------------------------------
// Colonne `extra`
// ---------------------------------------------------------------------------

/// Erreur de codec de la colonne `extra` : le texte n'est pas du JSON valide.
///
/// Le message est conserve en `String` plutot qu'en `serde_json::Error` : cela
/// rend l'erreur comparable et cloneable, donc utilisable dans une assertion,
/// ce que la source ne peut pas etre.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("extra n'est pas du JSON valide : {0}")]
pub struct ExtraError(String);

/// Encode une valeur JSON pour la colonne `extra`.
///
/// La colonne est en `mode: "json"`, donc l'ORM y applique `JSON.stringify` et
/// y attend un **texte**. Une valeur `null` produit le texte `"null"`, qui
/// n'est pas `NULL` SQL : les deux sont distincts jusqu'a la lecture.
pub fn encode_extra(value: &serde_json::Value) -> Result<String, ExtraError> {
    serde_json::to_string(value).map_err(|e| ExtraError(e.to_string()))
}

/// Relit le texte JSON de la colonne `extra`.
///
/// Aucun controle de forme : la source ne porte ici **aucun** `$type`, et le
/// contrat de l'application est `Schema.Unknown`. Un objet, un tableau, une
/// chaine, un nombre ou `null` sont donc tous acceptes ; seul un texte qui
/// n'est pas du JSON est refuse. Imposer une forme ici serait inventer une
/// contrainte que ni le DDL ni l'ORM ne portent.
pub fn decode_extra(raw: &str) -> Result<serde_json::Value, ExtraError> {
    serde_json::from_str(raw).map_err(|e| ExtraError(e.to_string()))
}

// ---------------------------------------------------------------------------
// `type` : ce que la source ecrit, ce que la base autorise
// ---------------------------------------------------------------------------

/// Les types d'espace de travail que le depot livre en dur.
///
/// Liste de reference, **pas** une contrainte. `registerAdapter` permet a un
/// greffon d'enregistrer le type de son choix pour un projet donne, et c'est
/// ce mecanisme qui alimente `syncList`. Rien dans le DDL ne les connait.
pub const BUILTIN_WORKSPACE_TYPES: [&str; 1] = ["worktree"];

/// Le `type` lu est-il l'un des types livres en dur ?
///
/// Rend `false` sans erreur : c'est un **signalement**, pas un refus. Un
/// `false` sur une ligne lue de la base signifie que la base contient un type
/// que le depot ne connait pas lui-meme, ce qui est normal des qu'un greffon
/// en enregistre un.
pub fn is_builtin_workspace_type(value: &str) -> bool {
    BUILTIN_WORKSPACE_TYPES.contains(&value)
}

// ---------------------------------------------------------------------------
// Contraintures, sous forme de fonctions pures
// ---------------------------------------------------------------------------

/// Premiere cle en double dans une tranche d'espaces de travail.
///
/// C'est la violation de la cle primaire, vue sur les donnees. Une tranche
/// valide n'en contient aucune : le retour `None` prouve que les lignes sont
/// coherentes avec le DDL. "Premiere" signifie premiere dans l'ordre de la
/// tranche, ce qui rend le resultat stable.
pub fn first_duplicate_id(rows: &[WorkspaceRow]) -> Option<&WorkspaceId> {
    let mut vues: BTreeSet<&WorkspaceId> = BTreeSet::new();
    for row in rows {
        if !vues.insert(&row.id) {
            return Some(&row.id);
        }
    }
    None
}

/// Les espaces de travail qui subsistent apres la suppression d'un projet.
///
/// C'est la cascade `ON DELETE CASCADE` de la cle etrangere, vue comme une
/// projection sur une tranche : disparait tout ce qui appartient au projet
/// donne, rien d'autre. L'ordre des lignes restantes est preserve, comme apres
/// un `DELETE` SQL, qui ne reordonne rien.
///
/// Sans `PRAGMA foreign_keys`, SQLite ignore silencieusement cette cascade :
/// la fonction decrit donc l'intention du schema, pas un comportement du
/// moteur.
pub fn workspaces_kept_after_project_delete<'a>(
    rows: &'a [WorkspaceRow],
    project_id: &str,
) -> Vec<&'a WorkspaceRow> {
    rows.iter().filter(|row| row.project_id != project_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers -----------------------------------------------------------

    fn ligne(id: &str, kind: &str, projet: &str) -> WorkspaceRow {
        WorkspaceRow::nouvelle(id.to_string(), kind.to_string(), projet.to_string(), 0)
    }

    // -- le DDL, compare au fichier genere ---------------------------------

    #[test]
    fn workspace_ddl_matches_the_generated_file() {
        // Ces huit chaines viennent de `database/schema.gen.ts`, pas de la
        // lecture du code declaratif. Si elles sont justes, le branchement SQL
        // futur n'aura pas a les deviner.
        let attendu = [
            "`id` text PRIMARY KEY",
            "`type` text NOT NULL",
            "`name` text DEFAULT '' NOT NULL",
            "`branch` text",
            "`directory` text",
            "`extra` text",
            "`project_id` text NOT NULL",
            "`time_used` integer NOT NULL",
        ];
        let obtenu: Vec<String> = WORKSPACE_COLUMNS.iter().map(WorkspaceColumn::ddl_line).collect();
        assert_eq!(obtenu, attendu);
    }

    #[test]
    fn foreign_key_clause_matches_the_generated_file() {
        assert_eq!(WORKSPACE_TABLE, "workspace");
        assert_eq!(
            workspace_foreign_key_clause(),
            "CONSTRAINT `fk_workspace_project_id_project_id_fk` \
             FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE"
        );
    }

    #[test]
    fn not_null_and_primary_key_flags_are_independent() {
        // `id` porte `.primaryKey()` et rien d'autre : son drapeau `not_null` est
        // donc FAUX. Le poser parce que la colonne ne peut pas etre nulle en
        // pratique serait ajouter une contrainte que le DDL n'a pas, et
        // changerait la ligne de `schema.gen.ts`.
        let id = WORKSPACE_COLUMNS[0];
        assert!(id.flags.primary_key);
        assert!(!id.flags.not_null);

        // L'inverse, sur `type` : `NOT NULL` sans `PRIMARY KEY`.
        let kind = WORKSPACE_COLUMNS[1];
        assert!(kind.flags.not_null);
        assert!(!kind.flags.primary_key);
    }

    #[test]
    fn only_one_column_carries_a_sql_default() {
        // `name` est la seule. Surtout pas `time_used` : la migration
        // `20260507164347` a bien pose `DEFAULT 0` sur les bases existantes, mais
        // le DDL regenere n'en porte pas, donc la valeur doit etre fournie.
        let avec_default: Vec<&str> = WORKSPACE_COLUMNS
            .iter()
            .filter(|colonne| colonne.default_sql.is_some())
            .map(|colonne| colonne.name)
            .collect();
        assert_eq!(avec_default, vec![COLUMN_NAME]);
        assert_eq!(WORKSPACE_COLUMNS[2].default_sql, Some("''"));
        assert_eq!(WORKSPACE_COLUMNS[7].default_sql, None);
        assert_eq!(WORKSPACE_COLUMNS[7].ddl_line(), "`time_used` integer NOT NULL");
    }

    // -- forme des lignes --------------------------------------------------

    #[test]
    fn row_exposes_exactly_the_eight_columns() {
        // Le contrat de la ligne et celui de la table doivent rester le meme
        // ensemble de noms. Sans ce test, un champ ajoute au struct et oublie
        // dans `WORKSPACE_COLUMNS` passerait la compilation.
        let json = serde_json::to_value(ligne("wrk_1", "worktree", "pro_1")).unwrap();
        let objet = json.as_object().unwrap();
        for colonne in WORKSPACE_COLUMNS {
            assert!(objet.contains_key(colonne.name), "la colonne {} doit exister", colonne.name);
        }
        assert_eq!(objet.len(), WORKSPACE_COLUMNS.len(), "aucune colonne en trop ni en trop peu");
    }

    #[test]
    fn row_never_serializes_the_contract_camel_case() {
        // Le piege invisible du fichier. `fromRow` (`workspace.ts` lignes 49-60)
        // rebatit `row.project_id` en `projectID` et `row.time_used` en
        // `timeUsed` : ecrire ces noms dans la ligne passerait la compilation
        // ET tous les tests de logique, et casserait a l'echec avec le
        // TypeScript.
        let json = serde_json::to_value(ligne("wrk_1", "worktree", "pro_1")).unwrap();
        for interdit in ["projectID", "timeUsed", "workspaceID", "projectId", "timeUsedAt"] {
            assert!(json.get(interdit).is_none(), "{interdit} n est pas un nom de colonne");
        }
        assert_eq!(json["project_id"], "pro_1");
        assert_eq!(json["time_used"], 0);
        assert_eq!(json["type"], "worktree", "la colonne se nomme type dans le DDL");
    }

    #[test]
    fn the_eight_columns_follow_create_table_order() {
        let noms: Vec<&str> = WORKSPACE_COLUMNS.iter().map(|c| c.name).collect();
        assert_eq!(
            noms,
            [
                "id",
                "type",
                "name",
                "branch",
                "directory",
                "extra",
                "project_id",
                "time_used",
            ]
        );
    }

    #[test]
    fn json_round_trip_preserves_the_row() {
        let mut originale = ligne("wrk_1", "worktree", "pro_1");
        originale.branch = Some("main".to_string());
        originale.directory = Some("/srv/depot".to_string());
        originale.extra = Some("{\"url\":\"https://exemple\"}".to_string());
        originale.time_used = 1_700_000_000_000;
        let json = serde_json::to_string(&originale).unwrap();
        let relue: WorkspaceRow = serde_json::from_str(&json).unwrap();
        assert_eq!(relue, originale);
    }

    #[test]
    fn a_new_row_defaults_the_name_and_keeps_the_supplied_time_used() {
        let l = ligne("wrk_1", "worktree", "pro_1");
        assert_eq!(l.name, "", "le DEFAULT est la chaine vide, pas NULL");
        assert_eq!(l.branch, None);
        assert_eq!(l.directory, None);
        assert_eq!(l.extra, None);
        assert_eq!(l.time_used, 0);

        // Aucun DEFAULT SQL sur time_used : la valeur vient de l'appelant.
        let horodate = ligne("wrk_2", "worktree", "pro_1");
        let explicit = WorkspaceRow::nouvelle("wrk_3".into(), "worktree".into(), "pro_1".into(), 42);
        assert_eq!(explicit.time_used, 42);
        assert_eq!(horodate.id, "wrk_2");
    }

    #[test]
    fn touch_moves_only_time_used() {
        // `projector.ts` : `UPDATE workspace SET time_used = Date.now() WHERE
        // id = ?`. Rien d'autre ne bouge, et surtout pas le nom.
        let mut l = ligne("wrk_1", "worktree", "pro_1");
        l.name = "mon-espace".to_string();
        l.touch(500);
        assert_eq!(l.time_used, 500);
        assert_eq!(l.name, "mon-espace");
        assert_eq!(l.id, "wrk_1");
    }

    // -- `name` : l'absence se lit `""` -------------------------------------

    #[test]
    fn a_missing_name_becomes_the_empty_string_and_never_null() {
        // La source ecrit `name: config.name ?? null` dans une colonne
        // `NOT NULL` : SQLite refuserait cette ligne. Ce que la base autorise,
        // c'est `''`, la valeur du DEFAULT.
        assert_eq!(name_or_default(None), "");
        assert_eq!(name_or_default(Some("mon-espace")), "mon-espace");
        // Coalescent de NULLITE : la chaine vide est une valeur, pas une absence.
        assert_eq!(name_or_default(Some("")), "");
    }

    #[test]
    fn row_rejects_a_null_name_when_read_back() {
        // Le type non nullable rend l'absence impossible a la construction, et
        // il la rend aussi impossible a la relecture : une ligne portant
        // `"name": null` est rejetee, ce que ferait la colonne `NOT NULL`.
        // Contraste avec les trois colonnes optionnelles, qui acceptent `null`.
        let mut objet = serde_json::to_value(ligne("wrk_1", "worktree", "pro_1")).unwrap();
        objet["name"] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<WorkspaceRow>(objet.clone()).is_err(),
            "un nom nul ne peut pas etre relu"
        );
        objet["name"] = serde_json::Value::String(String::new());
        let relue: WorkspaceRow = serde_json::from_value(objet).unwrap();
        assert_eq!(relue.name, "", "l absence se lit comme un nom vide");
    }

    // -- le piege `?` contre `??` ------------------------------------------

    #[test]
    fn an_empty_branch_string_survives_the_column_write() {
        // `branch: config.branch ?? null` teste la NULLITE : la chaine vide
        // part en base telle quelle.
        assert_eq!(branch_de_colonne(Some("main")), Some("main".to_string()));
        assert_eq!(branch_de_colonne(Some("")), Some("".to_string()));
        assert_eq!(branch_de_colonne(None), None);
    }

    #[test]
    fn an_empty_branch_string_disappears_in_the_adapter() {
        // `...(config.branch ? { branch: config.branch } : {})` teste la
        // VERACITE : la chaine vide fait tomber la cle du lot.
        assert_eq!(branch_transmis(Some("main")), Some("main".to_string()));
        assert_eq!(branch_transmis(Some("")), None);
        assert_eq!(branch_transmis(None), None);
    }

    #[test]
    fn the_two_branch_helpers_differ_only_on_the_empty_string() {
        // Le test qui verrouille le piege. Sur les trois entrees possibles, les
        // deux fonctions ne se separent qu'a `Some("")`. Un `Option<&str>` unique
        // qui "traiterait la chaine vide comme absente" casserait l'ecriture ;
        // l'inverse casserait l'adaptateur.
        for entree in [None, Some(""), Some("main")] {
            let ecrit = branch_de_colonne(entree);
            let transmis = branch_transmis(entree);
            let attendu = entree.map(str::to_string);
            if entree == Some("") {
                assert_eq!(ecrit, Some(String::new()), "l'ecriture garde la chaine vide");
                assert_eq!(transmis, None, "l'adaptateur ne voit plus de branche");
            } else {
                assert_eq!(ecrit, attendu, "les deux fonctions coincident sur {entree:?}");
                assert_eq!(transmis, attendu, "les deux fonctions coincident sur {entree:?}");
            }
        }
    }

    #[test]
    fn a_missing_directory_becomes_the_empty_string_and_leaves_a_present_empty_string_alone() {
        // `space.directory ?? ""` : encore un coalescent de nullite. La chaine
        // vide presente reste la chaine vide, elle n'est pas "corrigee".
        let mut l = ligne("wrk_1", "worktree", "pro_1");
        assert_eq!(l.directory_ou_vide(), "");
        l.directory = Some(String::new());
        assert_eq!(l.directory_ou_vide(), "");
        l.directory = Some("/srv/depot".to_string());
        assert_eq!(l.directory_ou_vide(), "/srv/depot");
    }

    // -- `extra` : du JSON sans forme imposee -------------------------------

    #[test]
    fn a_missing_extra_is_sql_null_and_not_the_json_null_string() {
        // Deux choses differentes, et les confondre ferait lire `"null"` la ou
        // la colonne est vide.
        let vide = ligne("wrk_1", "worktree", "pro_1");
        let encode = encode_extra(&serde_json::Value::Null).unwrap();
        assert_eq!(encode, "null");
        let avec_null_json = WorkspaceRow { extra: Some(encode), ..vide.clone() };
        assert_ne!(vide.extra, avec_null_json.extra);
        assert_eq!(serde_json::to_value(&avec_null_json).unwrap()["extra"], "null");
        assert_eq!(serde_json::to_value(&vide).unwrap()["extra"], serde_json::Value::Null);
    }

    #[test]
    fn an_extra_json_of_any_shape_is_accepted() {
        // La colonne est en `mode: "json"` mais **sans** `$type`, et le contrat
        // de l'application est `Schema.Unknown`. Un `struct` ici inventerait une
        // contrainte que ni le DDL ni l'ORM ne portent. La comparaison porte sur
        // la valeur, pas sur la chaine : `serde_json` sans `preserve_order`
        // trie les cles d objet (BTreeMap), la ou `JSON.stringify` garde
        // l ordre d insertion. `{"url":..,"port":..}` revient donc trie.
        for brut in [
            r#"{"url":"https://exemple","port":8080}"#,
            r#"["a","b"]"#,
            r#""chaine""#,
            "42",
            "true",
        ] {
            let relu = decode_extra(brut).unwrap_or_else(|e| panic!("{brut} aurait du etre accepte : {e}"));
            let reencode = encode_extra(&relu).unwrap();
            let valeur_origine: serde_json::Value = serde_json::from_str(brut).unwrap();
            let valeur_reencodee: serde_json::Value = serde_json::from_str(&reencode).unwrap();
            assert_eq!(valeur_reencodee, valeur_origine, "l aller-retour doit preserver la valeur");
        }
    }

    #[test]
    fn a_malformed_extra_is_rejected() {
        // Seul cas de refus : le texte n'est pas du JSON. La source ferait
        // lever un `SyntaxError` a la lecture.
        assert!(decode_extra("{oops").is_err());
        assert!(decode_extra("").is_err());
        assert!(decode_extra("'chaine'").is_err(), "les apostrophes ne sont pas du JSON");
    }

    // -- `type` : ce que la source ecrit, ce que la base autorise ------------

    #[test]
    fn an_unlisted_type_is_still_readable() {
        // `registerAdapter` laisse un greffon choisir son type, et les tests du
        // depot inserent `"local"`. Un `enum` refuserait ces lignes, alors que
        // la base les accepte : la liste est un signalement, pas un refus.
        let l = ligne("wrk_1", "local", "pro_1");
        let json = serde_json::to_value(&l).unwrap();
        let relue: WorkspaceRow = serde_json::from_value(json).unwrap();
        assert_eq!(relue.r#type, "local");
        assert!(!is_builtin_workspace_type("local"), "et la divergence est signalable");
        assert!(is_builtin_workspace_type("worktree"));
        assert_eq!(BUILTIN_WORKSPACE_TYPES, ["worktree"]);
        assert!(!is_builtin_workspace_type(""));
    }

    // -- contraintes, en donnees --------------------------------------------

    #[test]
    fn the_primary_key_rejects_duplicate_ids() {
        let tranche = vec![
            ligne("wrk_1", "worktree", "pro_1"),
            ligne("wrk_2", "worktree", "pro_1"),
            ligne("wrk_1", "worktree", "pro_2"),
        ];
        assert_eq!(first_duplicate_id(&tranche), Some(&"wrk_1".to_string()));
        assert_eq!(first_duplicate_id(&tranche[..2]), None);
        let vide: Vec<WorkspaceRow> = Vec::new();
        assert_eq!(first_duplicate_id(&vide), None);
    }

    #[test]
    fn the_cascade_removes_the_deleted_projects_workspaces() {
        // `ON DELETE CASCADE` sur `project_id`. Le meme `id` dans deux projets
        // reste deux lignes distinctes, et c'est ce qui empeche la cascade
        // d'emporter l'espace de travail d'un autre projet.
        let tranche = vec![
            ligne("wrk_1", "worktree", "pro_1"),
            ligne("wrk_2", "worktree", "pro_1"),
            ligne("wrk_1", "worktree", "pro_2"),
        ];
        let gardes = workspaces_kept_after_project_delete(&tranche, "pro_1");
        assert_eq!(gardes.len(), 1);
        assert_eq!(gardes[0].project_id, "pro_2");
        assert_eq!(gardes[0].id, "wrk_1", "l identifiant homonyme survit");
        assert_eq!(
            workspaces_kept_after_project_delete(&tranche, "pro_inexistant").len(),
            3,
            "une suppression qui ne concerne personne ne retire rien"
        );
    }
}
