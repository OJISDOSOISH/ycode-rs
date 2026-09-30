//! Portage Rust de `opencode/packages/core/src/project/sql.ts`.
//!
//! ## Ce que porte reellement la source
//!
//! Le fichier d'origine fait 35 lignes, et il ne contient **aucune fonction** :
//! deux appels a `sqliteTable`, l'un pour `project`, l'autre pour
//! `project_directory`. Il n'y a pas de requete a porter, pas de lecture, pas
//! d'ecriture, pas de migration. Ce qui y decrit est un **contrat de stockage**,
//! et sa seule traduction honnete sans dependance SQL est la forme des deux
//! lignes, plus les contraintes que ces lignes portent, exprimees sous forme de
//! fonctions pures.
//!
//! ## Pourquoi il n'y a pas de SQL ici
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : il n'existe
//! aucun moyen d'ecrire une couche de requete, et ajouter une dependance n'est
//! pas du ressort d'un portage de fichier. Le SQL sera branche plus tard, par
//! decision du projet. Ce fichier prepare donc le contrat que cette couche
//! viendra consommer (noms de table, de colonnes, de cle etrangere, ordre du
//! `CREATE TABLE`) sans pretendre executer quoi que ce soit, et sans ecrire la
//! moindre migration.
//!
//! ## La forme exacte est deja connue : ce n'est pas une invention
//!
//! `packages/core/src/database/schema.gen.ts` contient le DDL genere par
//! Drizzle. Ce sont ces chaines qui font foi, et non le code declaratif, parce
//! que ce sont elles que SQLite executera un jour :
//!
//! ```sql
//! CREATE TABLE `project` (
//!   `id` text PRIMARY KEY,
//!   `worktree` text NOT NULL,
//!   `vcs` text,
//!   `name` text,
//!   `icon_url` text,
//!   `icon_url_override` text,
//!   `icon_color` text,
//!   `time_created` integer NOT NULL,
//!   `time_updated` integer NOT NULL,
//!   `time_initialized` integer,
//!   `sandboxes` text NOT NULL,
//!   `commands` text
//! );
//! CREATE TABLE `project_directory` (
//!   `project_id` text NOT NULL,
//!   `directory` text NOT NULL,
//!   `type` text,
//!   `strategy` text,
//!   `time_created` integer NOT NULL,
//!   CONSTRAINT `project_directory_pk` PRIMARY KEY(`project_id`, `directory`),
//!   CONSTRAINT `fk_project_directory_project_id_project_id_fk`
//!     FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
//! );
//! ```
//!
//! ## Le point central : deux drapeaux independants, pas un seul
//!
//! Drizzle tient **deux** drapeaux distincts sur une colonne, que la source
//! confond d'un coup d'oeil :
//!
//! - `.notNull()`, pose colonne par colonne ;
//! - `.primaryKey()`, lui aussi pose colonne par colonne, mais qui produit un
//!   `PRIMARY KEY` **en ligne**.
//!
//! Et il existe en plus une troisieme voie, **au niveau de la table** :
//! `primaryKey({ columns: [...] })` dans le troisieme argument de `sqliteTable`,
//! qui produit un `PRIMARY KEY(...)` en constraint nommee. Les deux formes
//! ecivent le meme mot dans le DDL, mais par des mecanismes sans rapport, et
//! Drizzle les expose separement.
//!
//! Les deux tables de ce fichier utilisent une forme differente :
//!
//! - `ProjectTable.id` porte `.primaryKey()` **et rien d'autre**. Le DDL dit
//!   `id text PRIMARY KEY`, sans `NOT NULL`.
//! - `ProjectDirectoryTable.project_id` et `.directory` ne portent ni
//!   `.primaryKey()` ni aucun `primaryKey()` de tableau : le `NOT NULL` vient de
//!   `.notNull()`, et le `PRIMARY KEY` vient de la troisieme forme. Une colonne
//!   peut donc avoir le drapeau `primary_key` a `false` et se retrouver
//!   pourtant dans la cle primaire de sa table.
//!
//! [`ColumnFlags`] garde les deux drapeaux separes plutot que de les fusionner
//! en un `Constraint` unique, pour que cette distinction reste visible et ne
//! puisse pas se perdre dans un refactoring. Deux tests la verrouillent.
//!
//! ## Ce que la source ecrit, ce que la base autorise
//!
//! La separation n'est pas academique, elle a un effet mesurable.
//!
//! `project.id` : la source y ecrit toujours une chaine (`Project.ID` est une
//! marque qui n'existe qu'a la compilation), donc la colonne est typee
//! [`ProjectId`] et non `Option<ProjectId>`. Mais le DDL ne dit pas `NOT NULL`,
//! et **SQLite autorise la valeur nulle dans une colonne `TEXT PRIMARY KEY`**
//! d'une table a `rowid` : c'est une exception historique de SQLite au regard
//! de la norme SQL, preservee pour compatibilite, et elle n'est levee que par
//! `WITHOUT ROWID` ou par un `INTEGER PRIMARY KEY`. La base autorise donc
//! davantage que ce que la source ecrit. Le type reste non nullable : c'est ce
//! que la source ecrit, et porter l'impossibilite du moteur reviendrait a
//! inventer une contrainte.
//!
//! `project_directory.type` : le `$type<"main" | "root" | "git_worktree">()`
//! est une annotation **de compilation uniquement**. Le DDL autorise n'importe
//! quel texte, et la colonne reste donc `Option<String>`. On ne declare pas
//! d'`enum` : il refuserait a la lecture des lignes que la base, elle, accepte
//! vraiment, et ce serait une perte de fidelite. Les trois valeurs sont
//! listees dans [`PROJECT_DIRECTORY_TYPES`] et testees par
//! [`is_known_directory_type`], qui **signale** une divergence au lieu de la
//! **refuser**.
//!
//! Contraste volontaire : `project_directory.time_created` a bien un
//! `$default(() => Date.now())`, mais la colonne `commands` est en
//! `text({ mode: "json" })`, et la seule difference avec un `text()` nu est que
//! l'ORM y applique `JSON.stringify` / `JSON.parse`. Sur cette colonne la, le
//! `$type<{ start?: string }>()` **est** porteur, parce que le codec le
//! verifie reellement. D'ou [`ProjectCommands`], qui est un type a decodage
//! effectif, alors que l'union de `type` n'en est pas un.
//!
//! ## Le piege `?` contre l'absence de `?`, sur les chemins
//!
//! `database/path.ts` definit deux colonnes presque identiques :
//!
//! ```ts
//! export const absoluteColumn = customType({ toDriver: (i) => absolute(i), ... })
//! export const directoryColumn = customType({
//!   toDriver(input) { return input ? absolute(input) : input },
//!   fromDriver(input) { return input ? toPlatform(absolute(input)) : input },
//! })
//! ```
//!
//! `directoryColumn` teste la **veracite** (`input ? ...`), donc la chaine
//! vide traverse intacte. `absoluteColumn` n'a pas cette garde et appelle
//! `absolute` inconditionnellement, donc la chaine vide **leve** une erreur.
//!
//! Ce fichier n'utilise que `absoluteColumn` (pour `worktree` et `directory`)
//! et `absoluteArrayColumn` (pour `sandboxes`). Il n'y a donc **aucun**
//! equivalent de la garde de veracite a porter : le chemin vide est refuse.
//! C'est le genre de detail qui se perd si l'on confond les deux colonnes, d'ou
//! [`absolute`] tel qu'il est ecrit ici, sans garde, et un test qui le fixe.
//!
//! Pour le reste, le transport des chemins est fidele : a l'ecriture
//! (`toDriver`) un chemin Windows voit ses antislashs devenir des slashs, a la
//! lecture (`fromDriver`) un chemin de type Windows revient avec des
//! antislashs. Un chemin `/srv/depot` reste `/srv/depot` sur les deux
//! plateformes, car `isWindowsStoragePath` le rejette. Le materiau
//! (`StoragePlatform`) est un **parametre**, jamais `process.platform` : les
//! tests restent instantanes et sans acces disque.
//!
//! ## Ce qui n'est pas porte
//!
//! - Les lectures et ecritures de `project/directories.ts`, qui consomment ces
//!   deux tables. Ellesont un fichier de leur portage ; les recopier ici ferait
//!   diverger deux implementations du meme comportement. La cle composite
//!   qu'elles exploitent (`onConflictDoUpdate`, ligne 72) est neanmoins rendue
//!   ici sous forme pure, via [`first_duplicate_directory`] et
//!   [`insertable_directory_rows`].
//! - La construction de `Project.ID` lui-meme (`Identifier.ascending()` et
//!   le prefixe `pro_`) : c'est le ressort du portage de `schema.ts`.
//! - Les requetes de `packages/opencode/src/project/project.ts`, qui suppriment
//!   les `project_directory` d'un projet avant de le renommer. La cascade
//!   `ON DELETE CASCADE` decrite ici ne s'applique qu'a la suppression du
//!   projet lui-meme, et [`directories_kept_after_project_delete`] ne dit rien
//!   d'autre.
//!
//! ## Un choix assume sur `time_updated`
//!
//! `database/schema.sql.ts` donne a `time_created` un
//! `$default(() => Date.now())` et a `time_updated` un
//! `$onUpdate(() => Date.now())`. Les deux colonnes sont `NOT NULL` dans le DDL
//! genere, mais **aucune des deux n'a de `DEFAULT` SQL** : ce sont des crochets
//! cote ORM, appliques au moment de construire les requetes. Consequence
//! concrete : `time_updated` doit etre fournie a l'insertion, faute de quoi la
//! base refuse la ligne. On ecrit le `now` fourni dans les deux colonnes.
//!
//! `project_directory` ne declare qu'un `time_created`, avec le meme
//! `$default`, et le `NOW` n'est donc pas de son ressort. Ce qui n'est pas dans
//! son DDL n'est pas dans sa ligne.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub use crate::core::session::schema::{AbsolutePath, ProjectId};

