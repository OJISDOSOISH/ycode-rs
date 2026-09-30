//! Portage Rust de `opencode/packages/core/src/config/compaction.ts`.
//!
//! La source tient en quinze lignes et ne contient que deux declarations de
//! schema, sans aucune logique :
//!
//! ```ts
//! export * as ConfigCompaction from "./compaction"
//!
//! import { Schema } from "effect"
//! import { NonNegativeInt } from "../schema"
//!
//! export class Keep extends Schema.Class<Keep>("ConfigV2.Compaction.Keep")({
//!   tokens: NonNegativeInt.pipe(Schema.optional),
//! }) {}
//!
//! export class Info extends Schema.Class<Info>("ConfigV2.Compaction")({
//!   auto: Schema.Boolean.pipe(Schema.optional),
//!   prune: Schema.Boolean.pipe(Schema.optional),
//!   keep: Keep.pipe(Schema.optional),
//!   buffer: NonNegativeInt.pipe(Schema.optional),
//! }) {}
//! ```
//!
//! Ce fichier est donc la forme de la section `compaction` du fichier de
//! configuration, celle qui regle quand et comment une session est compactee.
//! La **logique** de compaction n'est pas ici : elle vit dans
//! `packages/core/src/session/compaction.ts`, deja portee dans
//! `src/core/session/compaction.rs`, et c'est la que se trouvent les valeurs par
//! defaut. On ne les duplique pas ici, sinon deux sources de verite
//! divergeraient des que l'une bouge.
//!
//! ## Les noms de champs : ici, pas de piege
//!
//! Les cinq noms du schema sont des **mots bascules** : `auto`, `prune`, `keep`,
//! `buffer`, `tokens`. Aucun n'est en `camelCase`, donc aucun `rename` n'est
//! necessaire pour la correction. On en pose quand meme un explicite, comme
//! dans `config_formatter.rs` et `config_tool_output.rs` : il ecrit ce que le nom
//! du champ Rust dit deja, et sert de verrou si le champ est renomme un jour.
//! Le JSON doit rester `{"auto": true, "keep": {"tokens": 2000}}`.
//!
//! ## `NonNegativeInt`
//!
//! `NonNegativeInt` vient de `packages/schema/src/schema.ts` et vaut
//! `Schema.Int.check(Schema.isGreaterThanOrEqualTo(0))` : un entier positif **ou
//! nul**. On le porte par `NonNegativeInt = u64`.
//!
//! Contrairement au `PositiveInt` de `config_tool_output.rs`, ici **aucune
//! fonction de controle supplementaire n'est necessaire** : le type non signe
//! refuse deja les negatifs, et le schema accepte `0`, que `u64` accepte
//! aussi. La traduction par `u64` est donc exactement equivalente au
//! `check(Schema.isGreaterThanOrEqualTo(0))`, sans reste. C'est le point a
//! verifier en relecture, parce qu'il est facile d recopier machinalement le
//! `est_positif` du fichier voisin alors qu'il serait faux ici.
//!
//! ## Le piege `?` contre `??` : le zero
//!
//! Le consommateur, `session/compaction.ts`, lit ces champs ainsi :
//!
//! ```ts
//! auto: current.auto ?? result.auto,
//! buffer: current.buffer ?? result.buffer,
//! tokens: current.keep?.tokens ?? result.tokens,
//! ```
//!
//! Ce sont des **coalescents** : ils ne remplacent que `null` et `undefined`.
//! Donc `keep: { tokens: 0 }` **survit**, il n'est pas remplace par le defaut de
//! 8 000 tokens. Une traduction par veracite le jetterait, puisque `0` est falsy
//! en JavaScript. Pour la meme raison, `keep: { }` donne bien `undefined` pour
//! `tokens` (le `?.` court-circuite sur `keep` present mais vide), alors que
//! `keep: { tokens: 0 }` ne le donne pas.
//!
//! Les methodes `auto_ou`, `buffer_ou` et `tokens_ou` ci-dessous encodent
//! exactement ce `??` : absence de valeur, et rien d'autre. Elles **ne viennent
//! pas** de `config/compaction.ts`, qui est purement declaratif ; elles sont la
//! traduction du seul endroit ou ces champs sont lus. Le defaut leur est passe
//! en parametre, jamais ecrit en dur, pour ne pas creer une deuxieme source de
//! verite face aux constantes de `src/core/session/compaction.rs`
//! (`DEFAULT_BUFFER = 20_000`, `DEFAULT_KEEP_TOKENS = 8_000`).
//!
//! ## `prune` n'est lu nulle part dans le coeur
//!
//! `prune` est declare dans le schema, et c'est tout : dans
//! `packages/core/src`, le seul type `Settings` de `session/compaction.ts` ne
//! contient que `auto`, `buffer` et `tokens`. Seul `v1/config/migrate.ts` recopie
//! le champ, et `v1/config/config.ts` le declare avec la description
//! "Enable pruning of old tool outputs (default: false)". Le champ est donc
//! porte ici pour rester fidele au JSON de configuration et a la migration, pas
//! parce qu'il piloterait quoi que ce soit. On ne lui invente pas de
//! comportement.
//!
//! ## Proprietes inconnues
//!
//! Comme en TypeScript, les champs en trop sont ignores et non refuses :
//! `Schema.Class` ne pose pas de controle d'exces. On ne met donc pas
//! `deny_unknown_fields`.

