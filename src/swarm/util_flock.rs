//! Portage de `packages/core/src/util/flock.ts`.
//!
//! La source fait 358 lignes autour d un verrou par fichier : un repertoire
//! `locks/<sha1(key)>.lock` avec `heartbeat` + `meta.json`, une detection de
//! proprietaire mort (stale), un braquage `.breaker` pour le nettoyage, une
//! boucle d attente avec gigue et un battement de coeur periodique.
//!
//! Seule la partie pure est portee ici, sans filesystem ni `Effect` :
//! - les constantes de temporisation par defaut ;
//! - la resolution des options (`??` = nullite, pas veracite) ;
//! - les chemins (`locks/`, `<hash>.lock`, `heartbeat`, `meta.json`) ;
//! - le predicat de rassissement, l intervalle de battement, la suite des
//!   delais (`* 1.7`, plafond) et les bornes de la gigue ;
//! - les messages d erreur litteraux et la verification du jeton a la levee.
//!
//! Volontairement non porte : `mkdir`, `stat`, `rm`, `readFile`, `writeFile`,
//! `utimes`, `setInterval`, `AbortSignal`, `onWait`, le `Effect.acquireRelease`
//! final et tout ce qui dort ou touche le disque. Ces parties dependent du
//! runtime et sont signalees comme sautees.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// `staleMs` par defaut : un verrou sans battement depuis plus d une minute
/// est considere comme abandonne.
pub const STALE_MS_PAR_DEFAUT: u64 = 60_000;
/// `timeoutMs` par defaut : cinq minutes d attente avant abandon.
pub const TIMEOUT_MS_PAR_DEFAUT: u64 = 5 * 60_000;
/// `baseDelayMs` par defaut : cent millisecondes avant la premiere reessai.
pub const DELAI_BASE_PAR_DEFAUT_MS: u64 = 100;
/// `maxDelayMs` par defaut : le delai ne depasse jamais deux secondes.
pub const DELAI_MAX_PAR_DEFAUT_MS: u64 = 2_000;
/// Facteur multiplicatif de la boucle d attente (`delay * 1.7`).
pub const FACTEUR_DELAI: f64 = 1.7;
/// Part de gigue : `floor(ms * 0.3)` de chaque cote.
pub const PART_GIGUE: f64 = 0.3;

/// Nom du sous-repertoire des verrous sous l etat global.
pub const DOSSIER_VERROUS: &str = "locks";
/// Fichier touche a chaque battement de coeur.
pub const FICHIER_BATTEMENT: &str = "heartbeat";
/// Fichier de metadonnees du proprietaire.
pub const FICHIER_META: &str = "meta.json";
/// Suffixe du repertoire de braquage pendant le nettoyage d un mort.
pub const SUFFIXE_BRAQUAGE: &str = ".breaker";
/// Extension des repertoires de verrou.
pub const EXTENSION_VERROU: &str = ".lock";

/// Message quand le global n a jamais ete pose (`Flock global not set`).
pub const MESSAGE_GLOBAL_ABSENT: &str = "Flock global not set";
/// Prefixe du message d echec d attente (`Timed out waiting for lock: <key>`).
pub const PREFIXE_DELAI_DEPASSE: &str = "Timed out waiting for lock: ";
/// Message quand le battement existait deja a la creation.
pub const MESSAGE_BATTEMENT_COMPROMIS: &str =
    "Lock acquired but heartbeat already existed (possible compromise).";
/// Message quand `meta.json` existait deja a la creation.
pub const MESSAGE_META_COMPROMIS: &str =
    "Lock acquired but meta.json already existed (possible compromise).";
/// Message quand les metadonnees ont disparu avant la levee.
pub const MESSAGE_LEVEE_SANS_META: &str =
    "Refusing to release: lock is compromised (metadata missing).";
/// Message quand les metadonnees ne parsent plus avant la levee.
pub const MESSAGE_LEVEE_META_INVALIDE: &str =
    "Refusing to release: lock is compromised (metadata invalid).";
/// Message quand le jeton relu n est pas celui du proprietaire.
pub const MESSAGE_LEVEE_JETON_DIFFERENT: &str =
    "Refusing to release: lock token mismatch (not the owner).";