// ---------------------------------------------------------------------------
// Noms de tables et de contraintes
// ---------------------------------------------------------------------------

/// Nom de la table des projets, tel que declare dans le DDL genere.
pub const PROJECT_TABLE: &str = "project";

/// Nom de la table des repertoires de projet, tel que declare dans le DDL genere.
pub const PROJECT_DIRECTORY_TABLE: &str = "project_directory";

/// Nom de la cle primaire composee de `project_directory`.
///
/// Cette contrainte ne peut pas s'ecrire sur une colonne : elle porte sur deux
/// colonnes a la fois. C'est la raison d'etre de la troisieme forme de cle
/// primaire de Drizzle, decrite dans l'en-tete du module.
pub const PROJECT_DIRECTORY_PRIMARY_KEY_NAME: &str = "project_directory_pk";

/// Nom de la cle etrangere de `project_directory.project_id`.
///
/// Le nom n'a de sens que si le mode `PRAGMA foreign_keys` est actif. Le
/// declencher n'est pas du ressort de ce fichier.
pub const PROJECT_DIRECTORY_FOREIGN_KEY_NAME: &str =
    "fk_project_directory_project_id_project_id_fk";

// ---------------------------------------------------------------------------
// Noms de colonnes
// ---------------------------------------------------------------------------

/// Cle primaire du projet.
pub const COLUMN_ID: &str = "id";

/// Repertoire de travail du projet.
pub const COLUMN_WORKTREE: &str = "worktree";

/// Localisation du depot git, en JSON, vide s'il n'y en a pas.
pub const COLUMN_VCS: &str = "vcs";

/// Nom d'affichage du projet.
pub const COLUMN_NAME: &str = "name";

/// URL d'icone decouverte automatiquement.
pub const COLUMN_ICON_URL: &str = "icon_url";

/// URL d'icone imposee par l'utilisateur, prioritaire sur la precedente.
pub const COLUMN_ICON_URL_OVERRIDE: &str = "icon_url_override";

/// Couleur d'icone imposee par l'utilisateur.
pub const COLUMN_ICON_COLOR: &str = "icon_color";

/// Horodatage de creation, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_CREATED: &str = "time_created";

/// Horodatage de mise a jour, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_UPDATED: &str = "time_updated";

/// Horodatage d'initialisation du projet, en millisecondes depuis l'epoch.
///
/// Nullable, et ce n'est pas un oubli : la source ne pose ni `.notNull()` ni
/// `$default` dessus, donc le DDL dit `time_initialized integer`, sans rien
/// d'autre.
pub const COLUMN_TIME_INITIALIZED: &str = "time_initialized";

/// Tableau JSON des repertoires de bac a sable, serialize en un seul texte.
pub const COLUMN_SANDBOXES: &str = "sandboxes";

/// Document JSON des commandes de demarrage du projet.
pub const COLUMN_COMMANDS: &str = "commands";

/// Projet proprietaire d'un repertoire.
pub const COLUMN_PROJECT_ID: &str = "project_id";

/// Repertoire enregistre, en chemin absolu.
pub const COLUMN_DIRECTORY: &str = "directory";

/// Nature du repertoire. `type` est un mot cle reserve en Rust, d'ou
/// `r#type` comme nom de champ et cette constante pour le nom de colonne.
pub const COLUMN_TYPE: &str = "type";

/// Strategie de copie associee au repertoire.
pub const COLUMN_STRATEGY: &str = "strategy";

/// Les douze colonnes de `project`, dans l'ordre du `CREATE TABLE`.
///
/// Cet ordre n'est pas indifferent : c'est celui qu'un futur
/// `INSERT INTO project (...) VALUES (...)` devra suivre. Il est aussi l'ordre
/// des champs de [`ProjectRow`], ce qui permet de verifier d'un coup d'oeil que
/// la ligne et la table ne derivent pas l'une de l'autre.
pub const PROJECT_COLUMNS: [ColumnSpec; 12] = [
    ColumnSpec { name: COLUMN_ID, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PRIMARY_KEY },
    ColumnSpec { name: COLUMN_WORKTREE, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::NOT_NULL },
    ColumnSpec { name: COLUMN_VCS, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_NAME, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_ICON_URL, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_ICON_URL_OVERRIDE, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_ICON_COLOR, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_TIME_CREATED, sql_type: SQL_TYPE_INTEGER, flags: ColumnFlags::NOT_NULL },
    ColumnSpec { name: COLUMN_TIME_UPDATED, sql_type: SQL_TYPE_INTEGER, flags: ColumnFlags::NOT_NULL },
    ColumnSpec { name: COLUMN_TIME_INITIALIZED, sql_type: SQL_TYPE_INTEGER, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_SANDBOXES, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::NOT_NULL },
    ColumnSpec { name: COLUMN_COMMANDS, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
];

/// Les cinq colonnes de `project_directory`, dans l'ordre du `CREATE TABLE`.
pub const PROJECT_DIRECTORY_COLUMNS: [ColumnSpec; 5] = [
    ColumnSpec { name: COLUMN_PROJECT_ID, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::NOT_NULL },
    ColumnSpec { name: COLUMN_DIRECTORY, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::NOT_NULL },
    ColumnSpec { name: COLUMN_TYPE, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_STRATEGY, sql_type: SQL_TYPE_TEXT, flags: ColumnFlags::PLAIN },
    ColumnSpec { name: COLUMN_TIME_CREATED, sql_type: SQL_TYPE_INTEGER, flags: ColumnFlags::NOT_NULL },
];

/// Les deux colonnes de la cle primaire composee, dans leur ordre de tri.
///
/// L'ordre compte : `Ord`, utilise par [`BTreeSet`] et par SQLite pour un
/// `CREATE TABLE` composite, compare `project_id` d'abord, puis `directory`.
pub const PROJECT_DIRECTORY_PRIMARY_KEY_COLUMNS: [&str; 2] = [COLUMN_PROJECT_ID, COLUMN_DIRECTORY];

/// Type SQL des colonnes de texte.
pub const SQL_TYPE_TEXT: &str = "text";

/// Type SQL des colonnes d'horodatage.
pub const SQL_TYPE_INTEGER: &str = "integer";

// ---------------------------------------------------------------------------
// Drapeaux de colonne
// ---------------------------------------------------------------------------

/// Les deux drapeaux que Drizzle tient **separement** sur une colonne.
///
/// Ils sont ici deux champs et non une enum, precisement pour que la
/// distinction de l'en-tete reste lisible dans la declaration d'une colonne.
/// Les confondre en un seul `Constraint` supprimerait exactement
/// l'information que le portage doit conserver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnFlags {
    /// Drapeau `.notNull()`, pose colonne par colonne.
    pub not_null: bool,
    /// Drapeau `.primaryKey()`, lui aussi pose colonne par colonne.
    ///
    /// Attention : il ne couvre **pas** les cles primaires composees, qui
    /// passent par le troisieme argument de `sqliteTable`. Voir
    /// [`PROJECT_DIRECTORY_PRIMARY_KEY_COLUMNS`].
    pub primary_key: bool,
}

