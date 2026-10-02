//! Portage de `packages/core/src/util/effect-flock.ts`.
//!
//! La source fait 284 lignes autour du meme verrou par fichier que
//! `util/flock.ts`, mais en version `Effect` : erreurs typess
//! (`LockTimeoutError`, `LockCompromisedError`), temporisations figees,
//! schema `meta.json`, service `acquire` / `withLock` et couche `Layer`.
//!
//! Seule la partie pure est portee ici, sans `Effect`, sans filesystem :
//! - les constantes figees (stale, timeout, delais, battement) ;
//! - les types d erreur du domaine avec leurs details litteraux ;
//! - le schema des metadonnees et son encodage ;
//! - les chemins et le predicat de rassissement ;
//! - les regles de reessai (tant que `NotAcquired`, jusqu au timeout).
//!
//! Volontairement non porte : `Context.Service`, `Layer.effect`,
//! `FSUtil.Service`, `Global.Service`, `Schedule.exponential`, les fibres de
//! battement, `acquireRelease` et tout ce qui dort ou touche le disque.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Delai au-dela duquel un verrou sans battement est abandonne (60 s).
pub const STALE_MS: u64 = 60_000;
/// Delai d attente avant `LockTimeoutError` (5 minutes).
pub const TIMEOUT_MS: u64 = 5 * 60_000;
/// Delai initial entre deux tentatives (100 ms).
pub const DELAI_BASE_MS: u64 = 100;
/// Plafond du delai entre deux tentatives (2 s).
pub const DELAI_MAX_MS: u64 = 2_000;
/// Battement : `max(100, floor(STALE_MS / 3))`, soit 20 s.
pub const BATTEMENT_MS: u64 = 20_000;
/// Facteur exponentiel du reessai (`Schedule.exponential(100, 1.7)`).
pub const FACTEUR_EXPONENTIEL: f64 = 1.7;

/// Erreur quand le verrou reste pris jusqu au timeout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErreurDelaiVerrou {
    #[serde(rename = "key")]
    pub cle: String,
}

impl ErreurDelaiVerrou {
    pub fn nouvelle(cle: impl Into<String>) -> Self {
        ErreurDelaiVerrou { cle: cle.into() }
    }

    pub fn etiquette() -> &'static str {
        "LockTimeoutError"
    }
}

/// Erreur quand le repertoire a ete compromis entre-temps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErreurVerrouCompromis {
    #[serde(rename = "detail")]
    pub detail: String,
}

impl ErreurVerrouCompromis {
    pub fn nouvelle(detail: impl Into<String>) -> Self {
        ErreurVerrouCompromis {
            detail: detail.into(),
        }
    }

    pub fn etiquette() -> &'static str {
        "LockCompromisedError"
    }
}

/// Signal interne "verrou pris, reessayer" : ne fuit jamais vers l appelant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonAcquis;

impl NonAcquis {
    pub fn etiquette() -> &'static str {
        "NotAcquired"
    }
}

/// Union des erreurs visibles par l appelant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum ErreurVerrou {
    LockTimeoutError(ErreurDelaiVerrou),
    LockCompromisedError(ErreurVerrouCompromis),
}

/// Metadonnees ecrites dans `meta.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaVerrou {
    pub token: String,
    pub pid: u32,
    pub hostname: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

impl MetaVerrou {
    pub fn nouvelle(
        token: impl Into<String>,
        pid: u32,
        hostname: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {
        MetaVerrou {
            token: token.into(),
            pid,
            hostname: hostname.into(),
            created_at: created_at.into(),
        }
    }
}

/// Racine des verrous : `path.join(global.state, "locks")`.
pub fn racine_verrous(etat_dir: impl AsRef<Path>) -> PathBuf {
    etat_dir.as_ref().join("locks")
}

/// Chemin du verrou : `path.join(dir, Hash.fast(key) + ".lock")`.
///
/// Le hachage vit dans `util_hash.rs` ; ici on ne fait que joindre une
/// empreinte deja calculee, pour garder ce module independant.
pub fn chemin_verrou(dir: impl AsRef<Path>, empreinte_hex: &str) -> PathBuf {
    dir.as_ref().join(format!("{}.lock", empreinte_hex))
}

/// Vrai si l empreinte est rassis : `now - mtime > STALE_MS`, stricte.
pub fn est_rassis(maintenant_ms: u64, mtime_ms: u64) -> bool {
    maintenant_ms.saturating_sub(mtime_ms) > STALE_MS
}

/// Delai suivant : `min(MAX, delay * 1.7)`, partie entiere inferieure.
pub fn delai_suivant_ms(delai_ms: u64) -> u64 {
    let grandi = (delai_ms as f64 * FACTEUR_EXPONENTIEL).floor() as u64;
    std::cmp::min(DELAI_MAX_MS, grandi)
}

/// Vrai si l echec merite une nouvelle tentative : seulement `NotAcquired`.
///
/// Toute autre erreur (timeout, compromis) sort de la boucle.
pub fn doit_reessayer(etiquette_erreur: &str) -> bool {
    etiquette_erreur == NonAcquis::etiquette()
}

/// Vrai si le temps ecoule autorise encore une tentative (`elapsed < TIMEOUT_MS`).
pub fn dans_le_delai(ecoule_ms: u64) -> bool {
    ecoule_ms < TIMEOUT_MS
}

/// Construit l erreur de timeout pour une cle.
pub fn erreur_delai(cle: impl Into<String>) -> ErreurVerrou {
    ErreurVerrou::LockTimeoutError(ErreurDelaiVerrou::nouvelle(cle))
}

/// Detail quand le battement existait deja a la creation exclusive.
pub const DETAIL_BATTEMENT_EXISTANT: &str = "heartbeat already existed";
/// Detail quand `meta.json` existait deja a la creation exclusive.
pub const DETAIL_META_EXISTANT: &str = "meta.json already existed";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_temporisations_figees_sont_celles_de_la_source() {
        assert_eq!(STALE_MS, 60_000);
        assert_eq!(TIMEOUT_MS, 300_000);
        assert_eq!(DELAI_BASE_MS, 100);
        assert_eq!(DELAI_MAX_MS, 2_000);
        assert_eq!(BATTEMENT_MS, 20_000);
    }

