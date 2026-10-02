//! Portage de `packages/core/src/flag/flag.ts`.
//!
//! La source tient en 78 lignes et exporte deux choses : la fonction
//! `truthy(key)` et l objet `Flag`. `truthy` lit `process.env[key]`,
//! le passe en minuscules et rend vrai pour `"true"` ou `"1"`.
//! Tout le reste de l objet en derive : drapeaux lus a l import,
//! drapeaux evalues a l acces (getters), et replis par plateforme
//! (`win32` active `DISABLE_FFF` et `DISABLE_COPY_ON_SELECT` par defaut).
//!
//! Le portage ne lit jamais l environnement reel : chaque fonction recoit
//! la valeur candidate en parametre, ce qui la rend pure et testable.
//! `process.env` devient `Option<&str>`, `?? "cli"` devient un repli
//! explicite, et `process.platform` devient un booleen `est_windows`.

/// Le client par defaut quand `OPENCODE_CLIENT` est absent.
pub const CLIENT_DEFAUT: &str = "cli";

/// Interprete une valeur d environnement comme un booleen.
///
/// C est `truthy` : minuscules d abord, puis vrai pour `"true"` ou `"1"`.
/// Une absence vaut faux, comme `undefined?.toLowerCase()` qui ne vaut
/// ni `"true"` ni `"1"`. Une chaine vide vaut faux aussi.
pub fn est_vrai(valeur: Option<&str>) -> bool {
    match valeur {
        None => false,
        Some(texte) => {
            let minuscules = texte.to_lowercase();
            minuscules == "true" || minuscules == "1"
        }
    }
}

/// Active par l experimental, sauf remplacement explicite.
///
/// C est `enabledByExperimental` : si `cle` est absente, on lit
/// `OPENCODE_EXPERIMENTAL`, sinon on lit `cle`.
pub fn active_par_experimental(cle: Option<&str>, experimental: Option<&str>) -> bool {
    match cle {
        None => est_vrai(experimental),
        Some(_) => est_vrai(cle),
    }
}

/// Repli FFF : sur Windows l absence vaut active, ailleurs inactif.
///
/// Traduit `fff === undefined ? process.platform === "win32"`
/// pour `OPENCODE_DISABLE_FFF` et son jumeau copy-on-select.
pub fn repli_windows_si_absent(valeur: Option<&str>, est_windows: bool) -> bool {
    match valeur {
        None => est_windows,
        Some(_) => est_vrai(valeur),
    }
}

/// Le client a afficher, `"cli"` quand la variable est absente.
///
/// Traduit `process.env["OPENCODE_CLIENT"] ?? "cli"`. Une chaine vide
/// presente reste une chaine vide : seul `None` declenche le repli.
pub fn client_ou_defaut(valeur: Option<&str>) -> String {
    match valeur {
        None => CLIENT_DEFAUT.to_string(),
        Some(texte) => texte.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_absence_vaut_faux_comme_un_undefined() {
        assert!(!est_vrai(None));
    }

    #[test]
    fn true_et_1_sont_vrais_en_toute_casse() {
        assert!(est_vrai(Some("true")));
        assert!(est_vrai(Some("TRUE")));
        assert!(est_vrai(Some("True")));
        assert!(est_vrai(Some("1")));
    }

    #[test]
    fn les_autres_valeurs_sont_fausses_meme_en_casse_mixte() {
        assert!(!est_vrai(Some("false")));
        assert!(!est_vrai(Some("0")));
        assert!(!est_vrai(Some("yes")));
        assert!(!est_vrai(Some("")));
        assert!(!est_vrai(Some("2")));
        assert!(!est_vrai(Some(" true ")));
    }

    #[test]
    fn l_experimental_ne_sert_que_si_la_cle_est_absente() {
        assert!(active_par_experimental(None, Some("true")));
        assert!(!active_par_experimental(None, Some("false")));
        assert!(!active_par_experimental(None, None));
        assert!(active_par_experimental(Some("1"), Some("false")));
        assert!(!active_par_experimental(Some("0"), Some("true")));
    }

    #[test]
    fn l_absence_sur_windows_vaut_active_ailleurs_inactif() {
        assert!(repli_windows_si_absent(None, true));
        assert!(!repli_windows_si_absent(None, false));
        assert!(repli_windows_si_absent(Some("true"), false));
        assert!(!repli_windows_si_absent(Some("false"), true));
    }

    #[test]
    fn seul_l_absent_declenche_le_client_par_defaut() {
        assert_eq!(client_ou_defaut(None), "cli");
        assert_eq!(client_ou_defaut(Some("tui")), "tui");
        assert_eq!(client_ou_defaut(Some("")), "");
    }
}
