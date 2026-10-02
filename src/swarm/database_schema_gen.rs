//! Portage Rust de `packages/core/src/database/schema.gen.ts`.
//!
//! ## Ce que contient la source
//!
//! 274 lignes, un seul export par defaut :
//!
//! ```ts
//! export default {
//!   up(tx) { return Effect.gen(function* () { yield* tx.run(`CREATE TABLE ...`); ... }) },
//! } satisfies Omit<DatabaseMigration.Migration, "id">
//! ```
//!
//! Dix-neuf `CREATE TABLE` puis dix-sept `CREATE INDEX`, executes dans
//! l'ordre. Le `satisfies Omit<Migration, "id">` dit que ce paquet n'a pas
//! d'identifiant : c'est le schema initial, pas une migration numerotee.
//!
//! ## Methode de portage
//!
//! Chaque ordre SQL est recopie comme donnee (`TABLE_DDL`, `INDEX_DDL`), dans
//! l'ordre de la source, face a son nom (`TABLE_NAMES`, `INDEX_NAMES`). Aucun
//! pilote n'existe dans `Cargo.toml`, donc rien n'est execute : les fonctions
//! pures ci-dessous interrogent le contrat (recherche par nom, unicite) et les
//! tests verrouillent les effectifs, l'ordre et les points de contrainte.
//!
//! La forme declarative d'origine (les `sqliteTable` des modules `*/sql.ts`)
//! est portee dans les modules voisins (`account_sql`, `event_sql`, ...) :
//! ce fichier ne la duplique pas, il porte le DDL genere qui fait foi a
//! l'execution.

/// Noms des dix-neuf tables, dans l'ordre des `tx.run` de la source.
pub const TABLE_NAMES: [&str; 19] = [
    "workspace",
    "data_migration",
    "account_state",
    "account",
    "control_account",
    "credential",
    "event_sequence",
    "event",
    "permission",
    "project_directory",
    "project",
    "message",
    "part",
    "session_context_epoch",
    "session_input",
    "session_message",
    "session",
    "todo",
    "session_share",
];

