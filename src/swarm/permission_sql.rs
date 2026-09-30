//! Portage Rust de `opencode/packages/core/src/permission/sql.ts`.
//!
//! ## Ce que porte reellement la source
//!
//! Le fichier d'origine fait 20 lignes, et ce sont 20 lignes de definition de
//! table Drizzle :
//!
//! ```ts
//! export const PermissionTable = sqliteTable(
//!   "permission",
//!   {
//!     id: text().$type<PermissionSaved.ID>().primaryKey(),
//!     project_id: text().$type<ProjectV2.ID>().notNull()
//!       .references(() => ProjectTable.id, { onDelete: "cascade" }),
//!     action: text().notNull(),
//!     resource: text().notNull(),
//!     ...Timestamps,
//!   },
//!   (table) => [uniqueIndex("permission_project_action_resource_idx")
//!     .on(table.project_id, table.action, table.resource)],
//! )
//! ```
//!
//! Il n'y a **aucune** fonction, aucun calcul, aucune requete : rien a
//! executer. Ce qui y est decrit est un **contrat de stockage**, et sa seule
//! traduction honnete sans dependance SQL est la forme de la ligne, plus les
//! contraintes que cette ligne porte, exprimees sous forme de fonctions pures.
//!
//! ## Pourquoi il n'y a pas de SQL ici
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : il n'existe
//! aucun moyen d'ecrire une couche de requete, et ajouter une dependance
//! n'est pas du ressort d'un portage de fichier. Le SQL sera branche plus
//! tard, par decision du projet. Ce fichier prepare donc le contrat que cette
//! couche viendra consommer (noms de table, de colonnes, d'index et de cle
//! etrangere) sans pretendre executer quoi que ce soit.
//!
//! ## La forme exacte est deja connue : ce n'est pas une invention
//!
//! `packages/core/src/database/schema.gen.ts` contient le DDL genere par
//! Drizzle, lignes 90 a 98 pour la table, ligne 97 pour la cle etrangere,
//! ligne 242 pour l'index unique. Ce sont ces chaines qui font foi, et non le
//! code declaratif, parce que ce sont elles que SQLite executera un jour :
//!
//! ```sql
//! CREATE TABLE `permission` (
//!   `id` text PRIMARY KEY,
//!   `project_id` text NOT NULL,
//!   `action` text NOT NULL,
//!   `resource` text NOT NULL,
//!   `time_created` integer NOT NULL,
//!   `time_updated` integer NOT NULL,
//!   CONSTRAINT `fk_permission_project_id_project_id_fk`
//!     FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
//! );
//! CREATE UNIQUE INDEX `permission_project_action_resource_idx`
//!   ON `permission` (`project_id`,`action`,`resource`);
//! ```
//!
//! On notera que **ce fichier ne cree aucune table** : aucune migration
//! n'est ecrite, pour la meme raison qu'aucune requete ne l'est.
//!
//! ## Ce qui est porte, et ce qui ne l'est pas
//!
//! - La forme de la ligne : six colonnes, dans l'ordre du DDL.
//! - L'index unique, sous forme de cle a trois colonnes et de fonction
//!   d'insertion. C'est la seule logique reellement contenue dans la source.
//! - La cascade `ON DELETE CASCADE` de `project_id`, sous forme de fonction
//!   de projection sur une tranche de lignes.
//! - Les deux horodatages apportes par `...Timestamps`, vus comme des
//!   millisecondes depuis l'epoch, comme le veut la conversion de
//!   `Schema.DateTimeUtc`.
//!
//! En revanche, ce fichier ne porte **pas** :
//!
//! - `list`, `add` et `remove` : ils sont dans `permission/saved.ts`, pas ici.
//!   On ne les reecrit pas, sinon deux fichiers du meme module se recouvriraient.
//! - La generation des identifiants. `PermissionSaved.ID.create()` vaut
//!   `"psv_" + Identifier.ascending()`, d'apres
//!   `packages/schema/src/permission-saved.ts` ligne 10. C'est le ressort du
//!   portage de `saved.ts` : la colonne `id` est donc un `String` ici, et le
//!   prefixe `psv_` n'est note que pour memoire.
//!
//! ## Le piege de nommage, qui est reel dans ce fichier
//!
//! La colonne s'appelle `project_id` dans la base, tandis que le contrat JSON
//! `PermissionSaved.Info` expose `projectID` en camelCase (voir
//! `permission-saved.ts` ligne 16). Les deux doivent coexister sans jamais se
//! confondre. `PermissionRow` est explicitement la **forme de la ligne SQL**,
//! dont les noms sont ceux du DDL ; `projectID` n'apparait nulle part ici, et
//! un test le verifie. C'est l'inverse du cas habituel, ou la faute consiste a
//! Serializer la ligne avec les noms du JSON.
//!
//! ## Un choix assume sur `time_updated`
//!
//! `database/schema.sql.ts` donne a `time_created` un
//! `$default(() => Date.now())` et a `time_updated` un
//! `$onUpdate(() => Date.now())`. Les deux colonnes sont `NOT NULL` dans le
//! DDL genere, mais **aucune des deux n'a de `DEFAULT` SQL** : ce sont des
//! crochets cote ORM, appliques au moment de construire les requetes, et non
//! des contraintes de la base. Consequence concrete : `time_updated` doit
//! etre fournie a l'insertion, faute de quoi la base refuse la ligne.
//!
//! `saved.ts` ne la fournit pas (ligne 59 a 64, seules quatre valeurs sont
//! inserees), et c'est donc le portage de `saved.ts` qui tranchera ce que
//! vaut `time_updated` a l'insertion. Ici, on ecrit le `now` fourni dans les
//! deux colonnes. C'est un choix explicite et documente, pas une deduction :
//! ecrire `0` ou laisser le champ a zero en silence serait une invention
//! bien plus difficile a voir.
//!
//! ## Un piege du voisinage, signale sans le porter
//!
//! `saved.ts` ligne 46 filtre les lignes ainsi :
//!
//! ```ts
//! .where(input?.projectID ? eq(PermissionTable.project_id, input.projectID) : undefined)
//! ```
//!
//! C'est un **ternaire**, donc un test de veracite, et non un coalescent. Un
//! `projectID` valant chaine vide est donc traite comme absent, et la liste
//! renvoyee n'est **pas** restreinte a un projet. C'est le piege `?` contre `??`
//! du contexte, et il se pose pile sur la colonne `project_id` de cette table.
//!
//! On ne le corrige pas ici, et on n'ecrit pas non plus la fonction de filtrage :
//! `list` appartient a `saved.ts`, et le recopier dans les deux fichiers ferait
//! diverger deux implementations du meme comportement. C'est note ici pour que
//! le portage de `saved.ts` le traite, et pour qu'un futur `rows_for_project`
//! n'helite pas entre les deux lectures.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Nom de la table, tel que declare dans le DDL genere.
pub const TABLE_NAME: &str = "permission";

