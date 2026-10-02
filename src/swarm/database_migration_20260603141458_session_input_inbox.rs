//! Portage de `packages/core/src/database/migration/20260603141458_session_input_inbox.ts`.
//!
//! La source fait 25 lignes et n'exporte qu'un objet migration : un `id` et
//! une fonction `up` qui enchaine deux ordres SQL via `tx.run`. Il n'y a
//! aucune lecture, aucun branchement, aucune valeur de retour : `up` execute
//! les ordres dans l'ordre, et echoue si l'un d'eux echoue.
//!
//! `Cargo.toml` ne declare ni `sqlx`, ni `rusqlite`, ni `diesel` : ce portage
//! ne peut donc ni ouvrir de base ni executer quoi que ce soit. Il transcrit
//! le contrat de la migration sous forme de donnees pures, interrogeables
//! sans pilote : l'identifiant et la liste ordonnee des ordres SQL, tels
//! qu'ils sont passes a `tx.run` dans la source.
//!
//! Ce que fait `up`, dans l'ordre (lignes 8 a 22 de la source) :
//!
//! 1. `CREATE TABLE session_input` : cree la boite de reception des entrees
//!    de session, avec sept colonnes. `seq` est un entier auto-incremente qui
//!    sert de cle primaire, `id` est un texte unique, `session_id` reference
//!    `session(id)` avec suppression en cascade, `prompt` et `delivery` sont
//!    des textes non nuls, `promoted_seq` est un entier nul par defaut, et
//!    `time_created` est un entier non nul.
//! 2. `CREATE INDEX session_input_session_pending_seq_idx ON session_input
//!    (session_id, promoted_seq, seq)` : lecture des entrees en attente
//!    d'une session dans l'ordre d'arrivee.
//!
//! Note sur la forme : la source ecrit le `CREATE TABLE` sur plusieurs lignes
//! dans un gabarit, avec retours a la ligne et indentation. La chaine portee
//! ici en est la version normalisee sur une seule ligne : SQLite ignore les
//! blancs hors chaines, les deux formes executent donc le meme ordre. Les
//! noms de table, de colonnes, de contrainte et d'index sont repris a
//! l'identique.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260603141458_session_input_inbox";

/// Nom de la table creee par la migration.
pub const TABLE_NAME: &str = "session_input";

/// Nom de l'index cree par la migration.
pub const INDEX_NAME: &str = "session_input_session_pending_seq_idx";

/// Ordres SQL executes par `up`, dans l'ordre de la source.
///
/// Chaque entree est la chaine passee a `tx.run` (normalisee sur une ligne
/// pour le `CREATE TABLE` multi-ligne de la source) : le point-virgule final
/// en fait partie, et l'ordre des entrees est l'ordre d'execution.
pub const STATEMENTS: &[&str] = &[
    "CREATE TABLE `session_input` (`seq` integer PRIMARY KEY AUTOINCREMENT, `id` text NOT NULL UNIQUE, `session_id` text NOT NULL, `prompt` text NOT NULL, `delivery` text NOT NULL, `promoted_seq` integer, `time_created` integer NOT NULL, CONSTRAINT `fk_session_input_session_id_session_id_fk` FOREIGN KEY (`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE);",
    "CREATE INDEX `session_input_session_pending_seq_idx` ON `session_input` (`session_id`,`promoted_seq`,`seq`);",
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
        assert_eq!(MIGRATION_ID, "20260603141458_session_input_inbox");
        assert_eq!(migration_id(), MIGRATION_ID);
    }

    #[test]
    fn up_execute_exactement_deux_ordres() {
        assert_eq!(STATEMENTS.len(), 2);
        assert_eq!(statements().len(), 2);
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
    fn le_premier_ordre_cree_la_table_session_input() {
        assert!(STATEMENTS[0].starts_with("CREATE TABLE `session_input`"));
        assert_eq!(TABLE_NAME, "session_input");
    }

    #[test]
    fn la_table_porte_les_sept_colonnes_declarees() {
        let creation = STATEMENTS[0];
        for colonne in [
            "`seq` integer PRIMARY KEY AUTOINCREMENT",
            "`id` text NOT NULL UNIQUE",
            "`session_id` text NOT NULL",
            "`prompt` text NOT NULL",
            "`delivery` text NOT NULL",
            "`promoted_seq` integer",
            "`time_created` integer NOT NULL",
        ] {
            assert!(creation.contains(colonne), "colonne absente : {colonne}");
        }
    }

    #[test]
    fn la_cle_etrangere_cible_la_session_avec_suppression_en_cascade() {
        let creation = STATEMENTS[0];
        assert!(creation.contains("fk_session_input_session_id_session_id_fk"));
        assert!(creation.contains("FOREIGN KEY (`session_id`)"));
        assert!(creation.contains("REFERENCES `session`(`id`)"));
        assert!(creation.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn le_second_ordre_indexe_les_entrees_en_attente_par_session() {
        assert!(STATEMENTS[1].starts_with("CREATE INDEX"));
        assert!(STATEMENTS[1].contains(INDEX_NAME));
        assert!(STATEMENTS[1].contains("`session_input`"));
        assert!(STATEMENTS[1].contains("`session_id`"));
        assert!(STATEMENTS[1].contains("`promoted_seq`"));
        assert!(STATEMENTS[1].contains("`seq`"));
    }

    #[test]
    fn la_creation_de_la_table_precede_celle_de_son_index() {
        assert!(STATEMENTS[0].starts_with("CREATE TABLE"));
        assert!(STATEMENTS[1].starts_with("CREATE INDEX"));
    }
}
