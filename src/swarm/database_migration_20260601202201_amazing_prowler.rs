//! Portage Rust de `opencode/packages/core/src/database/migration/20260601202201_amazing_prowler.ts`.
//!
//! ## Ce que fait la source
//!
//! Le fichier d'origine fait 11 lignes et n'exporte qu'une migration :
//!
//! ```ts
//! export default {
//!   id: "20260601202201_amazing_prowler",
//!   up(tx) {
//!     return Effect.gen(function* () {
//!       yield* tx.run(`DROP TABLE \`permission\`;`)
//!     })
//!   },
//! } satisfies DatabaseMigration.Migration
//! ```
//!
//! La migration supprime la table `permission`, sans condition ni garde.
//! Elle forme un diptyque avec la migration suivante
//! (`20260602002951_lowly_union_jack`), qui recree la meme table avec un
//! schema corrige : la suppression puis la recreation remplacent un `ALTER`
//! que SQLite ne saurait pas exprimer sur les contraintes.
//!
//! ## Ce qui est porte, et ce qui ne l'est pas
//!
//! Portes : l'identifiant, le nom de la table supprimee, l'instruction, et la
//! projection pure qu'un `DROP TABLE` applique a une liste de tables.
//!
//! Non porte : `Effect.gen`, la transaction, l'execution SQLite. `Cargo.toml`
//! ne declare aucun pilote SQL.

/// Identifiant de la migration, tel que declare dans le fichier TypeScript.
pub const MIGRATION_ID: &str = "20260601202201_amazing_prowler";

/// Table supprimee par la migration.
pub const TABLE_NAME: &str = "permission";

/// Instruction unique de la migration.
pub const DROP_TABLE_SQL: &str = "DROP TABLE `permission`;";

/// Les instructions de la migration, dans leur ordre d'execution.
pub const UP_STATEMENTS: [&str; 1] = [DROP_TABLE_SQL];

/// Les tables qui subsistent apres le `DROP TABLE`.
///
/// C'est la suppression vue comme une projection sur une tranche : disparait
/// tout ce qui porte le nom de la table donne, rien d'autre. L'ordre des
/// tables restantes est preserve, comme apres un `DROP` SQL, qui ne
/// reordonne rien. Une table absente de la tranche ne provoque aucune
/// erreur ici : c'est l'execution SQLite, non portee, qui leverait
/// l'erreur sur une table inexistante.
pub fn tables_after_drop<'a>(tables: &[&'a str], dropped: &str) -> Vec<&'a str> {
    tables.iter().copied().filter(|name| *name != dropped).collect()
}

/// Les tables qui subsistent apres cette migration.
pub fn tables_after_migration<'a>(tables: &[&'a str]) -> Vec<&'a str> {
    tables_after_drop(tables, TABLE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_est_celui_du_fichier_ts() {
        assert_eq!(MIGRATION_ID, "20260601202201_amazing_prowler");
    }

    #[test]
    fn l_instruction_supprime_bien_la_table_permission() {
        assert_eq!(DROP_TABLE_SQL, "DROP TABLE `permission`;");
        assert_eq!(UP_STATEMENTS, ["DROP TABLE `permission`;"]);
        assert_eq!(TABLE_NAME, "permission");
    }

    #[test]
    fn la_table_permission_disparait_et_les_autres_restent_en_ordre() {
        let tables = ["session", "permission", "project", "event"];
        assert_eq!(tables_after_migration(&tables), vec!["session", "project", "event"]);
    }

    #[test]
    fn une_table_absente_ne_change_rien() {
        // La projection pure ne leve pas d'erreur sur une table absente :
        // c'est SQLite qui refuserait, pas la description.
        let tables = ["session", "project"];
        assert_eq!(tables_after_migration(&tables), vec!["session", "project"]);
    }

    #[test]
    fn une_tranche_vide_reste_vide() {
        let vide: [&str; 0] = [];
        assert!(tables_after_migration(&vide).is_empty());
    }

    #[test]
    fn seules_les_tables_homonymes_exactes_disparaissent() {
        // La comparaison est une egalite stricte : `permissions` ou
        // `Permission` ne sont pas `permission`.
        let tables = ["permission", "permissions", "Permission"];
        assert_eq!(tables_after_migration(&tables), vec!["permissions", "Permission"]);
    }
}
