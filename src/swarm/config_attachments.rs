//! Portage de `packages/core/src/config/attachments.ts`.
//!
//! La source tient en 15 lignes et declare deux classes de schema Effect :
//! `Image` (quatre champs optionnels) et `Info` (un champ `image`
//! optionnel). Aucune fonction, aucune valeur par defaut, aucun test
//! TypeScript pour ce module.
//!
//! Le portage est en donnees pures : deux structs serialisables avec des
//! `Option`, et les invariants que la source impose (champs tous absents par
//! defaut, entier strictement positif quand present). `PositiveInt` n existe
//! qu a la compilation cote TypeScript ; ici c est un `i64` valide par
//! `est_entier_positif`.

use serde::{Deserialize, Serialize};

/// Image jointe : les quatre reglages sont optionnels.
///
/// `auto_resize` est un booleen libre, les trois autres sont des entiers qui
/// doivent etre strictement positifs quand ils sont presents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Image {
    /// Redimensionnement automatique, absent par defaut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resize: Option<bool>,
    /// Largeur maximale, strictement positive quand presente.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<i64>,
    /// Hauteur maximale, strictement positive quand presente.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<i64>,
    /// Taille maximale en base64, strictement positive quand presente.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_base64_bytes: Option<i64>,
}

/// Section `attachments` de la configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Info {
    /// Reglages d image, absents par defaut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Image>,
}

/// Vrai si la valeur est un entier strictement positif.
///
/// C est le predicat `PositiveInt` de la source, vu comme une fonction pure
/// pour rester testable sans le codec Effect.
pub fn est_entier_positif(valeur: i64) -> bool {
    valeur > 0
}

/// Les champs numeriques de `image` sont-ils tous valides ?
///
/// Un champ absent est valide ; un champ present doit etre strictement
/// positif. Un `0` ou un negatif rend `false`.
pub fn image_est_valide(image: &Image) -> bool {
    [image.max_width, image.max_height, image.max_base64_bytes]
        .iter()
        .all(|champ| champ.map(est_entier_positif).unwrap_or(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_config_vide_ne_serialise_aucune_cle() {
        let info = Info::default();
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn une_image_vide_ne_serialise_aucune_cle() {
        let json = serde_json::to_value(&Image::default()).unwrap();
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn les_quatre_champs_sont_optionnels_et_independants() {
        let image = Image { auto_resize: Some(true), ..Image::default() };
        assert!(image_est_valide(&image));
        let json = serde_json::to_value(&image).unwrap();
        assert_eq!(json.get("auto_resize"), Some(&serde_json::json!(true)));
        assert!(json.get("max_width").is_none());
    }

    #[test]
    fn un_zero_ou_un_negatif_est_refuse_sur_les_trois_champs() {
        assert!(!est_entier_positif(0));
        assert!(!est_entier_positif(-5));
        assert!(est_entier_positif(1));
        let image = Image { max_width: Some(0), ..Image::default() };
        assert!(!image_est_valide(&image));
        let image = Image { max_height: Some(-2), ..Image::default() };
        assert!(!image_est_valide(&image));
    }

    #[test]
    fn un_absent_vaut_valide_et_un_present_positif_aussi() {
        let image = Image {
            max_width: Some(800),
            max_height: Some(600),
            max_base64_bytes: Some(1_000_000),
            ..Image::default()
        };
        assert!(image_est_valide(&image));
    }

    #[test]
    fn l_aller_retour_json_conserve_la_forme() {
        let info = Info {
            image: Some(Image { auto_resize: Some(false), max_width: Some(100), ..Image::default() }),
        };
        let texte = serde_json::to_string(&info).unwrap();
        let relue: Info = serde_json::from_str(&texte).unwrap();
        assert_eq!(relue, info);
    }
}
