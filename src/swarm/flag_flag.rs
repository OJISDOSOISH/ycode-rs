//! Portage Rust de `packages/core/src/flag/flag.ts`.
//!
//! La source lit `process.env` et `process.platform` pour exposer un objet
//! `Flag` : des booleens (`truthy`), des chaines brutes, deux valeurs par
//! defaut dependantes de la plateforme (desactive sur Windows quand la
//! variable est absente) et des accesseurs evalues a chaque lecture.
//!
//! Ce module ne lit ni l environnement ni la plateforme : les deux sont des
//! parametres (`env` et [`Platform`]). Les trois regles de la source
//! deviennent trois fonctions pures, testables avec une table factice.

use std::collections::HashMap;

/// Plateforme d execution, en parametre plutot qu en lecture globale.
///
/// La source lit `process.platform`. Le rendre explicite garde les valeurs
/// par defaut dependantes de la plateforme testables sans changer de machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Comportement de `process.platform === "win32"`.
    Windows,
    /// Tout le reste.
    Other,
}

/// Cle du mode experimental global, lu en repli par [`enabled_by_experimental`].
pub const KEY_EXPERIMENTAL: &str = "OPENCODE_EXPERIMENTAL";

/// Cle qui desactive `fff`, avec defaut Windows.
pub const KEY_DISABLE_FFF: &str = "OPENCODE_DISABLE_FFF";

/// Cle qui desactive la copie sur selection, avec defaut Windows.
pub const KEY_DISABLE_COPY_ON_SELECT: &str = "OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT";

/// Cle du client, avec defaut `"cli"` quand la variable est absente.
pub const KEY_CLIENT: &str = "OPENCODE_CLIENT";

/// Valeur du client quand `OPENCODE_CLIENT` n est pas defini.
pub const DEFAULT_CLIENT: &str = "cli";

/// Reproduit `truthy` : la variable, en minuscules, vaut `"true"` ou `"1"`.
///
/// Une variable absente vaut faux. Il n y a pas de `trim` dans la source :
/// `" 1"` n est pas `"1"`, et seule la casse est ignoree.
pub fn truthy(env: &HashMap<String, String>, key: &str) -> bool {
    match env.get(key) {
        None => false,
        Some(raw) => {
            let bas = raw.to_lowercase();
            bas == "true" || bas == "1"
        }
    }
}

/// Reproduit `enabledByExperimental` : quand la cle est absente, c est le
/// mode experimental global qui decide, sinon c est la cle elle-meme.
pub fn enabled_by_experimental(env: &HashMap<String, String>, key: &str) -> bool {
    if !env.contains_key(key) {
        truthy(env, KEY_EXPERIMENTAL)
    } else {
        truthy(env, key)
    }
}

/// Reproduit le defaut de `OPENCODE_DISABLE_FFF` : sauf definition explicite,
/// la valeur suit la plateforme (desactive sur Windows).
pub fn disable_fff(env: &HashMap<String, String>, platform: Platform) -> bool {
    if !env.contains_key(KEY_DISABLE_FFF) {
        platform == Platform::Windows
    } else {
        truthy(env, KEY_DISABLE_FFF)
    }
}

/// Reproduit le defaut de la copie sur selection : meme regle que `fff`.
pub fn disable_copy_on_select(env: &HashMap<String, String>, platform: Platform) -> bool {
    if !env.contains_key(KEY_DISABLE_COPY_ON_SELECT) {
        platform == Platform::Windows
    } else {
        truthy(env, KEY_DISABLE_COPY_ON_SELECT)
    }
}

/// Reproduit l accesseur `OPENCODE_CLIENT` : la variable, ou `"cli"`.
pub fn client(env: &HashMap<String, String>) -> String {
    env.get(KEY_CLIENT)
        .cloned()
        .unwrap_or_else(|| DEFAULT_CLIENT.to_string())
}

/// Construit une table d environnement factice pour les tests.
#[cfg(test)]
fn environnement(paires: &[(&str, &str)]) -> HashMap<String, String> {
    paires
        .iter()
        .map(|(cle, valeur)| (cle.to_string(), valeur.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_variable_absente_est_fausse() {
        let env = environnement(&[]);
        assert!(!truthy(&env, "OPENCODE_PURE"));
        assert!(!truthy(&env, KEY_EXPERIMENTAL));
    }

    #[test]
    fn truthy_ne_reconnait_que_true_et_1_sans_casse() {
        for vraie in ["true", "TRUE", "True", "1"] {
            let env = environnement(&[("CLE", vraie)]);
            assert!(truthy(&env, "CLE"), "{vraie} doit etre vraie");
        }
        for fausse in ["false", "0", "yes", "", " 1", "true "] {
            let env = environnement(&[("CLE", fausse)]);
            assert!(!truthy(&env, "CLE"), "{fausse} doit etre fausse");
        }
    }

    #[test]
    fn le_repli_experimental_suit_le_mode_global_quand_la_cle_manque() {
        let actif = environnement(&[(KEY_EXPERIMENTAL, "true")]);
        assert!(enabled_by_experimental(&actif, "OPENCODE_EXPERIMENTAL_WORKSPACES"));
        let inactif = environnement(&[]);
        assert!(!enabled_by_experimental(&inactif, "OPENCODE_EXPERIMENTAL_WORKSPACES"));
    }

    #[test]
    fn la_cle_definie_l_emporte_sur_le_mode_global() {
        let env = environnement(&[(KEY_EXPERIMENTAL, "true"), ("CLE", "false")]);
        assert!(!enabled_by_experimental(&env, "CLE"));
        let env = environnement(&[(KEY_EXPERIMENTAL, "false"), ("CLE", "1")]);
        assert!(enabled_by_experimental(&env, "CLE"));
    }

    #[test]
    fn fff_et_copie_suivent_windows_sans_definition_explicite() {
        let vide = environnement(&[]);
        assert!(disable_fff(&vide, Platform::Windows));
        assert!(!disable_fff(&vide, Platform::Other));
        assert!(disable_copy_on_select(&vide, Platform::Windows));
        assert!(!disable_copy_on_select(&vide, Platform::Other));
    }

    #[test]
    fn une_definition_explicite_l_emporte_sur_la_plateforme() {
        let env = environnement(&[(KEY_DISABLE_FFF, "false")]);
        assert!(!disable_fff(&env, Platform::Windows));
        let env = environnement(&[(KEY_DISABLE_FFF, "true")]);
        assert!(disable_fff(&env, Platform::Other));
        let env = environnement(&[(KEY_DISABLE_COPY_ON_SELECT, "0")]);
        assert!(!disable_copy_on_select(&env, Platform::Windows));
    }

    #[test]
    fn le_client_vaut_cli_par_defaut() {
        assert_eq!(client(&environnement(&[])), "cli");
        assert_eq!(client(&environnement(&[(KEY_CLIENT, "tui")])), "tui");
    }
}
