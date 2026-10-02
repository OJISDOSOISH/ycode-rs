//! Portage de `packages/core/src/database/migration/20260603160727_jittery_ezekiel_stane.ts`.
//!
//! La source fait 20 lignes et n'exporte qu'un objet migration : un `id` et
//! une fonction `up` qui enchaine quatre ordres SQL via `tx.run`. Il n'y a
//! aucune lecture, aucun branchement, aucune valeur de retour : `up` execute
//! les ordres dans l'ordre, et echoue si l'un d'eux echoue.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce portage
//! ne peut donc ni ouvrir de base ni executer quoi que ce soit. Il transcrit
//! le contrat de la migration sous forme de donnees pures, interrogeables
//! sans pilote : l'identifiant et la liste ordonnee des ordres SQL, tels
//! qu'ils sont passes a `tx.run` dans la source.
//!
//! Ce que fait `up`, dans l'ordre (lignes 8 a 17 de la source) :
//!
//! 1. `DROP INDEX IF EXISTS session_input_session_pending_seq_idx` : retire
//!    l'index pose par la migration `20260603141458_session_input_inbox`, que
//!    l'ordre suivant remplace par une version elargie.
//! 2. `CREATE INDEX IF NOT EXISTS event_aggregate_type_seq_idx ON event
//!    (aggregate_id, type, seq)` : index de rejeu des evenements d'un
//!    agregat, restreint a un type.
//! 3. `CREATE INDEX IF NOT EXISTS
//!    session_input_session_pending_delivery_seq_idx ON session_input
//!    (session_id, promoted_seq, delivery, seq)` : remplace l'index retire
//!    en ajoutant la colonne `delivery` au chemin d'acces des entrees en
//!    attente.
//! 4. `CREATE INDEX IF NOT EXISTS
//!    session_message_session_time_created_id_idx ON session_message
//!    (session_id, time_created, id)` : reaffirme l'index de lecture des
//!    projections dans l'ordre de creation, au cas ou une base anterieure
//!    ne l'aurait pas.
//!
//! Les trois creations sont conditionnelles grace a `IF NOT EXISTS` : rejouer
//! la migration sur une base qui porte deja ces index ne fait rien, au lieu
//! d'echouer.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260603160727_jittery_ezekiel_stane";

/// Ordres SQL executes par `up`, dans l'ordre de la source.
///
/// Chaque entree est la chaine passee a `tx.run` : le point-virgule final en
/// fait partie, et l'ordre des entrees est l'ordre d'execution.
pub const STATEMENTS: &[&str] = &[
    "DROP INDEX IF EXISTS `session_input_session_pending_seq_idx`;",
    "CREATE INDEX IF NOT EXISTS `event_aggregate_type_seq_idx` ON `event` (`aggregate_id`,`type`,`seq`);",
    "CREATE INDEX IF NOT EXISTS `session_input_session_pending_delivery_seq_idx` ON `session_input` (`session_id`,`promoted_seq`,`delivery`,`seq`);",
    "CREATE INDEX IF NOT EXISTS `session_message_session_time_created_id_idx` ON `session_message` (`session_id`,`time_created`,`id`);",
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
        assert_eq!(MIGRATION_ID, "20260603160727_jittery_ezekiel_stane");
        assert_eq!(migration_id(), MIGRATION_ID);
    }

    #[test]
    fn up_execute_exactement_quatre_ordres() {
        assert_eq!(STATEMENTS.len(), 4);
        assert_eq!(statements().len(), 4);
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
    fn le_premier_ordre_retire_l_index_remplace_de_la_boite_de_reception() {
        assert!(STATEMENTS[0].starts_with("DROP INDEX IF EXISTS"));
        assert!(STATEMENTS[0].contains("session_input_session_pending_seq_idx"));
    }

    #[test]
    fn le_deuxieme_ordre_indexe_les_evenements_par_agregat_et_type() {
        assert!(STATEMENTS[1].starts_with("CREATE INDEX IF NOT EXISTS"));
        assert!(STATEMENTS[1].contains("event_aggregate_type_seq_idx"));
        assert!(STATEMENTS[1].contains("`event`"));
        assert!(STATEMENTS[1].contains("`type`"));
        assert!(STATEMENTS[1].contains("`seq`"));
    }

    #[test]
    fn le_troisieme_ordre_elargit_l_index_avec_la_colonne_delivery() {
        assert!(STATEMENTS[2].contains("session_input_session_pending_delivery_seq_idx"));
        assert!(STATEMENTS[2].contains("`session_input`"));
        assert!(STATEMENTS[2].contains("`delivery`"));
        assert!(!STATEMENTS[2].contains("`admitted_seq`"));
    }

    #[test]
    fn le_dernier_ordre_reaffirme_l_index_temporel_des_messages() {
        assert!(STATEMENTS[3].contains("session_message_session_time_created_id_idx"));
        assert!(STATEMENTS[3].contains("`session_message`"));
        assert!(STATEMENTS[3].contains("`time_created`"));
    }

    #[test]
    fn les_creations_sont_conditionnelles_mais_pas_la_suppression() {
        assert!(!STATEMENTS[0].contains("IF NOT EXISTS"));
        for ordre in &STATEMENTS[1..] {
            assert!(
                ordre.contains("IF NOT EXISTS"),
                "rejouer la creation doit etre sans effet : {ordre}"
            );
        }
    }
}
