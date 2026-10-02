//! Portage de `packages/core/src/database/migration.gen.ts`.
//!
//! La source fait 44 lignes et n'exporte qu'une constante : `migrations`,
//! le tableau des 38 migrations dans leur ordre d'application, construit en
//! important chaque module de `database/migration/` puis en prenant son
//! export par defaut. Le `satisfies DatabaseMigration.Migration[]` verifie
//! que chaque module expose bien un `id` et un `up`.
//!
//! `Cargo.toml` ne declare aucun pilote SQL et les 38 modules de migration
//! vivent dans d'autres portages : ce fichier ne peut donc ni importer les
//! migrations ni les executer. Il transcrit ce que la source garantit sans
//! base : l'ordre canonique des identifiants, et la regle qui en decoule
//! pour `applyOnly` (`database/migration.ts`, porte par une autre voie) :
//! une migration deployee est sautee, une migration inconnue du journal est
//! appliquee, et l'ordre du tableau est l'ordre d'application.
//!
//! Les identifiants sont repris a l'identique, dans l'ordre des lignes 5 a
//! 42 de la source. Les cinq migrations de la voie `lane/17` occupent les
//! positions 25 a 29 (comptees depuis zero).

use std::collections::BTreeSet;

/// Les 38 identifiants de migration, dans l'ordre d'application.
///
/// C'est l'ordre des `import()` de la source : l'ordre fait partie du
/// contrat, il n'est pas cosmetique. Une migration ne doit jamais passer
/// devant une autre, car chacune suppose le schema laisse par la precedente.
pub const MIGRATION_IDS: &[&str] = &[
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
pub const MIGRATION_COUNT: usize = 38;

/// Dit si un identifiant appartient au registre.
pub fn is_known(id: &str) -> bool {
    MIGRATION_IDS.contains(&id)
}

/// Position d'une migration dans l'ordre d'application.
///
/// `None` si l'identifiant n'appartient pas au registre : c'est le cas
/// qu'`applyOnly` ne rencontre jamais, car le journal ne contient que des
/// identifiants issus de ce tableau.
pub fn position(id: &str) -> Option<usize> {
    MIGRATION_IDS.iter().position(|known| *known == id)
}

/// Migrations restant a appliquer, dans l'ordre.
///
/// C'est la boucle d'`applyOnly` vue comme une fonction pure : les
/// migrations dont l'identifiant figure dans `completed` sont sautees, les
/// autres sont rendues dans l'ordre du registre. Un identifiant termine
/// inconnu est ignore, comme un nom de journal orphelin qui n'a pas de
/// module correspondant.
pub fn pending<'a>(completed: &[&str], all: &'a [&'a str]) -> Vec<&'a str> {
    let done: BTreeSet<&str> = completed.iter().copied().collect();
    all.iter().copied().filter(|id| !done.contains(id)).collect()
}

/// Migrations restant a appliquer dans le registre canonique.
pub fn pending_migrations(completed: &[&str]) -> Vec<&'static str> {
    pending(completed, MIGRATION_IDS)
}

/// Migrations strictement posterieures a `id`, dans l'ordre.
///
/// `None` si `id` n'appartient pas au registre. Une liste vide signifie que
/// la migration est la derniere, pas qu'elle est inconnue.
pub fn migrations_after(id: &str) -> Option<Vec<&'static str>> {
    position(id).map(|index| MIGRATION_IDS[index + 1..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_registre_contient_exactement_trente_huit_migrations() {
        assert_eq!(MIGRATION_IDS.len(), 38);
        assert_eq!(MIGRATION_COUNT, 38);
        assert_eq!(MIGRATION_COUNT, MIGRATION_IDS.len());
    }

    #[test]
    fn les_identifiants_sont_uniques_et_non_vides() {
        let uniques: BTreeSet<&str> = MIGRATION_IDS.iter().copied().collect();
        assert_eq!(uniques.len(), MIGRATION_IDS.len());
        for id in MIGRATION_IDS {
            assert!(!id.is_empty());
        }
    }

    #[test]
    fn les_cinq_migrations_lane_17_sont_consecutives_aux_positions_25_a_29() {
        let lane_17 = [
            "20260603001617_session_message_projection_indexes",
            "20260603040000_session_message_projection_order",
            "20260603141458_session_input_inbox",
            "20260603160727_jittery_ezekiel_stane",
            "20260604172448_event_sourced_session_input",
        ];
        for (offset, id) in lane_17.iter().enumerate() {
            assert_eq!(position(id), Some(25 + offset), "position de {id}");
            assert_eq!(MIGRATION_IDS[25 + offset], *id);
        }
    }

    #[test]
    fn la_premiere_et_la_derniere_migration_encadrent_le_registre() {
        assert_eq!(MIGRATION_IDS[0], "20260127222353_familiar_lady_ursula");
        assert_eq!(MIGRATION_IDS[37], "20260622202450_simplify_session_input");
        assert_eq!(position(MIGRATION_IDS[0]), Some(0));
        assert_eq!(position(MIGRATION_IDS[37]), Some(37));
    }

    #[test]
    fn un_identifiant_inconnu_n_a_ni_position_ni_suite() {
        assert!(!is_known("20200101000000_absente"));
        assert_eq!(position("20200101000000_absente"), None);
        assert_eq!(migrations_after("20200101000000_absente"), None);
        assert!(is_known(MIGRATION_IDS[10]));
    }

    #[test]
    fn sans_journal_tout_est_a_appliquer_dans_l_ordre() {
        let reste = pending_migrations(&[]);
        assert_eq!(reste, MIGRATION_IDS);
    }

    #[test]
    fn un_journal_complet_ne_laisse_rien_a_appliquer() {
        assert!(pending_migrations(MIGRATION_IDS).is_empty());
    }

    #[test]
    fn les_migrations_terminees_sont_sautees_et_l_ordre_est_conserve() {
        let terminees = ["20260127222353_familiar_lady_ursula", "20260603040000_session_message_projection_order"];
        let reste = pending_migrations(&terminees);
        assert_eq!(reste.len(), 36);
        assert!(!reste.contains(&"20260127222353_familiar_lady_ursula"));
        assert!(!reste.contains(&"20260603040000_session_message_projection_order"));
        assert_eq!(reste[0], "20260211171708_add_project_commands");
        let positions: Vec<usize> = reste.iter().map(|id| position(id).expect("connue")).collect();
        let mut triees = positions.clone();
        triees.sort();
        assert_eq!(positions, triees, "l'ordre du registre est preserve");
    }

    #[test]
    fn un_double_dans_le_journal_ne_change_rien() {
        let terminees = ["20260323234822_events", "20260323234822_events"];
        let reste = pending_migrations(&terminees);
        assert_eq!(reste.len(), 37);
        assert!(!reste.contains(&"20260323234822_events"));
    }

    #[test]
    fn les_suivantes_d_une_migration_sont_sa_queue_dans_le_registre() {
        let suite = migrations_after("20260604172448_event_sourced_session_input").expect("connue");
        assert_eq!(suite[0], "20260605003541_add_session_context_snapshot");
        assert_eq!(suite.len(), 8);
        let derniere = migrations_after("20260622202450_simplify_session_input").expect("connue");
        assert!(derniere.is_empty());
        let premiere = migrations_after("20260127222353_familiar_lady_ursula").expect("connue");
        assert_eq!(premiere.len(), 37);
    }
}