/// Les dix-neuf ordres `CREATE TABLE`, dans le meme ordre que [`TABLE_NAMES`].
pub const TABLE_DDL: [&str; 19] = [
    r#"CREATE TABLE `workspace` (
      `id` text PRIMARY KEY,
      `type` text NOT NULL,
      `name` text DEFAULT '' NOT NULL,
      `branch` text,
      `directory` text,
      `extra` text,
      `project_id` text NOT NULL,
      `time_used` integer NOT NULL,
      CONSTRAINT `fk_workspace_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `data_migration` (
      `name` text PRIMARY KEY,
      `time_completed` integer NOT NULL
    );"#,
    r#"CREATE TABLE `account_state` (
      `id` integer PRIMARY KEY,
      `active_account_id` text,
      `active_org_id` text,
      CONSTRAINT `fk_account_state_active_account_id_account_id_fk` FOREIGN KEY (`active_account_id`) REFERENCES `account`(`id`) ON DELETE SET NULL
    );"#,
    r#"CREATE TABLE `account` (
      `id` text PRIMARY KEY,
      `email` text NOT NULL,
      `url` text NOT NULL,
      `access_token` text NOT NULL,
      `refresh_token` text NOT NULL,
      `token_expiry` integer,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL
    );"#,
    r#"CREATE TABLE `control_account` (
      `email` text NOT NULL,
      `url` text NOT NULL,
      `access_token` text NOT NULL,
      `refresh_token` text NOT NULL,
      `token_expiry` integer,
      `active` integer NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      CONSTRAINT `control_account_pk` PRIMARY KEY(`email`, `url`)
    );"#,
    r#"CREATE TABLE `credential` (
      `id` text PRIMARY KEY,
      `integration_id` text,
      `label` text NOT NULL,
      `value` text NOT NULL,
      `connector_id` text,
      `method_id` text,
      `active` integer,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL
    );"#,
    r#"CREATE TABLE `event_sequence` (
      `aggregate_id` text PRIMARY KEY,
      `seq` integer NOT NULL,
      `owner_id` text
    );"#,
    r#"CREATE TABLE `event` (
      `id` text PRIMARY KEY,
      `aggregate_id` text NOT NULL,
      `seq` integer NOT NULL,
      `type` text NOT NULL,
      `data` text NOT NULL,
      CONSTRAINT `fk_event_aggregate_id_event_sequence_aggregate_id_fk` FOREIGN KEY (`aggregate_id`) REFERENCES `event_sequence`(`aggregate_id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `permission` (
      `id` text PRIMARY KEY,
      `project_id` text NOT NULL,
      `action` text NOT NULL,
      `resource` text NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      CONSTRAINT `fk_permission_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `project_directory` (
      `project_id` text NOT NULL,
      `directory` text NOT NULL,
      `type` text,
      `strategy` text,
      `time_created` integer NOT NULL,
      CONSTRAINT `project_directory_pk` PRIMARY KEY(`project_id`, `directory`),
      CONSTRAINT `fk_project_directory_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `project` (
      `id` text PRIMARY KEY,
      `worktree` text NOT NULL,
      `vcs` text,
      `name` text,
      `icon_url` text,
      `icon_url_override` text,
      `icon_color` text,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      `time_initialized` integer,
      `sandboxes` text NOT NULL,
      `commands` text
    );"#,
    r#"CREATE TABLE `message` (
      `id` text PRIMARY KEY,
      `session_id` text NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      `data` text NOT NULL,
      CONSTRAINT `fk_message_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `part` (
      `id` text PRIMARY KEY,
      `message_id` text NOT NULL,
      `session_id` text NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      `data` text NOT NULL,
      CONSTRAINT `fk_part_message_id_message_id_fk` FOREIGN KEY (`message_id`) REFERENCES `message`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `session_context_epoch` (
      `session_id` text PRIMARY KEY,
      `baseline` text NOT NULL,
      `snapshot` text NOT NULL,
      `baseline_seq` integer NOT NULL,
      CONSTRAINT `fk_session_context_epoch_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `session_input` (
      `id` text PRIMARY KEY,
      `session_id` text NOT NULL,
      `prompt` text NOT NULL,
      `delivery` text NOT NULL,
      `admitted_seq` integer NOT NULL,
      `promoted_seq` integer,
      `time_created` integer NOT NULL,
      CONSTRAINT `fk_session_input_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `session_message` (
      `id` text PRIMARY KEY,
      `session_id` text NOT NULL,
      `type` text NOT NULL,
      `seq` integer NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      `data` text NOT NULL,
      CONSTRAINT `fk_session_message_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `session` (
      `id` text PRIMARY KEY,
      `project_id` text NOT NULL,
      `workspace_id` text,
      `parent_id` text,
      `slug` text NOT NULL,
      `directory` text NOT NULL,
      `path` text,
      `title` text NOT NULL,
      `version` text NOT NULL,
      `share_url` text,
      `summary_additions` integer,
      `summary_deletions` integer,
      `summary_files` integer,
      `summary_diffs` text,
      `metadata` text,
      `cost` real DEFAULT 0 NOT NULL,
      `tokens_input` integer DEFAULT 0 NOT NULL,
      `tokens_output` integer DEFAULT 0 NOT NULL,
      `tokens_reasoning` integer DEFAULT 0 NOT NULL,
      `tokens_cache_read` integer DEFAULT 0 NOT NULL,
      `tokens_cache_write` integer DEFAULT 0 NOT NULL,
      `revert` text,
      `permission` text,
      `agent` text,
      `model` text,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      `time_compacting` integer,
      `time_archived` integer,
      CONSTRAINT `fk_session_project_id_project_id_fk` FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `todo` (
      `session_id` text NOT NULL,
      `content` text NOT NULL,
      `status` text NOT NULL,
      `priority` text NOT NULL,
      `position` integer NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      CONSTRAINT `todo_pk` PRIMARY KEY(`session_id`, `position`),
      CONSTRAINT `fk_todo_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
    );"#,
    r#"CREATE TABLE `session_share` (
      `session_id` text PRIMARY KEY,
      `id` text NOT NULL,
      `secret` text NOT NULL,
      `url` text NOT NULL,
      `time_created` integer NOT NULL,
      `time_updated` integer NOT NULL,
      CONSTRAINT `fk_session_share_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
    );"#,
];

/// Noms des dix-sept index, dans l'ordre des `tx.run` de la source.
pub const INDEX_NAMES: [&str; 17] = [
    "event_aggregate_seq_idx",
    "event_aggregate_type_seq_idx",
    "permission_project_action_resource_idx",
    "message_session_time_created_id_idx",
    "part_message_id_id_idx",
    "part_session_idx",
    "session_input_session_pending_delivery_seq_idx",
    "session_input_session_admitted_seq_idx",
    "session_input_session_promoted_seq_idx",
    "session_message_session_seq_idx",
    "session_message_session_type_seq_idx",
    "session_message_session_time_created_id_idx",
    "session_message_time_created_idx",
    "session_project_idx",
    "session_workspace_idx",
    "session_parent_idx",
    "todo_session_idx",
];

/// Les dix-sept ordres `CREATE INDEX`, dans le meme ordre que [`INDEX_NAMES`].
pub const INDEX_DDL: [&str; 17] = [
    r#"CREATE UNIQUE INDEX `event_aggregate_seq_idx` ON `event` (`aggregate_id`,`seq`);"#,
    r#"CREATE INDEX `event_aggregate_type_seq_idx` ON `event` (`aggregate_id`,`type`,`seq`);"#,
    r#"CREATE UNIQUE INDEX `permission_project_action_resource_idx` ON `permission` (`project_id`,`action`,`resource`);"#,
    r#"CREATE INDEX `message_session_time_created_id_idx` ON `message` (`session_id`,`time_created`,`id`);"#,
    r#"CREATE INDEX `part_message_id_id_idx` ON `part` (`message_id`,`id`);"#,
    r#"CREATE INDEX `part_session_idx` ON `part` (`session_id`);"#,
    r#"CREATE INDEX `session_input_session_pending_delivery_seq_idx` ON `session_input` (`session_id`,`promoted_seq`,`delivery`,`admitted_seq`);"#,
    r#"CREATE UNIQUE INDEX `session_input_session_admitted_seq_idx` ON `session_input` (`session_id`,`admitted_seq`);"#,
    r#"CREATE UNIQUE INDEX `session_input_session_promoted_seq_idx` ON `session_input` (`session_id`,`promoted_seq`);"#,
    r#"CREATE UNIQUE INDEX `session_message_session_seq_idx` ON `session_message` (`session_id`,`seq`);"#,
    r#"CREATE INDEX `session_message_session_type_seq_idx` ON `session_message` (`session_id`,`type`,`seq`);"#,
    r#"CREATE INDEX `session_message_session_time_created_id_idx` ON `session_message` (`session_id`,`time_created`,`id`);"#,
    r#"CREATE INDEX `session_message_time_created_idx` ON `session_message` (`time_created`);"#,
    r#"CREATE INDEX `session_project_idx` ON `session` (`project_id`);"#,
    r#"CREATE INDEX `session_workspace_idx` ON `session` (`workspace_id`);"#,
    r#"CREATE INDEX `session_parent_idx` ON `session` (`parent_id`);"#,
    r#"CREATE INDEX `todo_session_idx` ON `todo` (`session_id`);"#,
];

/// Noms des cinq index uniques, dans l'ordre de la source.
pub const UNIQUE_INDEX_NAMES: [&str; 5] = [
    "event_aggregate_seq_idx",
    "permission_project_action_resource_idx",
    "session_input_session_admitted_seq_idx",
    "session_input_session_promoted_seq_idx",
    "session_message_session_seq_idx",
];

/// Nombre de tables du schema initial.
pub fn nombre_tables() -> usize {
    TABLE_NAMES.len()
}

/// Nombre d'index du schema initial.
pub fn nombre_index() -> usize {
    INDEX_NAMES.len()
}

/// L'ordre `CREATE TABLE` d'une table, ou `None` si le nom est inconnu.
pub fn ddl_table(nom: &str) -> Option<&'static str> {
    TABLE_NAMES
        .iter()
        .position(|table| {
            let table: &str = table;
            table == nom
        })
        .map(|position| TABLE_DDL[position])
}

/// L'ordre `CREATE INDEX` d'un index, ou `None` si le nom est inconnu.
pub fn ddl_index(nom: &str) -> Option<&'static str> {
    INDEX_NAMES
        .iter()
        .position(|index| {
            let index: &str = index;
            index == nom
        })
        .map(|position| INDEX_DDL[position])
}

/// Dit si un index est unique (`CREATE UNIQUE INDEX` dans la source).
pub fn est_index_unique(nom: &str) -> bool {
    UNIQUE_INDEX_NAMES.contains(&nom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_schema_compte_dix_neuf_tables_et_dix_sept_index() {
        assert_eq!(nombre_tables(), 19);
        assert_eq!(TABLE_DDL.len(), 19);
        assert_eq!(nombre_index(), 17);
        assert_eq!(INDEX_DDL.len(), 17);
    }

    #[test]
    fn la_premiere_table_est_workspace_et_la_derniere_session_share() {
        assert_eq!(TABLE_NAMES[0], "workspace");
        assert_eq!(TABLE_NAMES[TABLE_NAMES.len() - 1], "session_share");
    }

    #[test]
    fn chaque_ddl_de_table_cree_bien_sa_table() {
        for (nom, ddl) in TABLE_NAMES.iter().zip(TABLE_DDL.iter()) {
            let attente = format!("CREATE TABLE `{nom}`");
            assert!(
                ddl.contains(attente.as_str()),
                "le DDL de {nom} doit commencer par {attente}"
            );
        }
    }

    #[test]
    fn un_nom_inconnu_ne_rend_aucun_ddl() {
        assert!(ddl_table("absente").is_none());
        assert!(ddl_index("absent_idx").is_none());
    }

    #[test]
    fn la_recherche_par_nom_rend_le_ddl_de_la_bonne_table() {
        let ddl = ddl_table("session").expect("table session");
        assert!(ddl.contains("`cost` real DEFAULT 0 NOT NULL"));
        assert!(ddl.contains("`tokens_input` integer DEFAULT 0 NOT NULL"));
        assert!(ddl.contains("fk_session_project_id_project_id_fk"));
    }

    #[test]
    fn l_etat_des_comptes_cascade_la_suppression_du_compte() {
        let ddl = ddl_table("account_state").expect("table account_state");
        assert!(ddl.contains("ON DELETE SET NULL"));
    }

    #[test]
    fn les_cles_primaires_composites_gardent_leur_nom_de_contrainte() {
        let heritage = ddl_table("control_account").expect("table control_account");
        assert!(heritage.contains("CONSTRAINT `control_account_pk` PRIMARY KEY(`email`, `url`)"));
        let todo = ddl_table("todo").expect("table todo");
        assert!(todo.contains("CONSTRAINT `todo_pk` PRIMARY KEY(`session_id`, `position`)"));
    }

    #[test]
    fn cinq_index_sont_uniques_et_eux_seuls() {
        assert_eq!(UNIQUE_INDEX_NAMES.len(), 5);
        for nom in UNIQUE_INDEX_NAMES {
            let ddl = ddl_index(nom).expect("index unique connu");
            assert!(ddl.contains("CREATE UNIQUE INDEX"), "{nom} doit etre unique");
            assert!(est_index_unique(nom));
        }
        assert!(!est_index_unique("event_aggregate_type_seq_idx"));
        assert!(!est_index_unique("session_project_idx"));
        assert!(!est_index_unique("inconnu_idx"));
    }

    #[test]
    fn chaque_ddl_d_index_vise_sa_table() {
        let paires = [
            ("event_aggregate_seq_idx", "`event`"),
            ("permission_project_action_resource_idx", "`permission`"),
            ("message_session_time_created_id_idx", "`message`"),
            ("session_project_idx", "`session`"),
            ("todo_session_idx", "`todo`"),
        ];
        for (nom, table) in paires {
            let ddl = ddl_index(nom).expect("index connu");
            assert!(ddl.contains(table), "{nom} doit viser {table}");
        }
    }

    #[test]
    fn le_paquet_initial_n_a_pas_d_identifiant_de_migration() {
        // `satisfies Omit<Migration, "id">` : le schema initial scelle le
        // journal au lieu d'y entrer. Aucune table ne s'appelle `migration`.
        assert!(!TABLE_NAMES.contains(&"migration"));
        assert!(!TABLE_NAMES.contains(&"__drizzle_migrations"));
    }
}
