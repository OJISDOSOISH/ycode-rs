//! Portage de `packages/core/src/database/migration/20260410174513_workspace-name.ts`.
//!
//! La migration ajoute la colonne `name` a la table `workspace` par
//! reconstruction : SQLite ne permet pas d'ajouter une colonne `NOT NULL`
//! avec defaut de facon portable sur une table existante, donc le code
//! d'origine cree `__new_workspace`, y recopie les lignes, supprime
//! `workspace` puis renomme. Quand la colonne `name` existe deja (base
//! partiellement migree), sa valeur est reprise ; sinon la migration ecrit
//! la chaine vide `''`.
//!
//! Sans pilote SQL dans ce crate (`Cargo.toml` ne declare ni `sqlx`, ni
//! `rusqlite`, ni `diesel`), ce fichier porte le contrat sous forme de
//! donnees pures : identifiant, noms de tables, listes de colonnes, textes
//! SQL exacts, et les deux fonctions pures qui replacent le ternaire de la
//! source (`columns.some(...) ? "`name`" : "''"`).

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260410174513_workspace-name";

/// Table visee par la migration.
pub const WORKSPACE_TABLE: &str = "workspace";

/// Table temporaire de reconstruction.
pub const NEW_WORKSPACE_TABLE: &str = "__new_workspace";

/// Nom de la contrainte de cle etrangere vers `project(id)`.
pub const FK_WORKSPACE_PROJECT: &str = "fk_workspace_project_id_project_id_fk";

/// Inspection du schema avant migration (`tx.all` dans la source).
pub const PRAGMA_TABLE_INFO_SQL: &str = "PRAGMA table_info(`workspace`)";

/// Desactivation des cles etrangeres pendant la reconstruction.
pub const DISABLE_FOREIGN_KEYS_SQL: &str = "PRAGMA foreign_keys=OFF;";

/// Reactivation des cles etrangeres apres le renommage.
pub const ENABLE_FOREIGN_KEYS_SQL: &str = "PRAGMA foreign_keys=ON;";

/// Suppression de l'ancienne table.
pub const DROP_WORKSPACE_SQL: &str = "DROP TABLE `workspace`;";

/// Renommage de la table reconstruite.
pub const RENAME_NEW_WORKSPACE_SQL: &str = "ALTER TABLE `__new_workspace` RENAME TO `workspace`;";

