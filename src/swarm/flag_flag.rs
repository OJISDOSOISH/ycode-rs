//! Portage de `packages/core/src/flag/flag.ts`.
//!
//! La source lit des variables d'environnement et n'expose que deux
//! predicats et un objet `Flag`. Le point delicat est le moment de lecture :
//! la plupart des champs sont figes au chargement du module, sauf huit
//! accesseurs evalues a chaque acces parce que les tests, la CLI et les
//! outils externes modifient l'environnement a l'execution.
//!
//! Comme les tests Rust ne doivent pas muter l'environnement global du
//! runner, ce fichier ne lit jamais `std::env` : chaque fonction prend une
//! table explicite `env` (cle vers valeur brute) et, quand la source en
//! depend, un drapeau `windows`. C'est la meme semantique, sous forme pure
//! et testable. `Config.boolean(...).withDefault(false)` d'Effect est repris
//! comme `truthy`, qui est exactement ce qu'il fait sur `"true"` et `"1"`.

use std::collections::HashMap;

/// Vrai quand la valeur brute, ignoree de la casse, vaut `"true"` ou `"1"`.
///
/// C'est le corps de `truthy` : `process.env[key]?.toLowerCase()`. Une cle
/// absente (`None`) rend `false`, comme le `?.` de la source. Une chaine vide
/// rend `false` elle aussi : ce n'est ni `"true"` ni `"1"`.
pub fn est_vrai(valeur: Option<&str>) -> bool {
    match valeur {
        None => false,
        Some(texte) => {
            let minuscule = texte.to_lowercase();
            minuscule == "true" || minuscule == "1"
        }
    }
}

/// Le `truthy(key)` de la source, lu dans une table explicite.
pub fn truthy_dans(env: &HashMap<String, String>, cle: &str) -> bool {
    est_vrai(env.get(cle).map(String::as_str))
}

/// Le `enabledByExperimental(key)` de la source : quand la cle est absente,
/// le drapeau general `OPENCODE_EXPERIMENTAL` decide ; sinon la cle decide.
pub fn active_par_experimental(env: &HashMap<String, String>, cle: &str) -> bool {
    match env.get(cle) {
        None => truthy_dans(env, "OPENCODE_EXPERIMENTAL"),
        Some(_) => truthy_dans(env, cle),
    }
}

/// Le `OPENCODE_DISABLE_FFF` fige au chargement : sans variable, la valeur
/// depend de la plateforme (`win32` desactive) ; sinon `truthy` decide.
///
/// `windows` reprend `process.platform === "win32"`.
pub fn fff_desactive(env: &HashMap<String, String>, windows: bool) -> bool {
    match env.get("OPENCODE_DISABLE_FFF") {
        None => windows,
        Some(_) => truthy_dans(env, "OPENCODE_DISABLE_FFF"),
    }
}

/// Le `OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT` fige au chargement :
/// meme regle que `fff_desactive`, sur l'autre cle.
pub fn copie_sur_selection_desactivee(
    env: &HashMap<String, String>,
    windows: bool,
) -> bool {
    match env.get("OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT") {
        None => windows,
        Some(_) => truthy_dans(env, "OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT"),
    }
}

/// Le `get OPENCODE_CLIENT()` de la source : la variable, ou `"cli"` quand
/// elle est absente.
///
/// La source utilise `??` : une chaine vide reste une valeur presente et ne
/// retombe pas sur `"cli"`.
pub fn client<'a>(env: &'a HashMap<String, String>) -> &'a str {
    env.get("OPENCODE_CLIENT")
        .map(String::as_str)
        .unwrap_or("cli")
}

