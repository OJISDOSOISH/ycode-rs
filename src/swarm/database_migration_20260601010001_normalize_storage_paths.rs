//! Portage Rust de `opencode/packages/core/src/database/migration/20260601010001_normalize_storage_paths.ts`.
//!
//! ## Ce que fait la source
//!
//! Le fichier d'origine fait 22 lignes et n'exporte qu'une migration, sans
//! garde d'idempotence : quatre `UPDATE` qui remplacent les antislashs
//! Windows par des slashs dans les chemins stockes en base.
//!
//! ```ts
//! export default {
//!   id: "20260601010001_normalize_storage_paths",
//!   up(tx) {
//!     return Effect.gen(function* () {
//!       yield* tx.run(
//!         `UPDATE project SET worktree = REPLACE(worktree, char(92), '/') WHERE worktree GLOB '[A-Za-z]:' || char(92) || '*' OR worktree LIKE char(92) || char(92) || '%';`,
//!       )
//!       yield* tx.run(
//!         `UPDATE project SET sandboxes = REPLACE(sandboxes, char(92) || char(92), '/') WHERE instr(sandboxes, char(92)) > 0 AND (worktree GLOB '[A-Za-z]:*' OR worktree LIKE '//%');`,
//!       )
//!       yield* tx.run(
//!         `UPDATE session SET directory = REPLACE(directory, char(92), '/') WHERE directory GLOB '[A-Za-z]:' || char(92) || '*' OR directory LIKE char(92) || char(92) || '%';`,
//!       )
//!       yield* tx.run(
//!         `UPDATE session SET path = REPLACE(path, char(92), '/') WHERE path IS NOT NULL AND instr(path, char(92)) > 0 AND (directory GLOB '[A-Za-z]:*' OR directory LIKE '//%');`,
//!       )
//!     })
//!   },
//! } satisfies DatabaseMigration.Migration
//! ```
//!
//! `char(92)` est l'antislash. Les quatre requetes sont ordonnees : `worktree`
//! est normalise avant que la condition sur `sandboxes` ne le relise, et
//! `directory` avant que la condition sur `path` ne le relise. C'est pour cela
//! que les conditions sur `sandboxes` et `path` testent des formes a slash
//! (`'[A-Za-z]:*'` sans antislash, `'//%'`) alors que les conditions sur
//! `worktree` et `directory` testent des formes a antislash.
//!
//! ## Trois pieges portes tels quels
//!
//! 1. `GLOB` est sensible a la casse, `LIKE` ne l'est pas par defaut pour
//!    l'ASCII. Ici les deux motifs lettre-lecteur ecrivent `[A-Za-z]`, donc la
//!    distinction ne change rien au resultat : le predicat porte teste les
//!    deux cas.
//! 2. Le remplacement sur `sandboxes` ne porte pas sur un antislash seul mais
//!    sur la paire `char(92) || char(92)` : `REPLACE` y remplace chaque
//!    occurrence de la sequence de deux antislashs par un seul slash. Un
//!    antislash isole dans `sandboxes` survit donc a la migration, alors qu'un
//!    antislash isole dans `worktree` est remplace. Les deux fonctions de
//!    normalisation sont donc separees.
//! 3. `path IS NOT NULL` : la colonne `path` de `session` est nullable. Le
//!    predicat porte prend une `Option` et rend faux sur `None`, sans jamais
//!    paniquer.
//!
//! ## Ce qui est porte, et ce qui ne l'est pas
//!
//! Portes : l'identifiant, les quatre instructions dans leur ordre
//! d'execution, les deux normalisations, et les quatre predicats de
//! selection (`WHERE`) sous forme de fonctions pures sur des chaines.
//!
//! Non porte : `Effect.gen`, la transaction, l'execution SQLite. `Cargo.toml`
//! ne declare aucun pilote SQL.

/// Identifiant de la migration, tel que declare dans le fichier TypeScript.
pub const MIGRATION_ID: &str = "20260601010001_normalize_storage_paths";

/// Premiere instruction : normalise `project.worktree`.
pub const UPDATE_PROJECT_WORKTREE_SQL: &str = "UPDATE project SET worktree = REPLACE(worktree, char(92), '/') WHERE worktree GLOB '[A-Za-z]:' || char(92) || '*' OR worktree LIKE char(92) || char(92) || '%';";

/// Deuxieme instruction : normalise `project.sandboxes` (paires d'antislashs).
pub const UPDATE_PROJECT_SANDBOXES_SQL: &str = "UPDATE project SET sandboxes = REPLACE(sandboxes, char(92) || char(92), '/') WHERE instr(sandboxes, char(92)) > 0 AND (worktree GLOB '[A-Za-z]:*' OR worktree LIKE '//%');";