    #[test]
    fn le_battement_vaut_le_tiers_du_rassissement_avec_plancher() {
        assert_eq!(BATTEMENT_MS, std::cmp::max(100, STALE_MS / 3));
    }

    #[test]
    fn l_erreur_de_delai_porte_la_cle_et_son_etiquette() {
        let erreur = ErreurDelaiVerrou::nouvelle("ma-cle");
        assert_eq!(erreur.cle, "ma-cle");
        assert_eq!(ErreurDelaiVerrou::etiquette(), "LockTimeoutError");
        let valeur = serde_json::to_value(&erreur).unwrap();
        assert_eq!(valeur.get("key").and_then(|v| v.as_str()), Some("ma-cle"));
    }

    #[test]
    fn l_erreur_de_compromis_porte_son_detail_et_son_etiquette() {
        let erreur = ErreurVerrouCompromis::nouvelle("heartbeat already existed");
        assert_eq!(ErreurVerrouCompromis::etiquette(), "LockCompromisedError");
        let valeur = serde_json::to_value(&erreur).unwrap();
        assert_eq!(
            valeur.get("detail").and_then(|v| v.as_str()),
            Some("heartbeat already existed")
        );
    }

    #[test]
    fn seul_not_acquired_merite_une_nouvelle_tentative() {
        assert!(doit_reessayer("NotAcquired"));
        assert!(!doit_reessayer("LockTimeoutError"));
        assert!(!doit_reessayer("LockCompromisedError"));
        assert!(!doit_reessayer(""));
    }

    #[test]
    fn le_delai_autorise_les_tentatives_strictement_avant_timeout() {
        assert!(dans_le_delai(0));
        assert!(dans_le_delai(TIMEOUT_MS - 1));
        assert!(!dans_le_delai(TIMEOUT_MS));
        assert!(!dans_le_delai(TIMEOUT_MS + 1));
    }

    #[test]
    fn le_delai_suivant_multiplie_par_1_virgule_7_avec_plafond() {
        assert_eq!(delai_suivant_ms(100), 170);
        assert_eq!(delai_suivant_ms(1_500), 2_000);
        assert_eq!(delai_suivant_ms(2_000), 2_000);
    }

    #[test]
    fn le_rassissement_est_strict() {
        assert!(!est_rassis(1_000, 940));
        assert!(est_rassis(61_001, 1_000));
    }

    #[test]
    fn la_meta_garde_la_casse_created_at() {
        let meta = MetaVerrou::nouvelle("jeton", 123, "hote", "2026-01-01T00:00:00Z");
        let valeur = serde_json::to_value(&meta).unwrap();
        assert_eq!(valeur.get("createdAt").and_then(|v| v.as_str()), Some("2026-01-01T00:00:00Z"));
        assert!(valeur.get("created_at").is_none());
        let relue: MetaVerrou = serde_json::from_value(valeur).unwrap();
        assert_eq!(relue, meta);
    }

    #[test]
    fn le_chemin_du_verrou_joint_le_lock_a_la_racine() {
        let chemin = chemin_verrou("/tmp/locks", "abc123");
        assert_eq!(chemin.file_name().unwrap().to_string_lossy(), "abc123.lock");
        let racine = racine_verrous("/tmp/etat");
        assert!(racine.to_string_lossy().ends_with("locks"));
    }

    #[test]
    fn l_erreur_de_delai_fabriquee_porte_la_bonne_variante() {
        match erreur_delai("cle-1") {
            ErreurVerrou::LockTimeoutError(interne) => assert_eq!(interne.cle, "cle-1"),
            autre => panic!("variante inattendue : {:?}", autre),
        }
    }
}