/// Lecture simple d'une variable textuelle, comme les champs directs de
/// `Flag` (`OPENCODE_CONFIG`, `OPENCODE_DB`, ...).
pub fn texte<'a>(env: &'a HashMap<String, String>, cle: &str) -> Option<&'a str> {
    env.get(cle).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(paires: &[(&str, &str)]) -> HashMap<String, String> {
        paires
            .iter()
            .map(|(cle, valeur)| (cle.to_string(), valeur.to_string()))
            .collect()
    }

    #[test]
    fn vrai_reconnait_true_et_1_sans_casse_et_rien_d_autre() {
        assert!(est_vrai(Some("true")));
        assert!(est_vrai(Some("TRUE")));
        assert!(est_vrai(Some("True")));
        assert!(est_vrai(Some("1")));
        assert!(!est_vrai(None));
        assert!(!est_vrai(Some("")));
        assert!(!est_vrai(Some("0")));
        assert!(!est_vrai(Some("false")));
        assert!(!est_vrai(Some("yes")));
        assert!(!est_vrai(Some(" true ")));
    }

    #[test]
    fn une_cle_absente_vaut_faux_comme_le_point_d_interrogation_de_la_source() {
        let table = env(&[]);
        assert!(!truthy_dans(&table, "OPENCODE_DISABLE_PRUNE"));
        let table = env(&[("OPENCODE_DISABLE_PRUNE", "1")]);
        assert!(truthy_dans(&table, "OPENCODE_DISABLE_PRUNE"));
    }

    #[test]
    fn le_drapeau_experimental_releve_la_cle_absente() {
        let table = env(&[("OPENCODE_EXPERIMENTAL", "true")]);
        assert!(active_par_experimental(&table, "OPENCODE_EXPERIMENTAL_WORKSPACES"));
        let table = env(&[]);
        assert!(!active_par_experimental(&table, "OPENCODE_EXPERIMENTAL_WORKSPACES"));
    }

    #[test]
    fn la_cle_prioritaire_l_emporte_sur_le_drapeau_experimental() {
        let table = env(&[
            ("OPENCODE_EXPERIMENTAL", "true"),
            ("OPENCODE_EXPERIMENTAL_WORKSPACES", "0"),
        ]);
        assert!(!active_par_experimental(&table, "OPENCODE_EXPERIMENTAL_WORKSPACES"));
        let table = env(&[
            ("OPENCODE_EXPERIMENTAL", "0"),
            ("OPENCODE_EXPERIMENTAL_WORKSPACES", "true"),
        ]);
        assert!(active_par_experimental(&table, "OPENCODE_EXPERIMENTAL_WORKSPACES"));
    }

    #[test]
    fn sans_variable_fff_suit_la_plateforme_puis_truthy() {
        assert!(fff_desactive(&env(&[]), true));
        assert!(!fff_desactive(&env(&[]), false));
        assert!(fff_desactive(&env(&[("OPENCODE_DISABLE_FFF", "1")]), false));
        assert!(!fff_desactive(&env(&[("OPENCODE_DISABLE_FFF", "0")]), true));
    }

    #[test]
    fn sans_variable_la_copie_suit_la_plateforme_puis_truthy() {
        assert!(copie_sur_selection_desactivee(&env(&[]), true));
        assert!(!copie_sur_selection_desactivee(&env(&[]), false));
        assert!(copie_sur_selection_desactivee(
            &env(&[("OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT", "true")]),
            false
        ));
    }

    #[test]
    fn le_client_vaut_cli_par_defaut_mais_garde_la_chaine_vide() {
        assert_eq!(client(&env(&[])), "cli");
        assert_eq!(client(&env(&[("OPENCODE_CLIENT", "tui")])), "tui");
        // `??` ne confond pas vide et absent.
        assert_eq!(client(&env(&[("OPENCODE_CLIENT", "")])), "");
    }

    #[test]
    fn la_lecture_textuelle_distribue_absent_et_present() {
        let table = env(&[("OPENCODE_CONFIG", "/tmp/c.json")]);
        assert_eq!(texte(&table, "OPENCODE_CONFIG"), Some("/tmp/c.json"));
        assert_eq!(texte(&table, "OPENCODE_DB"), None);
    }
}
