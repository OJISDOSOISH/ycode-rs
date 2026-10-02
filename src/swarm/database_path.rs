//! Portage de `packages/core/src/database/path.ts`.
//!
//! La source definit quatre helpers de chemins et quatre colonnes Drizzle
//! qui les appliquent a l'aller et au retour :
//!
//! - `storagePath` : sur Windows, remplace `\` par `/` ; ailleurs, identite.
//! - `isWindowsStoragePath` : `^[A-Za-z]:\/` ou prefixe `//`.
//! - `absolute` : normalise puis exige un absolu posix, ou un chemin de
//!   stockage Windows quand on est sur Windows, sinon leve
//!   `Path is not absolute: <input>`.
//! - `toPlatform` : sur Windows et pour un chemin de stockage Windows,
//!   remplace `/` par `\` ; sinon identite.
//!
//! Les colonnes `absoluteColumn`, `directoryColumn`, `pathColumn` et
//! `absoluteArrayColumn` sont toutes du `text`. `directoryColumn` laisse
//! passer la chaine vide sans validation (sessions heritees sans repertoire) ;
//! `absoluteArrayColumn` serialise un tableau JSON.
//!
//! `process.platform` est remplace par un booleen `sur_windows` : les
//! fonctions restent pures et testables. `nodePath.posix.isAbsolute` vaut
//! `commence par '/'`.

use serde_json::Value;

/// Vrai si le chemin est un chemin de stockage Windows.
///
/// Soit une lettre de lecteur (`C:/...`), soit un chemin UNC (`//...`).
pub fn est_chemin_stockage_windows(chemin: &str) -> bool {
    if chemin.starts_with("//") {
        return true;
    }
    let octets = chemin.as_bytes();
    octets.len() >= 3
        && octets[0].is_ascii_alphabetic()
        && octets[1] == b':'
        && octets[2] == b'/'
}

/// Normalise un chemin pour le stockage.
///
/// Sur Windows, les `\` deviennent des `/`. Ailleurs, identite.
pub fn chemin_stockage(entree: &str, sur_windows: bool) -> String {
    if sur_windows {
        entree.replace('\\', "/")
    } else {
        entree.to_string()
    }
}

/// Vrai si le chemin est absolu au sens posix (`/` initial).
pub fn est_absolu_posix(chemin: &str) -> bool {
    chemin.starts_with('/')
}

/// Valide et normalise un chemin absolu.
///
/// Rend le chemin de stockage, ou l'erreur `Path is not absolute: <input>`
/// avec l'entree d'origine (non normalisee), comme la source.
pub fn absolu(entree: &str, sur_windows: bool) -> Result<String, String> {
    let normalise = chemin_stockage(entree, sur_windows);
    let accepte = est_absolu_posix(&normalise)
        || (sur_windows && est_chemin_stockage_windows(&normalise));
    if accepte {
        Ok(normalise)
    } else {
        Err(format!("Path is not absolute: {}", entree))
    }
}

/// Rend le chemin utilisable par la plateforme.
///
/// Sur Windows et pour un chemin de stockage Windows, les `/` deviennent
/// des `\`. Sinon identite.
pub fn vers_plateforme(entree: &str, sur_windows: bool) -> String {
    if sur_windows && est_chemin_stockage_windows(entree) {
        entree.replace('/', "\\")
    } else {
        entree.to_string()
    }
}

/// Ecriture `toDriver` de `absoluteColumn` : exige un absolu.
pub fn colonne_absolue_vers_pilote(entree: &str, sur_windows: bool) -> Result<String, String> {
    absolu(entree, sur_windows)
}

/// Lecture `fromDriver` de `absoluteColumn` : valide puis rend plateforme.
pub fn colonne_absolue_depuis_pilote(entree: &str, sur_windows: bool) -> Result<String, String> {
    let valide = absolu(entree, sur_windows)?;
    Ok(vers_plateforme(&valide, sur_windows))
}

/// Ecriture de `directoryColumn` : la chaine vide passe sans validation.
pub fn colonne_repertoire_vers_pilote(entree: &str, sur_windows: bool) -> Result<String, String> {
    if entree.is_empty() {
        return Ok(String::new());
    }
    absolu(entree, sur_windows)
}

/// Lecture de `directoryColumn` : meme regle que l'ecriture, plus conversion.
pub fn colonne_repertoire_depuis_pilote(entree: &str, sur_windows: bool) -> Result<String, String> {
    if entree.is_empty() {
        return Ok(String::new());
    }
    let valide = absolu(entree, sur_windows)?;
    Ok(vers_plateforme(&valide, sur_windows))
}

/// Ecriture et lecture de `pathColumn` : simple normalisation, jamais d'erreur.
pub fn colonne_chemin(entree: &str, sur_windows: bool) -> String {
    chemin_stockage(entree, sur_windows)
}

