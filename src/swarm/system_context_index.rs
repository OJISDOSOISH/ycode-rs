//! Portage Rust de `opencode/packages/core/src/system-context/index.ts`.
//!
//! La source modelise le contexte systeme privilegie comme un ensemble de
//! sources typees rafraichissables independamment. Chaque source possede une
//! cle stable namescapee, un chargement, un rendu baseline, un rendu update et
//! un rendu removed optionnel. Les contextes se combinent, s observent une
//! fois, puis produisent un texte visible par le modele et un snapshot durable.
//!
//! Non porte : `Effect`, `Schema`, les codecs JSON, l observation concurrente.
//! Le portage retient le contrat de donnees : cles, snapshots, generation,
//! reconciliation, remplacement, et les regles de rendu.

use std::collections::{BTreeMap, BTreeSet};

/// Prefixe de marque utilise par la source pour le symbole d indisponibilite.
pub const UNAVAILABLE_SYMBOL: &str = "@opencode/SystemContext.Unavailable";

/// Erreur de cle dupliquee, miroir de `DuplicateKeyError`.
pub const DUPLICATE_KEY_MESSAGE_PREFIX: &str = "Duplicate system context key: ";

/// Erreur d initialisation bloquee, miroir de `InitializationBlocked`.
pub const INITIALIZATION_BLOCKED_PREFIX: &str =
    "System context initialization blocked by unavailable sources: ";

/// Verifie la forme d une cle `namespace/nom`.
/// La source exige `^[a-z0-9][a-z0-9._-]*\/[a-z0-9][a-z0-9._/-]*$`.
pub fn is_valid_key(key: &str) -> bool {
    let Some((ns, name)) = key.split_once('/') else {
        return false;
    };
    if ns.is_empty() || name.is_empty() {
        return false;
    }
    if !is_key_head(ns.as_bytes()[0]) || !is_key_head(name.as_bytes()[0]) {
        return false;
    }
    if !ns.bytes().all(is_ns_char) {
        return false;
    }
    if !name.bytes().all(is_name_char) {
        return false;
    }
    if name.contains("//") {
        return false;
    }
    true
}

fn is_key_head(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit()
}

fn is_ns_char(b: u8) -> bool {
    is_key_head(b) || b == b'.' || b == b'_' || b == b'-'
}

fn is_name_char(b: u8) -> bool {
    is_ns_char(b) || b == b'/'
}

/// Snapshot durable d une source admise : valeur JSON + texte de retrait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub value: String,
    pub removed: Option<String>,
}

impl SourceSnapshot {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            removed: None,
        }
    }

    pub fn with_removed(mut self, text: impl Into<String>) -> Self {
        self.removed = Some(text.into());
        self
    }
}

/// Snapshot durable d une generation : une entree par cle.
pub type Snapshot = BTreeMap<String, SourceSnapshot>;

/// Generation initiale : texte baseline + snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub baseline: String,
    pub snapshot: Snapshot,
}

/// Assemble les parties de rendu comme la source : jointure par `\n\n`.
pub fn render(parts: &[String]) -> String {
    parts.join("\n\n")
}

/// Exige un texte de rendu non vide, comme `requireText` de la source.
pub fn require_text(key: &str, kind: &str, text: String) -> Result<String, String> {
    if text.is_empty() {
        return Err(format!("System context source {key} rendered an empty {kind}"));
    }
    Ok(text)
}

/// Verifie l unicite des cles, comme `assertUniqueKeys` + `combine`.
pub fn assert_unique_keys(keys: &[&str]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(*key) {
            return Err(format!("{DUPLICATE_KEY_MESSAGE_PREFIX}{key}"));
        }
    }
    Ok(())
}

/// Message d erreur de `InitializationBlocked` pour les cles donnees.
pub fn initialization_blocked_message(keys: &[String]) -> String {
    format!("{INITIALIZATION_BLOCKED_PREFIX}{}", keys.join(", "))
}

/// Resultat de reconciliation simplifie : miroir de `ReconcileResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileOutcome {
    Unchanged,
    Updated { text: String, snapshot: Snapshot },
    Replace,
    ReplacementBlocked,
}

