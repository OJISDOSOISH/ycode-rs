//! Portage de `packages/core/src/event/sql.ts`.
//!
//! ## Ce que contient la source
//!
//! Vingt-cinq lignes, deux exports, et **aucune fonction**. Le fichier declare
//! deux tables Drizzle et deux index, rien d autre :
//!
//! ```ts
//! export const EventSequenceTable = sqliteTable("event_sequence", {
//!   aggregate_id: text().notNull().primaryKey(),
//!   seq: integer().notNull(),
//!   owner_id: text(),
//! })
//!
//! export const EventTable = sqliteTable("event", { ... }, (table) => [
//!   uniqueIndex("event_aggregate_seq_idx").on(table.aggregate_id, table.seq),
//!   index("event_aggregate_type_seq_idx").on(table.aggregate_id, table.type, table.seq),
//! ])
//! ```
//!
//! Il n y a donc aucune logique metier a isoler. La source ne contient pas
//! davantage de logique dans ses details : elle ne lit pas, elle n ecrit pas, elle
//! ne trie pas. Ce qu elle fait, c est *decrire*.
//!
//! ## Ce qui n a pas de traduction ici, et pourquoi
//!
//! Tout l appareil de Drizzle est un generateur de SQL. Il transforme une
//! declaration TypeScript en `CREATE TABLE` et en `CREATE INDEX` au moment ou
//! la migration est produite. Aucun equivalent n existe du cote Rust dans ce
//! lot, pour une raison simple : **`Cargo.toml` ne declare aucun crate SQL**
//! (ni `sqlx`, ni `rusqlite`, ni `diesel`, ni `libsql`). Le crate de
//! dependances ne peut donc ni ouvrir de base, ni produire de DDL.
//!
//! Concretement, ces fragments de la source n ont pas de traduction, et il ne
//! faut pas en inventer :
//!
//! - `sqliteTable`, `text`, `integer`, `index`, `uniqueIndex` : le constructeur
//!   de requetes et le producteur de migrations de Drizzle.
//! - `.notNull()`, `.primaryKey()`, `.references(..., { onDelete: "cascade" })` :
//!   chainage de contraintes au niveau du generateur. Ce sont des
//!   *invariants*, pas des instructions ; ils sont repris plus bas comme des
//!   donnees, ce qui est la seule chose honnete a faire sans pilote.
//! - `text({ mode: "json" })` : le mode de serialisation d une colonne texte
//!   Drizzle. Repris comme une donnee, colonne par colonne.
//! - `.$type<EventV2.ID>()` et `.$type<Record<string, unknown>>()` : types
//!   fantomes, **effet nul a l execution**. Ils documentent le contenu attendu
//!   et disparaissent a la compilation. `EventV2.ID` vit dans
//!   `packages/core/src/event`, qui n est pas porte par ce fichier : le type
//!   Rust de l identifiant d evenement n est donc pas defini ici, il viendra du
//!   portage de ce module-la.
//!
//! **Aucun DDL n est ecrit dans ce fichier.** Ni `CREATE TABLE`, ni
//! `CREATE INDEX`, ni squelette de migration, ni chaine SQL, ni couche
//! d interrogation. La decision du projet est que la couche SQL sera branchee
//! plus tard, par quelqu un d autre. Ce fichier ne la prejuge pas et ne la
//! double pas.
//!
//! ## Ce qui est porte, et pourquoi ce n est pas de la fabrication
//!
//! Deux choses, et deux seulement.
//!
//! 1. **La declaration elle-meme, en donnees pures.** Noms des tables, noms des
//!    colonnes dans leur ordre de declaration, type de stockage, nullabilite,
//!    cle primaire, mode json, cle etrangere et sa regle de suppression, noms
//!    et colonnes des deux index. C est une transcription exacte de ce que la
//!    source declare, sous une forme que l on peut interroger sans base de
//!    donnees. Le jour ou la couche SQL arrive, c est cette description qu elle
//!    pourra consommer, ou qu elle remplacera.
//!
//! 2. **Les invariants que ces declarations imposent, en fonctions pures** sur
//!    une tranche de donnees. Un index unique, une cle etrangere et une
//!    regle `on delete cascade` sont des contraintes. La base les applique ; en
//!    son absence, on peut en revanche ecrire ce qu elles *signifient*, et le
//!    tester sur un `&[EventRow]`. Trois fonctions, trois contraintes :
//!
//!    - `conflits_de_sequence` : ce que l index unique sur
//!      `(aggregate_id, seq)` interdit.
//!    - `agregats_orphelins` : ce que la cle etrangere vers `event_sequence`
//!      interdit, avant que la base ne le refuse a l ecriture.
//!    - `evenements_survivants_a_la_suppression` : ce que `on delete cascade`
//!      produit sur la table enfant.
//!
//!    Rien de plus. Aucune fonction n invente de regle qui ne soit pas ecrite
//!    dans la source.
//!
//! ## Noms de champs
//!
//! Cas exceptionel pour ce portage : les noms de colonnes sont en **snake_case**
//! (`aggregate_id`, `owner_id`), et non en camelCase. Aucun `#[serde(rename)]`
//! n est donc requis pour elles. Le seul nom qui pose probleme est `type`, qui
//! est un mot cle Rust : le champ est appele `event_type` et porte un
//! `#[serde(rename = "type")]`, qui le fait ressortir sous le nom exact de la
//! colonne. Ce renommage est donc la seule subtilite du fichier, et le test
//! `le_champ_type_sort_du_cote_json_sous_le_nom_type` le verrouille.
//!
//! Un mot sur la nullabilite, car c est le piege du fichier : `notNull()`
//! interdit `NULL`, **pas la chaine vide**. Une ligne dont `aggregate_id` vaut
//! `""` est donc parfaitement recevable, et ce fichier l accepte. Le
//! comportement inverse, traiter la chaine vide comme un absent, casserait le
//! portage : c est le piege `?` contre `??` du projet, transpose ici.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

