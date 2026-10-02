//! Portage Rust de `opencode/packages/core/src/database/migration.gen.ts`.
//!
//! La source tient en quarante-trois lignes : elle importe les trente-huit
//! migrations du repertoire `database/migration/` et les reexporte dans un
//! tableau `migrations` passe a `DatabaseMigration.applyOnly`, dans l'ordre
//! chronologique. Aucun des trente-huit modules n'est redefini ici : ce
//! fichier ne porte que le registre, c'est-a-dire la liste ordonnee des
//! identifiants et les predicats purs qui la decrivent (appartenance,
//! position, recherche par prefixe horodate).
//!
//! `Cargo.toml` ne declare aucun pilote SQL : ce fichier n'execute rien et
//! n'importe aucun module de migration. Les cinq identifiants portes par
//! lane/18 (`20260605003541` a `20260612174303`) y figurent comme de simples
//! chaines, a la meme place que dans la source.
//!
//! L'ordre n'est pas cosmetique : `applyOnly` saute les migrations deja
//! journalisees et joue les autres dans l'ordre du tableau. Une inversion
//! silencieuse jouerait `lush_chimera` avant `credential` et supprimerait une
//! table qui n'existe pas encore.

/// Les trente-huit identifiants de migration, dans l'ordre de la source.
///
/// L'ordre est celui des imports du fichier genere : croissant sur le prefixe
/// horodate `AAAAMMJJHHMMSS`, qui est aussi l'ordre d'application en base.
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

/// Le nombre de migrations du registre.
pub const MIGRATION_COUNT: usize = MIGRATION_IDS.len();

/// Les cinq identifiants portes par lane/18, dans l'ordre du registre.
pub const LANE_18_IDS: [&str; 5] = [
    "20260605003541_add_session_context_snapshot",
    "20260605042240_add_context_epoch_agent",
    "20260611035744_credential",
    "20260611192811_lush_chimera",
    "20260612174303_project_dir_strategy",
];

/// La liste ordonnee des identifiants.
pub fn ids() -> &'static [&'static str] {
    &MIGRATION_IDS
}

/// Le nombre de migrations du registre.
pub fn count() -> usize {
    MIGRATION_COUNT
}

/// Vrai si `id` figure au registre.
pub fn contains(id: &str) -> bool {
    MIGRATION_IDS.contains(&id)
}

/// La position de `id` dans le registre, ou `None` si inconnu.
pub fn index_of(id: &str) -> Option<usize> {
    MIGRATION_IDS.iter().position(|candidate| *candidate == id)
}

/// La migration dont l'identifiant commence par le prefixe horodate `prefix`.
///
/// C'est la recherche qu'`applyOnly` fait pour apparier l'ancien journal
/// Drizzle (`__drizzle_migrations`) au nouveau : le prefixe est la date
/// formatee `%Y%m%d%H%M%S` de l'entree heritee.
pub fn find_by_prefix(prefix: &str) -> Option<&'static str> {
    MIGRATION_IDS.iter().copied().find(|id| id.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_registre_contient_trente_huit_migrations() {
        assert_eq!(count(), 38);
        assert_eq!(ids().len(), 38);
        assert_eq!(MIGRATION_COUNT, 38);
    }

    #[test]
    fn le_premier_et_le_dernier_sont_ceux_de_la_source() {
        assert_eq!(ids()[0], "20260127222353_familiar_lady_ursula");
        assert_eq!(ids()[37], "20260622202450_simplify_session_input");
    }

    #[test]
    fn les_cinq_identifiants_de_lane_18_sont_contigus() {
        let positions: Vec<usize> =
            LANE_18_IDS.iter().map(|id| index_of(id).expect("id de lane/18")).collect();
        assert_eq!(positions, vec![30, 31, 32, 33, 34]);
        for (attendu, id) in LANE_18_IDS.iter().enumerate() {
            assert_eq!(ids()[30 + attendu], *id);
        }
    }

    #[test]
    fn l_ordre_chronologique_est_preserve() {
        let mut tries = ids().to_vec();
        tries.sort_unstable();
        assert_eq!(tries, ids().to_vec(), "le registre est deja trie");
    }

    #[test]
    fn aucun_identifiant_n_est_duplique() {
        let mut vus = std::collections::BTreeSet::new();
        for id in ids() {
            assert!(vus.insert(*id), "doublon : {id}");
        }
    }

    #[test]
    fn la_recherche_par_prefixe_horodate_trouve_la_bonne_migration() {
        assert_eq!(
            find_by_prefix("20260611035744"),
            Some("20260611035744_credential")
        );
        assert_eq!(
            find_by_prefix("20260611192811"),
            Some("20260611192811_lush_chimera")
        );
    }

    #[test]
    fn un_prefixe_inconnu_ne_trouve_rien() {
        assert_eq!(find_by_prefix("19990101000000"), None);
        assert_eq!(find_by_prefix(""), Some(ids()[0]), "le prefixe vide matche tout");
    }

    #[test]
    fn un_identifiant_inconnu_est_absent_et_sans_position() {
        assert!(!contains("20260611035744_credential_bis"));
        assert_eq!(index_of("20260611035744_credential_bis"), None);
        assert!(contains("20260611035744_credential"));
        assert_eq!(index_of("20260611035744_credential"), Some(32));
    }
}
