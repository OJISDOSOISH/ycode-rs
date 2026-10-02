//! Portage de `packages/core/src/database/migration/20260604172448_event_sourced_session_input.ts`.
//!
//! La source fait 47 lignes et n'exporte qu'un objet migration : un `id` et
//! une fonction `up` qui enchaine dix-huit ordres SQL via `tx.run`. Il n'y a
//! aucune lecture, aucun branchement, aucune valeur de retour : `up` execute
//! les ordres dans l'ordre, et echoue si l'un d'eux echoue.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce portage
//! ne peut donc ni ouvrir de base ni executer quoi que ce soit. Il transcrit
//! le contrat de la migration sous forme de donnees pures, interrogeables
//! sans pilote : l'identifiant et la liste ordonnee des ordres SQL, tels
//! qu'ils sont passes a `tx.run` dans la source.
//!
//! Ce que fait `up`, dans l'ordre (lignes 8 a 44 de la source) :
//!
//! 1. Quatre `DELETE FROM` sur `session_input`, `session_message`, `event` et
//!    `event_sequence` : vide les tables dont le contenu pre-existant ne peut
//!    pas recevoir un ordre d'agregat veridique.
//! 2. `UPDATE session SET workspace_id = NULL`, puis `DELETE FROM workspace` :
//!    detache les sessions de leurs espaces avant de vider la table des
//!    espaces.
//! 3. `DROP INDEX` puis `CREATE UNIQUE INDEX event_aggregate_seq_idx ON event
//!    (aggregate_id, seq)` : la position d'un evenement dans son agregat
//!    devient unique.
//! 4. `DROP INDEX` puis `CREATE UNIQUE INDEX session_message_session_seq_idx
//!    ON session_message (session_id, seq)` : la position d'un message
//!    projete dans sa session devient unique.
//! 5. Reconstruction de `session_input` avec les cles etrangeres coupees :
//!    `PRAGMA foreign_keys=OFF`, `CREATE TABLE __new_session_input` (huit
//!    colonnes dont la nouvelle `admitted_seq` non nulle, `id` texte en cle
//!    primaire, meme cle etrangere vers `session`), `DROP TABLE
//!    session_input`, `ALTER TABLE __new_session_input RENAME TO
//!    session_input`, `PRAGMA foreign_keys=ON`.
//! 6. Trois index sur la table reconstruite : `CREATE INDEX
//!    session_input_session_pending_delivery_seq_idx` sur `(session_id,
//!    promoted_seq, delivery, admitted_seq)`, `CREATE UNIQUE INDEX
//!    session_input_session_admitted_seq_idx` sur `(session_id,
//!    admitted_seq)`, et `CREATE UNIQUE INDEX
//!    session_input_session_promoted_seq_idx` sur `(session_id,
//!    promoted_seq)`.
//!
//! Notes sur la forme : la source ecrit le `CREATE TABLE` sur plusieurs
//! lignes dans un gabarit, avec retours a la ligne et indentation. La chaine
//! portee ici en est la version normalisee sur une seule ligne : SQLite
//! ignore les blancs hors chaines, les deux formes executent donc le meme
//! ordre. Les noms de table, de colonnes, de contrainte et d'index sont
//! repris a l'identique.
//!
//! Le piege de cette migration est l'unicite de `promoted_seq` : la colonne
//! reste nulle quand l'entree n'est pas promue, et SQLite considere deux
//! `NULL` comme distincts, si bien que l'index unique ne bloque que les
//! doublons de valeurs promues.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260604172448_event_sourced_session_input";

/// Ordres SQL executes par `up`, dans l'ordre de la source.
///
/// Chaque entree est la chaine passee a `tx.run` (normalisee sur une ligne
/// pour le `CREATE TABLE` multi-ligne de la source) : le point-virgule final
/// en fait partie, et l'ordre des entrees est l'ordre d'execution.
pub const STATEMENTS: &[&str] = &[
    "DELETE FROM `session_input`;",
    "DELETE FROM `session_message`;",
    "DELETE FROM `event`;",
    "DELETE FROM `event_sequence`;",
    "UPDATE `session` SET `workspace_id` = NULL;",
    "DELETE FROM `workspace`;",
    "DROP INDEX IF EXISTS `event_aggregate_seq_idx`;",
    "CREATE UNIQUE INDEX `event_aggregate_seq_idx` ON `event` (`aggregate_id`,`seq`);",
    "DROP INDEX IF EXISTS `session_message_session_seq_idx`;",
    "CREATE UNIQUE INDEX `session_message_session_seq_idx` ON `session_message` (`session_id`,`seq`);",
    "PRAGMA foreign_keys=OFF;",
    "CREATE TABLE `__new_session_input` (`id` text PRIMARY KEY, `session_id` text NOT NULL, `prompt` text NOT NULL, `delivery` text NOT NULL, `admitted_seq` integer NOT NULL, `promoted_seq` integer, `time_created` integer NOT NULL, CONSTRAINT `fk_session_input_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE);",
    "DROP TABLE `session_input`;",
    "ALTER TABLE `__new_session_input` RENAME TO `session_input`;",
    "PRAGMA foreign_keys=ON;",
    "CREATE INDEX `session_input_session_pending_delivery_seq_idx` ON `session_input` (`session_id`,`promoted_seq`,`delivery`,`admitted_seq`);",
    "CREATE UNIQUE INDEX `session_input_session_admitted_seq_idx` ON `session_input` (`session_id`,`admitted_seq`);",
    "CREATE UNIQUE INDEX `session_input_session_promoted_seq_idx` ON `session_input` (`session_id`,`promoted_seq`);",
];