use serde::{Deserialize, Serialize};

/// Entier positif ou nul.
///
/// Equivalent de `NonNegativeInt` dans `packages/schema/src/schema.ts`, qui
/// vaut `Schema.Int.check(Schema.isGreaterThanOrEqualTo(0))`.
///
/// Le type non signe fait tout le travail du `check` : il refuse les negatifs,
/// et il accepte `0`, ce que le schema accepte aussi.
pub type NonNegativeInt = u64;

/// Nombre de jetons d'historique recent a preserver verbatim.
///
/// Schema d'origine : `ConfigV2.Compaction.Keep`.
///
/// Le seul champ est lui-meme optionnel : un objet `keep` present mais vide est
/// valide, et ne fixe donc aucun budget. C'est ce que fait
/// `current.keep?.tokens ?? result.tokens` cote consommateur.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Keep {
    /// Budget de jetons conserves tels quels apres une compaction.
    #[serde(rename = "tokens", default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<NonNegativeInt>,
}

impl Keep {
    /// Budget de jetons, ou le defaut fourni si l'utilisateur n'en a fixe aucun.
    ///
    /// Traduit `current.keep?.tokens ?? result.tokens`. Le defaut vient de
    /// l'appelant, jamais d'ici : il vit dans
    /// `src/core/session::compaction::DEFAULT_KEEP_TOKENS`.
    ///
    /// Le point non evident est `Some(0)` : il est renvoye tel quel. Le
    /// coalescent JavaScript ne remplace que `null` et `undefined`, donc un
    /// zero explicite est une demande de ne preserver **rien**, et elle doit
    /// survivre.
    pub fn tokens_ou(&self, defaut: NonNegativeInt) -> NonNegativeInt {
        match self.tokens {
            Some(valeur) => valeur,
            None => defaut,
        }
    }
}

/// Contenu du champ `compaction` de la configuration.
///
/// Schema d'origine : `ConfigV2.Compaction`.
///
/// Les quatre champs sont optionnels : une section `compaction` absente du
/// fichier de configuration, ou presente mais vide, est valide et laisse
/// l'appelant appliquer ses propres valeurs par defaut. Aucune valeur par
/// defaut n'est inscrite dans la structure.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Info {
    /// Declenche la compaction automatiquement quand le contexte est plein.
    #[serde(rename = "auto", default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,

    /// Elague lesvieilles sorties d'outils. Declare mais non lu dans le coeur.
    #[serde(rename = "prune", default, skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,

    /// Budget d'historique recent a preserver verbatim.
    #[serde(rename = "keep", default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<Keep>,

    /// Marge de securite conservee libre avant de declencher la compaction.
    #[serde(rename = "buffer", default, skip_serializing_if = "Option::is_none")]
    pub buffer: Option<NonNegativeInt>,
}

