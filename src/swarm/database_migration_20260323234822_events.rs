//! Portage de `packages/core/src/database/migration/20260323234822_events.ts`.
//!
//! La source tient en 26 lignes et exporte une migration `DatabaseMigration`
//! qui execute deux `tx.run` dans l ordre : creation de la table
//! `event_sequence`, puis creation de la table `event` avec une cle
//! etrangere vers `event_sequence` en `ON DELETE CASCADE`.
//!
//! Sans pilote SQL dans ce crate, le portage est en donnees pures :
//! identifiant, deux instructions dans l ordre, noms des tables et des
//! colonnes, et helpers qui decrivent la cle etrangere. Aucune base n est
//! ouverte ici.

/// Identifiant de la migration, tel qu ecrit dans la source.
pub const MIGRATION_ID: &str = "20260323234822_events";

/// Premiere instruction : creation de la table `event_sequence`.
pub const STATEMENT_CREATE_SEQUENCE: &str = "CREATE TABLE `event_sequence` (`aggregate_id` text PRIMARY KEY, `seq` integer NOT NULL);";

/// Seconde instruction : creation de la table `event`.
pub const STATEMENT_CREATE_EVENT: &str = "CREATE TABLE `event` (`id` text PRIMARY KEY, `aggregate_id` text NOT NULL, `seq` integer NOT NULL, `type` text NOT NULL, `data` text NOT NULL, CONSTRAINT `fk_event_aggregate_id_event_sequence_aggregate_id_fk` FOREIGN KEY (`aggregate_id`) REFERENCES `event_sequence`(`aggregate_id`) ON DELETE CASCADE);";

/// Les instructions de la migration, dans l ordre d execution de `up`.
pub const STATEMENTS: [&str; 2] = [STATEMENT_CREATE_SEQUENCE, STATEMENT_CREATE_EVENT];

/// Nom de la contrainte de cle etrangere de `event`.
pub const EVENT_FOREIGN_KEY_NAME: &str =
    "fk_event_aggregate_id_event_sequence_aggregate_id_fk";

/// Colonnes de `event_sequence`, dans l ordre du `CREATE TABLE`.
pub const SEQUENCE_COLUMNS: [&str; 2] = ["aggregate_id", "seq"];

/// Colonnes de `event`, dans l ordre du `CREATE TABLE`.
pub const EVENT_COLUMNS: [&str; 5] = ["id", "aggregate_id", "seq", "type", "data"];

/// Le nombre d instructions de la migration.
pub fn nombre_d_instructions() -> usize {
    STATEMENTS.len()
}

/// Vrai si la table `event` declare sa cle etrangere vers `event_sequence`.
pub fn declare_cle_vers_sequence() -> bool {
    STATEMENT_CREATE_EVENT.contains("REFERENCES `event_sequence`(`aggregate_id`)")
        && STATEMENT_CREATE_EVENT.contains("ON DELETE CASCADE")
}

/// Le nom de la contrainte est-il repris dans l instruction ?
pub fn contrainte_est_nommee() -> bool {
    STATEMENT_CREATE_EVENT.contains(EVENT_FOREIGN_KEY_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260323234822_events");
    }

    #[test]
    fn la_migration_execute_deux_instructions() {
        assert_eq!(nombre_d_instructions(), 2);
    }

    #[test]
    fn l_ordre_d_execution_cree_la_sequence_puis_l_evenement() {
        assert!(STATEMENTS[0].contains("CREATE TABLE `event_sequence`"));
        assert!(STATEMENTS[1].contains("CREATE TABLE `event`"));
    }

    #[test]
    fn les_colonnes_suivent_l_ordre_du_ddl() {
        assert_eq!(SEQUENCE_COLUMNS.to_vec(), vec!["aggregate_id", "seq"]);
        assert_eq!(
            EVENT_COLUMNS.to_vec(),
            vec!["id", "aggregate_id", "seq", "type", "data"]
        );
    }

    #[test]
    fn la_cle_etrangere_cascade_vers_la_sequence() {
        assert!(declare_cle_vers_sequence());
        assert!(contrainte_est_nommee());
    }

    #[test]
    fn la_sequence_ne_reference_personne() {
        assert!(!STATEMENT_CREATE_SEQUENCE.contains("REFERENCES"));
        assert!(!STATEMENT_CREATE_SEQUENCE.contains("FOREIGN KEY"));
    }
}
