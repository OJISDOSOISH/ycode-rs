//! Portage Rust de `packages/core/src/database/path.ts`.
//!
//! ## Ce que contient la source
//!
//! 91 lignes, quatre colonnes Drizzle et trois helpers :
//!
//! ```ts
//! function storagePath(input: string) { ... }
//! function isWindowsStoragePath(input: string) { ... }
//! function absolute(input: string) { ... }
//! function toPlatform(input: string) { ... }
//! export const absoluteColumn = customType<{...}>({ ... })
//! export const directoryColumn = customType<{...}>({ ... })
//! export const pathColumn = customType<{...}>({ ... })
//! export const absoluteArrayColumn = customType<{...}>({ ... })
//! ```
//!
//! L'idee centrale : la base stocke les chemins en notation POSIX (`/`), meme
//! sur Windows. `storagePath` normalise vers le stockage, `toPlatform` revient
//! vers la plateforme, et `absolute` valide. `AbsolutePath` (de `../schema`)
//! est une chaine marquee a la compilation, une chaine ordinaire a
//! l'execution : ici c'est `String`.
//!
//! ## La plateforme est un parametre, pas une lecture
//!
//! La source lit `process.platform` a chaque appel. Ce module recoit
//! `est_windows` en parametre : le comportement des deux plateformes est
//! testable depuis n'importe quelle machine, et l'appelant choisit une fois.
//!
//! ## Le cas herite du repertoire vide, qui est reel
//!
//! `directoryColumn` laisse passer `""` dans les deux sens (commentaire de la
//! source : les sessions historiques peuvent persister un repertoire vide),
//! mais valide et normalise tout le reste. La chaine vide n'est donc jamais
//! confondue avec un chemin relatif invalide.

/// Message d'erreur quand un chemin n'est pas absolu, forme de la source.
pub fn message_erreur_non_absolu(entree: &str) -> String {
    format!("Path is not absolute: {entree}")
}

/// Normalise un chemin vers sa forme de stockage, comme `storagePath`.
///
/// Hors Windows, identite. Sur Windows, chaque `\` devient `/`.
pub fn vers_stockage(entree: &str, est_windows: bool) -> String {
    if est_windows {
        entree.replace('\\', "/")
    } else {
        entree.to_string()
    }
}

/// Dit si une forme de stockage designe un chemin Windows absolu.
///
/// Transcription de `/^[A-Za-z]:\//.test(input) || input.startsWith("//")` :
/// lettre de lecteur suivie de `:/`, ou chemin UNC en notation stockee.
pub fn est_chemin_stockage_windows(entree: &str) -> bool {
    let octets = entree.as_bytes();
    if octets.len() >= 3
        && octets[0].is_ascii_alphabetic()
        && octets[1] == b':'
        && octets[2] == b'/'
    {
        return true;
    }
    entree.starts_with("//")
}

/// Valide un chemin absolu et rend sa forme de stockage, comme `absolute`.
///
/// Sur POSIX, seul `/...` passe. Sur Windows s'y ajoutent les formes
/// `C:/...` et `//...` (apres normalisation des `\`). Un chemin relatif rend
/// l'erreur de la source.
pub fn valider_absolu(entree: &str, est_windows: bool) -> Result<String, String> {
    let normalise = vers_stockage(entree, est_windows);
    let absolu = normalise.starts_with('/')
        || (est_windows && est_chemin_stockage_windows(&normalise));
    if absolu {
        Ok(normalise)
    } else {
        Err(message_erreur_non_absolu(entree))
    }
}

/// Revient de la forme de stockage vers la plateforme, comme `toPlatform`.
///
/// Sur Windows, un chemin de stockage Windows (`C:/...` ou `//...`) redevient
/// separe par `\`. Tout le reste traverse tel quel.
pub fn vers_plateforme(entree: &str, est_windows: bool) -> String {
    if est_windows && est_chemin_stockage_windows(entree) {
        entree.replace('/', "\\")
    } else {
        entree.to_string()
    }
}

/// `absoluteColumn.toDriver` : valide et normalise vers le stockage.
pub fn colonne_absolue_vers_pilote(entree: &str, est_windows: bool) -> Result<String, String> {
    valider_absolu(entree, est_windows)
}

/// `absoluteColumn.fromDriver` : valide, puis revient vers la plateforme.
pub fn colonne_absolue_depuis_pilote(stocke: &str, est_windows: bool) -> Result<String, String> {
    valider_absolu(stocke, est_windows).map(|valide| vers_plateforme(&valide, est_windows))
}

/// `directoryColumn.toDriver` : le repertoire vide historique traverse tel
/// quel, tout le reste est valide comme une colonne absolue.
pub fn colonne_repertoire_vers_pilote(entree: &str, est_windows: bool) -> Result<String, String> {
    if entree.is_empty() {
        Ok(String::new())
    } else {
        valider_absolu(entree, est_windows)
    }
}

/// `directoryColumn.fromDriver` : symetrique, avec retour vers la plateforme.
pub fn colonne_repertoire_depuis_pilote(stocke: &str, est_windows: bool) -> Result<String, String> {
    if stocke.is_empty() {
        Ok(String::new())
    } else {
        colonne_absolue_depuis_pilote(stocke, est_windows)
    }
}

/// `pathColumn` dans les deux sens : normalisation vers le stockage, sans
/// validation (ce n'est pas une colonne de chemin absolu).
pub fn colonne_chemin_vers_pilote(entree: &str, est_windows: bool) -> String {
    vers_stockage(entree, est_windows)
}

/// `pathColumn.fromDriver` : la source applique aussi `storagePath` a la
/// lecture, pas seulement a l'ecriture.
pub fn colonne_chemin_depuis_pilote(stocke: &str, est_windows: bool) -> String {
    vers_stockage(stocke, est_windows)
}