impl ColumnFlags {
    /// Aucun des deux drapeaux, colonne nullable et hors de toute cle.
    pub const PLAIN: ColumnFlags = ColumnFlags { not_null: false, primary_key: false };

    /// `.notNull()` seul.
    pub const NOT_NULL: ColumnFlags = ColumnFlags { not_null: true, primary_key: false };

    /// `.primaryKey()` seul. C'est le cas de `project.id`.
    pub const PRIMARY_KEY: ColumnFlags = ColumnFlags { not_null: false, primary_key: true };
}

/// Une colonne, telle que le DDL genere la decrit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnSpec {
    /// Nom de colonne, avec la casse exacte du DDL.
    pub name: &'static str,
    /// Type SQL, `text` ou `integer` dans ce fichier.
    pub sql_type: &'static str,
    /// Les deux drapeaux, independants.
    pub flags: ColumnFlags,
}

impl ColumnSpec {
    /// La ligne de colonne du `CREATE TABLE`, exactement comme le fichier
    /// genere l'ecrit.
    ///
    /// Ce n'est **pas** une instruction : c'est une description, destinee a
    /// etre comparee au DDL de reference dans les tests. Aucune migration n'est
    /// ecrite par ce fichier, et aucune ne doit l'etre a partir de cette
    /// chaine.
    pub fn ddl_line(&self) -> String {
        let mut ligne = format!("`{}` {}", self.name, self.sql_type);
        if self.flags.primary_key {
            ligne.push_str(" PRIMARY KEY");
        }
        if self.flags.not_null {
            ligne.push_str(" NOT NULL");
        }
        ligne
    }
}

/// La contrainte de cle primaire composee, telle que le fichier genere l'ecrit.
///
/// Description seulement, pour la meme raison que [`ColumnSpec::ddl_line`].
pub fn project_directory_primary_key_clause() -> String {
    let colonnes: Vec<String> = PROJECT_DIRECTORY_PRIMARY_KEY_COLUMNS
        .iter()
        .map(|c| format!("`{c}`"))
        .collect();
    format!(
        "CONSTRAINT `{}` PRIMARY KEY({})",
        PROJECT_DIRECTORY_PRIMARY_KEY_NAME,
        colonnes.join(", ")
    )
}

/// La contrainte de cle etrangere, telle que le fichier genere l'ecrit.
pub fn project_directory_foreign_key_clause() -> String {
    format!(
        "CONSTRAINT `{}` FOREIGN KEY (`{}`) REFERENCES `{}`(`{}`) ON DELETE CASCADE",
        PROJECT_DIRECTORY_FOREIGN_KEY_NAME, COLUMN_PROJECT_ID, PROJECT_TABLE, COLUMN_ID
    )
}

// ---------------------------------------------------------------------------
// Chemins : portage des colonnes personnalisees de `database/path.ts`
// ---------------------------------------------------------------------------

/// Plateforme de stockage, en parametre plutot qu'en variable globale.
///
/// La source lit `process.platform`. Le rendre explicite garde ce fichier
/// testable : les tests n'ont ni horloge, ni thread, ni disque, et choisissent
/// les deux plateformes explicitement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePlatform {
    /// Comportement de `process.platform === "win32"`.
    Windows,
    /// Tout le reste.
    Unix,
}

/// Un chemin refuse parce qu'il n'est pas absolu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Le chemin fourni n'est absolu sur la plateforme demandee.
    ///
    /// Le champ contient le chemin **d'origine**, avant normalisation, comme le
    /// fait le message d'erreur de la source.
    NotAbsolute(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::NotAbsolute(input) => write!(f, "Path is not absolute: {input}"),
        }
    }
}

impl std::error::Error for PathError {}

/// Erreur d'un codec de colonne : soit un chemin, soit du JSON mal forme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnError {
    /// La validation de chemin a echoue.
    Path(PathError),
    /// Le JSON n'a pas pu etre encode ou decode.
    ///
    /// Le message est conserve en `String` plutot qu'en `serde_json::Error` :
    /// cela rend `ColumnError` comparable et cloneable, donc utilisable dans
    /// des assertions, ce que la source ne peut pas etre.
    Json(String),
}

/// Le chemin est-il de la forme d'un chemin de stockage Windows ?
///
/// Reproduit `/^[A-Za-z]:\//.test(input) || input.startsWith("//")`.
pub fn is_windows_storage_path(input: &str) -> bool {
    let octets = input.as_bytes();
    let avec_lecteur = octets.len() >= 3
        && octets[0].is_ascii_alphabetic()
        && octets[1] == b':'
        && octets[2] == b'/';
    avec_lecteur || input.starts_with("//")
}

/// Forme du chemin telle qu'ecrite en base (`toDriver`).
///
/// Sur Windows, les antislashs deviennent des slashs. ailleurs, le chemin est
/// rendu tel quel, octet pour octet.
pub fn storage_path(input: &str, platform: StoragePlatform) -> String {
    match platform {
        StoragePlatform::Windows => input.replace('\\', "/"),
        StoragePlatform::Unix => input.to_string(),
    }
}

/// Valide un chemin et le rend sous sa forme de stockage (`toDriver`).
///
/// C'est la fonction `absolute` de `database/path.ts`. Elle **n'a pas** de
/// garde de veracite : une chaine vide est rejettee comme un chemin relatif.
/// C'est le comportement de `absoluteColumn`, que ce fichier utilise, a
/// distinguer de `directoryColumn` qui garde la chaine vide.
pub fn absolute(input: &str, platform: StoragePlatform) -> Result<String, PathError> {
    let resultat = storage_path(input, platform);
    let absolu_posix = resultat.starts_with('/');
    let absolu_windows =
        platform == StoragePlatform::Windows && is_windows_storage_path(&resultat);
    if absolu_posix || absolu_windows {
        Ok(resultat)
    } else {
        Err(PathError::NotAbsolute(input.to_string()))
    }
}

/// Forme du chemin telle que relue par l'application (`fromDriver`).
///
/// Le chemin doit d'abord avoir passe par [`absolute`]. Seul un chemin de type
/// Windows est retransforme : `/srv/depot` reste `/srv/depot` sur les deux
/// plateformes, car [`is_windows_storage_path`] le rejette.
pub fn to_platform(input: &str, platform: StoragePlatform) -> String {
    if platform == StoragePlatform::Windows && is_windows_storage_path(input) {
        input.replace('/', "\\")
    } else {
        input.to_string()
    }
}

// ---------------------------------------------------------------------------
// Table `project`
// ---------------------------------------------------------------------------