/// Retourne l'identifiant de la migration.
pub fn migration_id() -> &'static str {
    MIGRATION_ID
}

/// Retourne les ordres SQL de la migration, dans leur ordre d'execution.
pub fn statements() -> &'static [&'static str] {
    STATEMENTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_celui_declare_dans_la_source() {
        assert_eq!(MIGRATION_ID, "20260604172448_event_sourced_session_input");
        assert_eq!(migration_id(), MIGRATION_ID);
    }

    #[test]
    fn up_execute_exactement_dix_huit_ordres() {
        assert_eq!(STATEMENTS.len(), 18);
        assert_eq!(statements().len(), 18);
    }

    #[test]
    fn tous_les_ordres_sont_non_vides_et_termines_par_un_point_virgule() {
        for ordre in STATEMENTS {
            assert!(!ordre.trim().is_empty(), "aucun ordre ne doit etre vide");
            assert!(
                ordre.ends_with(';'),
                "chaque ordre se termine par un point-virgule : {ordre}"
            );
        }
    }

    #[test]
    fn les_accesseurs_renvoient_les_constantes_du_module() {
        assert_eq!(migration_id(), MIGRATION_ID);
        assert_eq!(statements(), STATEMENTS);
    }

    #[test]
    fn les_quatre_premiers_ordres_vident_les_tables_evenementielles() {
        assert_eq!(STATEMENTS[0], "DELETE FROM `session_input`;");
        assert_eq!(STATEMENTS[1], "DELETE FROM `session_message`;");
        assert_eq!(STATEMENTS[2], "DELETE FROM `event`;");
        assert_eq!(STATEMENTS[3], "DELETE FROM `event_sequence`;");
    }

    #[test]
    fn les_sessions_sont_detachees_avant_la_vidange_des_espaces() {
        assert_eq!(STATEMENTS[4], "UPDATE `session` SET `workspace_id` = NULL;");
        assert_eq!(STATEMENTS[5], "DELETE FROM `workspace`;");
    }

    #[test]
    fn les_positions_d_evenement_et_de_message_deviennent_uniques() {
        assert!(STATEMENTS[6].starts_with("DROP INDEX IF EXISTS"));
        assert!(STATEMENTS[7].starts_with("CREATE UNIQUE INDEX `event_aggregate_seq_idx`"));
        assert!(STATEMENTS[7].contains("`event`"));
        assert!(STATEMENTS[8].starts_with("DROP INDEX IF EXISTS"));
        assert!(STATEMENTS[9].starts_with("CREATE UNIQUE INDEX `session_message_session_seq_idx`"));
        assert!(STATEMENTS[9].contains("`session_message`"));
    }

    #[test]
    fn la_reconstruction_coupe_puis_retablit_les_cles_etrangeres() {
        assert_eq!(STATEMENTS[10], "PRAGMA foreign_keys=OFF;");
        assert!(STATEMENTS[11].starts_with("CREATE TABLE `__new_session_input`"));
        assert_eq!(STATEMENTS[12], "DROP TABLE `session_input`;");
        assert_eq!(
            STATEMENTS[13],
            "ALTER TABLE `__new_session_input` RENAME TO `session_input`;"
        );
        assert_eq!(STATEMENTS[14], "PRAGMA foreign_keys=ON;");
    }

    #[test]
    fn la_table_reconstruite_ajoute_admitted_seq_non_nul() {
        let creation = STATEMENTS[11];
        assert!(creation.contains("`id` text PRIMARY KEY"));
        assert!(creation.contains("`admitted_seq` integer NOT NULL"));
        assert!(creation.contains("`promoted_seq` integer"));
        assert!(creation.contains("fk_session_input_session_id_session_id_fk"));
        assert!(creation.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn les_trois_derniers_ordres_indexent_la_table_reconstruite() {
        assert!(STATEMENTS[15].contains("session_input_session_pending_delivery_seq_idx"));
        assert!(STATEMENTS[15].contains("`admitted_seq`"));
        assert!(STATEMENTS[16].starts_with("CREATE UNIQUE INDEX"));
        assert!(STATEMENTS[16].contains("session_input_session_admitted_seq_idx"));
        assert!(STATEMENTS[17].starts_with("CREATE UNIQUE INDEX"));
        assert!(STATEMENTS[17].contains("session_input_session_promoted_seq_idx"));
    }
}