/// `absoluteArrayColumn.toDriver` : chaque element est valide, le tableau est
/// serialise en JSON.
pub fn colonne_tableau_vers_pilote(
    entrees: &[&str],
    est_windows: bool,
) -> Result<String, String> {
    let mut valides = Vec::with_capacity(entrees.len());
    for entree in entrees {
        valides.push(valider_absolu(entree, est_windows)?);
    }
    serde_json::to_string(&valides).map_err(|e| e.to_string())
}

/// `absoluteArrayColumn.fromDriver` : parse le JSON, valide chaque element et
/// revient vers la plateforme.
pub fn colonne_tableau_depuis_pilote(
    stocke: &str,
    est_windows: bool,
) -> Result<Vec<String>, String> {
    let lus: Vec<String> =
        serde_json::from_str(stocke).map_err(|e| e.to_string())?;
    lus.into_iter()
        .map(|entree| {
            valider_absolu(&entree, est_windows)
                .map(|valide| vers_plateforme(&valide, est_windows))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_stockage_normalise_les_antislashs_uniquement_sur_windows() {
        assert_eq!(vers_stockage("C:\\a\\b", true), "C:/a/b");
        assert_eq!(vers_stockage("C:\\a\\b", false), "C:\\a\\b");
        assert_eq!(vers_stockage("/a/b", true), "/a/b");
        assert_eq!(vers_stockage("/a/b", false), "/a/b");
    }

    #[test]
    fn la_detection_windows_reconnait_lecteur_et_unc_stockes() {
        assert!(est_chemin_stockage_windows("C:/a"));
        assert!(est_chemin_stockage_windows("Z:/"));
        assert!(est_chemin_stockage_windows("//serveur/partage"));
        assert!(!est_chemin_stockage_windows("/a/b"));
        assert!(!est_chemin_stockage_windows("relatif/a"));
        assert!(!est_chemin_stockage_windows("C:relatif"));
    }

    #[test]
    fn l_absolu_posix_ne_connait_que_la_barre_initiale() {
        assert_eq!(valider_absolu("/a/b", false), Ok("/a/b".to_string()));
        assert!(valider_absolu("C:/a", false).is_err());
        assert!(valider_absolu("relatif", false).is_err());
        assert!(valider_absolu("", false).is_err());
    }

    #[test]
    fn l_absolu_windows_accepte_les_trois_formes_apres_normalisation() {
        assert_eq!(valider_absolu("/a/b", true), Ok("/a/b".to_string()));
        assert_eq!(valider_absolu("C:/a", true), Ok("C:/a".to_string()));
        assert_eq!(valider_absolu("C:\\a\\b", true), Ok("C:/a/b".to_string()));
        assert_eq!(
            valider_absolu("//serveur/x", true),
            Ok("//serveur/x".to_string())
        );
        assert!(valider_absolu("relatif", true).is_err());
    }

    #[test]
    fn l_erreur_reprend_le_texte_d_entree_tel_quel() {
        assert_eq!(
            valider_absolu("relatif", false).unwrap_err(),
            "Path is not absolute: relatif"
        );
    }

    #[test]
    fn la_plateforme_ne_reecrit_que_les_chemins_windows_stockes() {
        assert_eq!(vers_plateforme("C:/a/b", true), "C:\\a\\b");
        assert_eq!(vers_plateforme("//s/x", true), "\\\\s\\x");
        assert_eq!(vers_plateforme("/a/b", true), "/a/b");
        assert_eq!(vers_plateforme("C:/a/b", false), "C:/a/b");
    }

    #[test]
    fn la_colonne_absolue_fait_l_aller_retour_vers_la_plateforme() {
        let stocke = colonne_absolue_vers_pilote("C:\\a\\b", true).unwrap();
        assert_eq!(stocke, "C:/a/b");
        assert_eq!(
            colonne_absolue_depuis_pilote(&stocke, true).unwrap(),
            "C:\\a\\b"
        );
        assert!(colonne_absolue_vers_pilote("relatif", true).is_err());
    }

    #[test]
    fn le_repertoire_vide_historique_traverse_sans_validation() {
        assert_eq!(colonne_repertoire_vers_pilote("", true), Ok(String::new()));
        assert_eq!(colonne_repertoire_depuis_pilote("", true), Ok(String::new()));
        // Mais un repertoire non vide reste valide comme un absolu.
        assert!(colonne_repertoire_vers_pilote("relatif", true).is_err());
        assert_eq!(
            colonne_repertoire_vers_pilote("/a", false).unwrap(),
            "/a"
        );
    }

    #[test]
    fn la_colonne_chemin_normalise_sans_valider() {
        // Meme un relatif traverse : ce n'est pas une colonne d'absolu.
        assert_eq!(colonne_chemin_vers_pilote("a\\b", true), "a/b");
        assert_eq!(colonne_chemin_depuis_pilote("a\\b", true), "a/b");
        assert_eq!(colonne_chemin_vers_pilote("a\\b", false), "a\\b");
    }

    #[test]
    fn le_tableau_valide_chaque_element_et_serialise_en_json() {
        let stocke = colonne_tableau_vers_pilote(&["/a", "/b"], false).unwrap();
        assert_eq!(stocke, "[\"/a\",\"/b\"]");
        assert_eq!(
            colonne_tableau_depuis_pilote(&stocke, false).unwrap(),
            vec!["/a".to_string(), "/b".to_string()]
        );
        assert!(colonne_tableau_vers_pilote(&["/a", "relatif"], false).is_err());
        assert!(colonne_tableau_depuis_pilote("pas du json", false).is_err());
    }
}
