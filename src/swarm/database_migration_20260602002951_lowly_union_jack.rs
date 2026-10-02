//! Portage Rust de `opencode/packages/core/src/database/migration/20260602002951_lowly_union_jack.ts`.
//!
//! ## Ce que fait la source
//!
//! Le fichier d'origine fait 24 lignes et n'exporte qu'une migration :
//!
//! ```ts
//! export default {
//!   id: "20260602002951_lowly_union_jack",
//!   up(tx) {
//!     return Effect.gen(function* () {
//!       yield* tx.run(`
//!         CREATE TABLE \`permission\` (
//!           \`id\` text PRIMARY KEY,
//!           \`project_id\` text NOT NULL,
//!           \`action\` text NOT NULL,
//!           \`resource\` text NOT NULL,
//!           \`time_created\` integer NOT NULL,
//!           \`time_updated\` integer NOT NULL,
//!           CONSTRAINT \`fk_permission_project_id_project_id_fk\` FOREIGN KEY (\`project_id\`) REFERENCES \`project\`(\`id\`) ON DELETE CASCADE
//!         );
//!       `)
//!       yield* tx.run(
//!         `CREATE UNIQUE INDEX \`permission_project_action_resource_idx\` ON \`permission\` (\`project_id\`,\`action\`,\`resource\`);`,
//!       )
//!     })
//!   },
//! } satisfies DatabaseMigration.Migration
//! ```
//!
//! La migration recree la table `permission` supprimee par la precedente
//! (`20260601202201_amazing_prowler`) avec son index unique. Le diptyque
//! suppression puis recreation remplace un `ALTER` que SQLite ne saurait pas
//! exprimer sur les contraintes.
//!
//! ## Rapports avec le reste du depot
//!
//! La forme de la ligne `permission` (six colonnes, index unique sur le
//! triplet, cascade vers `project`) est decrite et teste en detail dans
//! `crate::swarm::permission_sql`. Ce fichier ne la redeclare pas : il porte
//! la migration, c'est-a-dire l'identifiant, les deux instructions dans leur
//! ordre, et les noms de contraintes qu'elles creent. Un test verifie que les
//! deux fichiers designent la meme table et le meme index, sans dupliquer
//! leurs contenus.
//!
//! Non porte : `Effect.gen`, la transaction, l'execution SQLite. `Cargo.toml`
//! ne declare aucun pilote SQL.

/// Identifiant de la migration, tel que declare dans le fichier TypeScript.
pub const MIGRATION_ID: &str = "20260602002951_lowly_union_jack";

/// Table creee par la migration.
pub const TABLE_NAME: &str = "permission";

/// Index unique cree par la migration.
pub const UNIQUE_INDEX_NAME: &str = "permission_project_action_resource_idx";

/// Cle etrangere creee par la migration.
pub const PROJECT_FOREIGN_KEY_NAME: &str = "fk_permission_project_id_project_id_fk";

/// Colonne de la cle primaire.
pub const COLUMN_ID: &str = "id";

/// Colonne du projet proprietaire, cible de la cascade.
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
pub const COLUMNS: [&str; 6] = [
    COLUMN_ID,
    COLUMN_PROJECT_ID,
    COLUMN_ACTION,
    COLUMN_RESOURCE,
    COLUMN_TIME_CREATED,
    COLUMN_TIME_UPDATED,
];

/// Les trois colonnes de l'index unique, dans leur ordre de tri.
pub const UNIQUE_INDEX_COLUMNS: [&str; 3] = [COLUMN_PROJECT_ID, COLUMN_ACTION, COLUMN_RESOURCE];

/// Premiere instruction : creation de la table et de sa cle etrangere.
///
/// La chaine reprend les memes jetons que le gabarit TypeScript (noms,
/// types, contraintes, ordre des colonnes) avec une mise en page
/// normalisee : SQLite ignore les espaces et les retours a la ligne, donc
/// les deux formes executent la meme creation.
pub const CREATE_TABLE_SQL: &str = "CREATE TABLE `permission` (`id` text PRIMARY KEY, `project_id` text NOT NULL, `action` text NOT NULL, `resource` text NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, CONSTRAINT `fk_permission_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE);";

/// Seconde instruction : creation de l'index unique.
pub const CREATE_INDEX_SQL: &str = "CREATE UNIQUE INDEX `permission_project_action_resource_idx` ON `permission` (`project_id`,`action`,`resource`);";

/// Les instructions de la migration, dans leur ordre d'execution.
///
/// La table precede l'index : indexer une table inexistante echouerait.
pub const UP_STATEMENTS: [&str; 2] = [CREATE_TABLE_SQL, CREATE_INDEX_SQL];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_est_celui_du_fichier_ts() {
        assert_eq!(MIGRATION_ID, "20260602002951_lowly_union_jack");
    }

    #[test]
    fn les_instructions_creent_la_table_puis_l_index() {
        assert_eq!(UP_STATEMENTS.len(), 2);
        assert!(UP_STATEMENTS[0].starts_with("CREATE TABLE `permission`"));
        assert!(UP_STATEMENTS[1].starts_with("CREATE UNIQUE INDEX `permission_project_action_resource_idx`"));
    }

    #[test]
    fn la_table_porte_les_six_colonnes_dans_l_ordre_du_create_table() {
        assert_eq!(
            COLUMNS,
            ["id", "project_id", "action", "resource", "time_created", "time_updated"]
        );
        for nom in COLUMNS {
            assert!(CREATE_TABLE_SQL.contains(nom), "la colonne {nom} doit figurer dans le CREATE TABLE");
        }
    }

    #[test]
    fn la_cle_etrangere_cible_le_projet_en_cascade() {
        assert!(CREATE_TABLE_SQL.contains("CONSTRAINT `fk_permission_project_id_project_id_fk`"));
        assert!(CREATE_TABLE_SQL.contains("REFERENCES `project`(`id`) ON DELETE CASCADE"));
        assert_eq!(PROJECT_FOREIGN_KEY_NAME, "fk_permission_project_id_project_id_fk");
    }

    #[test]
    fn l_index_unique_porte_sur_le_triplet_projet_action_ressource() {
        assert_eq!(UNIQUE_INDEX_COLUMNS, ["project_id", "action", "resource"]);
        assert!(CREATE_INDEX_SQL.contains("(`project_id`,`action`,`resource`)"));
    }

    #[test]
    fn la_migration_designe_la_meme_table_que_le_contrat_de_ligne() {
        // `permission_sql` decrit la ligne, ce fichier decrit la migration :
        // les deux doivent designer la meme table et le meme index, sans que
        // le contenu de l'un soit recopie dans l'autre.
        assert_eq!(TABLE_NAME, crate::swarm::permission_sql::TABLE_NAME);
        assert_eq!(UNIQUE_INDEX_NAME, crate::swarm::permission_sql::UNIQUE_INDEX_NAME);
        assert_eq!(
            PROJECT_FOREIGN_KEY_NAME,
            crate::swarm::permission_sql::PROJECT_FOREIGN_KEY_NAME
        );
    }
}