/// Troisieme instruction : normalise `session.directory`.
pub const UPDATE_SESSION_DIRECTORY_SQL: &str = "UPDATE session SET directory = REPLACE(directory, char(92), '/') WHERE directory GLOB '[A-Za-z]:' || char(92) || '*' OR directory LIKE char(92) || char(92) || '%';";

/// Quatrieme instruction : normalise `session.path`.
pub const UPDATE_SESSION_PATH_SQL: &str = "UPDATE session SET path = REPLACE(path, char(92), '/') WHERE path IS NOT NULL AND instr(path, char(92)) > 0 AND (directory GLOB '[A-Za-z]:*' OR directory LIKE '//%');";

/// Les quatre instructions, dans leur ordre d'execution.
///
/// L'ordre n'est pas indifferent : `worktree` est normalise avant que la
/// condition sur `sandboxes` ne le relise, et `directory` avant que la
/// condition sur `path` ne le relise.
pub const UP_STATEMENTS: [&str; 4] = [
    UPDATE_PROJECT_WORKTREE_SQL,
    UPDATE_PROJECT_SANDBOXES_SQL,
    UPDATE_SESSION_DIRECTORY_SQL,
    UPDATE_SESSION_PATH_SQL,
];

/// Normalise un chemin isole : chaque antislash devient un slash.
///
/// C'est le `REPLACE(colonne, char(92), '/')` des instructions 1, 3 et 4.
/// Sur Unix (aucun antislash), la chaine est rendue telle quelle.
pub fn normalize_storage_path(input: &str) -> String {
    input.replace('\\', "/")
}

/// Normalise le contenu de `project.sandboxes`.
///
/// C'est le `REPLACE(sandboxes, char(92) || char(92), '/')` de l'instruction 2 :
/// seule la sequence de deux antislashs est remplacee, par un seul slash.
/// Un antislash isole survit. Cette difference avec
/// [`normalize_storage_path`] est voulue : c'est ce que le SQL ecrit.
pub fn normalize_sandboxes_storage(input: &str) -> String {
    input.replace("\\\\", "/")
}

/// Le chemin commence-t-il par une lettre de lecteur suivie de `:\` ?
///
/// Reproduit `colonne GLOB '[A-Za-z]:' || char(92) || '*'` : trois caracteres
/// minimum, premiere lettre ASCII, puis `:` et antislash.
pub fn is_drive_backslash_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

/// Le chemin commence-t-il par deux antislashs (chemin UNC) ?
///
/// Reproduit `colonne LIKE char(92) || char(92) || '%'` : `LIKE` reste ici un
/// simple test de prefixe, car le motif ne contient ni `%` ni `_` avant la fin.
pub fn is_unc_backslash_prefix(value: &str) -> bool {
    value.starts_with("\\\\")
}

/// Le chemin (deja normalise a slashs) commence-t-il par une lettre de lecteur ?
///
/// Reproduit `colonne GLOB '[A-Za-z]:*'` : deux caracteres minimum, premiere
/// lettre ASCII puis `:`. Contrairement a [`is_drive_backslash_prefix`], aucun
/// troisieme caractere n'est exige.
pub fn is_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Le chemin (deja normalise a slashs) commence-t-il par `//` ?
///
/// Reproduit `colonne LIKE '//%'`.
pub fn is_slash_unc_prefix(value: &str) -> bool {
    value.starts_with("//")
}

/// La ligne `project` voit-elle son `worktree` reecrit par l'instruction 1 ?
pub fn project_worktree_needs_update(worktree: &str) -> bool {
    is_drive_backslash_prefix(worktree) || is_unc_backslash_prefix(worktree)
}

/// La ligne `session` voit-elle son `directory` reecrit par l'instruction 3 ?
pub fn session_directory_needs_update(directory: &str) -> bool {
    is_drive_backslash_prefix(directory) || is_unc_backslash_prefix(directory)
}

/// La ligne `project` voit-elle son `sandboxes` reecrit par l'instruction 2 ?
///
/// Le `worktree` lu ici est celui **apres** l'instruction 1, donc sous forme a
/// slashs : les motifs testes sont `'[A-Za-z]:*'` et `'//%'`, sans antislash.
pub fn project_sandboxes_needs_update(sandboxes: &str, worktree: &str) -> bool {
    sandboxes.contains('\\') && (is_drive_prefix(worktree) || is_slash_unc_prefix(worktree))
}