/// Nom de l'index unique, tel que declare dans le DDL genere.
pub const UNIQUE_INDEX_NAME: &str = "permission_project_action_resource_idx";

/// Nom de la cle etrangere, tel que declare dans le DDL genere.
///
/// Le nom est important : SQLite ne le respecte que si le mode
/// `PRAGMA foreign_keys` est actif. Le declencher n'est pas du ressort de ce
/// fichier.
pub const PROJECT_FOREIGN_KEY_NAME: &str = "fk_permission_project_id_project_id_fk";

/// Colonne de la cle primaire.
pub const COLUMN_ID: &str = "id";

/// Colonne du projet proprietaire, et cible de la cascade.
pub const COLUMN_PROJECT_ID: &str = "project_id";

/// Colonne du motif d'action.
pub const COLUMN_ACTION: &str = "action";

/// Colonne du motif de ressource.
pub const COLUMN_RESOURCE: &str = "resource";

/// Colonne de creation, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_CREATED: &str = "time_created";

/// Colonne de mise a jour, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_UPDATED: &str = "time_updated";

/// Les six colonnes de la table, dans l'ordre du `CREATE TABLE`.
///
/// L'ordre n'est pas indifferent : il est celui du DDL genere, lignes 91 a 96,
/// et c'est lui qu'un futur `INSERT INTO permission (...) VALUES (...)` devra
/// suivre. Il est aussi l'ordre des champs de [`PermissionRow`], ce qui permet
/// de verifier d'un coup d'oeil que la ligne et la table ne derivent pas l'une
/// de l'autre.
pub const COLUMNS: [&str; 6] = [
    COLUMN_ID,
    COLUMN_PROJECT_ID,
    COLUMN_ACTION,
    COLUMN_RESOURCE,
    COLUMN_TIME_CREATED,
    COLUMN_TIME_UPDATED,
];

