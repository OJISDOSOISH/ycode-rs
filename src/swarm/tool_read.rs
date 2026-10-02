//! Portage de `packages/core/src/tool/read.ts`.
//!
//! La source tient en 117 lignes et declare l outil `read` : lecture d un
//! fichier texte ou d une image supportee, pagination d un gros fichier par
//! offset de ligne, ou listage d un repertoire. Les chemins relatifs se
//! resolvent depuis la Location active.
//!
//! Le portage retient ce qui est pur et observable sans Effect : le nom de
//! l outil, la liste des MIMEs d image supportes, la decision de sortie
//! modele pour une image, et les messages d erreur. L enregistrement
//! `Tools.register`, la resolution `LocationMutation` et les appels
//! `ReadToolFileSystem` appartiennent au moteur, pas a ce fichier.

use serde::{Deserialize, Serialize};

/// Nom de l outil tel qu enregistre.
pub const NOM_OUTIL: &str = "read";

/// MIMEs d image que la sortie modele sait restituer.
pub const MIMES_IMAGE_SUPPORTES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

/// Entree de l outil : chemin plus pagination optionnelle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entree {
    /// Chemin demande, relatif a la Location ou absolu interne.
    pub path: String,
    /// Offset 1-based de ligne ou d entree, absent par defaut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    /// Nombre maximal d entrees ou de lignes, absent par defaut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// Vrai si le MIME est une image restituee au modele.
pub fn est_image_supportee(mime: &str) -> bool {
    MIMES_IMAGE_SUPPORTES.contains(&mime)
}

/// Vrai si la sortie lue doit produire une piece jointe modele.
///
/// Traduit `toModelOutput` : il faut un encodage `base64` et un MIME supporte.
pub fn produit_sortie_image(encodage: Option<&str>, mime: &str) -> bool {
    matches!(encodage, Some("base64")) && est_image_supportee(mime)
}

/// Texte modele renvoye quand une image est lue avec succes.
pub fn texte_image_lue() -> &'static str {
    "Image read successfully"
}

/// Message d echec quand la lecture echoue sans erreur metier connue.
///
/// Traduit `` `Unable to read ${input.path}` ``.
pub fn message_echec_lecture(path: &str) -> String {
    format!("Unable to read {}", path)
}

/// Vrai si le contenu base64 non image doit echouer en `BinaryFileError`.
///
/// Traduit la branche `encoding === "base64"` hors MIMEs supportes.
pub fn est_fichier_binaire(encodage: Option<&str>, mime: &str) -> bool {
    matches!(encodage, Some("base64")) && !est_image_supportee(mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_nom_enregistre_est_read() {
        assert_eq!(NOM_OUTIL, "read");
    }

    #[test]
    fn les_quatre_mimes_images_sont_reconnus() {
        assert!(est_image_supportee("image/jpeg"));
        assert!(est_image_supportee("image/png"));
        assert!(est_image_supportee("image/gif"));
        assert!(est_image_supportee("image/webp"));
        assert!(!est_image_supportee("image/svg+xml"));
        assert!(!est_image_supportee("text/plain"));
    }

    #[test]
    fn seule_une_image_base64_produit_une_sortie_modele() {
        assert!(produit_sortie_image(Some("base64"), "image/png"));
        assert!(!produit_sortie_image(Some("utf8"), "image/png"));
        assert!(!produit_sortie_image(Some("base64"), "application/pdf"));
        assert!(!produit_sortie_image(None, "image/jpeg"));
    }

    #[test]
    fn le_texte_image_reprend_la_chaine_exacte() {
        assert_eq!(texte_image_lue(), "Image read successfully");
    }

    #[test]
    fn l_echec_reprend_le_chemin_demande() {
        assert_eq!(message_echec_lecture("a/b.txt"), "Unable to read a/b.txt");
    }

    #[test]
    fn un_base64_non_image_est_un_fichier_binaire() {
        assert!(est_fichier_binaire(Some("base64"), "application/zip"));
        assert!(!est_fichier_binaire(Some("base64"), "image/gif"));
        assert!(!est_fichier_binaire(Some("utf8"), "application/zip"));
    }
}