/// Message d abandon (`Aborted`).
pub const MESSAGE_ABANDON: &str = "Aborted";

/// Evenement `onWait` de la source : cle, numero de tentative, delai, deja attendu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvenementAttente {
    pub key: String,
    pub attempt: u64,
    pub delay: u64,
    pub waited: u64,
}

/// Options de `Flock.acquire`, sous la forme exacte de l interface TypeScript :
/// tout est optionnel, `None` signifie `undefined`.
///
/// `onWait` et `signal` ne sont pas portables en pur (callback et
/// `AbortSignal`) et sont donc absents : la boucle qui les appelle n est pas
/// portee non plus.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsVerrou {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_delay_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_delay_ms: Option<u64>,
}

/// Options apres application des defauts de la source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsResolues {
    pub stale_ms: u64,
    pub timeout_ms: u64,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl OptionsResolues {
    /// Applique `{ staleMs = 60_000, timeoutMs = 300_000, baseDelayMs = 100,
    /// maxDelayMs = 2_000 }`.
    ///
    /// Les destructurations de la source testent `undefined`, donc la
    /// NULLITE : une option presente qui vaut `0` est conservee.
    pub fn resoudre(options: &OptionsVerrou) -> Self {
        OptionsResolues {
            stale_ms: options.stale_ms.unwrap_or(STALE_MS_PAR_DEFAUT),
            timeout_ms: options.timeout_ms.unwrap_or(TIMEOUT_MS_PAR_DEFAUT),
            base_delay_ms: options.base_delay_ms.unwrap_or(DELAI_BASE_PAR_DEFAUT_MS),
            max_delay_ms: options.max_delay_ms.unwrap_or(DELAI_MAX_PAR_DEFAUT_MS),
        }
    }
}

/// Contenu de `meta.json` : jeton, pid, hote, date de creation ISO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaVerrou {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl MetaVerrou {
    pub fn nouveau(token: impl Into<String>) -> Self {
        MetaVerrou {
            token: token.into(),
            pid: None,
            hostname: None,
            created_at: None,
        }
    }
}

/// Racine des verrous : `path.join(global.state, "locks")`.
pub fn racine_verrous(etat_dir: impl AsRef<Path>) -> PathBuf {
    etat_dir.as_ref().join(DOSSIER_VERROUS)
}

/// Nom du repertoire de verrou pour une empreinte deja calculee :
/// `<hex>.lock`.
///
/// La source calcule `Hash.fast(key) + ".lock"` ; le hachage lui-meme vit
/// dans `util_hash.rs` et n est pas recalcule ici pour garder ce module
/// independant des autres modules du lot.
pub fn nom_verrou_pour_empreinte(empreinte_hex: &str) -> String {
    format!("{}{}", empreinte_hex, EXTENSION_VERROU)
}

/// Chemin complet du verrou : `path.join(dir, Hash.fast(key) + ".lock")`.
pub fn chemin_verrou(dir: impl AsRef<Path>, empreinte_hex: &str) -> PathBuf {
    dir.as_ref().join(nom_verrou_pour_empreinte(empreinte_hex))
}

/// Chemin du fichier de battement dans un repertoire de verrou.
pub fn chemin_battement(repertoire_verrou: impl AsRef<Path>) -> PathBuf {
    repertoire_verrou.as_ref().join(FICHIER_BATTEMENT)
}

/// Chemin de `meta.json` dans un repertoire de verrou.
pub fn chemin_meta(repertoire_verrou: impl AsRef<Path>) -> PathBuf {
    repertoire_verrou.as_ref().join(FICHIER_META)
}

/// Chemin du braquage pendant le nettoyage : `lockDir + ".breaker"`.
pub fn chemin_braquage(repertoire_verrou: impl AsRef<Path>) -> PathBuf {
    let mut texte = repertoire_verrou.as_ref().as_os_str().to_owned();
    texte.push(SUFFIXE_BRAQUAGE);
    PathBuf::from(texte)
}

/// Vrai si l empreinte est consideree comme rassis : `now - mtime > staleMs`.
///
/// La comparaison est stricte : une egalite exacte n est pas rassis.
pub fn est_rassis(maintenant_ms: u64, mtime_ms: u64, stale_ms: u64) -> bool {
    maintenant_ms.saturating_sub(mtime_ms) > stale_ms
}