/// Identifiant de projet.
///
/// Reexport de `crate::core::session::schema::ProjectId`, qui correspond a
/// `ProjectV2.ID` de la source, lui-meme identique a `ProjectID` de
/// `packages/schema/src/project-id.ts`. On reexporte plutot que de redeclarer
/// pour la meme raison que `src/swarm/project_schema.rs` : deux alias
/// independants du meme contrat peuvent diverger en silence, et cette
/// divergence n'apparaitrait qu'a l'echange avec le TypeScript.
pub use crate::core::session::schema::ProjectId;

/// Une ligne de la table `permission`.
///
/// C'est le type que rend un `select()` sur `PermissionTable` dans
/// `saved.ts`, ligne 43 a 44. Les six champs correspondent, dans l'ordre, aux
/// six colonnes du DDL genere.
///
/// Les `rename` ci-dessous sont identiques aux noms de champ. Ils sont
/// ecrits explicitement malgre cela : ils figent le contrat du DDL, qui est
/// invisible de l'interieur du code Rust, et c'est exactement ce que la
/// relecture doit pouvoir verifier d'un coup d'oeil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRow {
    /// Cle primaire, `text PRIMARY KEY`.
    ///
    /// Le type est marque `PermissionSaved.ID` dans le TypeScript, marque qui
    /// n'existe qu'a la compilation : a l'execution c'est un `String`, d'ou le
    /// type retenu ici.
    ///
    /// Subtilite SQLite a connaitre : `id` est la **seule** colonne du DDL sans
    /// `NOT NULL`, parce que la source n'appelle pas `.notNull()` dessus. Sur une
    /// colonne `TEXT PRIMARY KEY`, SQLite autorise en principe la valeur nulle,
    /// la ligne n'etant pas rejetee comme elle le serait avec `NOT NULL`. Cela ne
    /// se voit jamais en pratique : `saved.ts` ligne 60 appelle `ID.create()`,
    /// qui produit toujours une chaine non vide prefixee de `psv_`. Le type reste
    /// donc `String` et non `Option<String>` : c'est un choix de la couche ORM,
    /// non une obligation du moteur, et le porter en `Option` propagerait une
    /// possibilite que rien dans le depot ne produit.
    #[serde(rename = "id")]
    pub id: String,
    /// `text NOT NULL`, et cle etrangere vers `project.id` avec suppression en
    /// cascade.
    #[serde(rename = "project_id")]
    pub project_id: ProjectId,
    /// `text NOT NULL`. Motif d'action, par exemple `"edit"`.
    #[serde(rename = "action")]
    pub action: String,
    /// `text NOT NULL`. Motif de ressource, par exemple `"src/**"`.
    ///
    /// La colonne accepte la chaine vide : c'est une `TEXT` sans contrainte, et
    /// elle compte comme une ressource distincte pour l'index unique.
    #[serde(rename = "resource")]
    pub resource: String,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_created")]
    pub time_created: i64,
    /// `integer NOT NULL`, en millisecondes depuis l'epoch.
    #[serde(rename = "time_updated")]
    pub time_updated: i64,
}

impl PermissionRow {
    /// Construit une ligne a l'insertion, en positionnant les deux
    /// horodatages sur le meme `now`.
    ///
    /// Le `now` est fourni par l'appelant et non lu dans l'horloge : cette
    /// fonction reste pure et testable. L'appelant peut utiliser
    /// `crate::swarm::util_identifier::now_millis`, qui est le portage de
    /// `Date.now()`, mais ce fichier ne depend volontairement d'aucun autre
    /// module du swarm.
    pub fn new(id: String, project_id: ProjectId, action: String, resource: String, now: i64) -> Self {
        Self { id, project_id, action, resource, time_created: now, time_updated: now }
    }

    /// La cle que cet index unique protege pour cette ligne.
    pub fn key(&self) -> PermissionKey {
        PermissionKey::new(self.project_id.clone(), self.action.clone(), self.resource.clone())
    }

