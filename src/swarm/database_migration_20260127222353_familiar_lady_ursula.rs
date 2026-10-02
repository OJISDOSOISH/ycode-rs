//! Portage Rust de `packages/core/src/database/migration/20260127222353_familiar_lady_ursula.ts`.
//!
//! Migration initiale : cree les sept tables du schema (`project`, `message`,
//! `part`, `permission`, `session`, `todo`, `session_share`) puis les six index
//! qui les accompagnent. La source execute treize ordres SQL via `tx.run` dans
//! un `Effect.gen` ; ce module expose ces treize ordres comme des donnees
//! pures, sans pilote SQL.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce fichier
//! ne peut ni ouvrir de base ni executer quoi que ce soit. Il fige l `id` de
//! la migration et la liste des ordres, dans leur ordre d execution, pour
//! qu une couche ulterieure les consomme sans les redeviner.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260127222353_familiar_lady_ursula";

/// Ordre de creation de la table `project`.
pub const STATEMENT_CREATE_PROJECT: &str =
    "CREATE TABLE `project` (`id` text PRIMARY KEY, `worktree` text NOT NULL, `vcs` text, `name` text, `icon_url` text, `icon_color` text, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, `time_initialized` integer, `sandboxes` text NOT NULL)";

/// Ordre de creation de la table `message`, avec sa cle etrangere vers `session`.
pub const STATEMENT_CREATE_MESSAGE: &str =
    "CREATE TABLE `message` (`id` text PRIMARY KEY, `session_id` text NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, `data` text NOT NULL, CONSTRAINT `fk_message_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE)";

/// Ordre de creation de la table `part`, avec sa cle etrangere vers `message`.
pub const STATEMENT_CREATE_PART: &str =
    "CREATE TABLE `part` (`id` text PRIMARY KEY, `message_id` text NOT NULL, `session_id` text NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, `data` text NOT NULL, CONSTRAINT `fk_part_message_id_message_id_fk` FOREIGN KEY (`message_id`) REFERENCES `message`(`id`) ON DELETE CASCADE)";

/// Ordre de creation de la table `permission`, avec sa cle etrangere vers `project`.
pub const STATEMENT_CREATE_PERMISSION: &str =
    "CREATE TABLE `permission` (`project_id` text PRIMARY KEY, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, `data` text NOT NULL, CONSTRAINT `fk_permission_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE)";

/// Ordre de creation de la table `session`, avec sa cle etrangere vers `project`.
pub const STATEMENT_CREATE_SESSION: &str =
    "CREATE TABLE `session` (`id` text PRIMARY KEY, `project_id` text NOT NULL, `parent_id` text, `slug` text NOT NULL, `directory` text NOT NULL, `title` text NOT NULL, `version` text NOT NULL, `share_url` text, `summary_additions` integer, `summary_deletions` integer, `summary_files` integer, `summary_diffs` text, `revert` text, `permission` text, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, `time_compacting` integer, `time_archived` integer, CONSTRAINT `fk_session_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE)";

/// Ordre de creation de la table `todo`, avec sa cle primaire composee et sa cle vers `session`.
pub const STATEMENT_CREATE_TODO: &str =
    "CREATE TABLE `todo` (`session_id` text NOT NULL, `content` text NOT NULL, `status` text NOT NULL, `priority` text NOT NULL, `position` integer NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, CONSTRAINT `todo_pk` PRIMARY KEY(`session_id`, `position`), CONSTRAINT `fk_todo_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE)";

/// Ordre de creation de la table `session_share`, avec sa cle etrangere vers `session`.
pub const STATEMENT_CREATE_SESSION_SHARE: &str =
    "CREATE TABLE `session_share` (`session_id` text PRIMARY KEY, `id` text NOT NULL, `secret` text NOT NULL, `url` text NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, CONSTRAINT `fk_session_share_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE)";

/// Les six index crees par la migration, dans leur ordre d execution.
pub const STATEMENT_INDEXES: [&str; 6] = [
    "CREATE INDEX `message_session_idx` ON `message` (`session_id`)",
    "CREATE INDEX `part_message_idx` ON `part` (`message_id`)",
    "CREATE INDEX `part_session_idx` ON `part` (`session_id`)",
    "CREATE INDEX `session_project_idx` ON `session` (`project_id`)",
    "CREATE INDEX `session_parent_idx` ON `session` (`parent_id`)",
    "CREATE INDEX `todo_session_idx` ON `todo` (`session_id`)",
];

