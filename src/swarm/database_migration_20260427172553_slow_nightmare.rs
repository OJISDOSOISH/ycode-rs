//! Portage de `packages/core/src/database/migration/20260427172553_slow_nightmare.ts`.
//!
//! La migration renomme conceptuellement `session_entry` en
//! `session_message` : elle cree `session_message` avec le meme schema a six
//! colonnes, supprime les trois index de `session_entry`, cree les trois
//! index jumeaux sur `session_message`, puis supprime `session_entry`.
//!
//! Sans pilote SQL dans ce crate, ce fichier porte l'identifiant, les noms
//! de tables et d'index, les textes SQL exacts, et les fonctions pures qui
//! font tenir la correspondance (table source vers table cible, index source
//! vers index cible, colonnes couvertes).

/// Identifiant de la migration, repris tel quel de la source.
pub const MIGRATION_ID: &str = "20260427172553_slow_nightmare";

/// Table supprimee en fin de migration.
pub const OLD_TABLE: &str = "session_entry";

/// Table creee par la migration.
pub const NEW_TABLE: &str = "session_message";

/// Colonnes des deux tables, dans l'ordre du `CREATE TABLE`.
///
/// Les deux schemas sont identiques colonne pour colonne : c'est ce qui rend
/// le renommage honnete.
pub const COLUMNS: [&str; 6] = ["id", "session_id", "type", "time_created", "time_updated", "data"];

/// Les trois index supprimes, dans l'ordre des `DROP INDEX`.
pub const DROPPED_INDEXES: [&str; 3] = [
    "session_entry_session_idx",
    "session_entry_session_type_idx",
    "session_entry_time_created_idx",
];

/// Les trois index crees, dans l'ordre des `CREATE INDEX`.
pub const CREATED_INDEXES: [&str; 3] = [
    "session_message_session_idx",
    "session_message_session_type_idx",
    "session_message_time_created_idx",
];

/// Creation de `session_message`.
pub const CREATE_TABLE_SQL: &str = "CREATE TABLE `session_message` (
          `id` text PRIMARY KEY,
          `session_id` text NOT NULL,
          `type` text NOT NULL,
          `time_created` integer NOT NULL,
          `time_updated` integer NOT NULL,
          `data` text NOT NULL,
          CONSTRAINT `fk_session_message_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE
        );";

/// Nom de la contrainte de cle etrangere de la nouvelle table.
pub const FK_NEW_TABLE: &str = "fk_session_message_session_id_session_id_fk";

/// Rend l'index de `session_message` correspondant a un index de
/// `session_entry`, ou `None` si le nom ne suit pas la convention.
///
/// La correspondance est un simple remplacement de prefixe
/// `session_entry_` par `session_message_`.
pub fn index_jumeau(nom_ancien: &str) -> Option<&'static str> {
    match nom_ancien {
        "session_entry_session_idx" => Some("session_message_session_idx"),
        "session_entry_session_type_idx" => Some("session_message_session_type_idx"),
        "session_entry_time_created_idx" => Some("session_message_time_created_idx"),
        _ => None,
    }
}

/// Colonnes couvertes par un index de la nouvelle table.
pub fn colonnes_de_nouvel_index(nom: &str) -> Option<&'static [&'static str]> {
    match nom {
        "session_message_session_idx" => Some(&["session_id"]),
        "session_message_session_type_idx" => Some(&["session_id", "type"]),
        "session_message_time_created_idx" => Some(&["time_created"]),
        _ => None,
    }
}

/// Nombre d'ordres SQL emis par `up` : une creation, trois suppressions
/// d'index, trois creations d'index, une suppression de table.
pub fn nombre_ordres() -> usize {
    8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_reprend_le_nom_du_fichier() {
        assert_eq!(MIGRATION_ID, "20260427172553_slow_nightmare");
    }

    #[test]
    fn l_ancienne_et_la_nouvelle_table_ont_des_noms_distincts() {
        assert_eq!(OLD_TABLE, "session_entry");
        assert_eq!(NEW_TABLE, "session_message");
        assert_ne!(OLD_TABLE, NEW_TABLE);
    }

    #[test]
    fn les_deux_tables_partagent_le_meme_schema_a_six_colonnes() {
        assert_eq!(
            COLUMNS,
            ["id", "session_id", "type", "time_created", "time_updated", "data"]
        );
        assert!(CREATE_TABLE_SQL.contains("CREATE TABLE `session_message`"));
        assert!(CREATE_TABLE_SQL.contains("`id` text PRIMARY KEY"));
    }

    #[test]
    fn le_ddl_porte_la_cle_etrangere_en_cascade_vers_session() {
        assert!(CREATE_TABLE_SQL.contains(FK_NEW_TABLE));
        assert!(CREATE_TABLE_SQL.contains("REFERENCES `session`(`id`) ON DELETE CASCADE"));
    }

    #[test]
    fn les_index_supprimes_sont_ceux_de_l_ancienne_table() {
        assert_eq!(DROPPED_INDEXES.len(), 3);
        for index in DROPPED_INDEXES {
            assert!(
                index.starts_with("session_entry_"),
                "index inattendu : {}",
                index
            );
        }
    }

    #[test]
    fn les_index_crees_sont_ceux_de_la_nouvelle_table() {
        assert_eq!(CREATED_INDEXES.len(), 3);
        for index in CREATED_INDEXES {
            assert!(
                index.starts_with("session_message_"),
                "index inattendu : {}",
                index
            );
        }
    }

    #[test]
    fn chaque_index_supprime_a_exactement_un_jumeau_cree() {
        for ancien in DROPPED_INDEXES {
            let jumeau = index_jumeau(ancien).expect("jumeau attendu");
            assert!(
                CREATED_INDEXES.contains(&jumeau),
                "le jumeau {} manque aux index crees",
                jumeau
            );
        }
    }

    #[test]
    fn un_nom_hors_convention_n_a_pas_de_jumeau() {
        assert_eq!(index_jumeau("session_entry_inconnu_idx"), None);
        assert_eq!(index_jumeau("session_message_session_idx"), None);
        assert_eq!(index_jumeau(""), None);
    }

    #[test]
    fn chaque_nouvel_index_couvre_les_bonnes_colonnes() {
        assert_eq!(
            colonnes_de_nouvel_index("session_message_session_idx"),
            Some(&["session_id"][..])
        );
        assert_eq!(
            colonnes_de_nouvel_index("session_message_session_type_idx"),
            Some(&["session_id", "type"][..])
        );
        assert_eq!(
            colonnes_de_nouvel_index("session_message_time_created_idx"),
            Some(&["time_created"][..])
        );
    }

    #[test]
    fn chaque_colonne_d_index_existe_dans_le_schema() {
        for index in CREATED_INDEXES {
            let colonnes = colonnes_de_nouvel_index(index).expect("index connu");
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
    fn le_nombre_d_ordres_est_d_un_cree_trois_trois_puis_un_drop() {
        assert_eq!(nombre_ordres(), 8);
    }
}
