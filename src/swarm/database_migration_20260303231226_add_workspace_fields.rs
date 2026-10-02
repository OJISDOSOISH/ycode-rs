//! Portage de `packages/core/src/database/migration/20260303231226_add_workspace_fields.ts`.
//!
//! La source tient en 15 lignes et exporte une migration `DatabaseMigration`
//! qui execute cinq `tx.run` dans l ordre sur la table `workspace` : ajout de
//! `type NOT NULL`, ajout de `name`, ajout de `directory`, ajout de `extra`,
//! puis suppression de la colonne `config`.
//!
//! Sans pilote SQL dans ce crate (ni `sqlx`, ni `rusqlite`, ni `diesel`),
//! le portage est en donnees pures : identifiant, instructions dans l ordre,
//! et helpers qui decrivent ce que chaque instruction fait (colonne visee,
//! ajout ou suppression). Aucune base n est ouverte ici.

/// Identifiant de la migration, tel qu ecrit dans la source.
pub const MIGRATION_ID: &str = "20260303231226_add_workspace_fields";

/// Table visee par les cinq instructions.
pub const TABLE_WORKSPACE: &str = "workspace";

/// Les cinq instructions, dans l ordre d execution de `up`.
pub const STATEMENTS: [&str; 5] = [
    "ALTER TABLE `workspace` ADD `type` text NOT NULL;",
    "ALTER TABLE `workspace` ADD `name` text;",
    "ALTER TABLE `workspace` ADD `directory` text;",
    "ALTER TABLE `workspace` ADD `extra` text;",
    "ALTER TABLE `workspace` DROP COLUMN `config`;",
];

/// Sens d une instruction `ALTER TABLE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlterKind {
    /// `ADD <colonne>` : la colonne apparait.
    Add,
    /// `DROP COLUMN <colonne>` : la colonne disparait.
    Drop,
}

/// Une instruction lue comme une operation sur une colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlterOp {
    /// Colonne visee, sans backticks.
    pub column: &'static str,
    /// Ajout ou suppression.
    pub kind: AlterKind,
}

/// Les cinq operations, dans l ordre des instructions.
pub const OPERATIONS: [AlterOp; 5] = [
    AlterOp { column: "type", kind: AlterKind::Add },
    AlterOp { column: "name", kind: AlterKind::Add },
    AlterOp { column: "directory", kind: AlterKind::Add },
    AlterOp { column: "extra", kind: AlterKind::Add },
    AlterOp { column: "config", kind: AlterKind::Drop },
];

/// Le nombre d instructions de la migration.
pub fn nombre_d_instructions() -> usize {
    STATEMENTS.len()
}

/// Les colonnes ajoutees, dans l ordre.
pub fn colonnes_ajoutees() -> Vec<&'static str> {
    OPERATIONS
        .iter()
        .filter(|op| op.kind == AlterKind::Add)
        .map(|op| op.column)
        .collect()
}

/// Les colonnes supprimees, dans l ordre.
pub fn colonnes_supprimees() -> Vec<&'static str> {
    OPERATIONS
        .iter()
        .filter(|op| op.kind == AlterKind::Drop)
        .map(|op| op.column)
        .collect()
}

/// Vrai si l instruction ajoute une colonne `NOT NULL`.
pub fn est_ajout_not_null(instruction: &str) -> bool {
    instruction.contains("ADD") && instruction.contains("NOT NULL")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260303231226_add_workspace_fields");
    }

    #[test]
    fn la_migration_execute_cinq_instructions_sur_workspace() {
        assert_eq!(nombre_d_instructions(), 5);
        for instruction in STATEMENTS.iter() {
            assert!(
                instruction.contains("`workspace`"),
                "instruction hors table : {}",
                instruction
            );
        }
    }

    #[test]
    fn les_quatre_colonnes_ajoutees_sont_dans_l_ordre() {
        assert_eq!(colonnes_ajoutees(), vec!["type", "name", "directory", "extra"]);
    }

    #[test]
    fn la_seule_colonne_supprimee_est_config() {
        assert_eq!(colonnes_supprimees(), vec!["config"]);
    }

    #[test]
    fn seule_la_colonne_type_est_ajoutee_en_not_null() {
        let not_null: Vec<&&str> = STATEMENTS.iter().filter(|s| est_ajout_not_null(s)).collect();
        assert_eq!(not_null.len(), 1);
        assert!(not_null[0].contains("`type`"));
    }

    #[test]
    fn l_ajout_precede_toujours_la_suppression() {
        let position_drop = OPERATIONS.iter().position(|op| op.kind == AlterKind::Drop);
        assert_eq!(position_drop, Some(4));
    }

    #[test]
    fn une_instruction_d_ajout_simple_n_est_pas_not_null() {
        assert!(!est_ajout_not_null(STATEMENTS[1]));
        assert!(est_ajout_not_null(STATEMENTS[0]));
    }
}