/// Reconciliation pure : compare les valeurs courantes aux valeurs stockees.
///
/// `current` associe chaque cle a `None` (indisponible) ou `Some(valeur)`.
/// `removed_text` fournit le texte de retrait des cles disparues.
pub fn reconcile_observation(
    current: &BTreeMap<String, Option<String>>,
    previous: &Snapshot,
    removed_text: &BTreeMap<String, String>,
    baseline_of: &dyn Fn(&str, &str) -> String,
    update_of: &dyn Fn(&str, &str, &str) -> String,
) -> ReconcileOutcome {
    let keys: BTreeSet<&str> = current.keys().map(String::as_str).collect();
    // Une source stockee qui disparait sans texte de retrait force un remplacement.
    for key in previous.keys() {
        if !keys.contains(key.as_str()) && removed_text.get(key).is_none() {
            // Les snapshots sans `removed` ne savent pas se retirer.
            if previous[key].removed.is_none() {
                return ReconcileOutcome::Replace;
            }
        }
    }
    let mut snapshot: Snapshot = BTreeMap::new();
    let mut updates: Vec<String> = Vec::new();
    for (key, value) in current {
        match (value, previous.get(key)) {
            (None, Some(stored)) => {
                snapshot.insert(key.clone(), stored.clone());
            }
            (None, None) => {}
            (Some(v), Some(stored)) if stored.value == *v => {
                snapshot.insert(key.clone(), stored.clone());
            }
            (Some(v), Some(stored)) => {
                updates.push(update_of(key, &stored.value, v));
                snapshot.insert(
                    key.clone(),
                    SourceSnapshot {
                        value: v.clone(),
                        removed: stored.removed.clone(),
                    },
                );
            }
            (Some(v), None) => {
                updates.push(baseline_of(key, v));
                snapshot.insert(key.clone(), SourceSnapshot::new(v.clone()));
            }
        }
    }
    for key in previous.keys() {
        if keys.contains(key.as_str()) {
            continue;
        }
        if let Some(text) = removed_text.get(key) {
            updates.push(text.clone());
        } else if let Some(stored) = previous.get(key) {
            if let Some(text) = &stored.removed {
                updates.push(text.clone());
            }
        }
    }
    if updates.is_empty() {
        // Aucune mise a jour mais une incompatibilite de cles reste un remplacement.
        // Ici les valeurs sont des chaines opaques donc pas d incompatibilite.
        return ReconcileOutcome::Unchanged;
    }
    ReconcileOutcome::Updated {
        text: render(&updates),
        snapshot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_cle_namescapee_simple_est_valide() {
        assert!(is_valid_key("core/environment"));
        assert!(is_valid_key("core/date"));
        assert!(is_valid_key("core/reference-guidance"));
    }

    #[test]
    fn une_cle_sans_slash_est_rejetee() {
        assert!(!is_valid_key("core"));
        assert!(!is_valid_key(""));
    }

    #[test]
    fn une_cle_majuscule_est_rejetee() {
        assert!(!is_valid_key("Core/environment"));
        assert!(!is_valid_key("core/Environment"));
    }

    #[test]
    fn une_cle_qui_commence_par_un_tiret_est_rejetee() {
        assert!(!is_valid_key("-core/env"));
        assert!(!is_valid_key("core/-env"));
    }

    #[test]
    fn un_double_slash_dans_le_nom_est_rejetee() {
        assert!(!is_valid_key("core/a//b"));
    }

    #[test]
    fn le_rendu_joint_par_double_saut_de_ligne() {
        assert_eq!(
            render(&["a".to_string(), "b".to_string()]),
            "a\n\nb".to_string()
        );
        assert_eq!(render(&[]), "".to_string());
    }

    #[test]
    fn un_texte_vide_est_refuse() {
        assert!(require_text("core/date", "baseline", String::new()).is_err());
        assert!(require_text("core/date", "baseline", "x".to_string()).is_ok());
    }

    #[test]
    fn une_cle_dupliquee_est_signalee() {
        assert!(assert_unique_keys(&["a/b", "c/d"]).is_ok());
        let err = assert_unique_keys(&["a/b", "a/b"]).unwrap_err();
        assert!(err.contains("Duplicate system context key: a/b"));
    }

    #[test]
    fn le_message_d_initialisation_bloquee_liste_les_cles() {
        let msg = initialization_blocked_message(&["a/b".to_string(), "c/d".to_string()]);
        assert!(msg.contains("a/b"));
        assert!(msg.contains("c/d"));
    }

    #[test]
    fn sans_changement_la_reconciliation_est_inchangee() {
        let mut prev = Snapshot::new();
        prev.insert("core/date".to_string(), SourceSnapshot::new("v1"));
        let mut cur = BTreeMap::new();
        cur.insert("core/date".to_string(), Some("v1".to_string()));
        let out = reconcile_observation(
            &cur,
            &prev,
            &BTreeMap::new(),
            &|_, v| v.to_string(),
            &|_, _, v| v.to_string(),
        );
        assert_eq!(out, ReconcileOutcome::Unchanged);
    }

    #[test]
    fn une_valeur_modifiee_produit_un_update() {
        let mut prev = Snapshot::new();
        prev.insert("core/date".to_string(), SourceSnapshot::new("v1"));
        let mut cur = BTreeMap::new();
        cur.insert("core/date".to_string(), Some("v2".to_string()));
        let out = reconcile_observation(
            &cur,
            &prev,
            &BTreeMap::new(),
            &|_, v| format!("base:{v}"),
            &|_, p, c| format!("{p}->{c}"),
        );
        match out {
            ReconcileOutcome::Updated { text, snapshot } => {
                assert_eq!(text, "v1->v2");
                assert_eq!(snapshot["core/date"].value, "v2");
            }
            other => panic!("attendu Updated, obtenu {other:?}"),
        }
    }

    #[test]
    fn une_source_retiree_sans_texte_force_un_remplacement() {
        let mut prev = Snapshot::new();
        prev.insert("core/date".to_string(), SourceSnapshot::new("v1"));
        let cur = BTreeMap::new();
        let out = reconcile_observation(
            &cur,
            &prev,
            &BTreeMap::new(),
            &|_, v| v.to_string(),
            &|_, _, v| v.to_string(),
        );
        assert_eq!(out, ReconcileOutcome::Replace);
    }

    #[test]
    fn une_source_retiree_avec_texte_produit_un_update() {
        let mut prev = Snapshot::new();
        prev.insert(
            "core/date".to_string(),
            SourceSnapshot::new("v1").with_removed("gone"),
        );
        let cur = BTreeMap::new();
        let mut removed = BTreeMap::new();
        removed.insert("core/date".to_string(), "gone".to_string());
        let out = reconcile_observation(
            &cur,
            &prev,
            &removed,
            &|_, v| v.to_string(),
            &|_, _, v| v.to_string(),
        );
        match out {
            ReconcileOutcome::Updated { text, .. } => assert_eq!(text, "gone"),
            other => panic!("attendu Updated, obtenu {other:?}"),
        }
    }
}
