//! Portage de `packages/core/src/database/migration/20260413175956_chief_energizer.ts`.
//!
//! La migration cree la table `session_entry` et ses trois index. Chaque
//! entree porte un identifiant, la session mere, un type, deux horodatages
//! et une charge `data`. La cle etrangere vers `session(id)` supprime en
//! cascade.
//!
//! Sans pilote SQL dans ce crate, ce fichier porte le contrat sous forme de
//! donnees pures : identifiant, nom de table, colonnes dans l'ordre du
//! `CREATE TABLE`, cle etrangere, index dans l'ordre d'emission, textes SQL
//! exacts, et de petites fonctions pures qui decrivent les invariants
//! (unicite des index, colonnes couvertes).

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260413175956_chief_energizer";

/// Table creee par la migration.
pub const TABLE_NAME: &str = "session_entry";

/// Nom de la contrainte de cle etrangere vers `session(id)`.
pub const FK_SESSION_ENTRY_SESSION: &str = "fk_session_entry_session_id_session_id_fk";

/// Colonnes de `session_entry`, dans l'ordre du `CREATE TABLE`.
pub const COLUMNS: [&str; 6] = ["id", "session_id", "type", "time_created", "time_updated", "data"];

/// Index `session_entry_session_idx` sur `(session_id)`.
pub const INDEX_SESSION: &str = "session_entry_session_idx";

/// Index `session_entry_session_type_idx` sur `(session_id, type)`.
pub const INDEX_SESSION_TYPE: &str = "session_entry_session_type_idx";

/// Index `session_entry_time_created_idx` sur `(time_created)`.
pub const INDEX_TIME_CREATED: &str = "session_entry_time_created_idx";

/// Les trois index, dans l'ordre d'emission de la migration.
pub const INDEXES: [&str; 3] = [INDEX_SESSION, INDEX_SESSION_TYPE, INDEX_TIME_CREATED];

/// Ordre SQL de creation de la table.
pub const CREATE_TABLE_SQL: &str = "CREATE TABLE `session_entry` (
          `id` text PRIMARY KEY,
          `session_id` text NOT NULL,
          `type` text NOT NULL,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `data` text NOT NULL,
          CONSTRAINT `fk_session_entry_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
        );";

/// Ordre SQL du premier index.
pub const CREATE_INDEX_SESSION_SQL: &str =
    "CREATE INDEX `session_entry_session_idx` ON `session_entry` (`session_id`);";

/// Ordre SQL du deuxieme index.
pub const CREATE_INDEX_SESSION_TYPE_SQL: &str =
    "CREATE INDEX `session_entry_session_type_idx` ON `session_entry` (`session_id`,`type`);";

/// Ordre SQL du troisieme index.
pub const CREATE_INDEX_TIME_CREATED_SQL: &str =
    "CREATE INDEX `session_entry_time_created_idx` ON `session_entry` (`time_created`);";

/// Colonnes couvertes par un index, dans l'ordre de l'index.
pub fn colonnes_de_index(nom: &str) -> Option<&'static [&'static str]> {
    match nom {
        n if n == INDEX_SESSION => Some(&["session_id"]),
        n if n == INDEX_SESSION_TYPE => Some(&["session_id", "type"]),
        n if n == INDEX_TIME_CREATED => Some(&["time_created"]),
        _ => None,
    }
}

/// Vrai si la colonne fait partie de la cle etrangere vers `session`.
pub fn est_colonne_cle_etrangere(colonne: &str) -> bool {
    colonne == "session_id"
}

/// Nombre d'ordres SQL emis par `up` : une table puis trois index.
pub fn nombre_ordres() -> usize {
    4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260413175956_chief_energizer");
    }

    #[test]
    fn la_table_creee_s_appelle_session_entry() {
        assert_eq!(TABLE_NAME, "session_entry");
        assert!(CREATE_TABLE_SQL.contains("CREATE TABLE `session_entry`"));
    }

    #[test]
    fn les_six_colonnes_sont_dans_l_ordre_du_ddl() {
        assert_eq!(
            COLUMNS,
            ["id", "session_id", "type", "time_created", "time_updated", "data"]
        );
    }

    #[test]
    fn le_ddl_porte_la_cle_primaire_et_la_cle_etrangere_en_cascade() {
        assert!(CREATE_TABLE_SQL.contains("`id` text PRIMARY KEY"));
        assert!(CREATE_TABLE_SQL.contains(FK_SESSION_ENTRY_SESSION));
        assert!(CREATE_TABLE_SQL.contains("REFERENCES `session`(`id`) ON DELETE CASCADE"));
    }

    #[test]
    fn les_trois_index_sont_emis_dans_l_ordre_de_la_source() {
        assert_eq!(
            INDEXES,
            [
                "session_entry_session_idx",
                "session_entry_session_type_idx",
                "session_entry_time_created_idx"
            ]
        );
    }

    #[test]
    fn chaque_index_couvre_les_bonnes_colonnes_dans_le_bon_ordre() {
        assert_eq!(colonnes_de_index(INDEX_SESSION), Some(&["session_id"][..]));
        assert_eq!(
            colonnes_de_index(INDEX_SESSION_TYPE),
            Some(&["session_id", "type"][..])
        );
        assert_eq!(
            colonnes_de_index(INDEX_TIME_CREATED),
            Some(&["time_created"][..])
        );
    }

    #[test]
    fn un_index_inconnu_ne_couvre_aucune_colonne() {
        assert_eq!(colonnes_de_index("session_entry_inconnu_idx"), None);
        assert_eq!(colonnes_de_index(""), None);
    }

    #[test]
    fn seule_session_id_porte_la_cle_etrangere() {
        assert!(est_colonne_cle_etrangere("session_id"));
        for colonne in COLUMNS {
            if colonne != "session_id" {
                assert!(
                    !est_colonne_cle_etrangere(colonne),
                    "{} ne doit pas porter la cle",
                    colonne
                );
            }
        }
    }

    #[test]
    fn chaque_colonne_d_index_existe_dans_la_table() {
        for index in INDEXES {
            let colonnes = colonnes_de_index(index).expect("index connu");
            for colonne in colonnes {
                assert!(
                    COLUMNS.contains(colonne),
                    "l index {} cite une colonne absente : {}",
                    index,
                    colonne
                );
            }
        }
    }

    #[test]
    fn le_nombre_d_ordres_est_d_une_table_puis_trois_index() {
        assert_eq!(nombre_ordres(), 4);
        assert!(CREATE_INDEX_SESSION_SQL.contains(INDEX_SESSION));
        assert!(CREATE_INDEX_SESSION_TYPE_SQL.contains(INDEX_SESSION_TYPE));
        assert!(CREATE_INDEX_TIME_CREATED_SQL.contains(INDEX_TIME_CREATED));
    }
}
