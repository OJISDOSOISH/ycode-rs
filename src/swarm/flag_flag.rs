//! Portage de `packages/core/src/flag/flag.ts`.
//!
//! La source lit des variables d'environnement et en derive des drapeaux :
//!
//! ```ts
//! export function truthy(key: string) {
//!   const value = process.env[key]?.toLowerCase()
//!   return value === "true" || value === "1"
//! }
//! ```
//!
//! `truthy` teste la valeur, jamais la presence : une variable absente vaut
//! `undefined` et donne `false`, mais une variable renseignee a `"0"`,
//! `"false"` ou `""` donne aussi `false`. Seuls `"true"` et `"1"`, sans
//! distinction de casse, donnent `true`.
//!
//! Le reste du fichier applique cette regle a une vingtaine de cles, avec
//! trois cas particuliers portes ici sous forme pure :
//!
//! - `enabledByExperimental` : quand la cle manque, elle herite de
//!   `OPENCODE_EXPERIMENTAL` ;
//! - `OPENCODE_DISABLE_FFF` et `..._DISABLE_COPY_ON_SELECT` : quand la cle
//!   manque, le defaut est `process.platform === "win32"` ;
//! - `OPENCODE_CLIENT` : `?? "cli"`, donc chaine vide conservee.
//!
//! Les fonctions ci-dessous prennent la valeur lue (ou son absence) en
//! argument au lieu de lire l'environnement : elles restent pures et
//! testables. Deux minces enveloppes lisent vraiment `std::env` pour les
//! appelants.

/// Nom de la variable maitresse du mode experimental.
pub const EXPERIMENTAL_KEY: &str = "OPENCODE_EXPERIMENTAL";

/// Nom de la variable du client, avec defaut `"cli"`.
pub const CLIENT_KEY: &str = "OPENCODE_CLIENT";

/// Defaut du client quand la variable est absente.
pub const CLIENT_DEFAULT: &str = "cli";

/// Vrai si la valeur d'environnement active le drapeau.
///
/// Seuls `"true"` et `"1"`, en toute casse, activent. Tout le reste
/// desactive, y compris `"0"`, `"false"`, `""` et l'absence de valeur.
/// C'est la transcription exacte de `value === "true" || value === "1"`
/// apres `?.toLowerCase()`.
pub fn est_vrai(valeur: Option<&str>) -> bool {
    match valeur {
        None => false,
        Some(texte) => {
            let minuscule = texte.to_lowercase();
            minuscule == "true" || minuscule == "1"
        }
    }
}

/// Lit un drapeau dans l'environnement reel.
pub fn drapeau(cle: &str) -> bool {
    est_vrai(std::env::var(cle).ok().as_deref())
}

/// Resolution d'un drapeau experimental avec heritage.
///
/// Quand `valeur` est absente, le drapeau herite de `defaut_experimental`
/// (la valeur de `OPENCODE_EXPERIMENTAL`) ; sinon il suit sa propre valeur.
/// Transcription de `enabledByExperimental`.
pub fn resolu_experimental(valeur: Option<&str>, defaut_experimental: Option<&str>) -> bool {
    match valeur {
        None => est_vrai(defaut_experimental),
        Some(_) => est_vrai(valeur),
    }
}

/// Defaut dependant de la plateforme pour les deux drapeaux qui valent
/// `process.platform === "win32"` en l'absence de variable.
///
/// `sur_windows` remplace `process.platform` pour rester pur. Quand la
/// variable est renseignee, son contenu tranche comme d'habitude.
pub fn resolu_avec_defaut_windows(valeur: Option<&str>, sur_windows: bool) -> bool {
    match valeur {
        None => sur_windows,
        Some(_) => est_vrai(valeur),
    }
}

/// Client effectif : la variable si elle est presente, `"cli"` sinon.
///
/// Le `??` teste la nullite : une chaine vide est presente et survit.
pub fn client(valeur: Option<&str>) -> String {
    valeur.unwrap_or(CLIENT_DEFAULT).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_valeur_absente_desactive_le_drapeau() {
        assert!(!est_vrai(None));
    }

    #[test]
    fn seuls_true_et_1_activent_le_drapeau() {
        assert!(est_vrai(Some("true")));
        assert!(est_vrai(Some("1")));
        assert!(!est_vrai(Some("0")));
        assert!(!est_vrai(Some("false")));
        assert!(!est_vrai(Some("")));
        assert!(!est_vrai(Some("yes")));
    }

    #[test]
    fn la_casse_ne_change_rien_mais_les_espaces_comptent() {
        assert!(est_vrai(Some("TRUE")));
        assert!(est_vrai(Some("True")));
        assert!(!est_vrai(Some(" true")));
        assert!(!est_vrai(Some("true ")));
    }

    #[test]
    fn l_heritage_experimental_ne_joue_qu_en_l_absence_de_cle() {
        assert!(resolu_experimental(None, Some("true")));
        assert!(!resolu_experimental(None, None));
        assert!(!resolu_experimental(Some("false"), Some("true")));
        assert!(resolu_experimental(Some("true"), Some("false")));
    }

    #[test]
    fn le_defaut_windows_ne_joue_qu_en_l_absence_de_cle() {
        assert!(resolu_avec_defaut_windows(None, true));
        assert!(!resolu_avec_defaut_windows(None, false));
        assert!(!resolu_avec_defaut_windows(Some("0"), true));
        assert!(resolu_avec_defaut_windows(Some("1"), false));
    }

    #[test]
    fn le_client_vaut_cli_par_defaut_mais_garde_la_chaine_vide() {
        assert_eq!(client(None), "cli");
        assert_eq!(client(Some("tui")), "tui");
        // `??` contre `?` : une chaine vide est presente, pas absente.
        assert_eq!(client(Some("")), "");
    }

    #[test]
    fn les_noms_de_cles_sont_stables() {
        assert_eq!(EXPERIMENTAL_KEY, "OPENCODE_EXPERIMENTAL");
        assert_eq!(CLIENT_KEY, "OPENCODE_CLIENT");
        assert_eq!(CLIENT_DEFAULT, "cli");
    }
}