// ------------------------------------------------------------------ la declaration

/// Type de stockage physique d une colonne, dans le vocabulaire SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// `text()` : chaine de caractere, stockage texte.
    Text,
    /// `integer()` : entier signe 64 bits, stockage entier.
    Integer,
}

/// Mode de serialisation d une colonne texte Drizzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    /// `text()` : la valeur est une chaine, telle quelle.
    Plain,
    /// `text({ mode: "json" })` : la valeur est un objet JSON, serialise dans la
    /// colonne texte par le pilote.
    Json,
}

/// Regle appliquee quand la ligne referencee est supprimee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDelete {
    /// `onDelete: "cascade"` : les lignes enfants partent avec la ligne mere.
    Cascade,
}

/// Cle etrangere portee par une colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignKey {
    /// Table referencee, telle que nommee dans `sqliteTable`.
    pub target_table: &'static str,
    /// Colonne referencee dans la table referencee.
    pub target_column: &'static str,
    /// Regle de suppression, seule regle ecrite dans la source.
    pub on_delete: OnDelete,
}

/// Une colonne, avec les contraintes declarees autour d elle.
///
/// `nullable` vaut `false` quand la source ecrit `notNull()`, et aussi pour la
/// colonne qui porte `primaryKey()` : dans Drizzle, sur SQLite, `primaryKey()`
/// implique NOT NULL. C est le seul implicite assume par ce fichier, la source
/// n ecrivant pas les deux sur la colonne `id`. Voir le POINT FAIBLE du
/// rapport de portage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnDef {
    /// Nom de la colonne, identique a la cle de l objet de la source.
    pub name: &'static str,
    /// Type de stockage.
    pub storage: Storage,
    /// `true` si la colonne admet `NULL`, c est a dire si `notNull()` manque.
    pub nullable: bool,
    /// `true` si la colonne porte la cle primaire de sa table.
    pub primary_key: bool,
    /// Mode de serialisation pour une colonne texte.
    pub mode: TextMode,
    /// Cle etrangere eventuelle, portee par `references()`.
    pub references: Option<ForeignKey>,
}

