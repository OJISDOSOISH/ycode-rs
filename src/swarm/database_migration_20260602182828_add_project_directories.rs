//! Portage Rust de `opencode/packages/core/src/database/migration/20260602182828_add_project_directories.ts`.
//!
//! ## Ce que fait la source
//!
//! Le fichier d'origine fait 20 lignes et n'exporte qu'une migration :
//!
//! ```ts
//! export default {
//!   id: "20260602182828_add_project_directories",
//!   up(tx) {
//!     return Effect.gen(function* () {
//!       yield* tx.run(`
//!         CREATE TABLE \`project_directory\` (
//!           \`project_id\` text NOT NULL,
//!           \`directory\` text NOT NULL,
//!           \`type\` text NOT NULL,
//!           \`time_created\` integer NOT NULL,
//!           CONSTRAINT \`project_directory_pk\` PRIMARY KEY(\`project_id\`, \`directory\`),
//!           CONSTRAINT \`fk_project_directory_project_id_project_id_fk\` FOREIGN KEY (\`project_id\`) REFERENCES \`project\`(\`id\`) ON DELETE CASCADE
//!         );
//!       `)
//!     })
//!   },
//! } satisfies DatabaseMigration.Migration
//! ```
//!
//! La migration cree la table `project_directory`, avec une cle primaire
//! composee sur le couple (`project_id`, `directory`) et une cascade vers
//! `project`. Une seule instruction, sans garde ni index separe.
//!
//! ## Deux remarques de fidelite
//!
//! 1. Dans cette migration, la colonne `type` est `NOT NULL`. Le schema plus
//!    recent (voir `crate::swarm::project_sql`, issu du DDL genere) la
//!    declare nullable : la contrainte a ete assouplie entre les deux. Le
//!    portage reprend le `NOT NULL` de la migration, qui est ce que SQLite a
//!    reellement execute a cette etape, et le signale au lieu de l'aligner
//!    sur l'etat final.
//! 2. La forme de la ligne `project_directory` (cle composite, cascade,
//!    valeurs de `type`) est decrite et teste en detail dans
//!    `crate::swarm::project_sql`. Ce fichier ne la redeclare pas : il porte
//!    la migration, et un test verifie que les deux designent la meme table
//!    et la meme cle primaire.
//!
//! Non porte : `Effect.gen`, la transaction, l'execution SQLite. `Cargo.toml`
//! ne declare aucun pilote SQL.

/// Identifiant de la migration, tel que declare dans le fichier TypeScript.
pub const MIGRATION_ID: &str = "20260602182828_add_project_directories";

/// Table creee par la migration.
pub const TABLE_NAME: &str = "project_directory";

/// Cle primaire composee creee par la migration.
pub const PRIMARY_KEY_NAME: &str = "project_directory_pk";

/// Cle etrangere creee par la migration.
pub const PROJECT_FOREIGN_KEY_NAME: &str = "fk_project_directory_project_id_project_id_fk";

/// Projet proprietaire du repertoire, premiere colonne de la cle.
pub const COLUMN_PROJECT_ID: &str = "project_id";

/// Repertoire enregistre, seconde colonne de la cle.
pub const COLUMN_DIRECTORY: &str = "directory";

/// Nature du repertoire. `NOT NULL` a cette etape (voir l'en-tete).
pub const COLUMN_TYPE: &str = "type";

/// Horodatage de creation, en millisecondes depuis l'epoch.
pub const COLUMN_TIME_CREATED: &str = "time_created";

/// Les quatre colonnes de la table, dans l'ordre du `CREATE TABLE`.
pub const COLUMNS: [&str; 4] = [COLUMN_PROJECT_ID, COLUMN_DIRECTORY, COLUMN_TYPE, COLUMN_TIME_CREATED];

/// Les deux colonnes de la cle primaire composee, dans leur ordre de tri.
pub const PRIMARY_KEY_COLUMNS: [&str; 2] = [COLUMN_PROJECT_ID, COLUMN_DIRECTORY];