/// Creation de la table reconstruite, avec la colonne `name` en quatrieme
/// position (`text DEFAULT '' NOT NULL`).
pub const CREATE_NEW_WORKSPACE_SQL: &str = "CREATE TABLE `__new_workspace` (
          `id` text PRIMARY KEY,
          `type` text NOT NULL,
          `name` text DEFAULT '' NOT NULL,
          `branch` text,
          `directory` text,
          `extra` text,
          `project_id` text NOT NULL,
          CONSTRAINT `fk_workspace_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
        );";

/// Colonnes de `__new_workspace`, dans l'ordre du `CREATE TABLE`.
pub const NEW_WORKSPACE_COLUMNS: [&str; 7] =
    ["id", "type", "branch", "name", "directory", "extra", "project_id"];

/// Expression de recopie de `name` quand la colonne existe deja.
pub const NAME_EXPR_QUAND_PRESENTE: &str = "`name`";

/// Expression de recopie de `name` quand la colonne manque : chaine vide SQL.
pub const NAME_EXPR_QUAND_ABSENTE: &str = "''";

/// Vrai si la table `workspace` porte deja une colonne `name`.
///
/// Transcription directe de `columns.some((column) => column.name === "name")`.
/// La comparaison est exacte, comme en SQL : `Name` ne vaut pas `name`.
pub fn colonne_name_existe(colonnes: &[&str]) -> bool {
    colonnes.iter().any(|colonne| *colonne == "name")
}

/// Expression SQL a injecter dans le `SELECT` de recopie pour la colonne `name`.
pub fn expression_colonne_name(colonnes: &[&str]) -> &'static str {
    if colonne_name_existe(colonnes) {
        NAME_EXPR_QUAND_PRESENTE
    } else {
        NAME_EXPR_QUAND_ABSENTE
    }
}

/// Requete `INSERT ... SELECT` de recopie vers `__new_workspace`.
///
/// Le seul point variable est l'expression de `name` (quatrieme colonne du
/// `SELECT`) ; tout le reste est fixe. `true` signifie que la colonne existe.
pub fn requete_insert_workspace(colonne_existe: bool) -> String {
    let expression = if colonne_existe {
        NAME_EXPR_QUAND_PRESENTE
    } else {
        NAME_EXPR_QUAND_ABSENTE
    };
    format!(
        "INSERT INTO `__new_workspace`(`id`, `type`, `branch`, `name`, `directory`, `extra`, `project_id`) SELECT `id`, `type`, `branch`, {}, `directory`, `extra`, `project_id` FROM `workspace`;",
        expression
    )
}

/// Nombre d'ordres SQL emis par `up`, hors inspection initiale.
///
/// Sept : desactivation des cles, creation, recopie, suppression, renommage,
/// reactivation. L'inspection `PRAGMA table_info` n'est pas un ordre
/// d'ecriture.
pub fn nombre_ordres_ecriture() -> usize {
    6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260410174513_workspace-name");
    }

    #[test]
    fn la_table_temporaire_est_prefixee_de_deux_soulignes() {
        assert_eq!(NEW_WORKSPACE_TABLE, "__new_workspace");
        assert_eq!(WORKSPACE_TABLE, "workspace");
    }

    #[test]
    fn la_colonne_name_est_detectee_par_egalite_exacte() {
        assert!(colonne_name_existe(&["id", "type", "name"]));
        assert!(!colonne_name_existe(&["id", "type", "branch"]));
        assert!(!colonne_name_existe(&["Name"]));
        assert!(!colonne_name_existe(&[]));
    }

    #[test]
    fn l_expression_vaut_le_nom_de_colonne_quand_elle_existe() {
        assert_eq!(expression_colonne_name(&["id", "name"]), "`name`");
    }

    #[test]
    fn l_expression_vaut_la_chaine_vide_sql_quand_elle_manquee() {
        assert_eq!(expression_colonne_name(&["id", "type"]), "''");
    }

    #[test]
    fn la_requete_insert_reprend_la_colonne_quand_elle_existe() {
        let requete = requete_insert_workspace(true);
        assert!(
            requete.contains("SELECT `id`, `type`, `branch`, `name`, `directory`, `extra`, `project_id` FROM `workspace`"),
            "requete inattendue : {}",
            requete
        );
        assert!(requete.starts_with("INSERT INTO `__new_workspace`"));
    }

    #[test]
    fn la_requete_insert_ecrit_chaine_vide_quand_la_colonne_manque() {
        let requete = requete_insert_workspace(false);
        assert!(
            requete.contains("`branch`, '', `directory`"),
            "requete inattendue : {}",
            requete
        );
    }

    #[test]
    fn les_deux_requetes_insert_ne_different_que_sur_name() {
        let avec = requete_insert_workspace(true);
        let sans = requete_insert_workspace(false);
        assert_ne!(avec, sans);
        assert_eq!(avec.replace("`name`", "''"), sans);
    }

    #[test]
    fn la_nouvelle_table_porte_sept_colonnes_dans_l_ordre_du_ddl() {
        assert_eq!(
            NEW_WORKSPACE_COLUMNS,
            ["id", "type", "branch", "name", "directory", "extra", "project_id"]
        );
    }

    #[test]
    fn le_ddl_de_creation_mentionne_la_colonne_name_et_la_cle_project() {
        assert!(CREATE_NEW_WORKSPACE_SQL.contains("`name` text DEFAULT '' NOT NULL"));
        assert!(CREATE_NEW_WORKSPACE_SQL.contains(FK_WORKSPACE_PROJECT));
        assert!(CREATE_NEW_WORKSPACE_SQL.contains("REFERENCES `project`(`id`) ON DELETE CASCADE"));
    }

    #[test]
    fn les_pragmas_encadrent_la_reconstruction() {
        assert_eq!(DISABLE_FOREIGN_KEYS_SQL, "PRAGMA foreign_keys=OFF;");
        assert_eq!(ENABLE_FOREIGN_KEYS_SQL, "PRAGMA foreign_keys=ON;");
        assert!(DROP_WORKSPACE_SQL.starts_with("DROP TABLE"));
        assert!(RENAME_NEW_WORKSPACE_SQL.contains("RENAME TO `workspace`"));
    }

    #[test]
    fn le_nombre_d_ordres_d_ecriture_est_stable() {
        assert_eq!(nombre_ordres_ecriture(), 6);
    }
}
