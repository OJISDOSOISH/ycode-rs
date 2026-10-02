//! Portage de `packages/core/src/config/attachments.ts`.
//!
//! La source tient en quinze lignes : un reexport d'espace de noms vers
//! elle-meme, puis deux classes de schema `Image` et `Info`.
//!
//! ```ts
//! export class Image extends Schema.Class<Image>("ConfigV2.Attachments.Image")({
//!   auto_resize: Schema.Boolean.pipe(Schema.optional),
//!   max_width: PositiveInt.pipe(Schema.optional),
//!   max_height: PositiveInt.pipe(Schema.optional),
//!   max_base64_bytes: PositiveInt.pipe(Schema.optional),
//! }) {}
//! export class Info extends Schema.Class<Info>("ConfigV2.Attachments")({
//!   image: Image.pipe(Schema.optional),
//! }) {}
//! ```
//!
//! Ne pas confondre avec `packages/core/src/v1/config/attachment.ts`
//! (singulier, porte par `src/swarm/v1_config_attachment.rs`) : les champs
//! sont les memes, mais les identifiants different
//! (`"ConfigV2.Attachments.Image"` contre `"ImageAttachmentConfig"`) et la
//! forme est une `Schema.Class` ici contre une `Schema.Struct` la-bas. Ce
//! module porte le pluriel, avec ses propres identifiants, et rien d'autre.
//!
//! `PositiveInt` vient de `@opencode-ai/schema/schema` : un entier
//! strictement positif. Tous les champs sont optionnels, donc absents par
//! defaut. Les `#[serde(rename)]` ci-dessous sont volontairement redondants :
//! les noms sont deja en snake_case, ils figent le contrat d'echange.

use serde::{Deserialize, Serialize};

/// Identifiant de schema de `Image`, tel qu'ecrit dans la source.
pub const IMAGE_IDENTIFIER: &str = "ConfigV2.Attachments.Image";

/// Identifiant de schema de `Info`.
pub const INFO_IDENTIFIER: &str = "ConfigV2.Attachments";

/// Cle de section dans la configuration v2 (pluriel).
pub const SECTION_KEY: &str = "attachments";

/// Image de la configuration des pieces jointes : quatre reglages
/// optionnels, absents par defaut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Image {
    /// Redimensionnement automatique, absent par defaut.
    #[serde(rename = "auto_resize", skip_serializing_if = "Option::is_none")]
    pub auto_resize: Option<bool>,
    /// Largeur maximale en pixels, entier strictement positif si present.
    #[serde(rename = "max_width", skip_serializing_if = "Option::is_none")]
    pub max_width: Option<i64>,
    /// Hauteur maximale en pixels, entier strictement positif si present.
    #[serde(rename = "max_height", skip_serializing_if = "Option::is_none")]
    pub max_height: Option<i64>,
    /// Taille maximale en octets du base64, entier strictement positif.
    #[serde(rename = "max_base64_bytes", skip_serializing_if = "Option::is_none")]
    pub max_base64_bytes: Option<i64>,
}

/// Configuration des pieces jointes : une image optionnelle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Info {
    /// Sous-section image, absente par defaut.
    #[serde(rename = "image", skip_serializing_if = "Option::is_none")]
    pub image: Option<Image>,
}

/// Vrai si la valeur est un entier strictement positif.
///
/// Transcription de `PositiveInt` : `0` et les negatifs sont refuses.
pub fn est_entier_positif(valeur: i64) -> bool {
    valeur > 0
}

/// Valide une image : chaque borne renseignee doit etre strictement positive.
pub fn valider_image(image: &Image) -> Result<(), &'static str> {
    for borne in [image.max_width, image.max_height, image.max_base64_bytes] {
        if let Some(valeur) = borne {
            if !est_entier_positif(valeur) {
                return Err("la borne doit etre un entier strictement positif");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_identifiants_de_schema_sont_ceux_du_pluriel() {
        assert_eq!(IMAGE_IDENTIFIER, "ConfigV2.Attachments.Image");
        assert_eq!(INFO_IDENTIFIER, "ConfigV2.Attachments");
        assert_ne!(IMAGE_IDENTIFIER, "ImageAttachmentConfig");
    }

    #[test]
    fn la_cle_de_section_est_au_pluriel() {
        assert_eq!(SECTION_KEY, "attachments");
    }

    #[test]
    fn une_image_par_defaut_n_a_aucun_champ_renseigne() {
        let image = Image::default();
        assert_eq!(image.auto_resize, None);
        assert_eq!(image.max_width, None);
        assert_eq!(image.max_height, None);
        assert_eq!(image.max_base64_bytes, None);
        assert!(valider_image(&image).is_ok());
    }

    #[test]
    fn un_entier_positif_passe_et_zero_ou_negatif_echoue() {
        assert!(est_entier_positif(1));
        assert!(est_entier_positif(2_000));
        assert!(!est_entier_positif(0));
        assert!(!est_entier_positif(-1));
    }

    #[test]
    fn une_borne_a_zero_ou_negative_est_refusee() {
        let image = Image { max_width: Some(0), ..Image::default() };
        assert!(valider_image(&image).is_err());
        let image = Image { max_height: Some(-5), ..Image::default() };
        assert!(valider_image(&image).is_err());
    }

    #[test]
    fn une_borne_positive_est_acceptee() {
        let image = Image {
            auto_resize: Some(false),
            max_width: Some(2_000),
            max_height: Some(2_000),
            max_base64_bytes: Some(5 * 1024 * 1024),
        };
        assert!(valider_image(&image).is_ok());
    }

    #[test]
    fn les_noms_serialises_sont_ceux_du_typescript() {
        let image = Image { auto_resize: Some(true), max_width: Some(10), ..Image::default() };
        let json = serde_json::to_value(&image).expect("serialisation");
        assert_eq!(json.get("auto_resize"), Some(&serde_json::json!(true)));
        assert_eq!(json.get("max_width"), Some(&serde_json::json!(10)));
        assert!(json.get("autoResize").is_none());
        assert!(json.get("maxWidth").is_none());
    }

    #[test]
    fn les_champs_absents_disparaissent_du_json() {
        let image = Image::default();
        let json = serde_json::to_string(&image).expect("serialisation");
        assert_eq!(json, "{}");
        let info = Info::default();
        let json = serde_json::to_string(&info).expect("serialisation");
        assert_eq!(json, "{}");
    }

    #[test]
    fn une_info_avec_image_fait_l_aller_retour_json() {
        let info = Info {
            image: Some(Image { auto_resize: Some(false), ..Image::default() }),
        };
        let texte = serde_json::to_string(&info).expect("serialisation");
        let relue: Info = serde_json::from_str(&texte).expect("deserialisation");
        assert_eq!(relue, info);
    }

    #[test]
    fn une_valeur_fausse_survit_car_le_coalescent_teste_la_nullite() {
        // `image.auto_resize ?? true` conserve `false` : l'absence se dit
        // `None`, pas `Some(false)`.
        let image = Image { auto_resize: Some(false), ..Image::default() };
        let json = serde_json::to_value(&image).expect("serialisation");
        assert_eq!(json.get("auto_resize"), Some(&serde_json::json!(false)));
    }
}