/// Ecriture de `absoluteArrayColumn` : chaque element doit etre absolu.
pub fn colonne_tableau_absolu_vers_pilote(entreess: &[&str], sur_windows: bool) -> Result<String, String> {
    let mut normalisees = Vec::with_capacity(entreess.len());
    for entree in entreess {
        normalisees.push(absolu(entree, sur_windows)?);
    }
    serde_json::to_string(&normalisees).map_err(|e| e.to_string())
}

/// Lecture de `absoluteArrayColumn` : parse JSON puis valide chaque element.
pub fn colonne_tableau_absolu_depuis_pilote(stocke: &str, sur_windows: bool) -> Result<Vec<String>, String> {
    let brute: Vec<String> = serde_json::from_str(stocke).map_err(|e| e.to_string())?;
    let mut sortie = Vec::with_capacity(brute.len());
    for element in &brute {
        let valide = absolu(element, sur_windows)?;
        sortie.push(vers_plateforme(&valide, sur_windows));
    }
    Ok(sortie)
}

/// Le type de stockage des quatre colonnes est toujours `text`.
pub fn type_stockage_colonne() -> &'static str {
    "text"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_stockage_remplace_les_separateurs_uniquement_sur_windows() {
        assert_eq!(chemin_stockage("C:\\a\\b", true), "C:/a/b");
        assert_eq!(chemin_stockage("C:\\a\\b", false), "C:\\a\\b");
        assert_eq!(chemin_stockage("/a/b", true), "/a/b");
    }

    #[test]
    fn un_chemin_de_stockage_windows_est_lettre_ou_double_slash() {
        assert!(est_chemin_stockage_windows("C:/travail"));
        assert!(est_chemin_stockage_windows("z:/x"));
        assert!(est_chemin_stockage_windows("//serveur/partage"));
        assert!(!est_chemin_stockage_windows("/tmp/travail"));
        assert!(!est_chemin_stockage_windows("relatif/chemin"));
        assert!(!est_chemin_stockage_windows("C:relatif"));
    }

    #[test]
    fn un_absolu_posix_passe_partout_un_relatif_echoue() {
        assert_eq!(absolu("/tmp/x", false), Ok("/tmp/x".to_string()));
        assert_eq!(absolu("/tmp/x", true), Ok("/tmp/x".to_string()));
        assert!(absolu("relatif/x", false).is_err());
        assert!(absolu("relatif/x", true).is_err());
    }

    #[test]
    fn l_erreur_reprend_l_entree_d_origine() {
        let erreur = absolu("relatif/x", false).expect_err("relatif refuse");
        assert_eq!(erreur, "Path is not absolute: relatif/x");
    }

    #[test]
    fn un_lecteur_windows_n_est_accepte_que_sur_windows() {
        assert!(absolu("C:/travail", true).is_ok());
        assert!(absolu("C:/travail", false).is_err());
    }

    #[test]
    fn la_conversion_plateforme_ne_touche_que_windows_et_stockage() {
        assert_eq!(vers_plateforme("C:/a/b", true), "C:\\a\\b");
        assert_eq!(vers_plateforme("C:/a/b", false), "C:/a/b");
        assert_eq!(vers_plateforme("/tmp/x", true), "/tmp/x");
    }

    #[test]
    fn la_colonne_repertoire_laisse_passer_la_chaine_vide() {
        assert_eq!(colonne_repertoire_vers_pilote("", false), Ok(String::new()));
        assert_eq!(colonne_repertoire_depuis_pilote("", true), Ok(String::new()));
        assert!(colonne_repertoire_vers_pilote("relatif", false).is_err());
    }

    #[test]
    fn la_colonne_chemin_ne_valide_jamais() {
        assert_eq!(colonne_chemin("relatif", false), "relatif");
        assert_eq!(colonne_chemin("C:\\a", true), "C:/a");
    }

    #[test]
    fn le_tableau_absolu_fait_l_aller_retour_json() {
        let texte = colonne_tableau_absolu_vers_pilote(&["/a", "/b"], false).expect("ecriture");
        let valeur: Value = serde_json::from_str(&texte).expect("json");
        assert_eq!(valeur, serde_json::json!(["/a", "/b"]));
        let relu = colonne_tableau_absolu_depuis_pilote(&texte, false).expect("lecture");
        assert_eq!(relu, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[test]
    fn le_tableau_absolu_refuse_un_element_relatif() {
        assert!(colonne_tableau_absolu_vers_pilote(&["/a", "relatif"], false).is_err());
        assert!(colonne_tableau_absolu_depuis_pilote("[\"/a\", \"relatif\"]", false).is_err());
    }

    #[test]
    fn le_type_de_stockage_est_toujours_texte() {
        assert_eq!(type_stockage_colonne(), "text");
    }
}
