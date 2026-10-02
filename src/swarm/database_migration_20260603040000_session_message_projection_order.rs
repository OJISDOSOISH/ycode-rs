//! Portage de `packages/core/src/database/migration/20260603040000_session_message_projection_order.ts`.
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
//! 1. `DELETE FROM session_message` : vide la table des projections. Le
//!    commentaire de la source l'explique : les projections ecrites avant le
//!    lancement ne peuvent pas recevoir un ordre d'agregat veridique, car la
//!    persistance durable des evenements n'etait pas encore inconditionnelle.
//! 2. `ALTER TABLE session_message ADD COLUMN seq integer NOT NULL` : ajoute
//!    le numero de sequence, colonne non nulle. Elle ne peut etre ajoutee
//!    que sur une table vide, d'ou le `DELETE` qui la precede.
//! 3. `DROP INDEX IF EXISTS session_message_session_type_time_created_id_idx` :
//!    retire l'index de lecture par temps de creation, devenu obsolete.
//! 4. `CREATE INDEX session_message_session_seq_idx ON session_message
//!    (session_id, seq)` : lecture des messages d'une session par sequence.
//! 5. `CREATE INDEX session_message_session_type_seq_idx ON session_message
//!    (session_id, type, seq)` : meme lecture, restreinte a un type.
//!
//! Le piege de cette migration est l'ordre des deux premiers ordres :
//! ajouter une colonne `NOT NULL` sans defaut sur une table non vide
//! echouerait, c'est le `DELETE` prealable qui rend l'`ALTER TABLE` sur.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260603040000_session_message_projection_order";

/// Ordres SQL executes par `up`, dans l'ordre de la source.
///
/// Chaque entree est la chaine passee a `tx.run` : le point-virgule final en
/// fait partie, et l'ordre des entrees est l'ordre d'execution.
pub const STATEMENTS: &[&str] = &[
    "DELETE FROM `session_message`;",
    "ALTER TABLE `session_message` ADD COLUMN `seq` integer NOT NULL;",
    "DROP INDEX IF EXISTS `session_message_session_type_time_created_id_idx`;",
    "CREATE INDEX `session_message_session_seq_idx` ON `session_message` (`session_id`,`seq`);",
    "CREATE INDEX `session_message_session_type_seq_idx` ON `session_message` (`session_id`,`type`,`seq`);",
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
        assert_eq!(MIGRATION_ID, "20260603040000_session_message_projection_order");
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
    fn le_premier_ordre_vide_les_projections_avant_l_ajout_de_colonne() {
        assert_eq!(STATEMENTS[0], "DELETE FROM `session_message`;");
    }

    #[test]
    fn le_deuxieme_ordre_ajoute_une_colonne_seq_non_nulle() {
        assert!(STATEMENTS[1].starts_with("ALTER TABLE `session_message`"));
        assert!(STATEMENTS[1].contains("ADD COLUMN `seq`"));
        assert!(STATEMENTS[1].contains("integer NOT NULL"));
    }

    #[test]
    fn le_troisieme_ordre_retire_l_index_temporel_obsolete() {
        assert!(STATEMENTS[2].starts_with("DROP INDEX IF EXISTS"));
        assert!(STATEMENTS[2].contains("session_message_session_type_time_created_id_idx"));
    }

    #[test]
    fn les_deux_derniers_ordres_indexent_par_numero_de_sequence() {
        assert!(STATEMENTS[3].contains("session_message_session_seq_idx"));
        assert!(STATEMENTS[3].contains("`session_id`"));
        assert!(STATEMENTS[3].contains("`seq`"));
        assert!(STATEMENTS[4].contains("session_message_session_type_seq_idx"));
        assert!(STATEMENTS[4].contains("`type`"));
        assert!(STATEMENTS[4].contains("`seq`"));
    }

    #[test]
    fn la_vidange_precede_l_ajout_de_la_colonne_non_nulle() {
        let vidange = STATEMENTS
            .iter()
            .position(|ordre| *ordre == "DELETE FROM `session_message`;")
            .expect("la migration vide la table");
        let ajout = STATEMENTS
            .iter()
            .position(|ordre| ordre.contains("ADD COLUMN `seq`"))
            .expect("la migration ajoute la colonne seq");
        assert!(vidange < ajout, "le DELETE doit preceder le ALTER TABLE");
    }
}
