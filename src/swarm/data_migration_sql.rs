//! Portage de `packages/core/src/data-migration.sql.ts`.
//!
//! La source fait six lignes : une seule table Drizzle, `data_migration`,
//! avec deux colonnes et aucune fonction :
//!
//! ```ts
//! export const DataMigrationTable = sqliteTable("data_migration", {
//!   name: text().primaryKey(),
//!   time_completed: integer().notNull(),
//! })
//! ```
//!
//! Ce fichier ne cree aucune table : `Cargo.toml` ne declare aucun pilote
//! SQL. Il porte le contrat observable : le nom de la table, les deux
//! colonnes, leurs contraintes, et la forme de la ligne.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Nom de la table, repris tel quel de la source.
pub const TABLE_NAME: &str = "data_migration";

/// Colonnes de la table, dans l ordre de declaration.
pub const COLUMNS: [&str; 2] = ["name", "time_completed"];

/// Cle primaire declaree : la seule colonne `name`.
pub const PRIMARY_KEY: [&str; 1] = ["name"];

/// Colonnes dont la source ecrit explicitement `notNull()`.
pub const NOT_NULL_DECLARES: [&str; 1] = ["time_completed"];

/// Une ligne de la table `data_migration`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataMigrationRow {
    /// `text PRIMARY KEY`. La source n ecrit pas `notNull()` : voir
    /// [`admet_null_sans_pragma`] pour la lecture de ce cas.
    #[serde(rename = "name")]
    pub name: Option<String>,
    /// `integer NOT NULL`, horodatage de fin de migration.
    #[serde(rename = "time_completed")]
    pub time_completed: i64,
}

impl DataMigrationRow {
    /// Construit une ligne.
    pub fn new(name: Option<String>, time_completed: i64) -> Self {
        Self { name, time_completed }
    }
}

/// Lecture SQLite du couple de drapeaux Drizzle (`notNull` ecrit, `primaryKey`).
///
/// Sans `PRAGMA foreign_keys` ni contrainte `NOT NULL`, une colonne `text
/// PRIMARY KEY` d une table a `rowid` accepte `NULL` : deux `NULL` ne sont pas
/// vus comme egaux pour l unicite. Seul `notNull` ecrit refuse `NULL`.
pub fn admet_null_sans_pragma(not_null_ecrit: bool, _primaire: bool) -> bool {
    !not_null_ecrit
}

/// Premiere valeur `name` en double dans une tranche.
///
/// `None` ne collide jamais avec `None` : voir [`admet_null_sans_pragma`].
/// "Premiere" signifie premiere dans l ordre de la tranche.
pub fn first_duplicate_by_name(rows: &[DataMigrationRow]) -> Option<Option<String>> {
    let mut vues: BTreeSet<&Option<String>> = BTreeSet::new();
    for row in rows {
        if !vues.insert(&row.name) {
            return Some(row.name.clone());
        }
    }
    None
}

/// Les lignes triables par `time_completed` croissant, sans reordonner les
/// ex-aequo (tri stable, comme un `ORDER BY time_completed`).
pub fn trier_par_fin<'a>(rows: &[&'a DataMigrationRow]) -> Vec<&'a DataMigrationRow> {
    let mut copie: Vec<&DataMigrationRow> = rows.to_vec();
    copie.sort_by_key(|row| row.time_completed);
    copie
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(nom: Option<&str>, fini: i64) -> DataMigrationRow {
        DataMigrationRow::new(nom.map(str::to_string), fini)
    }

    #[test]
    fn le_nom_de_table_et_les_colonnes_sont_ceux_de_la_source() {
        assert_eq!(TABLE_NAME, "data_migration");
        assert_eq!(COLUMNS, ["name", "time_completed"]);
        assert_eq!(PRIMARY_KEY, ["name"]);
        assert_eq!(NOT_NULL_DECLARES, ["time_completed"]);
    }

    #[test]
    fn la_cle_primaire_ne_figure_pas_dans_les_not_null_ecrits() {
        for nom in PRIMARY_KEY {
            assert!(COLUMNS.contains(&nom));
        }
        assert!(!NOT_NULL_DECLARES.contains(&"name"));
    }

    #[test]
    fn la_colonne_name_admet_null_faute_de_not_null_ecrit() {
        assert!(admet_null_sans_pragma(false, true));
        assert!(!admet_null_sans_pragma(true, false));
    }

    #[test]
    fn la_ligne_serialise_exactement_deux_colonnes_en_snake_case() {
        let json = serde_json::to_value(ligne(Some("seed"), 7)).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert_eq!(json["name"], "seed");
        assert_eq!(json["time_completed"], 7);
        assert!(json.get("timeCompleted").is_none());
    }

    #[test]
    fn un_nom_nul_reste_un_json_null_distinct_d_une_colonne_absente() {
        let json = serde_json::to_value(ligne(None, 0)).unwrap();
        assert!(json.get("name").is_some());
        assert!(json["name"].is_null());
        let relue: DataMigrationRow = serde_json::from_value(json).unwrap();
        assert_eq!(relue.name, None);
    }

    #[test]
    fn une_ligne_relu_de_json_reprend_les_memes_valeurs() {
        let l = ligne(Some("m1"), 1_700_000_000_000);
        let relue: DataMigrationRow = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(relue, l);
    }

    #[test]
    fn deux_lignes_de_meme_nom_sont_un_doublon() {
        let tranche = vec![ligne(Some("a"), 1), ligne(Some("b"), 2), ligne(Some("a"), 3)];
        assert_eq!(first_duplicate_by_name(&tranche), Some(Some("a".to_string())));
    }

    #[test]
    fn deux_lignes_sans_nom_ne_sont_pas_un_doublon() {
        let tranche = vec![ligne(None, 1), ligne(None, 2)];
        assert_eq!(first_duplicate_by_name(&tranche), None);
    }

    #[test]
    fn une_tranche_sans_doublon_ni_ligne_ne_signale_rien() {
        let vide: Vec<DataMigrationRow> = Vec::new();
        assert_eq!(first_duplicate_by_name(&vide), None);
        assert_eq!(first_duplicate_by_name(&[ligne(Some("a"), 1)]), None);
    }

    #[test]
    fn le_tri_par_fin_ordonne_sans_reordonner_les_ex_aequo() {
        let a = ligne(Some("a"), 30);
        let b = ligne(Some("b"), 10);
        let c = ligne(Some("c"), 10);
        let triees = trier_par_fin(&[&a, &b, &c]);
        assert_eq!(triees[0].name.as_deref(), Some("b"));
        assert_eq!(triees[1].name.as_deref(), Some("c"));
        assert_eq!(triees[2].name.as_deref(), Some("a"));
    }
}