/// La ligne `session` voit-elle son `path` reecrit par l'instruction 4 ?
///
/// `None` reproduit `path IS NOT NULL` evalue a faux : aucune reecriture.
/// Le `directory` lu ici est celui **apres** l'instruction 3.
pub fn session_path_needs_update(path: Option<&str>, directory: &str) -> bool {
    match path {
        None => false,
        Some(value) => {
            value.contains('\\') && (is_drive_prefix(directory) || is_slash_unc_prefix(directory))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_de_migration_est_celui_du_fichier_ts() {
        assert_eq!(MIGRATION_ID, "20260601010001_normalize_storage_paths");
    }

    #[test]
    fn les_quatre_instructions_sont_dans_l_ordre_du_fichier_ts() {
        assert_eq!(UP_STATEMENTS.len(), 4);
        assert!(UP_STATEMENTS[0].starts_with("UPDATE project SET worktree"));
        assert!(UP_STATEMENTS[1].starts_with("UPDATE project SET sandboxes"));
        assert!(UP_STATEMENTS[2].starts_with("UPDATE session SET directory"));
        assert!(UP_STATEMENTS[3].starts_with("UPDATE session SET path"));
    }

    #[test]
    fn un_chemin_avec_lecteur_windows_est_normalise() {
        assert_eq!(normalize_storage_path("C:\\srv\\depot"), "C:/srv/depot");
    }

    #[test]
    fn un_chemin_unc_est_normalise() {
        assert_eq!(normalize_storage_path("\\\\serveur\\partage"), "//serveur/partage");
    }

    #[test]
    fn un_chemin_posix_traverse_intact() {
        assert_eq!(normalize_storage_path("/srv/depot"), "/srv/depot");
        assert_eq!(normalize_storage_path(""), "");
    }

    #[test]
    fn sandboxes_ne_remplace_que_les_paires_d_antislashs() {
        // `REPLACE(x, '\\\\', '/')` : la sequence de deux antislashs devient
        // un slash, mais un antislash isole survit. C'est la difference avec
        // `normalize_storage_path`, et elle vient du SQL, pas d'un choix.
        assert_eq!(normalize_sandboxes_storage("a\\\\b"), "a/b");
        assert_eq!(normalize_sandboxes_storage("a\\b"), "a\\b");
    }

    #[test]
    fn worktree_avec_lecteur_ou_unc_est_selectionne() {
        assert!(project_worktree_needs_update("C:\\srv"));
        assert!(project_worktree_needs_update("z:\\x"));
        assert!(project_worktree_needs_update("\\\\srv\\partage"));
        assert!(!project_worktree_needs_update("C:/srv"));
        assert!(!project_worktree_needs_update("/srv/depot"));
        assert!(!project_worktree_needs_update("relatif\\chemin"));
        assert!(!project_worktree_needs_update(""));
    }

    #[test]
    fn directory_avec_lecteur_ou_unc_est_selectionne() {
        assert!(session_directory_needs_update("D:\\travail"));
        assert!(session_directory_needs_update("\\\\srv\\partage"));
        assert!(!session_directory_needs_update("/srv/depot"));
        assert!(!session_directory_needs_update("C:/deja/normalise"));
    }

    #[test]
    fn sandboxes_exige_un_antislash_et_un_worktree_windows_normalise() {
        // Le worktree est lu apres l'instruction 1 : forme a slashs.
        assert!(project_sandboxes_needs_update("a\\\\b", "C:/srv"));
        assert!(project_sandboxes_needs_update("a\\b", "//serveur/partage"));
        assert!(!project_sandboxes_needs_update("sans antislash", "C:/srv"));
        assert!(!project_sandboxes_needs_update("a\\b", "/srv/depot"));
        assert!(!project_sandboxes_needs_update("a\\b", "relatif"));
    }

    #[test]
    fn path_nul_n_est_jamais_selectionne() {
        // `path IS NOT NULL` : `None` rend faux, sans paniquer.
        assert!(!session_path_needs_update(None, "C:/srv"));
        assert!(!session_path_needs_update(None, "//serveur/partage"));
    }

    #[test]
    fn path_exige_un_antislash_et_un_directory_windows_normalise() {
        assert!(session_path_needs_update(Some("C:\\fichier"), "C:/srv"));
        assert!(session_path_needs_update(Some("a\\b"), "//srv/partage"));
        assert!(!session_path_needs_update(Some("C:/fichier"), "C:/srv"));
        assert!(!session_path_needs_update(Some("a\\b"), "/srv/depot"));
        assert!(!session_path_needs_update(Some(""), "C:/srv"));
    }

    #[test]
    fn un_prefixe_incomplet_n_est_pas_un_lecteur() {
        // `"C:"` seul fait deux caracteres : `is_drive_prefix` le prend,
        // `is_drive_backslash_prefix` l'ignore faute d'antislash.
        assert!(is_drive_prefix("C:"));
        assert!(!is_drive_backslash_prefix("C:"));
        assert!(!is_drive_prefix("1:"));
        assert!(!is_drive_prefix(""));
        assert!(!is_drive_prefix(":"));
    }
}