/// Les treize ordres SQL de la migration, dans leur ordre d execution.
///
/// Les sept `CREATE TABLE` d abord, puis les six `CREATE INDEX`. C est l ordre
/// exact du `Effect.gen` de la source.
pub const STATEMENTS: [&str; 13] = [
    STATEMENT_CREATE_PROJECT,
    STATEMENT_CREATE_MESSAGE,
    STATEMENT_CREATE_PART,
    STATEMENT_CREATE_PERMISSION,
    STATEMENT_CREATE_SESSION,
    STATEMENT_CREATE_TODO,
    STATEMENT_CREATE_SESSION_SHARE,
    STATEMENT_INDEXES[0],
    STATEMENT_INDEXES[1],
    STATEMENT_INDEXES[2],
    STATEMENT_INDEXES[3],
    STATEMENT_INDEXES[4],
    STATEMENT_INDEXES[5],
];

/// Nombre d ordres executes par la migration.
pub fn statement_count() -> usize {
    STATEMENTS.len()
}

/// Vrai si l ordre cree une table.
pub fn is_create_table(statement: &str) -> bool {
    statement.trim_start().starts_with("CREATE TABLE")
}

/// Vrai si l ordre cree un index.
pub fn is_create_index(statement: &str) -> bool {
    statement.trim_start().starts_with("CREATE INDEX")
}

/// Les sept tables crees par la migration, dans leur ordre de creation.
pub fn created_tables() -> Vec<&'static str> {
    vec![
        "project",
        "message",
        "part",
        "permission",
        "session",
        "todo",
        "session_share",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(MIGRATION_ID, "20260127222353_familiar_lady_ursula");
    }

    #[test]
    fn la_migration_execute_treize_ordres() {
        assert_eq!(statement_count(), 13);
        assert_eq!(STATEMENTS.len(), 13);
    }

    #[test]
    fn les_tables_sont_creees_avant_les_index() {
        let tables = STATEMENTS.iter().filter(|s| is_create_table(s)).count();
        let index = STATEMENTS.iter().filter(|s| is_create_index(s)).count();
        assert_eq!(tables, 7);
        assert_eq!(index, 6);
        for stmt in STATEMENTS.iter().take(7) {
            assert!(is_create_table(stmt));
        }
        for stmt in STATEMENTS.iter().skip(7) {
            assert!(is_create_index(stmt));
        }
    }

    #[test]
    fn toutes_les_tables_attendues_sont_creees() {
        assert_eq!(
            created_tables(),
            vec![
                "project",
                "message",
                "part",
                "permission",
                "session",
                "todo",
                "session_share",
            ]
        );
        for table in created_tables() {
            assert!(
                STATEMENTS.iter().any(|s| s.contains(&format!("CREATE TABLE `{table}`"))),
                "la table {table} doit avoir son CREATE TABLE"
            );
        }
    }

    #[test]
    fn les_cles_etrangeres_en_cascade_sont_declarees() {
        assert!(STATEMENT_CREATE_MESSAGE.contains("ON DELETE CASCADE"));
        assert!(STATEMENT_CREATE_PART.contains("ON DELETE CASCADE"));
        assert!(STATEMENT_CREATE_PERMISSION.contains("ON DELETE CASCADE"));
        assert!(STATEMENT_CREATE_SESSION.contains("ON DELETE CASCADE"));
        assert!(STATEMENT_CREATE_TODO.contains("ON DELETE CASCADE"));
        assert!(STATEMENT_CREATE_SESSION_SHARE.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn les_index_attendus_sont_crees() {
        let noms = [
            "message_session_idx",
            "part_message_idx",
            "part_session_idx",
            "session_project_idx",
            "session_parent_idx",
            "todo_session_idx",
        ];
        for nom in noms {
            assert!(
                STATEMENTS.iter().any(|s| s.contains(nom)),
                "l index {nom} doit etre cree"
            );
        }
    }

    #[test]
    fn aucun_ordre_n_est_vide() {
        for stmt in STATEMENTS {
            assert!(!stmt.trim().is_empty());
        }
    }
}