/// Un index declare dans le troisieme argument de `sqliteTable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexDef {
    /// Nom de l index dans la base. Le suffixe `_idx` est une convention
    /// Drizzle, il ne fait pas partie du nom logique.
    pub name: &'static str,
    /// `true` pour `uniqueIndex`, `false` pour `index`.
    pub unique: bool,
    /// Colonnes indexees, **dans l ordre de la source** : l ordre fait partie
    /// de l index, il n est pas cosmetique.
    pub columns: &'static [&'static str],
}

/// Une table, telle que `sqliteTable` la decrit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableDef {
    /// Nom de la table dans la base, premier argument de `sqliteTable`.
    pub name: &'static str,
    /// Colonnes, dans l ordre de declaration de l objet de la source.
    pub columns: &'static [ColumnDef],
}

impl TableDef {
    /// Cherche une colonne par son nom, comme le fait le type de colonne
    /// Drizzle a l usage du schema.
    ///
    /// La recherche est insensible a la casse ? Non : elle est exacte, comme en
    /// SQL. Une colonne absente donne `None`.
    pub fn colonne(&self, nom: &str) -> Option<&'static ColumnDef> {
        let colonnes: &'static [ColumnDef] = self.columns;
        colonnes.iter().find(|colonne| colonne.name == nom)
    }

    /// Les noms de colonnes, dans l ordre de declaration.
    pub fn noms_de_colonnes(&self) -> Vec<&'static str> {
        self.columns.iter().map(|colonne| colonne.name).collect()
    }
}

