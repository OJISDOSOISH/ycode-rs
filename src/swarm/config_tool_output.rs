//! Portage Rust de `opencode/packages/core/src/config/tool-output.ts`.
//!
//! La source tient en neuf lignes et ne contient qu'une declaration de schema :
//!
//! ```ts
//! export * as ConfigToolOutput from "./tool-output"
//!
//! import { Schema } from "effect"
//! import { PositiveInt } from "../schema"
//!
//! export class Info extends Schema.Class<Info>("ConfigV2.ToolOutput")({
//!   max_lines: PositiveInt.pipe(Schema.optional),
//!   max_bytes: PositiveInt.pipe(Schema.optional),
//! }) {}
//! ```
//!
//! Il n'y a donc aucun calcul a porter : ce fichier est la forme de la section
//! `tool_output` du fichier de configuration, celle qui regle a quel point les
//! sorties d'outils sont tronquees. Le comportement de troncature n'est pas ici :
//! il vit dans `packages/core/src/tool-output-store.ts`, qui lit cette
//! configuration. Le type `ConfigV2.ToolOutput` de la premiere ligne
//! TypeScript n'est qu'un nom de schema, il ne produit aucun champ en plus.
//!
//! ## Les deux noms de champs : ici, pas de piege
//!
//! `max_lines` et `max_bytes` sont **deja** en `snake_case` dans le
//! TypeScript. Il ne faut surtout pas les renommer en `maxLines` et
//! `maxBytes` en Rust, et il ne faut pas non plus ajouter un
//! `#[serde(rename = "maxLines")]` : le JSON doit rester
//! `{"max_lines": ..., "max_bytes": ...}`. Le `#[serde(rename = "max_lines")]`
//! ci-dessous est volontairement redondant, il ecrit ce que le nom du champ
//! Rust dit deja, et sert de garde-fou si le champ est renomme un jour. C'est
//! aussi le style deja employe dans `src/question.rs`.
//!
//! ## `PositiveInt`
//!
//! `PositiveInt` vient de `packages/schema/src/schema.ts` et vaut
//! `Schema.Int.check(Schema.isGreaterThan(0))` : un entier **strictement
//! positif**. On le porte par le type `PositiveInt = u64`. Le type non signe
//! refuse les valeurs negatives des la deserialisation, ce qui couvre la
//! majeure du controle. Il reste une queue : le TypeScript refuse aussi `0`,
//! et `u64` l'accepte. C'est le seul comportement que le type ne peut pas
//! rendre, et c'est ce que fait `est_positif`.
//!
//! ## Aucune valeur par defaut ici
//!
//! La source ne declare aucun defaut, on n'en invente donc aucun. Les defauts
//! `MAX_LINES = 2_000` et `MAX_BYTES = 50 * 1024` sont declares dans
//! `packages/core/src/tool-output-store.ts`, et c'est la que se fait le
//! `configured.max_lines ?? MAX_LINES`. Les dupliquer ici creerait deux sources
//! de verite qui divergeraient des que l'une bouge.
//!
//! ## Rapport avec la compaction
//!
//! `src/core/session/compaction.rs` porte `TOOL_OUTPUT_MAX_CHARS = 2_000`.
//! C'est le meme ordre de grandeur que le `MAX_LINES = 2_000` du
//! `tool-output-store.ts`, mais ce n'est pas la meme regulation : celui-la
//! compte des **caracteres** injectes dans le contexte du modele, celui-ci des
//! **lignes** conservees dans le magasin de sorties d'outils. Le fichier de
//! compaction n'est ni modifie ni appele d'ici. On note la coincidence pour que
//! personne ne fusionne les deux seuils par erreur en croyant a un doublon.

use serde::{Deserialize, Serialize};

/// Entier strictement positif.
///
/// Equivalent de `PositiveInt` dans
/// `packages/schema/src/schema.ts`, qui vaut
/// `Schema.Int.check(Schema.isGreaterThan(0))`.
pub type PositiveInt = u64;

/// Verifie la partie du schema que le type seul ne peut pas exprimer.
///
/// `u64` refuse deja les negatifs. Il reste `0`, que `Schema.isGreaterThan(0)`
/// refuse en TypeScript et que `u64` accepte. Les appelants qui doivent
/// valider une valeur de configuration utilisent cette fonction.
pub const fn est_positif(valeur: PositiveInt) -> bool {
    valeur > 0
}

