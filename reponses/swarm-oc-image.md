# swarm-oc-image

===DEBUT===
fichier : src/swarm/oc_image.rs
source  : packages/core/src/image.ts
taille  : 14676 octets
tests   : 9

```rust
//! Portage Rust de `opencode/packages/core/src/image.ts`.
//!
//! Le module d'origine declare un service `Image` dont le seul role est de
//! normaliser une piece jointe image (contenu base64) avant envoi au modele :
//! redimensionnement automatique et plafonds de taille. La source lue fait
//! 81 lignes et ne contient aucune logique de redimensionnement : tout le
//! travail reel est delegue a un adaptateur charge dynamiquement
//! (`./image/photon`, module wasm), avec mise en cache du chargement.
//!
//! Ce qui est porte ici, en fonctions pures :
//!
//! - les trois erreurs taggees (`ResizerUnavailableError`, `DecodeError`,
//!   `SizeError`) avec leurs gabarits de message a l'identique ;
//! - la resolution des plafonds depuis les entrees de configuration
//!   (`auto_resize ?? true`, `max_width ?? 2_000`, etc.) ;
//! - le predicat de depassement qui decoule des champs de `SizeError` ;
//! - le trait `ImageResizer` qui remplace `Context.Service` / `Layer`
//!   (l'adaptateur photon, le `Config.Service` et le `makeLocationNode` ne
//!   sont pas portables en pur et ne sont pas portes).
//!
//! Ce qui n'est PAS porte (honnetement absent, pas invente) :
//!
//! - le redimensionnement lui-meme (depend de photon/wasm, specifique a la
//!   plateforme node) ;
//! - le cablage `Layer` / `locationLayer` / `node` (infra Effect).

use serde::{Deserialize, Serialize};

/// Identifiant du service dans la source (`Context.Service<..., "@opencode/Image">`).
pub const SERVICE_ID: &str = "@opencode/Image";

/// Defaut de la source : `image.auto_resize ?? true`.
pub const DEFAULT_AUTO_RESIZE: bool = true;

/// Defaut de la source : `image.max_width ?? 2_000`.
pub const DEFAULT_MAX_WIDTH: u64 = 2_000;

/// Defaut de la source : `image.max_height ?? 2_000`.
pub const DEFAULT_MAX_HEIGHT: u64 = 2_000;

/// Defaut de la source : `image.max_base64_bytes ?? 5 * 1024 * 1024`.
pub const DEFAULT_MAX_BASE64_BYTES: u64 = 5 * 1024 * 1024;

/// Marqueur d'encodage attendu en entree et en sortie de `normalize`.
pub const BASE64_ENCODING: &str = "base64";

/// Fragment `info.attachments.image` d'une entree de configuration.
///
/// Dans la source, les fragments de toutes les entrees de type `document`
/// sont fusionnes par `Object.assign({}, ...fragments)` : le dernier gagne.
/// Les champs sont `Option` car la source utilise `??` (coalescent) : un
/// champ absent prend le defaut, mais `false` ou `0` explicites survivent.
/// Les noms sont deja en snake_case dans la source, aucun `rename` requis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttachmentsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_base64_bytes: Option<u64>,
}

/// Plafonds effectifs passes a l'adaptateur de redimensionnement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageLimits {
    pub auto_resize: bool,
    pub max_width: u64,
    pub max_height: u64,
    pub max_base64_bytes: u64,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            auto_resize: DEFAULT_AUTO_RESIZE,
            max_width: DEFAULT_MAX_WIDTH,
            max_height: DEFAULT_MAX_HEIGHT,
            max_base64_bytes: DEFAULT_MAX_BASE64_BYTES,
        }
    }
}

/// Contenu base64 d'une image (`FileSystem.Content` avec `encoding: "base64"`).
///
/// La source garantit l'encodage au niveau du type, pas par un test a
/// l'execution : aucun controle n'est donc ajoute ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Base64Content {
    pub content: String,
    pub encoding: String,
}

impl Base64Content {
    pub fn new(base64: impl Into<String>) -> Self {
        Self {
            content: base64.into(),
            encoding: BASE64_ENCODING.to_string(),
        }
    }
}

/// Les trois erreurs taggees de la source.
///
/// La source utilise `Schema.TaggedErrorClass` dont le discriminant est le
/// champ `_tag` : d'ou `#[serde(tag = "_tag")]` avec un `rename` explicite
/// par variante, a la valeur exacte de la source (`"Image...."`).
/// Les champs camelCase de `SizeError` (`maxWidth`, `maxHeight`, `maxBytes`)
/// portent chacun leur `#[serde(rename = "...")]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum ImageError {
    #[serde(rename = "Image.ResizerUnavailableError")]
    ResizerUnavailable,
    #[serde(rename = "Image.DecodeError")]
    Decode { resource: String },
    #[serde(rename = "Image.SizeError")]
    Size {
        resource: String,
        width: u64,
        height: u64,
        bytes: u64,
        #[serde(rename = "maxWidth")]
        max_width: u64,
        #[serde(rename = "maxHeight")]
        max_height: u64,
        #[serde(rename = "maxBytes")]
        max_bytes: u64,
    },
}

