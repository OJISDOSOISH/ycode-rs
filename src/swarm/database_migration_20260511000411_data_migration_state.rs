//! Portage de `packages/core/src/database/migration/20260511000411_data_migration_state.ts`.
//!
//! La source tient seize lignes : un identifiant et un seul `CREATE TABLE`
//! qui cree la table de suivi des migrations de donnees :
//!
//! ```sql
//! CREATE TABLE `data_migration` (
//!   `name` text PRIMARY KEY,
//!   `time_completed` integer NOT NULL
//! );
//! ```
//!
//! Comme `Cargo.toml` ne declare aucun pilote SQL, ce fichier ne touche a
//! aucune base ; il porte l'identifiant, l'instruction, la forme de la ligne
//! et les invariants de la table (unicite du nom, horodatage obligatoire),
//! sous forme de donnees pures testables sans base.

use std::collections::BTreeSet;

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260511000411_data_migration_state";

/// Nom de la table creee.
pub const TABLE_DATA_MIGRATION: &str = "data_migration";

/// Les colonnes de la table, dans l'ordre du `CREATE TABLE`.
pub const COLONNES: [&str; 2] = ["name", "time_completed"];

/// L'unique instruction de `up`.
pub const CREATE_TABLE_SQL: &str = "CREATE TABLE `data_migration` (`name` text PRIMARY KEY, `time_completed` integer NOT NULL);";

/// Les instructions SQL de `up`, dans leur ordre d'execution.
pub const STATEMENTS: [&str; 1] = [CREATE_TABLE_SQL];

/// L'identifiant de la migration.
pub fn id() -> &'static str {
    MIGRATION_ID
}

/// Les instructions SQL, dans l'ordre d'execution.
pub fn instructions() -> &'static [&'static str] {
    &STATEMENTS
}

/// Le nombre d'instructions executees par `up`.
pub fn nombre_instructions() -> usize {
    STATEMENTS.len()
}

/// Une ligne de la table `data_migration`.
///
/// `name` est la cle primaire texte, `time_completed` l'horodatage de fin en
/// millisecondes depuis l'epoch, non nul par construction du type `i64`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneMigrationDonnees {
    /// Colonne `name`, cle primaire.
    pub name: String,
    /// Colonne `time_completed`, `integer NOT NULL`.
    pub time_completed: i64,
}

impl LigneMigrationDonnees {
    /// Construit une ligne, avec son horodatage de fin.
    pub fn new(name: &str, time_completed: i64) -> Self {
        Self { name: name.to_string(), time_completed }
    }
}

/// Premier nom en double dans une tranche, donc refuse par la cle primaire
/// `name`.
///
/// `None` signifie que la tranche respecte la contrainte. Une chaine vide
/// reste un nom comme un autre : la colonne est `NOT NULL`, pas `NON VIDE`.
pub fn premier_doublon(noms: &[&str]) -> Option<String> {
    let mut vus: BTreeSet<&str> = BTreeSet::new();
    for nom in noms {
        if !vus.insert(nom) {
            return Some((*nom).to_string());
        }
    }
    None
}

/// Les lignes candidates que la cle primaire accepterait, dans l'ordre
/// d'entree : un candidat dont le nom existe deja est ecarte.
pub fn lignes_inserables<'a>(
    existantes: &[LigneMigrationDonnees],
    candidates: &'a [LigneMigrationDonnees],
) -> Vec<&'a LigneMigrationDonnees> {
    let mut connus: BTreeSet<&str> =
        existantes.iter().map(|ligne| ligne.name.as_str()).collect();
    let mut retenues = Vec::new();
    for candidat in candidates {
        if connus.insert(candidat.name.as_str()) {
            retenues.push(candidat);
        }
    }
    retenues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_exactement_celui_de_la_source() {
        assert_eq!(id(), "20260511000411_data_migration_state");
        assert_eq!(MIGRATION_ID, "20260511000411_data_migration_state");
    }

    #[test]
    fn l_instruction_cree_la_table_data_migration_avec_ses_deux_colonnes() {
        assert_eq!(nombre_instructions(), 1);
        assert_eq!(instructions()[0], CREATE_TABLE_SQL);
        assert!(CREATE_TABLE_SQL.contains("`data_migration`"));
        assert!(CREATE_TABLE_SQL.contains("`name` text PRIMARY KEY"));
        assert!(CREATE_TABLE_SQL.contains("`time_completed` integer NOT NULL"));
        assert_eq!(COLONNES, ["name", "time_completed"]);
        assert_eq!(TABLE_DATA_MIGRATION, "data_migration");
    }

    #[test]
    fn une_ligne_conserve_son_nom_et_son_horodatage() {
        let ligne = LigneMigrationDonnees::new("remplissage_1", 1_700_000_000_000);
        assert_eq!(ligne.name, "remplissage_1");
        assert_eq!(ligne.time_completed, 1_700_000_000_000);
    }

    #[test]
    fn deux_lignes_de_meme_nom_sont_un_doublon_refuse_par_la_cle_primaire() {
        assert_eq!(premier_doublon(&["a", "b", "a"]), Some("a".to_string()));
        assert_eq!(premier_doublon(&["a", "b", "c"]), None);
        assert_eq!(premier_doublon(&[] as &[&str]), None);
    }

    #[test]
    fn une_chaine_vide_reste_un_nom_valide_car_not_null_n_interdit_pas_vide() {
        assert_eq!(premier_doublon(&["", ""]), Some(String::new()));
        let ligne = LigneMigrationDonnees::new("", 0);
        assert_eq!(ligne.name, "");
    }

    #[test]
    fn les_candidates_deja_connues_sont_ecartees_dans_l_ordre_d_entree() {
        let existantes = vec![LigneMigrationDonnees::new("a", 1)];
        let candidates = vec![
            LigneMigrationDonnees::new("a", 2),
            LigneMigrationDonnees::new("b", 3),
            LigneMigrationDonnees::new("b", 4),
            LigneMigrationDonnees::new("c", 5),
        ];
        let retenues = lignes_inserables(&existantes, &candidates);
        let noms: Vec<&str> = retenues.iter().map(|ligne| ligne.name.as_str()).collect();
        assert_eq!(noms, vec!["b", "c"]);
    }
}