/// Table `event_sequence` : une ligne par agregat, qui porte son dernier numero
/// de sequence et son proprietaire eventuel.
pub const TABLE_EVENT_SEQUENCE: TableDef = TableDef {
    name: "event_sequence",
    columns: &[
        ColumnDef {
            name: "aggregate_id",
            storage: Storage::Text,
            nullable: false,
            primary_key: true,
            mode: TextMode::Plain,
            references: None,
        },
        ColumnDef {
            name: "seq",
            storage: Storage::Integer,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        ColumnDef {
            name: "owner_id",
            storage: Storage::Text,
            nullable: true,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
    ],
};

/// Table `event` : une ligne par evenement, range par agregat puis par numero
/// de sequence.
pub const TABLE_EVENT: TableDef = TableDef {
    name: "event",
    columns: &[
        ColumnDef {
            name: "id",
            storage: Storage::Text,
            nullable: false,
            primary_key: true,
            mode: TextMode::Plain,
            references: None,
        },
        ColumnDef {
            name: "aggregate_id",
            storage: Storage::Text,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: Some(ForeignKey {
                target_table: "event_sequence",
                target_column: "aggregate_id",
                on_delete: OnDelete::Cascade,
            }),
        },
        ColumnDef {
            name: "seq",
            storage: Storage::Integer,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        ColumnDef {
            name: "type",
            storage: Storage::Text,
            nullable: false,
            primary_key: false,
            mode: TextMode::Plain,
            references: None,
        },
        ColumnDef {
            name: "data",
            storage: Storage::Text,
            nullable: false,
            primary_key: false,
            mode: TextMode::Json,
            references: None,
        },
    ],
};

/// Les deux index de la table `event`, dans l ordre du tableau de la source.
///
/// Le premier est unique et porte sur le couple `(aggregate_id, seq)`, ce qui
/// rend la position d un evenement dans son agregat unique. Le second n est pas
/// unique et ajoute `type` entre les deux : il sert a relire un sous ensemble
/// d evenements d un type donne, par agregat, dans l ordre du sequence.
pub const EVENT_INDEXES: [IndexDef; 2] = [
    IndexDef {
        name: "event_aggregate_seq_idx",
        unique: true,
        columns: &["aggregate_id", "seq"],
    },
    IndexDef {
        name: "event_aggregate_type_seq_idx",
        unique: false,
        columns: &["aggregate_id", "type", "seq"],
    },
];

// ------------------------------------------------------------------ les formes de lignes

/// La forme d une ligne de `event_sequence`, ecrite a plat pour etre testable
/// sans pilote.
///
/// Ce n est pas le type `EventSequence` du schema amont : c est la ligne telle
/// qu elle sort de la table, et son seul but ici est de donner une tranche de
/// donnees aux fonctions pures du fichier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceRow {
    /// Cle primaire de la table, et cible de la cle etrangere de `event`.
    pub aggregate_id: String,
    /// Dernier numero de sequence connu pour cet agregat.
    pub seq: i64,
    /// `text()` sans `notNull()` : la colonne admet l absence de valeur.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

/// La forme d une ligne de `event`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    /// `.$type<EventV2.ID>()` : un identifiant d evenement. Le type exact
    /// vient du portage de `packages/core/src/event`, il n est pas pose ici.
    pub id: String,
    /// Reference `event_sequence.aggregate_id`, `notNull`.
    pub aggregate_id: String,
    /// Numero de l evenement dans son agregat, `notNull`.
    pub seq: i64,
    /// Colonne `type`, renommee car `type` est un mot cle Rust.
    #[serde(rename = "type")]
    pub event_type: String,
    /// `text({ mode: "json" }).$type<Record<string, unknown>>()` : un objet
    /// JSON, donc jamais un simple nombre, jamais un tableau, jamais absent.
    pub data: serde_json::Value,
}

// ------------------------------------------------------------------ les invariants

/// Dit si une valeur tient dans la colonne `data`, c est a dire si elle est un
/// objet JSON, comme l impose `Record<string, unknown>`.
///
/// Un objet vide est valide : `{}` est bien un `Record` sans propriete. Un
/// tableau, une chaine, un nombre, un booleen ou `null` ne le sont pas.
pub fn donnee_est_un_objet_json(valeur: &serde_json::Value) -> bool {
    valeur.is_object()
}

/// Positions, dans l ordre, des lignes dont la colonne `data` n est pas un
/// objet JSON.
///
/// C est la seule regle que ce fichier verifie sur la forme d une ligne. Il ne
/// verifie **pas** la chaine vide : `notNull` interdit `NULL`, pas `""`.
pub fn positions_avec_donnee_invalide(evenements: &[EventRow]) -> Vec<usize> {
    evenements
        .iter()
        .enumerate()
        .filter(|(_, evenement)| !donnee_est_un_objet_json(&evenement.data))
        .map(|(position, _)| position)
        .collect()
}

/// Couples `(aggregate_id, seq)` utilises plus d une fois, donc refuses par
/// l index unique `event_aggregate_seq_idx`.
///
/// Les couples sont rendus tries par agregat puis par numero, donc deux
/// executions sur la meme tranche rendent la meme liste. Un couple n apparait
/// qu une fois, quel que soit le nombre de repetitions.
pub fn conflits_de_sequence(evenements: &[EventRow]) -> Vec<(String, i64)> {
    let mut occurrences: BTreeMap<(&str, i64), usize> = BTreeMap::new();
    for evenement in evenements {
        *occurrences
            .entry((evenement.aggregate_id.as_str(), evenement.seq))
            .or_insert(0) += 1;
    }
    occurrences
        .into_iter()
        .filter(|(_, nombre)| *nombre > 1)
        .map(|((aggregate_id, seq), _)| (aggregate_id.to_string(), seq))
        .collect()
}

/// Agregats dont les evenements n ont pas de ligne correspondante dans
/// `event_sequence`, donc refuses par la cle etrangere.
///
/// La liste est triee et sans repetition. Une ligne de `event_sequence` qui n a
/// aucun evenement, elle, n est pas signalee : rien dans la source ne l
/// interdit, et une table fille peut etre vide.
pub fn agregats_orphelins(evenements: &[EventRow], sequences: &[SequenceRow]) -> Vec<String> {
    let connus: BTreeSet<&str> = sequences
        .iter()
        .map(|sequence| sequence.aggregate_id.as_str())
        .collect();
    let mut orphelins: BTreeSet<&str> = BTreeSet::new();
    for evenement in evenements {
        if !connus.contains(evenement.aggregate_id.as_str()) {
            orphelins.insert(evenement.aggregate_id.as_str());
        }
    }
    orphelins.into_iter().map(str::to_string).collect()
}

/// Lignes de `event_sequence` qui n ont aucun evenement.
///
/// Information, pas violation : la cle etrangere va dans l autre sens, de
/// `event` vers `event_sequence`, et elle n impose rien sur la table mere.
pub fn sequences_sans_evenement(
    sequences: &[SequenceRow],
    evenements: &[EventRow],
) -> Vec<String> {
    let couverts: BTreeSet<&str> = evenements
        .iter()
        .map(|evenement| evenement.aggregate_id.as_str())
        .collect();
    sequences
        .iter()
        .filter(|sequence| !couverts.contains(sequence.aggregate_id.as_str()))
        .map(|sequence| sequence.aggregate_id.clone())
        .collect()
}

/// Lignes de `event` qui restent apres la suppression de la ligne de
/// `event_sequence` portant `aggregate_id`.
///
/// C est la traduction de `onDelete: "cascade"` : la suppression de l agregat
/// emporte ses evenements. La suppression d un agregat inconnu ne change rien,
/// comme en base.
pub fn evenements_survivants_a_la_suppression(
    evenements: &[EventRow],
    aggregate_id: &str,
) -> Vec<EventRow> {
    evenements
        .iter()
        .filter(|evenement| evenement.aggregate_id != aggregate_id)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un evenement bien forme, pour etre sur que les tests qui portent sur les
    /// contraintes ne testent pas, en meme temps, la forme de la ligne.
    fn evenement(aggregate_id: &str, seq: i64) -> EventRow {
        EventRow {
            id: format!("evt_{}_{}", aggregate_id, seq),
            aggregate_id: aggregate_id.to_string(),
            seq,
            event_type: "message.part.updated".to_string(),
            data: serde_json::json!({ "part": "text" }),
        }
    }

    /// Une ligne de sequencement qui porte `aggregate_id`.
    fn sequence(aggregate_id: &str, seq: i64) -> SequenceRow {
        SequenceRow {
            aggregate_id: aggregate_id.to_string(),
            seq,
            owner_id: Some("own_1".to_string()),
        }
    }

    #[test]
    fn la_table_de_sequencement_expose_ses_trois_colonnes_dans_leur_ordre_de_declaration() {
        assert_eq!(
            TABLE_EVENT_SEQUENCE.noms_de_colonnes(),
            vec!["aggregate_id", "seq", "owner_id"]
        );
    }

    #[test]
    fn la_table_event_expose_ses_cinq_colonnes_dans_leur_ordre_de_declaration() {
        assert_eq!(
            TABLE_EVENT.noms_de_colonnes(),
            vec!["id", "aggregate_id", "seq", "type", "data"]
        );
    }

    #[test]
    fn la_cle_primaire_de_l_evenement_est_son_identifiant_et_celle_du_sequencement_est_son_agregat() {
        assert!(TABLE_EVENT.colonne("id").expect("colonne id").primary_key);
        assert!(TABLE_EVENT_SEQUENCE
            .colonne("aggregate_id")
            .expect("colonne aggregate_id")
            .primary_key);

        // Aucune autre colonne ne porte la cle primaire.
        let autres: Vec<&str> = TABLE_EVENT
            .columns
            .iter()
            .filter(|colonne| colonne.primary_key && colonne.name != "id")
            .map(|colonne| colonne.name)
            .collect();
        assert!(autres.is_empty(), "cle primaire en trop : {:?}", autres);
    }

    #[test]
    fn une_colonne_absente_d_une_table_renvoie_rien() {
        assert!(TABLE_EVENT.colonne("inconnue").is_none());
        // Le nom de colonne existe dans l autre table, mais pas dans celle-ci.
        assert!(TABLE_EVENT.colonne("owner_id").is_none());
    }

    #[test]
    fn la_colonne_owner_id_est_la_seule_du_schema_a_admettre_l_absence_de_valeur() {
        let nullables: Vec<&str> = TABLE_EVENT
            .columns
            .iter()
            .chain(TABLE_EVENT_SEQUENCE.columns.iter())
            .filter(|colonne| colonne.nullable)
            .map(|colonne| colonne.name)
            .collect();
        assert_eq!(nullables, vec!["owner_id"]);
    }

    #[test]
    fn la_colonne_data_est_la_seule_colonne_json_et_les_autres_sont_du_texte_ou_de_l_entier() {
        let json: Vec<&str> = TABLE_EVENT
            .columns
            .iter()
            .filter(|colonne| colonne.mode == TextMode::Json)
            .map(|colonne| colonne.name)
            .collect();
        assert_eq!(json, vec!["data"]);

        // Le stockage suit le type de la source : texte partout, sauf `seq`.
        assert_eq!(
            TABLE_EVENT.colonne("seq").expect("colonne seq").storage,
            Storage::Integer
        );
        assert_eq!(
            TABLE_EVENT.colonne("data").expect("colonne data").storage,
            Storage::Text
        );
    }

    #[test]
    fn la_cible_de_la_cle_etrangere_est_l_agregat_du_sequencement_et_la_suppression_part_en_cascade() {
        let cle = TABLE_EVENT
            .colonne("aggregate_id")
            .expect("colonne aggregate_id")
            .references
            .expect("cle etrangere declaree en amont");
        assert_eq!(cle.target_table, "event_sequence");
        assert_eq!(cle.target_column, "aggregate_id");
        assert_eq!(cle.on_delete, OnDelete::Cascade);

        // Aucune autre colonne de la table ne porte de cle etrangere, et la
        // table de sequencement n en porte aucune non plus.
        let autres: Vec<&str> = TABLE_EVENT
            .columns
            .iter()
            .chain(TABLE_EVENT_SEQUENCE.columns.iter())
            .filter(|colonne| colonne.references.is_some())
            .map(|colonne| colonne.name)
            .collect();
        assert_eq!(autres, vec!["aggregate_id"]);
    }

    #[test]
    fn les_deux_index_ne_portent_pas_sur_les_memes_colonnes_et_l_unique_est_le_premier() {
        assert!(EVENT_INDEXES[0].unique, "le premier index est le unique");
        assert_eq!(EVENT_INDEXES[0].name, "event_aggregate_seq_idx");
        assert_eq!(EVENT_INDEXES[0].columns, &["aggregate_id", "seq"]);

        assert!(
            !EVENT_INDEXES[1].unique,
            "le second index n impose rien sur l unicite"
        );
        assert_eq!(EVENT_INDEXES[1].name, "event_aggregate_type_seq_idx");
        assert_eq!(
            EVENT_INDEXES[1].columns,
            &["aggregate_id", "type", "seq"]
        );

        assert_ne!(
            EVENT_INDEXES[0].columns, EVENT_INDEXES[1].columns,
            "l ordre des colonnes distingue les deux index"
        );
    }

    #[test]
    fn l_index_unique_interdit_deux_evenements_de_meme_agregat_et_de_meme_numero() {
        let evenements = vec![evenement("ses_1", 1), evenement("ses_1", 1)];
        assert_eq!(conflits_de_sequence(&evenements), vec![("ses_1".to_string(), 1)]);
    }

    #[test]
    fn un_meme_numero_dans_deux_agregats_differents_ne_entre_pas_en_conflit() {
        // Meme `seq`, deux `aggregate_id` : le couple est different, donc
        // l index unique est respecte.
        let evenements = vec![evenement("ses_1", 1), evenement("ses_2", 1)];
        assert!(conflits_de_sequence(&evenements).is_empty());
    }

    #[test]
    fn des_numeros_differents_dans_le_meme_agregat_ne_rientrent_pas_en_conflit() {
        let evenements = vec![evenement("ses_1", 1), evenement("ses_1", 2)];
        assert!(conflits_de_sequence(&evenements).is_empty());
    }

    #[test]
    fn les_conflits_sont_tries_et_ne_sont_comptes_qu_une_seule_fois() {
        let evenements = vec![
            evenement("ses_2", 3),
            evenement("ses_1", 7),
            evenement("ses_2", 3),
            evenement("ses_1", 7),
            evenement("ses_1", 7),
        ];
        assert_eq!(
            conflits_de_sequence(&evenements),
            vec![("ses_1".to_string(), 7), ("ses_2".to_string(), 3)]
        );
    }

    #[test]
    fn une_tranche_vide_ne_declenche_ni_conflit_ni_orphelin() {
        let vide: Vec<EventRow> = Vec::new();
        assert!(conflits_de_sequence(&vide).is_empty());
        assert!(positions_avec_donnee_invalide(&vide).is_empty());
        assert!(agregats_orphelins(&vide, &[]).is_empty());
    }

    #[test]
    fn un_seul_evenement_bien_forme_ne_declenche_aucun_probleme() {
        let evenements = vec![evenement("ses_1", 1)];
        assert!(conflits_de_sequence(&evenements).is_empty());
        assert!(positions_avec_donnee_invalide(&evenements).is_empty());
        assert!(agregats_orphelins(&evenements, &[sequence("ses_1", 1)]).is_empty());
        assert!(sequences_sans_evenement(&[sequence("ses_1", 1)], &evenements).is_empty());
    }

    #[test]
    fn un_evenement_dont_l_agregat_est_absent_du_sequencement_est_signale_comme_orphelin() {
        let evenements = vec![evenement("ses_1", 1), evenement("ses_2", 1)];
        let sequences = vec![sequence("ses_1", 1)];
        assert_eq!(agregats_orphelins(&evenements, &sequences), vec!["ses_2".to_string()]);
    }

    #[test]
    fn une_sequence_sans_evenement_est_lisible_mais_n_est_pas_une_erreur() {
        // La cle etrangere va de `event` vers `event_sequence` : une table mere
        // vide reste valide, et rien ici ne doit la signaler comme un conflit.
        let sequences = vec![sequence("ses_1", 1), sequence("ses_2", 1)];
        let evenements = vec![evenement("ses_1", 1)];
        assert_eq!(
            sequences_sans_evenement(&sequences, &evenements),
            vec!["ses_2".to_string()]
        );
        assert!(conflits_de_sequence(&evenements).is_empty());
    }

    #[test]
    fn la_suppression_d_un_agregat_emporte_tous_ses_evenements() {
        let evenements = vec![
            evenement("ses_1", 1),
            evenement("ses_1", 2),
            evenement("ses_2", 1),
        ];
        let survivants = evenements_survivants_a_la_suppression(&evenements, "ses_1");
        assert_eq!(survivants.len(), 1);
        assert_eq!(survivants[0].aggregate_id, "ses_2");
        assert_eq!(survivants[0].seq, 1);
    }

    #[test]
    fn la_suppression_d_un_agregat_absent_ne_change_rien() {
        let evenements = vec![evenement("ses_1", 1), evenement("ses_2", 1)];
        let survivants = evenements_survivants_a_la_suppression(&evenements, "ses_9");
        assert_eq!(survivants, evenements);
    }

    #[test]
    fn la_suppression_du_dernier_agregat_laisse_une_liste_vide() {
        let evenements = vec![evenement("ses_1", 1)];
        assert!(
            evenements_survivants_a_la_suppression(&evenements, "ses_1").is_empty()
        );
    }

    #[test]
    fn un_objet_json_vide_est_accepte_mais_un_ou_un_tableau_sont_refuses() {
        assert!(donnee_est_un_objet_json(&serde_json::json!({})));
        assert!(donnee_est_un_objet_json(&serde_json::json!({ "a": 1 })));

        assert!(!donnee_est_un_objet_json(&serde_json::json!([])));
        assert!(!donnee_est_un_objet_json(&serde_json::json!([1, 2])));
        assert!(!donnee_est_un_objet_json(&serde_json::json!("texte")));
        assert!(!donnee_est_un_objet_json(&serde_json::json!(1)));
        assert!(!donnee_est_un_objet_json(&serde_json::json!(true)));
        assert!(!donnee_est_un_objet_json(&serde_json::Value::Null));
    }

    #[test]
    fn la_position_de_la_ou_des_lignes_invalides_est_rendue_dans_leur_ordre() {
        let mut premiere = evenement("ses_1", 1);
        premiere.data = serde_json::json!([1, 2]);
        let mut troisieme = evenement("ses_1", 3);
        troisieme.data = serde_json::Value::Null;

        let evenements = vec![premiere, evenement("ses_1", 2), troisieme];
        assert_eq!(positions_avec_donnee_invalide(&evenements), vec![0, 2]);
    }

    #[test]
    fn une_chaine_vide_reste_valide_car_l_absence_de_valeur_est_interdite_mais_pas_la_chaine_vide() {
        // Piege `notNull` contre chaine vide. En SQL, `not null` interdit `NULL`,
        // il n interdit pas `""`. Une ligne dont les identifiants sont vides
        // reste donc acceptable ici, et surtout pas traitee comme un absent.
        let evenement = EventRow {
            id: String::new(),
            aggregate_id: String::new(),
            seq: 0,
            event_type: String::new(),
            data: serde_json::json!({}),
        };
        let evenements = vec![evenement];
        assert!(positions_avec_donnee_invalide(&evenements).is_empty());
        assert!(conflits_de_sequence(&evenements).is_empty());
        // Et la cle etrangere est satisfaite des lors qu une ligne de
        // sequencement porte cette chaine vide.
        let sequences = vec![SequenceRow {
            aggregate_id: String::new(),
            seq: 0,
            owner_id: None,
        }];
        assert!(agregats_orphelins(&evenements, &sequences).is_empty());
    }

    #[test]
    fn le_champ_type_sort_du_cote_json_sous_le_nom_type_et_pas_sous_event_type() {
        let evenement = evenement("ses_1", 1);
        let json = serde_json::to_value(&evenement).expect("serialisation");
        assert!(
            json.get("type").is_some(),
            "le nom de colonne doit rester type"
        );
        assert!(json.get("event_type").is_none());

        // Et les autres colonnes gardent leur nom, sans renommage cache.
        for nom in ["id", "aggregate_id", "seq", "data"] {
            assert!(json.get(nom).is_some(), "colonne manquante : {}", nom);
        }

        let relu: EventRow = serde_json::from_value(json).expect("deserialisation");
        assert_eq!(relu, evenement);
    }

    #[test]
    fn un_owner_id_absent_disparait_du_json_au_lieu_de_devenir_null() {
        let sans_proprietaire = SequenceRow {
            aggregate_id: "ses_1".to_string(),
            seq: 3,
            owner_id: None,
        };
        let json = serde_json::to_string(&sans_proprietaire).expect("serialisation");
        assert_eq!(json, "{\"aggregate_id\":\"ses_1\",\"seq\":3}");

        let avec_proprietaire = sequence("ses_1", 3);
        let json = serde_json::to_string(&avec_proprietaire).expect("serialisation");
        assert_eq!(json, "{\"aggregate_id\":\"ses_1\",\"seq\":3,\"owner_id\":\"own_1\"}");
    }
}