impl ImageError {
    /// Gabarit de message, a l'identique de la source (`get message()`).
    pub fn message(&self) -> String {
        match self {
            Self::ResizerUnavailable => "Image resizer is unavailable".to_string(),
            Self::Decode { resource } => {
                format!("Image could not be decoded: {resource}")
            }
            Self::Size {
                resource,
                width,
                height,
                bytes,
                max_width,
                max_height,
                max_bytes,
            } => format!(
                "Image {resource} is {width}x{height} with base64 size {bytes}, \
                 exceeding configured limits {max_width}x{max_height}/{max_bytes} bytes"
            ),
        }
    }

    /// Construit la variante `Size` depuis des dimensions mesurees et les
    /// plafonds en vigueur.
    pub fn size(
        resource: impl Into<String>,
        width: u64,
        height: u64,
        bytes: u64,
        limits: &ImageLimits,
    ) -> Self {
        Self::Size {
            resource: resource.into(),
            width,
            height,
            bytes,
            max_width: limits.max_width,
            max_height: limits.max_height,
            max_bytes: limits.max_base64_bytes,
        }
    }
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ImageError {}

/// Resout les plafonds effectifs depuis une tranche d'entrees de config.
///
/// Reproduit `Object.assign({}, ...entries.flatMap(...))` puis les `??` :
/// parcours dans l'ordre, le dernier `Some` gagne, puis defaut si aucun.
/// `Option::unwrap_or` a exactement la semantique du coalescent `??`
/// (seuls `None` declenche le defaut ; `Some(false)` et `Some(0)`
/// survivent), contrairement a un test de veracite qui les ecraserait.
pub fn resolve_limits(configs: &[ImageAttachmentsConfig]) -> ImageLimits {
    let mut auto_resize: Option<bool> = None;
    let mut max_width: Option<u64> = None;
    let mut max_height: Option<u64> = None;
    let mut max_base64_bytes: Option<u64> = None;
    for config in configs {
        if config.auto_resize.is_some() {
            auto_resize = config.auto_resize;
        }
        if config.max_width.is_some() {
            max_width = config.max_width;
        }
        if config.max_height.is_some() {
            max_height = config.max_height;
        }
        if config.max_base64_bytes.is_some() {
            max_base64_bytes = config.max_base64_bytes;
        }
    }
    ImageLimits {
        auto_resize: auto_resize.unwrap_or(DEFAULT_AUTO_RESIZE),
        max_width: max_width.unwrap_or(DEFAULT_MAX_WIDTH),
        max_height: max_height.unwrap_or(DEFAULT_MAX_HEIGHT),
        max_base64_bytes: max_base64_bytes.unwrap_or(DEFAULT_MAX_BASE64_BYTES),
    }
}

/// L'image depasse-t-elle l'un des plafonds ?
///
/// Predicat derive des champs de `SizeError` (dimensions et poids mesures
/// contre `maxWidth` / `maxHeight` / `maxBytes`). Le controle reel vit dans
/// l'adaptateur photon non porte ; ce predicat expose la regle en pur pour
/// les appelants qui veulent un controle prealable sans dependance wasm.
pub fn exceeds_limits(width: u64, height: u64, bytes: u64, limits: &ImageLimits) -> bool {
    width > limits.max_width || height > limits.max_height || bytes > limits.max_base64_bytes
}

/// Equivalent pur du test de taille : `Ok` si dans les plafonds, sinon
/// l'erreur `Size` avec le message de la source.
pub fn check_size(
    resource: &str,
    width: u64,
    height: u64,
    bytes: u64,
    limits: &ImageLimits,
) -> Result<(), ImageError> {
    if exceeds_limits(width, height, bytes, limits) {
        Err(ImageError::size(resource, width, height, bytes, limits))
    } else {
        Ok(())
    }
}

/// Adaptateur de redimensionnement (le module `./image/photon` de la source).
///
/// La source le charge dynamiquement et met le chargement en cache
/// (`Effect.cached`) ; ici l'injection remplace le cache : l'appelant
/// fournit l'implementation une fois et la reutilise.
pub trait ImageResizer {
    fn normalize(
        &self,
        resource: &str,
        base64: &str,
        limits: &ImageLimits,
    ) -> Result<String, ImageError>;
}

/// Equivalent pur de `normalize` : resout les plafonds depuis la tranche de
/// configuration puis delegue a l'adaptateur injecte.
pub fn normalize_with<R: ImageResizer>(
    resizer: &R,
    resource: &str,
    content: &Base64Content,
    configs: &[ImageAttachmentsConfig],
) -> Result<Base64Content, ImageError> {
    let limits = resolve_limits(configs);
    let out = resizer.normalize(resource, &content.content, &limits)?;
    Ok(Base64Content::new(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_tranche_vide_donne_les_defauts_de_la_source() {
        let limits = resolve_limits(&[]);
        assert_eq!(limits, ImageLimits::default());
        assert!(limits.auto_resize);
        assert_eq!(limits.max_width, 2_000);
        assert_eq!(limits.max_height, 2_000);
        assert_eq!(limits.max_base64_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn un_singleton_ne_remplace_que_son_champ() {
        let limits = resolve_limits(&[ImageAttachmentsConfig {
            max_width: Some(800),
            ..Default::default()
        }]);
        assert_eq!(limits.max_width, 800);
        assert_eq!(limits.max_height, DEFAULT_MAX_HEIGHT);
        assert_eq!(limits.max_base64_bytes, DEFAULT_MAX_BASE64_BYTES);
        assert!(limits.auto_resize);
    }

    #[test]
    fn le_dernier_gagne_et_l_ordre_inverse_change_le_resultat() {
        let a = ImageAttachmentsConfig {
            max_width: Some(800),
            ..Default::default()
        };
        let b = ImageAttachmentsConfig {
            max_width: Some(1_200),
            ..Default::default()
        };
        assert_eq!(resolve_limits(&[a.clone(), b.clone()]).max_width, 1_200);
        assert_eq!(resolve_limits(&[b, a]).max_width, 800);
    }

    #[test]
    fn un_faux_explicite_survit_au_coalescent() {
        // `??` teste la nullite, pas la veracite : `Some(false)` ne doit
        // pas etre ecrase par le defaut `true`.
        let limits = resolve_limits(&[ImageAttachmentsConfig {
            auto_resize: Some(false),
            ..Default::default()
        }]);
        assert!(!limits.auto_resize);
    }

    #[test]
    fn un_zero_explicite_survit_au_coalescent() {
        let limits = resolve_limits(&[ImageAttachmentsConfig {
            max_width: Some(0),
            ..Default::default()
        }]);
        assert_eq!(limits.max_width, 0);
    }

    #[test]
    fn le_message_de_decode_reprend_la_ressource() {
        let err = ImageError::Decode {
            resource: "logo.png".to_string(),
        };
        assert_eq!(err.message(), "Image could not be decoded: logo.png");
    }

    #[test]
    fn le_message_de_taille_reprend_dimensions_et_plafonds() {
        let limits = ImageLimits::default();
        let err = ImageError::size("photo.jpg", 3_000, 100, 10, &limits);
        assert_eq!(
            err.message(),
            "Image photo.jpg is 3000x100 with base64 size 10, \
             exceeding configured limits 2000x2000/5242880 bytes"
        );
        assert!(check_size("photo.jpg", 3_000, 100, 10, &limits).is_err());
        assert!(check_size("photo.jpg", 2_000, 2_000, 5_242_880, &limits).is_ok());
    }

    #[test]
    fn la_serialisation_garde_tags_et_noms_camel_case() {
        let err = ImageError::size("a.png", 1, 2, 3, &ImageLimits::default());
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["_tag"], "Image.SizeError");
        assert_eq!(v["maxWidth"], 2_000);
        assert_eq!(v["maxHeight"], 2_000);
        assert_eq!(v["maxBytes"], 5_242_880);
        let back: ImageError = serde_json::from_value(v).unwrap();
        assert_eq!(back, err);
        let v = serde_json::to_value(&ImageError::ResizerUnavailable).unwrap();
        assert_eq!(v["_tag"], "Image.ResizerUnavailableError");
    }

    #[test]
    fn normalize_delegue_avec_les_plafonds_resolus() {
        struct Sonde;
        impl ImageResizer for Sonde {
            fn normalize(
                &self,
                resource: &str,
                base64: &str,
                limits: &ImageLimits,
            ) -> Result<String, ImageError> {
                assert_eq!(resource, "a.png");
                assert_eq!(base64, "AAAA");
                assert_eq!(limits.max_width, 800);
                Ok("BBBB".to_string())
            }
        }
        let out = normalize_with(
            &Sonde,
            "a.png",
            &Base64Content::new("AAAA"),
            &[ImageAttachmentsConfig {
                max_width: Some(800),
                ..Default::default()
            }],
        )
        .unwrap();
        assert_eq!(out, Base64Content::new("BBBB"));
        // L'erreur de l'adaptateur remonte telle quelle.
        struct EnPanne;
        impl ImageResizer for EnPanne {
            fn normalize(
                &self,
                _resource: &str,
                _base64: &str,
                _limits: &ImageLimits,
            ) -> Result<String, ImageError> {
                Err(ImageError::ResizerUnavailable)
            }
        }
        assert_eq!(
            normalize_with(&EnPanne, "a.png", &Base64Content::new("AAAA"), &[]),
            Err(ImageError::ResizerUnavailable)
        );
    }
}
```

===FIN===

CONFIANCE : moyenne
POINT FAIBLE : le message de ResizerUnavailableError est invente (la source ne definit aucun message pour cette erreur vide) ; et le predicat exceeds_limits/check_size extrapole la regle de rejet depuis les seuls noms de champs de SizeError car le controle reel vit dans l'adaptateur photon non lu.