/// Une ligne de la table `project`, telle que lue ou ecrite en base.
///
/// Les douze champs correspondent, dans l'ordre, aux douze colonnes du DDL
/// genere. Les `rename` ci-dessous sont identiques aux noms de champ : ils sont
/// ecrits explicitement malgre cela, parce qu'ils figent le contrat du DDL,
/// invisible de l'interieur du code Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRow {
    /// `text PRIMARY KEY`. La seule colonne sans `NOT NULL` du DDL.
    ///
    /// Le type est marque `ProjectSchema.ID` dans le TypeScript, marque qui
    /// n'existe qu'a la compilation : a l'execution c'est une chaine, d'ou
    /// [`ProjectId`]. Le type reste non nullable alors que le DDL ne l'impose
    /// pas, parce que c'est ce que la source ecrit ; SQLite, lui, autoriserait
    /// la valeur nulle dans cette colonne `TEXT PRIMARY KEY`. Voir l'en-tete.
    #[serde(rename = "id")]
    pub id: ProjectId,
    /// `text NOT NULL`, chemin absolu du repertoire de travail.
    ///
    /// Valide par `absoluteColumn` : la chaine vide y est refusee.
    #[serde(rename = "worktree")]
    pub worktree: AbsolutePath,
    /// `text`, objet `{ type, store }` serialise en JSON, absent tant que le
    /// projet n'a pas de depot git. Le contrat de ce JSON est porte par
    /// `crate::swarm::project_schema::Vcs`, volontairement non redeclare ici.
    #[serde(rename = "vcs")]
    pub vcs: Option<String>,
    /// `text`, nom d'affichage, absent tant qu'il n'est pas renseigne.
    #[serde(rename = "name")]
    pub name: Option<String>,
    /// `text`, URL d'icone decouverte, absente tant qu'aucune icone n'a ete
    /// trouvee.
    #[serde(rename = "icon_url")]
    pub icon_url: Option<String>,
    /// `text`, URL d'icone imposee. Prioritaire sur `icon_url` a la lecture.
    #[serde(rename = "icon_url_override")]
    pub icon_url_override: Option<String>,
    /// `text`, couleur d'icone imposee.
    #[serde(rename = "icon_color")]
    pub icon_color: Option<String>,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_created")]
    pub time_created: i64,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_updated")]
    pub time_updated: i64,
    /// `integer`, en millisecondes depuis l'epoch. `None` tant que le projet
    /// n'a pas ete initialise.
    #[serde(rename = "time_initialized")]
    pub time_initialized: Option<i64>,
    /// `text NOT NULL`, **tableau JSON** de chemins absolus.
    ///
    /// La colonne est `NOT NULL` : la liste vide s'ecrit `[]`, jamais `NULL`.
    /// Le champ contient la donnee pilote, c'est-a-dire le texte brut, et non
    /// le tableau. Voir [`ProjectRow::set_sandboxes`] et
    /// [`ProjectRow::read_sandboxes`].
    #[serde(rename = "sandboxes")]
    pub sandboxes: String,
    /// `text`, document JSON `{ start?: string }`, absent tant que le projet
    /// n'a pas de commande de demarrage. Voir [`decode_commands`].
    #[serde(rename = "commands")]
    pub commands: Option<String>,
}

impl ProjectRow {
    /// Construit une ligne a l'insertion.
    ///
    /// `sandboxes_json` est le texte deja encode : la validation des chemins y
    /// passe par [`ProjectRow::set_sandboxes`], qui appelleant fournit apres
    /// coup. Les cinq colonnes optionnelles partent a `None`, ce qui est bien
    /// la valeur de leur colonne, et non une absence d'information.
    ///
    /// Le `now` est fourni par l'appelant et non lu dans l'horloge, pour que
    /// cette fonction reste pure. Les deux horodatages sont poses sur le meme
    /// instant : `time_created` par le `$default`, `time_updated` par choix
    /// documente dans l'en-tete, faute de `DEFAULT` SQL.
    pub fn new(
        id: ProjectId,
        worktree: AbsolutePath,
        sandboxes_json: String,
        now: i64,
    ) -> Self {
        ProjectRow {
            id,
            worktree,
            vcs: None,
            name: None,
            icon_url: None,
            icon_url_override: None,
            icon_color: None,
            time_created: now,
            time_updated: now,
            time_initialized: None,
            sandboxes: sandboxes_json,
            commands: None,
        }
    }

    /// Valide et enregistre le repertoire de travail (`toDriver`).
    ///
    /// Refuse un chemin relatif **et** la chaine vide, comme le fait
    /// `absoluteColumn`. Le champ n'est pas modifie en cas d'echec.
    pub fn set_worktree(
        &mut self,
        worktree: &str,
        platform: StoragePlatform,
    ) -> Result<(), ColumnError> {
        self.worktree = absolute(worktree, platform).map_err(ColumnError::Path)?;
        Ok(())
    }

    /// Le repertoire de travail relu par l'application (`fromDriver`).
    pub fn read_worktree(&self, platform: StoragePlatform) -> Result<AbsolutePath, ColumnError> {
        let chemin = absolute(&self.worktree, platform).map_err(ColumnError::Path)?;
        Ok(to_platform(&chemin, platform))
    }

    /// Valide puis encode la liste des bacs a sable (`toDriver`).
    ///
    /// Reproduit `JSON.stringify(input.map(absolute))` : chaque chemin est
    /// d'abord valide et mis sous forme de stockage, et l'echec d'un seul
    /// element fait echouer l'ensemble, sans rien ecrire. Sur Windows, un
    /// `C:\tmp` devient `C:/tmp`.
    pub fn set_sandboxes(
        &mut self,
        sandboxes: &[AbsolutePath],
        platform: StoragePlatform,
    ) -> Result<(), ColumnError> {
        let mut formes = Vec::with_capacity(sandboxes.len());
        for chemin in sandboxes {
            formes.push(absolute(chemin, platform).map_err(ColumnError::Path)?);
        }
        self.sandboxes = encode_sandboxes(&formes)?;
        Ok(())
    }

    /// Relit la liste des bacs a sable (`fromDriver`).
    ///
    /// Reproduit `JSON.parse(input).map(toPlatform(absolute(item)))`. Un texte
    /// qui n'est pas un tableau JSON de chaines est refuse : en JavaScript, un
    /// objet JSON y provoquerait un `TypeError` sur `.map`, donc l'erreur est
    /// le comportement fidele, pas une commodite ajoutee.
    pub fn read_sandboxes(
        &self,
        platform: StoragePlatform,
    ) -> Result<Vec<AbsolutePath>, ColumnError> {
        let formes = decode_sandboxes(&self.sandboxes)?;
        Ok(formes
            .iter()
            .map(|c| to_platform(c, platform))
            .collect())
    }

    /// Applique une mise a jour : seul `time_updated` bouge.
    ///
    /// Traduction de `$onUpdate(() => Date.now())`. `time_created` est la date
    /// de creation, pas celle du dernier enregistrement.
    pub fn touch(&mut self, now: i64) {
        self.time_updated = now;
    }
}

/// Encode une liste de chemins de stockage en texte JSON.
///
/// Fonction separee de [`ProjectRow::set_sandboxes`] pour que le format de la
/// colonne soit testable sans passer par une ligne entiere.
pub fn encode_sandboxes(sandboxes: &[String]) -> Result<String, ColumnError> {
    serde_json::to_string(sandboxes).map_err(|e| ColumnError::Json(e.to_string()))
}

/// Decode le texte JSON de la colonne `sandboxes` en chemins de stockage.
pub fn decode_sandboxes(raw: &str) -> Result<Vec<String>, ColumnError> {
    serde_json::from_str(raw).map_err(|e| ColumnError::Json(e.to_string()))
}

// ---------------------------------------------------------------------------
// Table `project_directory`
// ---------------------------------------------------------------------------

/// Une ligne de la table `project_directory`.
///
/// Les cinq champs correspondent, dans l'ordre, aux cinq colonnes du DDL. Le
/// `NOT NULL` de `project_id` vient de `.notNull()` et sa presence dans la cle
/// primaire vient de la contrainte de tableau : les deux sont independants, et
/// cette colonne ne porte donc pas le drapeau `primary_key`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDirectoryRow {
    /// `text NOT NULL`, cle etrangere vers `project.id` avec suppression en
    /// cascade. Seconde colonne de la cle primaire composee.
    #[serde(rename = "project_id")]
    pub project_id: ProjectId,
    /// `text NOT NULL`, chemin absolu. Premiere colonne de la cle primaire
    /// composee, apres `project_id`.
    #[serde(rename = "directory")]
    pub directory: AbsolutePath,
    /// `text`, nature du repertoire, texte libre.
    ///
    /// La source y ecrit `"main"`, `"root"` ou `"git_worktree"`, mais par
    /// `$type<...>()`, qui n'existe qu'a la compilation. Le type reste donc
    /// `Option<String>` : un `enum` refuserait des lignes que la base accepte.
    /// [`is_known_directory_type`] permet de signaler la divergence.
    ///
    /// `type` est un mot cle reserve en Rust, d'ou l'identifiant brut.
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    /// `text`, strategie de copie, texte libre. La colonne est la seule que
    /// `directories.ts` ligne 75 distingue de `NULL` par un `isNull`, suivi d'un
    /// `ne` qui exclut la chaine vide. Ce comportement de deux etats appartient
    /// au portage de ce fichier-la, pas ici.
    #[serde(rename = "strategy")]
    pub strategy: Option<String>,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_created")]
    pub time_created: i64,
}

