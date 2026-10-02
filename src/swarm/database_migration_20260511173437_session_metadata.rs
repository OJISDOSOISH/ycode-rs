//! Portage Rust de `opencode/packages/core/src/database/migration/20260511173437_session-metadata.ts`.
//!
//! ## Ce que fait la source
//!
//! Le fichier d'origine fait 16 lignes et n'exporte qu'une migration :
//!
//! ```ts
//! export default {
//!   id: "20260511173437_session-metadata",
//!   up(tx) {
//!     return Effect.gen(function* () {
//!       // This column briefly shipped again under 20260530232709_lovely_romulus.
//!       if (
//!         (yield* tx.all<{ name: string }>(`PRAGMA table_info(\`session\`)`)).some((column) => column.name === "metadata")
//!       )
//!         return
//!       yield* tx.run(`ALTER TABLE \`session\` ADD \`metadata\` text;`)
//!     })
//!   },
//! } satisfies DatabaseMigration.Migration
//! ```
//!
//! La logique est une garde d'idempotence : si la colonne `metadata` existe
//! deja dans la table `session`, la migration ne fait rien ; sinon elle
//! l'ajoute en `text` nullable. La garde existe parce que la colonne a
//! brievement ete livree une seconde fois sous une autre migration
//! (`20260530232709_lovely_romulus`, d'apres le commentaire) : sans elle,
//! rejouer cette migration sur une base deja migree echouerait.
//!
//! ## Ce qui est porte, et ce qui ne l'est pas
//!
//! Porte ici, en donnees et en fonctions pures :
//!
//! - l'identifiant de la migration, tel quel ;
//! - les deux instructions SQL, telles quelles ;
//! - la garde d'idempotence, sous forme de predicat sur la liste des colonnes
//!   que `PRAGMA table_info` renvoie.
//!
//! Non porte : `Effect.gen`, la transaction `tx`, l'execution SQLite. Comme
//! dans les autres portages du swarm, `Cargo.toml` ne declare aucun pilote
//! SQL : ce fichier decrit le contrat que la couche d'execution consommera,
//! sans pretendre executer quoi que ce soit.
//!
//! ## Le piege de ce fichier : la comparaison est exacte, pas insensible a la casse
//!
//! La source compare avec `===` contre `"metadata"`. Une colonne nommee
//! `Metadata` ou `METADATA` ne desarme donc pas la garde, et SQLite leverait
//! une erreur de colonne en double a l'execution. Le predicat porte ici
//! reproduit l'egalite stricte, et un test le verrouille.

/// Identifiant de la migration, tel que declare dans le fichier TypeScript.
pub const MIGRATION_ID: &str = "20260511173437_session-metadata";

/// Table ciblee par la migration.
pub const TABLE_NAME: &str = "session";

/// Colonne ajoutee par la migration.
pub const COLUMN_NAME: &str = "metadata";

/// Requete d'introspection lue avant d'ecrire.
///
/// C'est elle qui fournit la liste des `{ name }` que la garde inspecte.
pub const PRAGMA_SQL: &str = "PRAGMA table_info(`session`)";

/// Instruction appliquee quand la colonne est absente.
///
/// Colonne `text` sans `NOT NULL` ni valeur par defaut : les lignes
/// existantes recoivent `NULL`, ce qui est bien la valeur de leur colonne.
pub const ADD_COLUMN_SQL: &str = "ALTER TABLE `session` ADD `metadata` text;";

/// La colonne voulue figure-t-elle deja dans la liste du `PRAGMA` ?
///
/// Reproduit le `.some((column) => column.name === "metadata")` de la source :
/// egalite stricte, sensible a la casse. Une colonne `Metadata` ne compte pas.
pub fn column_present(columns: &[String], wanted: &str) -> bool {
    columns.iter().any(|name| name == wanted)
}

/// Faut-il appliquer l'`ALTER TABLE` ?
///
/// Vrai quand la colonne `metadata` est absente de la table `session`, faux
/// quand elle y est deja (cas de la double livraison signalee en commentaire
/// dans la source).
pub fn needs_add_column(columns: &[String]) -> bool {
    !column_present(columns, COLUMN_NAME)
}

/// Les instructions restant a appliquer pour cette migration.
///
/// Rend un vecteur vide quand la garde s'applique (colonne deja presente),
/// sinon le seul `ALTER TABLE`. L'ordre de sortie est l'ordre d'execution.
pub fn pending_statements(columns: &[String]) -> Vec<&'static str> {
    if needs_add_column(columns) {
        vec![ADD_COLUMN_SQL]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colonnes(noms: &[&str]) -> Vec<String> {
        noms.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn l_identifiant_de_migration_est_celui_du_fichier_ts() {
        assert_eq!(MIGRATION_ID, "20260511173437_session-metadata");
    }

    #[test]
    fn une_table_sans_colonne_metadata_demande_l_ajout() {
        let cols = colonnes(&["id", "project_id", "time_created"]);
        assert!(needs_add_column(&cols));
        assert_eq!(pending_statements(&cols), vec![ADD_COLUMN_SQL]);
    }

    #[test]
    fn une_table_avec_colonne_metadata_ne_demande_rien() {
        // C'est le cas de la double livraison : la colonne est deja la,
        // la migration doit se taire.
        let cols = colonnes(&["id", "metadata", "time_created"]);
        assert!(!needs_add_column(&cols));
        assert!(pending_statements(&cols).is_empty());
    }

    #[test]
    fn une_table_vide_demande_l_ajout() {
        // Cas limite : aucune colonne lue, la garde ne peut pas s'appliquer.
        let vide: Vec<String> = Vec::new();
        assert!(needs_add_column(&vide));
        assert_eq!(pending_statements(&vide), vec![ADD_COLUMN_SQL]);
    }

    #[test]
    fn la_detection_est_sensible_a_la_casse_comme_le_strict_egal_ts() {
        // `===` ne confond pas `Metadata` et `metadata` : porter une
        // comparaison insensible a la casse inventerait une garde que la
        // source n'a pas.
        let cols = colonnes(&["id", "Metadata"]);
        assert!(column_present(&cols, "Metadata"));
        assert!(!column_present(&cols, "metadata"));
        assert!(needs_add_column(&cols));
    }

    #[test]
    fn une_colonne_homonyme_partielle_ne_desarme_pas_la_garde() {
        // `session_metadata` ou `metadata_json` ne sont pas `metadata`.
        // La recherche est une egalite, pas une sous-chaine.
        let cols = colonnes(&["session_metadata", "metadata_json"]);
        assert!(needs_add_column(&cols));
    }

    #[test]
    fn le_sql_ajoute_une_colonne_texte_nullable_a_session() {
        assert!(ADD_COLUMN_SQL.starts_with("ALTER TABLE `session` ADD `metadata`"));
        assert!(ADD_COLUMN_SQL.contains("text"));
        assert!(!ADD_COLUMN_SQL.contains("NOT NULL"), "la colonne reste nullable pour les lignes existantes");
        assert_eq!(TABLE_NAME, "session");
        assert_eq!(COLUMN_NAME, "metadata");
    }

    #[test]
    fn le_pragma_interroge_bien_la_table_session() {
        assert!(PRAGMA_SQL.contains("table_info"));
        assert!(PRAGMA_SQL.contains("session"));
    }
}