/// Seuils de troncature des sorties d'outils (`ConfigV2.ToolOutput`).
///
/// Les deux champs sont optionnels : une section `tool_output` absente du
/// fichier de configuration, ou presente mais vide, est valide et laisse
/// l'appelant appliquer ses propres valeurs par defaut.
///
/// Les proprietes inconnues sont tolerees, comme en TypeScript : `Schema.Class`
/// ignore par defaut les champs en trop au lieu de refuser l'objet. On ne pose
/// donc pas `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Nombre maximal de lignes conservees dans une sortie d'outil.
    #[serde(rename = "max_lines", skip_serializing_if = "std::option::Option::is_none")]
    pub max_lines: Option<PositiveInt>,

    /// Nombre maximal d'octets conserves dans une sortie d'outil.
    #[serde(rename = "max_bytes", skip_serializing_if = "std::option::Option::is_none")]
    pub max_bytes: Option<PositiveInt>,
}

#[cfg(test)]
mod tests {
    use super::{est_positif, Info};

    #[test]
    fn une_section_sans_seuil_ne_serialise_que_des_accolades_vides() {
        // Une section vide ne doit pas polluer le JSON avec des `null`.
        let info = Info {
            max_lines: None,
            max_bytes: None,
        };
        assert_eq!(serde_json::to_string(&info).unwrap(), "{}");
    }

    #[test]
    fn les_noms_des_champs_restent_en_snake_case() {
        // Piege du portage : `max_lines` ne doit pas devenir `maxLines`.
        let info = Info {
            max_lines: Some(2_000),
            max_bytes: Some(51_200),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"max_lines\":2000"), "json inattendu : {}", json);
        assert!(json.contains("\"max_bytes\":51200"), "json inattendu : {}", json);
        assert!(!json.contains("maxLines"), "json inattendu : {}", json);
        assert!(!json.contains("maxBytes"), "json inattendu : {}", json);
    }

    #[test]
    fn une_section_vide_se_relu_sans_erreur() {
        let info: Info = serde_json::from_str("{}").unwrap();
        assert_eq!(info.max_lines, None);
        assert_eq!(info.max_bytes, None);
    }

    #[test]
    fn un_seul_seuil_lu_laisse_l_autre_absent() {
        // Les deux seuils sont independants : en donner un n'oblige pas a
        // donner l'autre.
        let info: Info = serde_json::from_str("{\"max_bytes\": 51200}").unwrap();
        assert_eq!(info.max_bytes, Some(51_200));
        assert_eq!(info.max_lines, None);
    }

    #[test]
    fn un_seuil_lu_puis_reecrit_redonne_la_meme_valeur() {
        let info: Info = serde_json::from_str("{\"max_lines\": 2000}").unwrap();
        assert_eq!(info.max_lines, Some(2_000));
        assert_eq!(serde_json::to_string(&info).unwrap(), "{\"max_lines\":2000}");
    }

    #[test]
    fn un_seuil_negatif_est_refuse_a_la_lecture() {
        // Le type non signe fait le travail du `check(Schema.isGreaterThan(0))`.
        assert!(serde_json::from_str::<Info>("{\"max_lines\": -1}").is_err());
        assert!(serde_json::from_str::<Info>("{\"max_bytes\": -51200}").is_err());
    }

    #[test]
    fn un_seuil_non_entier_est_refuse_a_la_lecture() {
        // Un flottant ou une chaine n'est pas un `Schema.Int`.
        assert!(serde_json::from_str::<Info>("{\"max_bytes\": 1.5}").is_err());
        assert!(serde_json::from_str::<Info>("{\"max_lines\": \"2000\"}").is_err());
    }

    #[test]
    fn zero_est_signale_comme_non_positif_par_le_controle() {
        // Seule divergence connue avec le TypeScript : `u64` accepte `0`, il
        // faut donc passer par le controle explicite.
        assert!(!est_positif(0));
        assert!(est_positif(1));
        assert!(est_positif(51_200));
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // `Schema.Class` ne refuse pas les champs en trop : on ne pose pas
        // `deny_unknown_fields`.
        let info: Info = serde_json::from_str("{\"max_lines\": 10, \"inconnu\": true}").unwrap();
        assert_eq!(info.max_lines, Some(10));
    }
}
