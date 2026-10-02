//! Portage Rust de `packages/core/src/database/migration.gen.ts`.
//!
//! ## Ce que contient la source
//!
//! 44 lignes, un seul export :
//!
//! ```ts
//! import type { DatabaseMigration } from "./migration"
//! export const migrations = (
//!   await Promise.all([
//!     import("./migration/20260127222353_familiar_lady_ursula"),
//!     ... // trente-huit imports, un par fichier du dossier migration/
//!   ])
//! ).map((module) => module.default) satisfies DatabaseMigration.Migration[]
//! ```
//!
//! C'est le registre : l'ordre du tableau est l'ordre d'application, et chaque
//! element est le `default` du fichier, qui satisfait `DatabaseMigration.Migration`
//! (`{ id, up }`, voir `database_migration`). Les trente-huit migrations
//! elles-memes sont portees par les lanes 12 a 19, pas ici : ce fichier ne
//! porte que le registre, c'est-a-dire la liste ordonnee des identifiants.
//!
//! ## La forme des identifiants, qui est un contrat
//!
//! Chaque identifiant est `<horodatage>_<nom>` : quatorze chiffres puis un
//! souligne puis le nom lisible. C'est ce prefixe que `migration.ts` utilise
//! pour apparier l'historique Drizzle (`startsWith(prefixe + "_")`, voir
//! `trouver_migration_pour_prefixe` dans `database_migration`). Les tests
//! verrouillent donc la forme en plus de l'ordre.

/// Les trente-huit identifiants de migration, dans l'ordre d'application.
///
/// L'ordre est celui des imports de la source : c'est l'ordre dans lequel
/// `applyOnly` les applique. Chaque entree est le nom du fichier sans
/// l'extension, qui est aussi le `id` du `default` qu'il exporte.
pub const MIGRATION_IDS: [&str; 38] = [
    "20260127222353_familiar_lady_ursula",
    "20260211171708_add_project_commands",
    "20260213144116_wakeful_the_professor",
    "20260225215848_workspace",
    "20260227213759_add_session_workspace_id",
    "20260228203230_blue_harpoon",
    "20260303231226_add_workspace_fields",
    "20260309230000_move_org_to_state",
    "20260312043431_session_message_cursor",
    "20260323234822_events",
    "20260410174513_workspace-name",
    "20260413175956_chief_energizer",
    "20260423070820_add_icon_url_override",
    "20260427172553_slow_nightmare",
    "20260428004200_add_session_path",
    "20260501142318_next_venus",
    "20260504145000_add_sync_owner",
    "20260507164347_add_workspace_time",
    "20260510033149_session_usage",
    "20260511000411_data_migration_state",
    "20260511173437_session-metadata",
    "20260601010001_normalize_storage_paths",
    "20260601202201_amazing_prowler",
    "20260602002951_lowly_union_jack",
    "20260602182828_add_project_directories",
    "20260603001617_session_message_projection_indexes",
    "20260603040000_session_message_projection_order",
    "20260603141458_session_input_inbox",
    "20260603160727_jittery_ezekiel_stane",
    "20260604172448_event_sourced_session_input",
    "20260605003541_add_session_context_snapshot",
    "20260605042240_add_context_epoch_agent",
    "20260611035744_credential",
    "20260611192811_lush_chimera",
    "20260612174303_project_dir_strategy",
    "20260622142730_simplify_session_context_epoch",
    "20260622170816_reset_v2_session_state",
    "20260622202450_simplify_session_input",
];

/// Nombre de migrations du registre.
pub fn nombre_migrations() -> usize {
    MIGRATION_IDS.len()
}

/// L'identifiant a une position donnee, ou `None` hors registre.
pub fn id_migration(position: usize) -> Option<&'static str> {
    MIGRATION_IDS.get(position).copied()
}

/// La position d'un identifiant dans l'ordre d'application, ou `None`.
pub fn position_migration(id: &str) -> Option<usize> {
    MIGRATION_IDS.iter().position(|connu| *connu == id)
}

/// Dit si un identifiant a la forme `<14 chiffres>_<nom>` du registre.
pub fn a_la_forme_de_registre(id: &str) -> bool {
    let mut caracteres = id.chars();
    for _ in 0..14 {
        match caracteres.next() {
            Some(c) if c.is_ascii_digit() => {}
            _ => return false,
        }
    }
    matches!(caracteres.next(), Some('_'))
        && caracteres.next().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn le_registre_compte_trente_huit_migrations() {
        assert_eq!(nombre_migrations(), 38);
    }

    #[test]
    fn la_premiere_et_la_derniere_sont_celles_de_la_source() {
        assert_eq!(
            id_migration(0),
            Some("20260127222353_familiar_lady_ursula")
        );
        assert_eq!(
            id_migration(37),
            Some("20260622202450_simplify_session_input")
        );
        assert_eq!(id_migration(38), None);
    }

    #[test]
    fn la_recherche_par_nom_rend_la_position_d_application() {
        assert_eq!(position_migration("20260323234822_events"), Some(9));
        assert_eq!(
            position_migration("20260511173437_session-metadata"),
            Some(20)
        );
        assert_eq!(position_migration("absente"), None);
    }

    #[test]
    fn chaque_identifiant_est_unique_dans_le_registre() {
        let uniques: BTreeSet<&&str> = MIGRATION_IDS.iter().collect();
        assert_eq!(uniques.len(), MIGRATION_IDS.len());
    }

    #[test]
    fn chaque_identifiant_a_la_forme_horodatage_souligne_nom() {
        for id in MIGRATION_IDS {
            assert!(
                a_la_forme_de_registre(id),
                "{id} doit etre <14 chiffres>_<nom>"
            );
        }
        assert!(!a_la_forme_de_registre("20260127_trop_court"));
        assert!(!a_la_forme_de_registre("20260127222353"));
        assert!(!a_la_forme_de_registre("20260127222353_"));
        assert!(!a_la_forme_de_registre("pas_un_horodatage_x"));
    }

    #[test]
    fn l_ordre_est_croissant_comme_les_imports_de_la_source() {
        let mut precedent = "";
        for id in MIGRATION_IDS {
            assert!(
                id > precedent,
                "{id} doit suivre {precedent} dans l'ordre"
            );
            precedent = id;
        }
    }
}
