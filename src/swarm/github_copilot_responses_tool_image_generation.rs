//! Portage de `github-copilot/responses/tool/image-generation.ts`.
//!
//! La source fait 114 lignes : schema strict des arguments (fond, fidelite,
//! masque, modele, moderation, compression, format, images partielles,
//! qualite, taille), schema de sortie (`result`) et fabrique d outil id
//! `"openai.image_generation"`.
//!
//! Seule la partie pure est portee ici, sans `@ai-sdk/provider-utils` :
//! types d arguments avec leurs unions litterales, validation des bornes
//! (`outputCompression` 0..100, `partialImages` 0..3) et sortie.
//!
//! Volontairement non porte : `createProviderToolFactoryWithOutputSchema`,
//! la validation zod et l execution de l outil.

use serde::{Deserialize, Serialize};

/// Identifiant de l outil tel qu enregistre par la fabrique.
pub const ID_OUTIL: &str = "openai.image_generation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fond {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "transparent")]
    Transparent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FideliteEntree {
    #[serde(rename = "low")]
    Basse,
    #[serde(rename = "high")]
    Haute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatSortie {
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(rename = "webp")]
    Webp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Qualite {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Basse,
    #[serde(rename = "medium")]
    Moyenne,
    #[serde(rename = "high")]
    Haute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Taille {
    #[serde(rename = "1024x1024")]
    Carre,
    #[serde(rename = "1024x1536")]
    Portrait,
    #[serde(rename = "1536x1024")]
    Paysage,
    #[serde(rename = "auto")]
    Auto,
}

/// Masque d inpainting : `fileId` et `imageUrl`, tous deux optionnels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasqueImage {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fileId")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
}

/// Arguments de la generation d image, tous optionnels.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArgumentsGeneration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<Fond>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inputFidelity")]
    pub fidelite_entree: Option<FideliteEntree>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inputImageMask")]
    pub masque: Option<MasqueImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moderation: Option<Moderation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outputCompression")]
    pub compression: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outputFormat")]
    pub format: Option<FormatSortie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "partialImages")]
    pub images_partielles: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<Qualite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Taille>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Moderation {
    #[serde(rename = "auto")]
    Auto,
}

/// Sortie : l image en base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortieGeneration {
    pub result: String,
}

/// Vrai si la compression est dans `0..=100` (ou absente).
pub fn compression_valide(compression: Option<u8>) -> bool {
    match compression {
        None => true,
        Some(v) => v <= 100,
    }
}

/// Vrai si les images partielles sont dans `0..=3` (ou absentes).
pub fn images_partielles_valides(nombre: Option<u8>) -> bool {
    match nombre {
        None => true,
        Some(v) => v <= 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(ID_OUTIL, "openai.image_generation");
    }

    #[test]
    fn les_arguments_par_defaut_sont_tous_absents() {
        let args = ArgumentsGeneration::default();
        let valeur = serde_json::to_value(&args).unwrap();
        assert_eq!(valeur, serde_json::json!({}));
    }

    #[test]
    fn les_litteraux_se_serialisent_en_minuscules() {
        assert_eq!(serde_json::to_string(&Fond::Transparent).unwrap(), "\"transparent\"");
        assert_eq!(serde_json::to_string(&FormatSortie::Webp).unwrap(), "\"webp\"");
        assert_eq!(serde_json::to_string(&Taille::Portrait).unwrap(), "\"1024x1536\"");
        assert_eq!(serde_json::to_string(&Qualite::Moyenne).unwrap(), "\"medium\"");
    }

    #[test]
    fn le_masque_garde_file_id_et_image_url_en_camel_case() {
        let masque = MasqueImage {
            file_id: Some("f-1".to_string()),
            image_url: None,
        };
        let valeur = serde_json::to_value(&masque).unwrap();
        assert_eq!(valeur.get("fileId").and_then(|v| v.as_str()), Some("f-1"));
        assert!(valeur.get("file_id").is_none());
        assert!(valeur.get("imageUrl").is_none());
    }

    #[test]
    fn la_compression_est_bornee_entre_0_et_100() {
        assert!(compression_valide(None));
        assert!(compression_valide(Some(0)));
        assert!(compression_valide(Some(100)));
    }

    #[test]
    fn les_images_partielles_sont_bornees_entre_0_et_3() {
        assert!(images_partielles_valides(None));
        assert!(images_partielles_valides(Some(0)));
        assert!(images_partielles_valides(Some(3)));
        assert!(!images_partielles_valides(Some(4)));
    }

    #[test]
    fn la_sortie_porte_le_resultat_base64() {
        let sortie = SortieGeneration {
            result: "aGVsbG8=".to_string(),
        };
        let valeur = serde_json::to_value(&sortie).unwrap();
        assert_eq!(valeur.get("result").and_then(|v| v.as_str()), Some("aGVsbG8="));
    }
}