/// Intervalle du battement : `max(100, floor(staleMs / 3))`.
pub fn intervalle_battement_ms(stale_ms: u64) -> u64 {
    std::cmp::max(100, stale_ms / 3)
}

/// Delai suivant de la boucle : `min(maxDelayMs, floor(delay * 1.7))`.
pub fn delai_suivant_ms(delai_ms: u64, max_delay_ms: u64) -> u64 {
    let grandi = (delai_ms as f64 * FACTEUR_DELAI).floor() as u64;
    std::cmp::min(max_delay_ms, grandi)
}

/// Bornes de la gigue pour un delai donne : `[max(0, ms - j), ms + j]` avec
/// `j = floor(ms * 0.3)`.
///
/// La source tire ensuite `floor(rand * (2j + 1)) - j` ; les bornes sont la
/// partie pure et testable, le tirage ne l est pas.
pub fn bornes_gigue_ms(delai_ms: u64) -> (u64, u64) {
    let j = (delai_ms as f64 * PART_GIGUE).floor() as u64;
    (delai_ms.saturating_sub(j), delai_ms.saturating_add(j))
}

/// Applique un tirage `tirage` dans `[0, 1)` au delai, comme la source :
/// `max(0, ms + (floor(tirage * (2j + 1)) - j))`.
pub fn appliquer_gigue_ms(delai_ms: u64, tirage: f64) -> u64 {
    let j = (delai_ms as f64 * PART_GIGUE).floor() as u64;
    let largeur = 2 * j + 1;
    let ecart = (tirage * largeur as f64).floor() as i64 - j as i64;
    (delai_ms as i64 + ecart).max(0) as u64
}

/// Message d echec d attente pour une cle : `Timed out waiting for lock: <key>`.
pub fn message_delai_depasse(cle: &str) -> String {
    format!("{}{}", PREFIXE_DELAI_DEPASSE, cle)
}

