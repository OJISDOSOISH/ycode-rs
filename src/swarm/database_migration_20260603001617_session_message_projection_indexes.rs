//! Portage de `packages/core/src/database/migration/20260603001617_session_message_projection_indexes.ts`.
//!
//! La source fait 19 lignes et n'exporte qu'un objet migration : un `id` et
//! une fonction `up` qui enchaine cinq ordres SQL via `tx.run`. Il n'y a
//! aucune lecture, aucun branchement, aucune valeur de retour : `up` execute
//! les ordres dans l'ordre, et echoue si l'un d'eux echoue.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce portage
//! ne peut donc ni ouvrir de base ni executer quoi que ce soit. Il transcrit
//! le contrat de la migration sous forme de donnees pures, interrogeables
//! sans pilote : l'identifiant et la liste ordonnee des ordres SQL, tels
//! qu'ils sont passes a `tx.run` dans la source.
//!
//! Ce que fait `up`, dans l'ordre (lignes 8 a 16 de la source) :
//!
//! 1. `DROP INDEX IF EXISTS session_message_session_idx` : retire l'ancien
//!    index de lecture des messages par session.
//! 2. `DROP INDEX IF EXISTS session_message_session_type_idx` : retire l'ancien
//!    index de lecture des messages par session et par type.
//! 3. `CREATE INDEX event_aggregate_seq_idx ON event (aggregate_id, seq)` :
//!    index de rejeu des agregats d'evenements par numero de sequence.
//! 4. `CREATE INDEX session_message_session_time_created_id_idx ON
//!    session_message (session_id, time_created, id)` : lecture des messages
//!    projetes d'une session dans l'ordre de creation.
//! 5. `CREATE INDEX session_message_session_type_time_created_id_idx ON
//!    session_message (session_id, type, time_created, id)` : meme lecture,
//!    restreinte a un type de message.
//!
//! Les suppressions precedent les creations : un index homonyme residuel ne
//! peut donc pas faire echouer une creation, et les deux `DROP` sont
//! idempotents grace a `IF EXISTS`.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260603001617_session_message_projection_indexes";

/// Ordres SQL executes par `up`, dans l'ordre de la source.
///
/// Chaque entree est la chaine passee a `tx.run` : le point-virgule final en
/// fait partie, et l'ordre des entrees est l'ordre d'execution.
pub const STATEMENTS: &[&str] = &[
    "DROP INDEX IF EXISTS `session_message_session_idx`;",
    "DROP INDEX IF EXISTS `session_message_session_type_idx`;",
    "CREATE INDEX `event_aggregate_seq_idx` ON `event` (`aggregate_id`,`seq`);",
    "CREATE INDEX `session_message_session_time_created_id_idx` ON `session_message` (`session_id`,`time_created`,`id`);",
    "CREATE INDEX `session_message_session_type_time_created_id_idx` ON `session_message` (`session_id`,`type`,`time_created`,`id`);",
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
        assert_eq!(MIGRATION_ID, "20260603001617_session_message_projection_indexes");
        assert_eq!(migration_id(), MIGRATION_ID);
    }

    #[test]
    fn up_execute_exactement_cinq_ordres() {
        assert_eq!(STATEMENTS.len(), 5);
        assert_eq!(statements().len(), 5);
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
    fn les_deux_premiers_ordres_suppriment_les_anciens_index() {
        assert!(STATEMENTS[0].starts_with("DROP INDEX IF EXISTS"));
        assert!(STATEMENTS[0].contains("session_message_session_idx"));
        assert!(STATEMENTS[1].starts_with("DROP INDEX IF EXISTS"));
        assert!(STATEMENTS[1].contains("session_message_session_type_idx"));
    }

    #[test]
    fn le_troisieme_ordre_indexe_les_agregats_d_evenements() {
        assert!(STATEMENTS[2].starts_with("CREATE INDEX"));
        assert!(STATEMENTS[2].contains("event_aggregate_seq_idx"));
        assert!(STATEMENTS[2].contains("`event`"));
        assert!(STATEMENTS[2].contains("`aggregate_id`"));
        assert!(STATEMENTS[2].contains("`seq`"));
    }

    #[test]
    fn les_deux_derniers_ordres_indexent_les_projections_de_messages() {
        assert!(STATEMENTS[3].contains("session_message_session_time_created_id_idx"));
        assert!(STATEMENTS[3].contains("`session_message`"));
        assert!(STATEMENTS[3].contains("`session_id`"));
        assert!(STATEMENTS[3].contains("`time_created`"));
        assert!(STATEMENTS[4].contains("session_message_session_type_time_created_id_idx"));
        assert!(STATEMENTS[4].contains("`session_message`"));
        assert!(STATEMENTS[4].contains("`type`"));
    }

    #[test]
    fn l_ordre_d_execution_place_les_suppressions_avant_les_creations() {
        let premier_create = STATEMENTS
            .iter()
            .position(|ordre| ordre.starts_with("CREATE"))
            .expect("la migration cree au moins un index");
        for ordre in &STATEMENTS[..premier_create] {
            assert!(
                ordre.starts_with("DROP"),
                "avant la premiere creation, que des suppressions : {ordre}"
            );
        }
        assert_eq!(premier_create, 2);
    }
}