/// Instruction unique de la migration.
///
/// Les jetons reprennent le gabarit TypeScript (noms, types, contraintes,
/// ordre des colonnes) avec une mise en page normalisee.
pub const CREATE_TABLE_SQL: &str = "CREATE TABLE `project_directory` (`project_id` text NOT NULL, `directory` text NOT NULL, `type` text NOT NULL, `time_created` integer NOT NULL, CONSTRAINT `project_directory_pk` PRIMARY KEY(`project_id`, `directory`), CONSTRAINT `fk_project_directory_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE);";

/// Les instructions de la migration, dans leur ordre d'execution.
pub const UP_STATEMENTS: [&str; 1] = [CREATE_TABLE_SQL];

/// La clause de cle primaire composee, telle que la migration l'ecrit.
pub fn primary_key_clause() -> String {
    format!(
        "CONSTRAINT `{PRIMARY_KEY_NAME}` PRIMARY KEY(`project_id`, `directory`)"
    )
}

/// La clause de cle etrangere, telle que la migration l'ecrit.
pub fn foreign_key_clause() -> String {
    format!(
        "CONSTRAINT `{PROJECT_FOREIGN_KEY_NAME}` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_est_celui_du_fichier_ts() {
        assert_eq!(MIGRATION_ID, "20260602182828_add_project_directories");
    }

    #[test]
    fn l_instruction_unique_cree_la_table_project_directory() {
        assert_eq!(UP_STATEMENTS.len(), 1);
        assert!(UP_STATEMENTS[0].starts_with("CREATE TABLE `project_directory`"));
    }

    #[test]
    fn la_table_porte_les_quatre_colonnes_dans_l_ordre_du_create_table() {
        assert_eq!(COLUMNS, ["project_id", "directory", "type", "time_created"]);
        for nom in COLUMNS {
            assert!(CREATE_TABLE_SQL.contains(nom), "la colonne {nom} doit figurer dans le CREATE TABLE");
        }
    }

    #[test]
    fn la_colonne_type_est_not_null_a_cette_etape() {
        // Etat historique : la migration dit `type text NOT NULL`, alors que
        // le schema genere plus recent le rend nullable. Aligner ce fichier
        // sur l'etat final effacerait l'histoire reelle de la base.
        assert!(CREATE_TABLE_SQL.contains("`type` text NOT NULL"));
    }

    #[test]
    fn la_cle_primaire_est_le_couple_projet_repertoire() {
        assert_eq!(PRIMARY_KEY_COLUMNS, ["project_id", "directory"]);
        assert_eq!(
            primary_key_clause(),
            "CONSTRAINT `project_directory_pk` PRIMARY KEY(`project_id`, `directory`)"
        );
        assert!(CREATE_TABLE_SQL.contains(&primary_key_clause()));
    }

    #[test]
    fn la_cle_etrangere_cible_le_projet_en_cascade() {
        assert_eq!(
            foreign_key_clause(),
            "CONSTRAINT `fk_project_directory_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE"
        );
        assert!(CREATE_TABLE_SQL.contains(&foreign_key_clause()));
    }

    #[test]
    fn la_migration_designe_la_meme_table_que_le_contrat_de_ligne() {
        // `project_sql` decrit les lignes, ce fichier decrit la migration :
        // meme table, meme cle primaire, sans recopie de contenu.
        assert_eq!(TABLE_NAME, crate::swarm::project_sql::PROJECT_DIRECTORY_TABLE);
        assert_eq!(
            PRIMARY_KEY_NAME,
            crate::swarm::project_sql::PROJECT_DIRECTORY_PRIMARY_KEY_NAME
        );
        assert_eq!(
            PROJECT_FOREIGN_KEY_NAME,
            crate::swarm::project_sql::PROJECT_DIRECTORY_FOREIGN_KEY_NAME
        );
    }
}