    /// Applique une mise a jour : seul `time_updated` bouge.
    ///
    /// C'est la traduction de `$onUpdate(() => Date.now())`. Le `id`,
    /// `project_id`, `action`, `resource` et `time_created` restent inchanges :
    /// une ligne mise a jour n'est pas une nouvelle permission, et
    /// `time_created` est la date de creation, pas celle du dernier
    /// enregistrement.
    pub fn touch(&mut self, now: i64) {
        self.time_updated = now;
    }
}

/// La cle de l'index unique : `(project_id, action, resource)`.
///
/// L'ordre des champs suit l'ordre des colonnes de l'index, ligne 242 du DDL
/// genere. Il n'est pas indifferent : `Ord`, utilise par [`BTreeSet`],
/// compare d'abord `project_id`, puis `action`, puis `resource`. C'est
/// l'ordre de tri d'un `CREATE INDEX` en SQLite, donc le comportement d'un
/// parcours d'index plus tard.
///
/// Une cle est un **triplet**. Deux lignes qui partagent le projet et la
/// ressource mais pas l'action sont deux cles distinctes, et c'est bien le
/// comportement voulu : autoriser `edit` sur `src/**` n'interdit pas d'y
/// autoriser `bash`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PermissionKey {
    /// Colonne `project_id`. La premiere composante de l'index, et donc le
    /// premier critere de tri.
    pub project_id: ProjectId,
    /// Colonne `action`. La deuxieme composante.
    pub action: String,
    /// Colonne `resource`. La troisieme et derniere composante.
    pub resource: String,
}

impl PermissionKey {
    /// Construit une cle a partir de ses trois colonnes.
    pub fn new(project_id: ProjectId, action: String, resource: String) -> Self {
        Self { project_id, action, resource }
    }
}

/// Premiere cle en double dans une tranche de lignes.
///
/// C'est la violation de l'index unique, vue sur les donnees. Une tranche
/// valide n'en contient aucune : le retour `None` prouve que les lignes sont
/// incoherentes avec le DDL, typiquement parce que l'index n'a pas ete cree
/// ou qu'une ecriture l'a contourne.
///
/// « Premiere » signifie premiere dans l'ordre de la tranche, pas premiere au
/// sens du tri, ce qui rend le resultat stable et dependant uniquement de
/// l'ordre de lecture.
pub fn first_duplicate(rows: &[PermissionRow]) -> Option<PermissionKey> {
    let mut vues: BTreeSet<PermissionKey> = BTreeSet::new();
    for row in rows {
        let cle = row.key();
        if !vues.insert(cle.clone()) {
            return Some(cle);
        }
    }
    None
}

/// Les lignes candidates que l'index unique accepterait reellement.
///
/// C'est la forme pure de la contrainte declaree par ce fichier, et elle
/// repond a la seule question que l'index pose : cette ligne existe-t-elle
/// deja ? Le portage de `saved.ts` pourra s'en servir pour implementer le
/// `onConflictDoNothing` de sa ligne 66, qui fait exactement ce tri.
///
/// L'ordre de sortie est l'ordre d'entree, et un doublon a l'interieur des
/// candidats lui-memes ne laisse passer que sa premiere occurrence. Le
/// contenu des lignes n'est pas compare : seules les trois colonnes de la
/// cle comptent, puisque ce sont les seules indexees.
pub fn insertable_rows<'a>(existing: &[PermissionRow], candidates: &'a [PermissionRow]) -> Vec<&'a PermissionRow> {
    let mut connues: BTreeSet<PermissionKey> = existing.iter().map(PermissionRow::key).collect();
    let mut retenues: Vec<&PermissionRow> = Vec::new();
    for candidat in candidates {
        if connues.insert(candidat.key()) {
            retenues.push(candidat);
        }
    }
    retenues
}