impl ProjectDirectoryRow {
    /// Construit une ligne a l'insertion, avec le `now` fourni.
    ///
    /// `time_created` recoit `$default(() => Date.now())` dans la source, mais
    /// aucun `DEFAULT` SQL : il doit donc etre fourni. Les deux colonnes
    /// optionnelles partent a `None`, ce qui est bien la valeur de leur
    /// colonne.
    pub fn new(project_id: ProjectId, directory: AbsolutePath, now: i64) -> Self {
        ProjectDirectoryRow {
            project_id,
            directory,
            r#type: None,
            strategy: None,
            time_created: now,
        }
    }

    /// Construit une ligne en validant le chemin du repertoire (`toDriver`).
    ///
    /// Refuse un chemin relatif et la chaine vide, comme
    /// [`ProjectRow::set_worktree`] pour `worktree` : les deux colonnes
    /// utilisent `absoluteColumn`.
    pub fn insert(
        project_id: ProjectId,
        directory: &str,
        now: i64,
        platform: StoragePlatform,
    ) -> Result<Self, PathError> {
        let directory = absolute(directory, platform)?;
        Ok(ProjectDirectoryRow::new(project_id, directory, now))
    }

    /// Le repertoire relu par l'application (`fromDriver`).
    pub fn read_directory(&self, platform: StoragePlatform) -> Result<AbsolutePath, PathError> {
        let chemin = absolute(&self.directory, platform)?;
        Ok(to_platform(&chemin, platform))
    }

    /// La cle que la contrainte de tableau protege pour cette ligne.
    pub fn key(&self) -> ProjectDirectoryKey {
        ProjectDirectoryKey {
            project_id: self.project_id.clone(),
            directory: self.directory.clone(),
        }
    }
}

/// La cle primaire composee de `project_directory` : `(project_id, directory)`.
///
/// Un couple. Deux repertoires differents du meme projet sont deux cles
/// distinctes, et un repertoire donne dans deux projets l'est aussi : la
/// colonne `directory` n'est jamais unique seule, ce que confirme la cascade,
/// puisque supprimer un projet ne doit pas effacer les repertoires homonymes
/// d'un autre projet.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectDirectoryKey {
    /// Colonne `project_id`, premier critere de tri.
    pub project_id: ProjectId,
    /// Colonne `directory`, second critere de tri.
    pub directory: String,
}

impl ProjectDirectoryKey {
    /// Construit une cle a partir de ses deux colonnes.
    pub fn new(project_id: ProjectId, directory: String) -> Self {
        ProjectDirectoryKey { project_id, directory }
    }
}

/// Les trois valeurs que le `$type` de la source autorise pour `type`.
///
/// Liste de reference, **pas** une contrainte. La colonne accepte n'importe
/// quel texte, et le DDL ne dit rien de tout cela. Rien dans
/// `packages/core` n'ecrit ces litteraux aujourd'hui : ils sont declares dans
/// le type et nulle part ailleurs.
pub const PROJECT_DIRECTORY_TYPES: [&str; 3] = ["main", "root", "git_worktree"];

/// Le `type` lu est-il l'un des trois que la source declare ?
///
/// Rend `false` sans erreur : c'est un **signalement**, pas un refus. Une
/// reponse `false` sur une ligne lue de la base signifie que la base contient
/// une valeur que le type de la source ne prevoit pas, ce qui est une
/// divergence a remonter, pas une ligne a rejeter.
pub fn is_known_directory_type(value: &str) -> bool {
    PROJECT_DIRECTORY_TYPES.contains(&value)
}

/// Premiere cle en double dans une tranche de repertoires.
///
/// C'est la violation de la cle primaire composee, vue sur les donnees. Une
/// tranche valide n'en contient aucune : le retour `None` prouve que les lignes
/// sont incoherentes avec le DDL, typiquement parce que la contrainte n'a pas
/// ete creee ou qu'une ecriture l'a contournee, ce que fait un
/// `INSERT OR IGNORE` absent.
///
/// « Premiere » signifie premiere dans l'ordre de la tranche, ce qui rend le
/// resultat stable et dependant uniquement de l'ordre de lecture.
pub fn first_duplicate_directory(rows: &[ProjectDirectoryRow]) -> Option<ProjectDirectoryKey> {
    let mut vues: BTreeSet<ProjectDirectoryKey> = BTreeSet::new();
    for row in rows {
        let cle = row.key();
        if !vues.insert(cle.clone()) {
            return Some(cle);
        }
    }
    None
}

/// Les lignes candidates que la cle primaire composee accepterait.
///
/// C'est la forme pure de la contrainte declaree par ce fichier, et elle repond
/// a la seule question que la cle pose : ce couple existe-t-il deja ? Le
/// portage de `project/directories.ts` pourra s'en servir pour son
/// `onConflictDoUpdate` de la ligne 72, qui cible exactement
/// `[project_id, directory]`.
///
/// L'ordre de sortie est l'ordre d'entree, et un doublon a l'interieur des
/// candidats eux-memes ne laisse passer que sa premiere occurrence. Seules les
/// deux colonnes de la cle sont comparees : `strategy` et `type` ne sont pas
/// dans la cle, et les mettre en comparaison ferait rejetter des mises a jour
/// legitimes.
pub fn insertable_directory_rows<'a>(
    existing: &[ProjectDirectoryRow],
    candidates: &'a [ProjectDirectoryRow],
) -> Vec<&'a ProjectDirectoryRow> {
    let mut connues: BTreeSet<ProjectDirectoryKey> =
        existing.iter().map(ProjectDirectoryRow::key).collect();
    let mut retenues: Vec<&ProjectDirectoryRow> = Vec::new();
    for candidat in candidates {
        if connues.insert(candidat.key()) {
            retenues.push(candidat);
        }
    }
    retenues
}

/// Les repertoires qui subsistent apres la suppression d'un projet.
///
/// C'est la cascade `ON DELETE CASCADE` de la cle etrangere, vue comme une
/// projection sur une tranche : disparait tout ce qui appartient au projet
/// donne, rien d'autre. L'ordre des lignes restantes est preserve, comme apres
/// un `DELETE` SQL, qui ne reordonne rien.
///
/// Sans `PRAGMA foreign_keys`, SQLite ignore silencieusement cette cascade :
/// la fonction decrit donc l'intention du schema, pas un comportement du
/// moteur. La table `permission` est construite exactement de la meme facon,
/// si bien qu'un defaut d'activation du pragma affecterait toutes les tables du
/// depot, pas seulement celle-ci.
pub fn directories_kept_after_project_delete<'a>(
    rows: &'a [ProjectDirectoryRow],
    project_id: &str,
) -> Vec<&'a ProjectDirectoryRow> {
    rows.iter().filter(|row| row.project_id != project_id).collect()
}

// ---------------------------------------------------------------------------
// Colonne `commands`
// ---------------------------------------------------------------------------

/// Le document JSON de la colonne `commands`.
///
/// `$type<{ start?: string }>()` est ici **porteur**, contrairement a l'union de
/// `type` : la colonne est en `text({ mode: "json" })`, donc l'ORM y applique
/// `JSON.parse` et le type est reellement verifie. Le champ est donc optionnel
/// cote JSON comme ici, et `commands` ne peut valoir que `{}` ou
/// `{ "start": "..." }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCommands {
    /// Commande de demarrage du projet, absente si le projet n'en a pas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

/// Encode [`ProjectCommands`] en texte JSON pour la colonne `commands`.
pub fn encode_commands(commands: &ProjectCommands) -> Result<String, ColumnError> {
    serde_json::to_string(commands).map_err(|e| ColumnError::Json(e.to_string()))
}