impl Info {
    /// Compactage automatique, ou le defaut fourni si l'utilisateur n'a rien
    /// dit.
    ///
    /// Traduit `current.auto ?? result.auto`. Comme pour les entiers, seule
    /// l'absence declenche le defaut : `Some(false)` est une desactivation
    /// explicite et doit etre honoree.
    pub fn auto_ou(&self, defaut: bool) -> bool {
        match self.auto {
            Some(valeur) => valeur,
            None => defaut,
        }
    }

    /// Marge de securite, ou le defaut fourni si l'utilisateur n'a rien dit.
    ///
    /// Traduit `current.buffer ?? result.buffer`. Le defaut vient de
    /// `src::core::session::compaction::DEFAULT_BUFFER`.
    pub fn buffer_ou(&self, defaut: NonNegativeInt) -> NonNegativeInt {
        match self.buffer {
            Some(valeur) => valeur,
            None => defaut,
        }
    }

    /// Budget de jetons a preserver, ou le defaut fourni si l'utilisateur n'a
    /// rien dit, ni sur `keep`, ni sur `keep.tokens`.
    ///
    /// Traduit `current.keep?.tokens ?? result.tokens` : l'absence de `keep` et
    /// celle de `tokens` Aboutissent au meme defaut, et seule une valeur
    /// ecrite dans le fichier de configuration l'ecarte.
    pub fn tokens_ou(&self, defaut: NonNegativeInt) -> NonNegativeInt {
        match &self.keep {
            Some(keep) => keep.tokens_ou(defaut),
            None => defaut,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Info, Keep};

    #[test]
    fn une_section_absente_ne_serialise_que_des_accolades_vides() {
        // Une section `compaction` vide ne doit pas polluer le JSON avec des
        // `null` : en TypeScript, `Schema.optional` retire la cle.
        let info = Info::default();
        assert_eq!(info.auto, None);
        assert_eq!(info.prune, None);
        assert_eq!(info.keep, None);
        assert_eq!(info.buffer, None);
        assert_eq!(serde_json::to_string(&info).unwrap(), "{}");
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Les cinq noms du TS sont des mots bascules, deja identiques a Rust.
        // Le `rename` explicite sert de verrou : si quelqu'un renomme un champ
        // en interne plus tard, le JSON produit ne bouge pas.
        let info = Info {
            auto: Some(true),
            prune: Some(false),
            keep: Some(Keep { tokens: Some(2_000) }),
            buffer: Some(10_000),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["auto"], true);
        assert_eq!(json["prune"], false);
        assert_eq!(json["keep"]["tokens"], 2_000);
        assert_eq!(json["buffer"], 10_000);
        assert_eq!(json.as_object().unwrap().len(), 4, "aucun champ en trop");
        assert_eq!(json["keep"].as_object().unwrap().len(), 1, "aucun champ en trop dans keep");
    }

    #[test]
    fn un_keep_vide_ne_serialise_que_des_accolades_vides() {
        let keep = Keep::default();
        assert_eq!(keep.tokens, None);
        assert_eq!(serde_json::to_string(&keep).unwrap(), "{}");
    }

    #[test]
    fn un_zero_explicite_survit_a_la_relecture_et_a_l_ecriture() {
        // Le piege `?` contre `??` de ce fichier. `0` est falsy en JavaScript,
        // mais le coalescent ne remplace que `null` et `undefined` : un zero
        // demande de ne preserver rien, et doit rester ecrit.
        let info: Info = serde_json::from_str("{\"keep\":{\"tokens\":0},\"buffer\":0}").unwrap();
        assert_eq!(info.keep.as_ref().unwrap().tokens, Some(0));
        assert_eq!(info.buffer, Some(0));
        assert_eq!(serde_json::to_string(&info).unwrap(), "{\"keep\":{\"tokens\":0},\"buffer\":0}");
    }

    #[test]
    fn un_zero_ne_donne_pas_le_defaut_lors_de_la_lecture_des_reglages() {
        // Memes donnees, lues comme le fait `session/compaction.ts`.
        let info: Info = serde_json::from_str("{\"keep\":{\"tokens\":0},\"buffer\":0}").unwrap();
        assert_eq!(info.tokens_ou(8_000), 0);
        assert_eq!(info.buffer_ou(20_000), 0);
    }

    #[test]
    fn un_keep_sans_tokens_laisse_le_defaut_s_appliquer() {
        // `current.keep?.tokens` vaut `undefined` quand `tokens` manque : le
        // defaut reprend la main, exactement comme quand `keep` manque
        // lui-meme.
        let info: Info = serde_json::from_str("{\"keep\":{}}").unwrap();
        assert_eq!(info.keep.as_ref().unwrap().tokens, None);
        assert_eq!(info.tokens_ou(8_000), 8_000);
    }

    #[test]
    fn un_desactivation_explicite_du_compactage_auto_est_honoree() {
        // `auto: false` ne doit pas etre confondu avec une absence de `auto`.
        // Le defaut de la source est `true` : confondre les deux
        // reactiverait une compaction que l'utilisateur a desactivee.
        let info: Info = serde_json::from_str("{\"auto\":false}").unwrap();
        assert!(!info.auto_ou(true));
        assert!(Info::default().auto_ou(true));
    }

    #[test]
    fn un_entier_negatif_est_refuse_a_la_lecture() {
        // Le type non signe fait exactement le travail du
        // `check(Schema.isGreaterThanOrEqualTo(0))`.
        assert!(serde_json::from_str::<Info>("{\"buffer\": -1}").is_err());
        assert!(serde_json::from_str::<Info>("{\"keep\":{\"tokens\": -1}}").is_err());
    }

    #[test]
    fn un_nombre_non_entier_est_refuse_a_la_lecture() {
        // Un flottant ou une chaine n'est pas un `Schema.Int`.
        assert!(serde_json::from_str::<Info>("{\"buffer\": 1.5}").is_err());
        assert!(serde_json::from_str::<Info>("{\"buffer\": \"20000\"}").is_err());
        assert!(serde_json::from_str::<Info>("{\"keep\":{\"tokens\": 1.5}}").is_err());
    }

    #[test]
    fn la_configuration_ne_porte_aucune_valeur_par_defaut() {
        // Les defauts appartiennent au consommateur, pas a la configuration :
        // une section vide ne doit rien figer. Les valeurs utilisees ici sont
        // celles de `session/compaction.ts`, passees en parametre.
        let vide = Info::default();
        assert!(vide.auto_ou(true));
        assert_eq!(vide.buffer_ou(20_000), 20_000);
        assert_eq!(vide.tokens_ou(8_000), 8_000);
        // Le defaut est fourni, jamais fige : un autre defaut donne un autre
        // resultat, ce qui prouve qu'il vient bien de l'appelant.
        assert_eq!(vide.tokens_ou(0), 0);
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // `Schema.Class` ne refuse pas les champs en trop : on ne pose pas
        // `deny_unknown_fields`.
        let info: Info = serde_json::from_str("{\"buffer\": 10000, \"inconnu\": true}").unwrap();
        assert_eq!(info.buffer, Some(10_000));
    }

    #[test]
    fn la_configuration_complete_se_relait_telle_quelle() {
        // Le cas du fichier de configuration reel, relu puis reecrit.
        let json = "{\"auto\":true,\"prune\":false,\"keep\":{\"tokens\":2000},\"buffer\":10000}";
        let info: Info = serde_json::from_str(json).unwrap();
        assert!(info.auto_ou(true));
        assert_eq!(info.prune, Some(false));
        assert_eq!(info.tokens_ou(8_000), 2_000);
        assert_eq!(info.buffer_ou(20_000), 10_000);
        assert_eq!(serde_json::to_string(&info).unwrap(), json);
    }
}