/// Verifie le jeton relu dans `meta.json` avant la levee.
///
/// - `None` (fichier absent) -> metadata missing ;
/// - `Some` different -> token mismatch ;
/// - `Some` egal -> levee autorisee.
///
/// La variante metadata invalid (JSON qui ne parse pas) n a pas de forme
/// `serde_json::Value` a tester ici : elle est documentee par la constante
/// `MESSAGE_LEVEE_META_INVALIDE`.
pub fn verifier_jeton(jeton_attendu: &str, jeton_relu: Option<&str>) -> Result<(), &'static str> {
    match jeton_relu {
        None => Err(MESSAGE_LEVEE_SANS_META),
        Some(lu) if lu == jeton_attendu => Ok(()),
        Some(_) => Err(MESSAGE_LEVEE_JETON_DIFFERENT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_defauts_sont_ceux_de_la_source() {
        let resolues = OptionsResolues::resoudre(&OptionsVerrou::default());
        assert_eq!(resolues.stale_ms, 60_000);
        assert_eq!(resolues.timeout_ms, 300_000);
        assert_eq!(resolues.base_delay_ms, 100);
        assert_eq!(resolues.max_delay_ms, 2_000);
    }

    #[test]
    fn une_option_a_zero_survit_au_defaut_car_c_est_nullite_pas_veracite() {
        let options = OptionsVerrou {
            stale_ms: Some(0),
            timeout_ms: Some(0),
            base_delay_ms: Some(0),
            max_delay_ms: Some(0),
            dir: None,
        };
        let resolues = OptionsResolues::resoudre(&options);
        assert_eq!(resolues.stale_ms, 0);
        assert_eq!(resolues.timeout_ms, 0);
        assert_eq!(resolues.base_delay_ms, 0);
        assert_eq!(resolues.max_delay_ms, 0);
    }

    #[test]
    fn la_racine_des_verrous_joint_locks_a_l_etat_global() {
        let racine = racine_verrous("/tmp/etat");
        assert_eq!(racine.join("x").parent(), Some(racine.as_path()));
        assert!(racine.as_os_str().to_string_lossy().ends_with("locks"));
    }

    #[test]
    fn le_nom_de_verrou_est_l_empreinte_plus_lock() {
        assert_eq!(nom_verrou_pour_empreinte("abc123"), "abc123.lock");
        let chemin = chemin_verrou("/tmp/locks", "abc123");
        assert_eq!(chemin.file_name().unwrap().to_string_lossy(), "abc123.lock");
    }

    #[test]
    fn les_chemins_de_battement_meta_et_braquage_sont_ceux_de_la_source() {
        let dir = PathBuf::from("/tmp/locks/abc.lock");
        assert_eq!(chemin_battement(&dir).file_name().unwrap().to_string_lossy(), "heartbeat");
        assert_eq!(chemin_meta(&dir).file_name().unwrap().to_string_lossy(), "meta.json");
        let braquage = chemin_braquage(&dir);
        assert!(braquage.to_string_lossy().ends_with(".lock.breaker"));
    }

    #[test]
    fn le_rassissement_est_strict_pas_large() {
        assert!(!est_rassis(1_000, 900, 100));
        assert!(est_rassis(1_001, 900, 100));
        assert!(!est_rassis(500, 900, 100));
    }

    #[test]
    fn l_intervalle_de_battement_vaut_le_tiers_avec_un_plancher_de_100() {
        assert_eq!(intervalle_battement_ms(60_000), 20_000);
        assert_eq!(intervalle_battement_ms(200), 100);
        assert_eq!(intervalle_battement_ms(300), 100);
        assert_eq!(intervalle_battement_ms(301), 100);
    }

    #[test]
    fn le_delai_suit_fois_1_virgule_7_avec_plafond() {
        assert_eq!(delai_suivant_ms(100, 2_000), 170);
        assert_eq!(delai_suivant_ms(170, 2_000), 289);
        assert_eq!(delai_suivant_ms(1_500, 2_000), 2_000);
        assert_eq!(delai_suivant_ms(2_000, 2_000), 2_000);
    }

    #[test]
    fn les_bornes_de_gigue_valent_moins_plus_30_pourcents() {
        assert_eq!(bornes_gigue_ms(100), (70, 130));
        assert_eq!(bornes_gigue_ms(0), (0, 0));
        assert_eq!(bornes_gigue_ms(10), (7, 13));
    }

    #[test]
    fn la_gigue_appliquee_reste_dans_ses_bornes() {
        for tirage in [0.0, 0.25, 0.5, 0.75, 0.9999] {
            let valeur = appliquer_gigue_ms(100, tirage);
            assert!((70..=130).contains(&valeur), "tirage {} -> {}", tirage, valeur);
        }
        assert_eq!(appliquer_gigue_ms(100, 0.0), 70);
    }

    #[test]
    fn le_message_de_delai_depasse_contient_la_cle() {
        assert_eq!(message_delai_depasse("ma-cle"), "Timed out waiting for lock: ma-cle");
    }

    #[test]
    fn la_verification_du_jeton_distribue_les_trois_cas() {
        assert_eq!(verifier_jeton("a", None), Err(MESSAGE_LEVEE_SANS_META));
        assert_eq!(verifier_jeton("a", Some("b")), Err(MESSAGE_LEVEE_JETON_DIFFERENT));
        assert_eq!(verifier_jeton("a", Some("a")), Ok(()));
    }

    #[test]
    fn la_meta_se_serialise_avec_le_jeton_en_premier() {
        let meta = MetaVerrou::nouveau("jeton-1");
        let valeur = serde_json::to_value(&meta).unwrap();
        assert_eq!(valeur.get("token").and_then(|v| v.as_str()), Some("jeton-1"));
        let relue: MetaVerrou = serde_json::from_value(valeur).unwrap();
        assert_eq!(relue.token, "jeton-1");
    }

    #[test]
    fn les_messages_litteraux_sont_ceux_de_la_source() {
        assert_eq!(MESSAGE_GLOBAL_ABSENT, "Flock global not set");
        assert_eq!(MESSAGE_BATTEMENT_COMPROMIS.contains("heartbeat"), true);
        assert_eq!(MESSAGE_META_COMPROMIS.contains("meta.json"), true);
        assert_eq!(MESSAGE_ABANDON, "Aborted");
    }
}