/// Decode le texte JSON de la colonne `commands`.
///
/// Un JSON qui n'est pas un objet `{ start?: string }` est refuse : c'est ce que
/// ferait `JSON.parse` suivi de l'assertion de forme cote TypeScript.
pub fn decode_commands(raw: &str) -> Result<ProjectCommands, ColumnError> {
    serde_json::from_str(raw).map_err(|e| ColumnError::Json(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ----------------------------------------------------------

    fn projet() -> ProjectRow {
        ProjectRow::new("pro_1".to_string(), "/srv/depot".to_string(), "[]".to_string(), 7)
    }

    fn repertoire(projet: &str, dossier: &str) -> ProjectDirectoryRow {
        ProjectDirectoryRow::new(projet.to_string(), dossier.to_string(), 0)
    }

    // -- le DDL, compare au fichier genere ---------------------------------

    #[test]
    fn le_ddl_de_project_est_celui_du_fichier_genere() {
        // Ces douze chaines viennent de `schema.gen.ts`, pas de la lecture du
        // code declaratif. Si elles sont justes, le branchement SQL futur
        // n'aura pas a les deviner.
        let attendu = [
            "`id` text PRIMARY KEY",
            "`worktree` text NOT NULL",
            "`vcs` text",
            "`name` text",
            "`icon_url` text",
            "`icon_url_override` text",
            "`icon_color` text",
            "`time_created` integer NOT NULL",
            "`time_updated` integer NOT NULL",
            "`time_initialized` integer",
            "`sandboxes` text NOT NULL",
            "`commands` text",
        ];
        let obtenu: Vec<String> = PROJECT_COLUMNS.iter().map(ColumnSpec::ddl_line).collect();
        assert_eq!(obtenu, attendu);
    }

    #[test]
    fn le_ddl_de_project_directory_est_celui_du_fichier_genere() {
        let attendu = [
            "`project_id` text NOT NULL",
            "`directory` text NOT NULL",
            "`type` text",
            "`strategy` text",
            "`time_created` integer NOT NULL",
        ];
        let obtenu: Vec<String> =
            PROJECT_DIRECTORY_COLUMNS.iter().map(ColumnSpec::ddl_line).collect();
        assert_eq!(obtenu, attendu);
        assert_eq!(
            project_directory_primary_key_clause(),
            "CONSTRAINT `project_directory_pk` PRIMARY KEY(`project_id`, `directory`)"
        );
        assert_eq!(
            project_directory_foreign_key_clause(),
            "CONSTRAINT `fk_project_directory_project_id_project_id_fk` \
             FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE"
        );
    }

    #[test]
    fn les_drapeaux_not_null_et_primary_key_sont_independants() {
        // `project.id` porte `.primaryKey()` et rien d'autre : le drapeau
        // `not_null` y est donc FAUX. L'inverse est le piege symetrique, et il
        // est lui aussi present dans ce fichier.
        let id = PROJECT_COLUMNS[0];
        assert!(id.flags.primary_key);
        assert!(!id.flags.not_null);

        let worktree = PROJECT_COLUMNS[1];
        assert!(worktree.flags.not_null);
        assert!(!worktree.flags.primary_key);
    }

    #[test]
    fn une_cle_primaire_composee_ne_pose_aucun_drfau_de_colonne() {
        // `project_directory.project_id` et `.directory` n'appellent
        // `.primaryKey()` : leur drapeau `primary_key` est faux. Le `NOT NULL`
        // vient de `.notNull()`, le `PRIMARY KEY` de la contrainte de tableau.
        // Confondre les deux voies ferait disparaitre la distinction.
        for spec in &PROJECT_DIRECTORY_COLUMNS {
            assert!(!spec.flags.primary_key, "{} ne doit pas porter le drapeau", spec.name);
        }
        let projet_id = PROJECT_DIRECTORY_COLUMNS[0];
        assert!(projet_id.flags.not_null);
        assert!(projet_id.ddl_line() == "`project_id` text NOT NULL");
        assert_eq!(
            PROJECT_DIRECTORY_PRIMARY_KEY_COLUMNS,
            ["project_id", "directory"]
        );
    }

    #[test]
    fn les_noms_de_table_sont_bien_ceux_du_fichier_genere() {
        assert_eq!(PROJECT_TABLE, "project");
        assert_eq!(PROJECT_DIRECTORY_TABLE, "project_directory");
        assert_eq!(PROJECT_DIRECTORY_PRIMARY_KEY_NAME, "project_directory_pk");
        assert_eq!(
            PROJECT_DIRECTORY_FOREIGN_KEY_NAME,
            "fk_project_directory_project_id_project_id_fk"
        );
    }

    // -- forme des lignes --------------------------------------------------

    #[test]
    fn la_ligne_project_expose_exactement_les_douze_colonnes() {
        // Le contrat de la ligne et celui de la table doivent rester le meme
        // ensemble de noms. Sans ce test, un champ ajoute au struct et oublie
        // dans `PROJECT_COLUMNS` passerait la compilation.
        let json = serde_json::to_value(projet()).unwrap();
        let objet = json.as_object().unwrap();
        for spec in PROJECT_COLUMNS {
            assert!(objet.contains_key(spec.name), "la colonne {} doit exister", spec.name);
        }
        assert_eq!(objet.len(), PROJECT_COLUMNS.len(), "aucune colonne en trop ni en trop peu");
    }

    #[test]
    fn la_ligne_project_ne_serialise_jamais_le_camelcase_du_json_public() {
        // `icon_url` en base, `iconURL` chez les appelants. Une faute ici
        // passerait la compilation et tous les tests de logique.
        let json = serde_json::to_value(projet()).unwrap();
        for interdit in ["iconURL", "iconURLOverride", "iconColor", "timeCreated", "timeUpdated"] {
            assert!(json.get(interdit).is_none(), "{interdit} n est pas un nom de colonne");
        }
        assert!(json.get("icon_url").is_some());
    }

    #[test]
    fn les_douze_colonnes_sont_dans_lordre_du_create_table() {
        // Cet ordre est celui d'un futur `INSERT`. Le test echoue des qu'une
        // colonne est ajoutee, renommee ou deplacee.
        let noms: Vec<&str> = PROJECT_COLUMNS.iter().map(|c| c.name).collect();
        assert_eq!(
            noms,
            [
                "id",
                "worktree",
                "vcs",
                "name",
                "icon_url",
                "icon_url_override",
                "icon_color",
                "time_created",
                "time_updated",
                "time_initialized",
                "sandboxes",
                "commands",
            ]
        );
    }

    #[test]
    fn la_ligne_repertoire_expose_exactement_les_cinq_colonnes() {
        let ligne = repertoire("pro_1", "/srv/depot");
        let json = serde_json::to_value(&ligne).unwrap();
        let objet = json.as_object().unwrap();
        for spec in PROJECT_DIRECTORY_COLUMNS {
            assert!(objet.contains_key(spec.name), "la colonne {} doit exister", spec.name);
        }
        assert_eq!(objet.len(), PROJECT_DIRECTORY_COLUMNS.len());
        assert!(objet.contains_key("type"), "la colonne se nomme type dans le DDL");
    }

    #[test]
    fn un_aller_retour_json_preserve_les_deux_lignes() {
        // Sens inverse : ce que l'ecriture produit doit se relire tel quel.
        let p = projet();
        let relue: ProjectRow = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(relue, p);

        let d = ProjectDirectoryRow {
            r#type: Some("git_worktree".to_string()),
            strategy: Some("copy".to_string()),
            ..repertoire("pro_1", "/srv/depot")
        };
        let relue: ProjectDirectoryRow =
            serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(relue, d);
    }

    #[test]
    fn la_creation_pose_les_deux_horodatages_sur_le_meme_instant() {
        // `time_created` vient du `$default`, `time_updated` n'a pas de
        // `DEFAULT` SQL et doit donc etre fourni : on ecrit `now` dans les deux,
        // choix documente dans l'en-tete.
        let p = projet();
        assert_eq!(p.time_created, 7);
        assert_eq!(p.time_updated, 7);
        assert_eq!(p.time_initialized, None, "le projet n est pas encore initialise");
        assert_eq!(repertoire("pro_1", "/srv/depot").time_created, 0);
    }

    #[test]
    fn une_mise_a_jour_ne_bouge_que_l_horodatage_de_maj() {
        // `$onUpdate(() => Date.now())` : `time_created` est la date de
        // creation, pas celle du dernier enregistrement.
        let mut p = projet();
        p.touch(500);
        assert_eq!(p.time_updated, 500);
        assert_eq!(p.time_created, 7);
        assert_eq!(p.id, "pro_1");
    }

    // -- `type` : ce que la source ecrit, ce que la base autorise ------------

    #[test]
    fn un_type_de_repertoire_hors_union_est_neanmoins_lisible() {
        // Le `$type<...>()` n'existe qu'a la compilation : une ligne contenant
        // un texte que l'union ne prevoit pas se relit sans erreur. Un `enum`
        // l'aurait refusee, ce qui aurait ete inventer une contrainte que le DDL
        // n'a pas.
        let json = r#"{"project_id":"pro_1","directory":"/x","type":"inconnu","time_created":0}"#;
        let ligne: ProjectDirectoryRow = serde_json::from_str(json).unwrap();
        assert_eq!(ligne.r#type.as_deref(), Some("inconnu"));
        assert!(!is_known_directory_type("inconnu"), "et la divergence est signalable");
    }

    #[test]
    fn les_trois_types_declares_par_la_source_sont_reconnus() {
        for t in PROJECT_DIRECTORY_TYPES {
            assert!(is_known_directory_type(t), "{t} fait partie de l union declaree");
        }
        assert_eq!(PROJECT_DIRECTORY_TYPES, ["main", "root", "git_worktree"]);
        assert!(!is_known_directory_type(""));
    }

    // -- cle primaire composee --------------------------------------------

    #[test]
    fn la_cle_de_repertoire_est_le_couple_projet_repertoire() {
        // Deux repertoires du meme projet, ou le meme repertoire dans deux
        // projets : deux cles distinctes dans les deux cas. Si la cle etait
        // reduite au repertoire, la cascade supprimerait des lignes d'un autre
        // projet.
        assert_eq!(
            repertoire("pro_1", "/a").key(),
            ProjectDirectoryKey::new("pro_1".to_string(), "/a".to_string())
        );
        assert_ne!(
            repertoire("pro_1", "/a").key(),
            repertoire("pro_2", "/a").key()
        );
        assert_ne!(
            repertoire("pro_1", "/a").key(),
            repertoire("pro_1", "/b").key()
        );
    }

    #[test]
    fn deux_repertoires_identiques_pour_un_projet_sont_un_doublon() {
        // La cle primaire composee l'interdit, et le retour `None` prouve au
        // contraire que la tranche est coherente avec le DDL.
        let tranche = vec![
            repertoire("pro_1", "/a"),
            repertoire("pro_1", "/b"),
            repertoire("pro_2", "/a"),
            repertoire("pro_1", "/a"),
        ];
        assert_eq!(
            first_duplicate_directory(&tranche),
            Some(ProjectDirectoryKey::new("pro_1".to_string(), "/a".to_string()))
        );
        assert!(first_duplicate_directory(&tranche[..3]).is_none());
        let vide: Vec<ProjectDirectoryRow> = Vec::new();
        assert!(first_duplicate_directory(&vide).is_none());
    }

    #[test]
    fn un_doublon_ne_laisse_passer_que_sa_premiere_occurrence() {
        let candidats = vec![
            repertoire("pro_1", "/a"),
            repertoire("pro_1", "/a"),
            ProjectDirectoryRow {
                strategy: Some("copy".to_string()),
                ..repertoire("pro_1", "/a")
            },
        ];
        let retenues = insertable_directory_rows(&[], &candidats);
        assert_eq!(retenues.len(), 1);
        assert_eq!(retenues[0].strategy, None, "seule la premiere ligne passe");
    }

    #[test]
    fn la_cle_ne_compare_ni_type_ni_strategy() {
        // Ces colonnes sont hors cle. Les mettre en comparaison ferait
        // rejetter des mises a jour legitimes, alors que le `onConflictDoUpdate`
        // de `directories.ts` cible exactement le couple projet/repertoire.
        let existant = vec![repertoire("pro_1", "/a")];
        let candidats = vec![ProjectDirectoryRow {
            r#type: Some("main".to_string()),
            strategy: Some("git_worktree".to_string()),
            ..repertoire("pro_1", "/a")
        }];
        assert!(insertable_directory_rows(&existant, &candidats).is_empty());
    }

    #[test]
    fn la_cle_se_trie_dabord_par_projet_puis_par_repertoire() {
        // L'ordre suit celui des colonnes de la contrainte, donc celui d'un
        // parcours d'index le jour ou la base existera.
        let mut cles = vec![
            ProjectDirectoryKey::new("pro_2".to_string(), "/a".to_string()),
            ProjectDirectoryKey::new("pro_1".to_string(), "/b".to_string()),
            ProjectDirectoryKey::new("pro_1".to_string(), "/a".to_string()),
        ];
        cles.sort();
        assert_eq!(
            cles,
            vec![
                ProjectDirectoryKey::new("pro_1".to_string(), "/a".to_string()),
                ProjectDirectoryKey::new("pro_1".to_string(), "/b".to_string()),
                ProjectDirectoryKey::new("pro_2".to_string(), "/a".to_string()),
            ]
        );
    }

    #[test]
    fn la_suppression_d_un_projet_emporte_ses_seuls_repertoires() {
        // La cascade emporte tout ce qui appartient au projet donne, et rien de
        // plus : le repertoire homonyme d'un autre projet survit.
        let tranche = vec![
            repertoire("pro_1", "/a"),
            repertoire("pro_1", "/b"),
            repertoire("pro_2", "/a"),
        ];
        let restantes = directories_kept_after_project_delete(&tranche, "pro_1");
        assert_eq!(restantes.len(), 1);
        assert_eq!(restantes[0].project_id, "pro_2");
        // Un projet inconnu ne supprime rien, comme un DELETE sans effet.
        assert_eq!(directories_kept_after_project_delete(&tranche, "pro_9").len(), 3);
        assert!(directories_kept_after_project_delete(&[], "pro_1").is_empty());
    }

    // -- chemins : le piege de veracite, et son absence ici ------------------

    #[test]
    fn le_chemin_vide_est_refuse_par_absolute_column() {
        // Piege `?` contre absence de `?`. `directoryColumn` de
        // `database/path.ts` teste la veracite et laisse passer la chaine
        // vide ; `absoluteColumn`, que ce fichier utilise pour `worktree` et
        // `directory`, ne le fait pas et leve. Il n'y a donc aucune garde a
        // porter ici : le vide est refuse.
        assert_eq!(
            absolute("", StoragePlatform::Unix),
            Err(PathError::NotAbsolute(String::new()))
        );
        assert!(absolute("", StoragePlatform::Windows).is_err());
    }

    #[test]
    fn un_chemin_relatif_est_refuse_sur_les_deux_plateformes() {
        assert_eq!(
            absolute("srv/depot", StoragePlatform::Unix),
            Err(PathError::NotAbsolute("srv/depot".to_string()))
        );
        assert!(absolute("srv\\depot", StoragePlatform::Windows).is_err());
        assert!(absolute(".", StoragePlatform::Unix).is_err());
        // Un lecteur de lettre suivi de deux-points sans slash n'est pas un
        // chemin de stockage Windows.
        assert!(absolute("C:depot", StoragePlatform::Windows).is_err());
    }

    #[test]
    fn sur_windows_l_ecriture_transforme_les_antislashs_en_slashs() {
        assert_eq!(storage_path("C:\\tmp", StoragePlatform::Windows), "C:/tmp");
        assert_eq!(storage_path("C:\\tmp", StoragePlatform::Unix), "C:\\tmp");
        assert_eq!(
            absolute("C:\\tmp", StoragePlatform::Windows),
            Ok("C:/tmp".to_string())
        );
    }

    #[test]
    fn un_chemi_unc_est_absolu_sur_windows_seulement() {
        // `//` est traite comme absolu par `isWindowsStoragePath`, mais la
        // branche n'est atteinte que sur win32.
        assert!(is_windows_storage_path("//serveur/partage"));
        assert!(is_windows_storage_path("C:/x"));
        assert!(is_windows_storage_path("z:/x"));
        assert!(!is_windows_storage_path("C:\\x"), "la forme windows ne l est pas");
        assert!(!is_windows_storage_path("/x"));
        assert!(!is_windows_storage_path("1:/x"), "le lecteur doit etre une lettre");
        assert!(absolute("//serveur/partage", StoragePlatform::Windows).is_ok());
        assert!(absolute("//serveur/partage", StoragePlatform::Unix).is_err());
    }

    #[test]
    fn a_la_lecture_un_chemin_windows_revient_avec_des_antislashs() {
        // `fromDriver` fait `toPlatform(absolute(input))`, dans cet ordre.
        assert_eq!(to_platform("C:/x", StoragePlatform::Windows), "C:\\x");
        assert_eq!(
            to_platform("C:\\x", StoragePlatform::Windows),
            "C:\\x",
            "un chemin deja windows traverse l aller-retour sans changer"
        );
        // Un chemin posix reste posix sur les deux plateformes, car
        // `isWindowsStoragePath` le rejette.
        assert_eq!(to_platform("/srv/x", StoragePlatform::Windows), "/srv/x");
        assert_eq!(to_platform("/srv/x", StoragePlatform::Unix), "/srv/x");
    }

    #[test]
    fn le_worktree_est_valide_a_l_ecriture_et_normalise_a_la_lecture() {
        let mut p = projet();
        p.set_worktree("C:\\srv\\depot", StoragePlatform::Windows).unwrap();
        assert_eq!(p.worktree, "C:/srv/depot");
        assert_eq!(p.read_worktree(StoragePlatform::Windows).unwrap(), "C:\\srv\\depot");
        assert_eq!(p.read_worktree(StoragePlatform::Unix).unwrap(), "C:/srv/depot");
    }

    #[test]
    fn un_worktree_refuse_ne_laisse_pas_de_trace() {
        // Le champ n'est pas modifie quand l'ecriture echoue.
        let mut p = projet();
        let avant = p.clone();
        assert!(p.set_worktree("srv/depot", StoragePlatform::Unix).is_err());
        assert_eq!(p, avant);
        assert!(matches!(
            p.set_worktree("", StoragePlatform::Unix),
            Err(ColumnError::Path(PathError::NotAbsolute(_)))
        ));
        assert_eq!(p.worktree, "/srv/depot");
    }

    #[test]
    fn un_repertoire_est_valide_a_l_insertion_par_la_meme_fonction() {
        // `directory` et `worktree` utilisent toutes deux `absoluteColumn` :
        // le comportement de rejet est donc identique, et c'est deliberement le
        // meme code qui l'applique.
        assert!(ProjectDirectoryRow::insert("pro_1".to_string(), "a/b", 0, StoragePlatform::Unix).is_err());
        let l = ProjectDirectoryRow::insert(
            "pro_1".to_string(),
            "C:\\srv\\depot",
            0,
            StoragePlatform::Windows,
        )
        .unwrap();
        assert_eq!(l.directory, "C:/srv/depot");
        assert_eq!(l.read_directory(StoragePlatform::Windows).unwrap(), "C:\\srv\\depot");
    }

    // -- colonne `sandboxes` -----------------------------------------------

    #[test]
    fn les_sandboxes_font_l_aller_retour() {
        // `absoluteArrayColumn` : chaque element est valide puis mis sous
        // forme de stockage a l'ecriture, et rendu sous forme de plateforme a
        // la lecture.
        let mut p = projet();
        p.set_sandboxes(
            &["/srv/a".to_string(), "/srv/b".to_string()],
            StoragePlatform::Unix,
        )
        .unwrap();
        assert_eq!(p.sandboxes, r#"["/srv/a","/srv/b"]"#);
        assert_eq!(
            p.read_sandboxes(StoragePlatform::Unix).unwrap(),
            vec!["/srv/a".to_string(), "/srv/b".to_string()]
        );
    }

    #[test]
    fn une_liste_de_sandboxes_vide_s_ecrit_et_non_null() {
        // La colonne est `NOT NULL` : l'absence s'ecrit `[]`, jamais `NULL`.
        let mut p = projet();
        p.set_sandboxes(&[], StoragePlatform::Unix).unwrap();
        assert_eq!(p.sandboxes, "[]");
        assert!(p.read_sandboxes(StoragePlatform::Unix).unwrap().is_empty());
    }

    #[test]
    fn un_sandbox_non_absolu_rejette_toute_la_liste() {
        // `JSON.stringify(input.map(absolute))` : l echec d un element fait
        // echouer l expression entiere, donc rien n est ecrit.
        let mut p = projet();
        let avant = p.sandboxes.clone();
        assert!(matches!(
            p.set_sandboxes(&["/srv/a".to_string(), "b".to_string()], StoragePlatform::Unix),
            Err(ColumnError::Path(PathError::NotAbsolute(_)))
        ));
        assert_eq!(p.sandboxes, avant);
    }

    #[test]
    fn des_sandboxes_en_ecriture_windows_reviennent_en_antislashs() {
        let mut p = projet();
        p.set_sandboxes(&["C:\\srv\\a".to_string()], StoragePlatform::Windows).unwrap();
        assert_eq!(p.sandboxes, r#"["C:/srv/a"]"#);
        assert_eq!(
            p.read_sandboxes(StoragePlatform::Windows).unwrap(),
            vec!["C:\\srv\\a".to_string()]
        );
    }

    #[test]
    fn une_colonne_sandboxes_invalide_est_refusee() {
        // En JavaScript, `JSON.parse("pas du json")` leve, et un objet JSON y
        // provoquerait un `TypeError` sur `.map` : dans les deux cas la lecture
        // echoue.
        for invalide in ["pas du json", r#"{"a":1}"#, r#"["/a",3]"#, ""] {
            assert!(
                matches!(decode_sandboxes(invalide), Err(ColumnError::Json(_))),
                "{invalide} ne devrait pas se decoder"
            );
        }
        let p = ProjectRow::new("pro_1".to_string(), "/x".to_string(), "nope".to_string(), 0);
        assert!(matches!(
            p.read_sandboxes(StoragePlatform::Unix),
            Err(ColumnError::Json(_))
        ));
    }

    // -- colonne `commands` ------------------------------------------------

    #[test]
    fn des_commandes_absentes_donnent_un_objet_vide() {
        // `{ start?: string }` : le champ est optionnel cote JSON comme ici,
        // et la colonne est par ailleurs nullable.
        assert_eq!(encode_commands(&ProjectCommands::default()).unwrap(), "{}");
        assert_eq!(decode_commands("{}").unwrap(), ProjectCommands::default());
        assert_eq!(ProjectCommands::default().start, None);
    }

    #[test]
    fn une_commande_de_demarrage_va_et_vient() {
        let c = ProjectCommands { start: Some("pnpm dev".to_string()) };
        let encodee = encode_commands(&c).unwrap();
        assert_eq!(encodee, r#"{"start":"pnpm dev"}"#);
        assert_eq!(decode_commands(&encodee).unwrap(), c);

        let mut p = projet();
        p.commands = Some(encodee);
        assert_eq!(decode_commands(p.commands.as_deref().unwrap()).unwrap().start.as_deref(), Some("pnpm dev"));
    }

    #[test]
    fn des_commandes_mal_formees_sont_refusees() {
        // Ici le `$type<{ start?: string }>()` est porteur, parce que la
        // colonne est en `mode: "json"` : l ORM y applique `JSON.parse`, et le
        // type est donc reellement verifie.
        for invalide in [r#"[]"#, r#""x""#, r#"{"start":1}"#, r#"{"demarrage":"x"}"#, "nope"] {
            assert!(
                matches!(decode_commands(invalide), Err(ColumnError::Json(_))),
                "{invalide} ne devrait pas se decoder"
            );
        }
    }

    #[test]
    fn une_ligne_project_nullable_peut_vraiment_l_etre() {
        // Sept colonnes sur douze sont nullable, et `serde` les ecrit `null`
        // plutot que de les omettre : c'est la valeur de la colonne, pas une
        // absence d'information.
        let json = serde_json::to_value(projet()).unwrap();
        for nom in [COLUMN_VCS, COLUMN_NAME, COLUMN_ICON_URL, COLUMN_ICON_URL_OVERRIDE, COLUMN_ICON_COLOR, COLUMN_COMMANDS] {
            assert!(json[nom].is_null(), "{nom} doit valoir null sur une ligne neuve");
        }
        assert!(json[COLUMN_TIME_INITIALIZED].is_null());
        // Les quatre colonnes `NOT NULL` ne le sont jamais.
        for nom in [COLUMN_ID, COLUMN_WORKTREE, COLUMN_TIME_CREATED, COLUMN_TIME_UPDATED, COLUMN_SANDBOXES] {
            assert!(!json[nom].is_null(), "{nom} est NOT NULL");
        }
    }
}