/// Les lignes qui subsistent apres la suppression d'un projet.
///
/// C'est la cascade `ON DELETE CASCADE` de la cle etrangere, vue comme une
/// projection sur une tranche : disparait tout ce qui appartient au projet
/// donne, rien d'autre. L'ordre des lignes restantes est preserve, comme
/// apres un `DELETE` SQL, qui ne reordonne rien.
///
/// Sans `PRAGMA foreign_keys`, SQLite ignore silencieusement cette cascade :
/// la fonction decrit donc l'intention du schema, pas un comportement du
/// moteur. La ligne `session`, `event` et `project_directory` sont construites
/// exactement de la meme facon, si bien qu'un defaut d'activation du pragma
/// affecterait toutes les tables du depot, pas seulement celle-ci.
pub fn rows_kept_after_project_delete<'a>(rows: &'a [PermissionRow], project_id: &str) -> Vec<&'a PermissionRow> {
    rows.iter().filter(|row| row.project_id != project_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(id: &str, projet: &str, action: &str, ressource: &str, now: i64) -> PermissionRow {
        PermissionRow::new(
            id.to_string(),
            projet.to_string(),
            action.to_string(),
            ressource.to_string(),
            now,
        )
    }

    fn cle(projet: &str, action: &str, ressource: &str) -> PermissionKey {
        PermissionKey::new(projet.to_string(), action.to_string(), ressource.to_string())
    }

    #[test]
    fn la_ligne_serialise_les_six_noms_de_colonnes_du_ddl() {
        // Le contrat de la ligne est celui de la base, pas celui du JSON.
        // Une faute sur `project_id` passerait la compilation et casserait le
        // branchement SQL plus tard.
        let l = ligne("psv_1", "prj_1", "edit", "src/**", 1_700_000_000_000);
        let json = serde_json::to_value(&l).unwrap();
        for nom in [COLUMN_ID, COLUMN_PROJECT_ID, COLUMN_ACTION, COLUMN_RESOURCE, COLUMN_TIME_CREATED, COLUMN_TIME_UPDATED] {
            assert!(json.get(nom).is_some(), "la colonne {nom} doit exister dans la ligne serialisee");
        }
        assert_eq!(json.as_object().unwrap().len(), 6, "la table n a que six colonnes");
        assert_eq!(json["id"], "psv_1");
        assert_eq!(json["time_created"], 1_700_000_000_000i64);
    }

    #[test]
    fn la_ligne_ne_serialize_jamais_le_camelcase_du_json_public() {
        // `PermissionSaved.Info` expose `projectID` : le melanger avec la
        // colonne `project_id` est l'erreur invisible de ce fichier.
        let json = serde_json::to_value(ligne("psv_1", "prj_1", "edit", "src/**", 0)).unwrap();
        assert!(json.get("projectID").is_none(), "projectID est le nom du JSON public, pas celui de la colonne");
        assert!(json.get("timeCreated").is_none(), "les horodatages restent en snake_case");
    }

    #[test]
    fn une_ligne_relu_de_json_reprend_les_memes_valeurs() {
        let l = ligne("psv_1", "prj_1", "edit", "src/**", 42);
        let json = serde_json::to_string(&l).unwrap();
        let relue: PermissionRow = serde_json::from_str(&json).unwrap();
        assert_eq!(relue, l);
    }

    #[test]
    fn la_creation_pose_les_deux_horodatages_sur_le_meme_instant() {
        // `time_created` vient du `$default`, `time_updated` n'a pas de
        // `DEFAULT` dans le DDL et doit donc etre fourni. On ecrit `now` dans
        // les deux, choix documente dans l'en-tete du module.
        let l = ligne("psv_1", "prj_1", "edit", "src/**", 7);
        assert_eq!(l.time_created, 7);
        assert_eq!(l.time_updated, 7);
    }

    #[test]
    fn une_mise_a_jour_ne_bouge_que_l_horodatage_de_maj() {
        let mut l = ligne("psv_1", "prj_1", "edit", "src/**", 100);
        l.touch(500);
        assert_eq!(l.time_updated, 500);
        assert_eq!(l.time_created, 100, "la date de creation ne recule jamais");
        assert_eq!(l.id, "psv_1");
        assert_eq!(l.key(), cle("prj_1", "edit", "src/**"), "la cle d index ne change pas");
    }

    #[test]
    fn une_tranche_vide_na_pas_de_doublon_et_n_accepte_rien_de_nouveau() {
        // Cas limite du tout debut : aucune donnee, donc aucune contrainte
        // violate et aucun candidat a inserer.
        let vide: Vec<PermissionRow> = Vec::new();
        assert!(first_duplicate(&vide).is_none());
        assert!(insertable_rows(&vide, &vide).is_empty());
    }

    #[test]
    fn deux_lignes_de_meme_cle_sont_un_doublon_que_lindex_interdit() {
        let tranche = vec![
            ligne("psv_1", "prj_1", "edit", "src/**", 0),
            ligne("psv_2", "prj_1", "edit", "docs/**", 0),
            ligne("psv_3", "prj_1", "edit", "src/**", 0),
        ];
        assert_eq!(first_duplicate(&tranche), Some(cle("prj_1", "edit", "src/**")));
    }

    #[test]
    fn une_tranche_sans_doublon_ne_signale_rien() {
        // Une seule ligne, puis trois lignes de cles distinctes : les deux
        // cas ou l'index est satisfait.
        assert!(first_duplicate(&[ligne("psv_1", "prj_1", "edit", "src/**", 0)]).is_none());
        let tranche = vec![
            ligne("psv_1", "prj_1", "edit", "src/**", 0),
            ligne("psv_2", "prj_1", "edit", "docs/**", 0),
            ligne("psv_3", "prj_2", "edit", "src/**", 0),
            ligne("psv_4", "prj_1", "bash", "src/**", 0),
        ];
        assert!(first_duplicate(&tranche).is_none());
    }

    #[test]
    fn lindex_unique_est_bien_sur_trois_colonnes_et_non_sur_deux() {
        // Meme projet, meme ressource, action differente : deux cles
        // distinctes, donc deux lignes legitimement coexistantes. Si l'index
        // n'avait que deux colonnes, ce cas serait signale a tort.
        let tranche = vec![
            ligne("psv_1", "prj_1", "edit", "src/**", 0),
            ligne("psv_2", "prj_1", "bash", "src/**", 0),
        ];
        assert!(first_duplicate(&tranche).is_none());
        assert_eq!(insertable_rows(&[], &tranche).len(), 2, "les deux actions sont acceptables");
    }

    #[test]
    fn une_ligne_deja_en_base_nest_pas_reinseree() {
        // C'est le `onConflictDoNothing` de `saved.ts`, ici sous forme pure.
        let existant = vec![ligne("psv_1", "prj_1", "edit", "src/**", 0)];
        let candidats = vec![
            ligne("psv_2", "prj_1", "edit", "src/**", 0),
            ligne("psv_3", "prj_1", "edit", "docs/**", 0),
        ];
        let retenues = insertable_rows(&existant, &candidats);
        assert_eq!(retenues.len(), 1);
        assert_eq!(retenues[0].id, "psv_3", "seule la ligne nouvelle passe, et l ordre d entree est conserve");
    }

    #[test]
    fn deux_candidats_identiques_ne_laissent_passer_que_le_premier() {
        // L'index unique porte sur le triplet, donc deux lignes de meme cle
        // et de `id` different ne peuvent pas coexister. C'est le cas limite
        // que l'insertion multiple de `saved.ts` peut produire si un appelant
        // passe deux fois la meme ressource.
        let candidats = vec![
            ligne("psv_1", "prj_1", "edit", "src/**", 0),
            ligne("psv_2", "prj_1", "edit", "src/**", 0),
        ];
        let retenues = insertable_rows(&[], &candidats);
        assert_eq!(retenues.len(), 1);
        assert_eq!(retenues[0].id, "psv_1");
    }

    #[test]
    fn la_suppression_d_un_projet_emporte_toutes_ses_permissions() {
        let tranche = vec![
            ligne("psv_1", "prj_1", "edit", "src/**", 0),
            ligne("psv_2", "prj_1", "bash", "src/**", 0),
            ligne("psv_3", "prj_2", "edit", "src/**", 0),
        ];
        let restantes = rows_kept_after_project_delete(&tranche, "prj_1");
        assert_eq!(restantes.len(), 1);
        assert_eq!(restantes[0].id, "psv_3", "seul un autre projet survit");
        // Un projet inconnu ne supprime rien, comme un DELETE sans effet.
        assert_eq!(rows_kept_after_project_delete(&tranche, "prj_9").len(), 3);
        assert!(rows_kept_after_project_delete(&[], "prj_1").is_empty());
    }

    #[test]
    fn une_ressource_vide_compte_comme_une_ressource_distincte() {
        // La colonne `resource` est une `TEXT` sans contrainte : la chaine
        // vide est une valeur comme une autre, ni absente ni nulle. Attention,
        // elle n'est pas non plus un joker : c'est la chaine vide, qui ne
        // correspond a rien dans `src/permission.rs`.
        let tranche = vec![
            ligne("psv_1", "prj_1", "edit", "", 0),
            ligne("psv_2", "prj_1", "edit", "src/**", 0),
        ];
        assert!(first_duplicate(&tranche).is_none());
        assert_eq!(insertable_rows(&[], &tranche).len(), 2);
        assert_eq!(serde_json::to_value(&tranche[0]).unwrap()["resource"], "");
    }

    #[test]
    fn la_cle_se_trie_dabord_par_projet_puis_action_puis_ressource() {
        // L'ordre de tri suit l'ordre des colonnes de l'index. Il ne change
        // rien a l'unicite, mais il doit rester previsible pour le jour ou
        // l'index sera reellement parcouru.
        let mut cles = vec![
            cle("prj_2", "edit", "a"),
            cle("prj_1", "edit", "b"),
            cle("prj_1", "edit", "a"),
        ];
        cles.sort();
        let attendu = vec![cle("prj_1", "edit", "a"), cle("prj_1", "edit", "b"), cle("prj_2", "edit", "a")];
        assert_eq!(cles, attendu);
    }

    #[test]
    fn les_noms_du_ddl_sont_bien_ceux_du_fichier_genere() {
        // Ces trois chaines ne viennent pas de la lecture du code declaratif
        // mais du DDL genere. Si elles sont justes, le branchement SQL futur
        // n'aura pas a les deviner.
        assert_eq!(TABLE_NAME, "permission");
        assert_eq!(UNIQUE_INDEX_NAME, "permission_project_action_resource_idx");
        assert_eq!(PROJECT_FOREIGN_KEY_NAME, "fk_permission_project_id_project_id_fk");
    }

    #[test]
    fn les_six_colonnes_sont_dans_lordre_du_create_table() {
        // L'ordre du DDL est l'ordre des colonnes du `INSERT` a venir. Le test
        // echoue des qu'une colonne est ajoutee, renommee ou deplacee, ce qui est
        // exactement le moment ou la table doit etre reconsideree.
        assert_eq!(
            COLUMNS,
            ["id", "project_id", "action", "resource", "time_created", "time_updated"]
        );
    }

    #[test]
    fn la_ligne_expose_exactement_les_colonnes_declarees() {
        // Le contrat de la ligne et celui de la table doivent rester le meme
        // ensemble de noms. Sans ce test, un champ ajoute au struct et oublie
        // dans le tableau `COLUMNS` passerait la compilation et casserait au
        // branchement.
        let json = serde_json::to_value(ligne("psv_1", "prj_1", "edit", "src/**", 0)).unwrap();
        let objet = json.as_object().unwrap();
        for nom in COLUMNS {
            assert!(objet.contains_key(nom), "la colonne {nom} doit exister dans la ligne");
        }
        assert_eq!(objet.len(), COLUMNS.len(), "aucune colonne en trop ni en trop peu");
    }

    #[test]
    fn une_ligne_sans_ressource_est_acceptee_par_la_forme_de_la_ligne() {
        // La colonne `resource` est `NOT NULL` mais sans contrainte de contenu :
        // une chaine vide traverse le contrat de la ligne. C'est le contrat SQL
        // qui l'autorise, pas la verification de motifs de `src/permission.rs`.
        let l = ligne("psv_1", "prj_1", "edit", "", 0);
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains(r#""resource":"""#), "la ressource vide doit se lire et se reecrire : {json}");
        assert_eq!(serde_json::from_str::<PermissionRow>(&json).unwrap(), l);
    }

    #[test]
    fn des_candidats_vides_against_une_table_deja_remplie_ne_ajoutent_rien() {
        // Sens inverse du cas limite du tout debut : il y a des donnees, mais
        // rien a ajouter. L'index n'a rien a dire, donc rien ne change.
        let existant = vec![ligne("psv_1", "prj_1", "edit", "src/**", 0)];
        let aucun: Vec<PermissionRow> = Vec::new();
        assert!(insertable_rows(&existant, &aucun).is_empty());
        assert_eq!(existant.len(), 1, "la table n est pas modifiee");
    }
}
