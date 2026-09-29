//! Portage Rust de `opencode/packages/core/src/session/context-epoch.ts`.
//!
//! L'epoch est le curseur qui dit « a partir d'ici, le contexte est de novo ».
//! C'est la piece qui decide si le systeme doit etre reconstruit ou seulement
//! reconcilie, et c'est ce qui rend le compactage vraiment economique : dans le
//! cas le plus courant, rien n'est reecrit.
//!
//! Deux chemin de reconciliation, exclusifs :
//!
//! - **remplacement** : une compaction est survenue apres le baseline courant.
//!   L'historique a ete condense, donc l'ancien baseline ne decrit plus rien
//!   d'utile : on repart de la compaction et on reconstruit entierement.
//! - **reconciliation** : aucune compaction plus recente. Le baseline tient
//!   toujours, seul le snapshot a change : on met a jour le snapshot sans
//!   toucher au baseline, ce qui evite de perdre la fenetre de contexte.

use crate::core::session::history::{latest_compaction, BaselineSeq, Entry};
use crate::schema::session_message::Message;

/// Resultat de la reconciliation entre le snapshot stocke et l'etat actuel du
/// systeme.
///
/// Equivalent des `_tag` du TS (`Unchanged`, `ReplacementReady`,
/// `ReplacementBlocked`). En TS c'est un ADT avec tag ; ici un enum, ce qui
/// oblige le compilateur a couvrir tous les cas — le TS pouvait en oublier un
/// sans erreur.
#[derive(Debug, Clone, PartialEq)]
pub enum Reconciliation {
    /// Le snapshot stocke decrit deja l'etat actuel. Ne rien faire.
    Unchanged,
    /// Le systeme a change et une compaction recente existe : reconstruire.
    ReplacementReady {
        baseline: String,
        /// Rang du message de compaction qui remplace l'ancien baseline.
        compaction_seq: i64,
    },
    /// Le systeme a change mais aucune compaction recente : mettre a jour le
    /// snapshot en conservant le baseline.
    SnapshotAdvanced { snapshot: String },
}

/// Etat de l'epoch d'une session, tel qu'il est stocke.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredEpoch {
    pub baseline: String,
    pub snapshot: String,
    pub baseline_seq: BaselineSeq,
}

/// Ce qu'il faut savoir du systeme pour decider, sans l'lier a la base.
///
/// Cette separation permet de tester la decision de reconciliation sans base de
/// donnees, ce que le TS ne permettait pas facilement a cause des effets.
#[derive(Debug, Clone)]
pub struct SystemState {
    /// Le systeme a-t-il change depuis le snapshot stocke ?
    pub changed: bool,
    /// Nouveau baseline propose, si le systeme a change.
    pub new_baseline: String,
    /// Nouveau snapshot propose, si le systeme a change.
    pub new_snapshot: String,
    /// Le changement est-il suffisamment grave pour justifier un remplacement ?
    /// `false` equivalent a `ReplacementBlocked` du TS.
    pub replacement_allowed: bool,
}

/// Prepare l'epoch pour un tour.
///
/// `entries` sert uniquement a localiser la derniere compaction. `stored` est
/// l'epoch deja enregistree, ou `None` pour une session neuve.
///
/// Le TS fait deux requetes en parallele (`Effect.all(..., concurrency:
/// "unbounded")`) puis decide. Meme chose ici, sans la concurrence : une seule
/// lecture en memoire suffit et evite deux allers-retours base.
pub fn prepare(
    stored: Option<&StoredEpoch>,
    entries: &[Entry],
    system: &SystemState,
) -> Result<Prepared, ReconciliationError> {
    let compaction_seq = latest_compaction(entries);

    let Some(stored) = stored else {
        // Session neuve : on part du dernier rang connu comme baseline. En
        // pratique 0, tout l'historique est alors visible.
        let baseline_seq = entries.last().map(|e| e.seq).unwrap_or(0);
        return Ok(Prepared {
            baseline: system.new_baseline.clone(),
            baseline_seq: BaselineSeq(baseline_seq),
            next: Some(StoredEpoch {
                baseline: system.new_baseline.clone(),
                snapshot: system.new_snapshot.clone(),
                baseline_seq: BaselineSeq(baseline_seq),
            }),
        });
    };

    if !system.changed {
        return Ok(Prepared {
            baseline: stored.baseline.clone(),
            baseline_seq: stored.baseline_seq,
            next: None,
        });
    }

    // Une compaction plus recente que le baseline invalide le baseline : ce qui
    // suivait a ete condense, le curseur ne designe plus rien de pertinent.
    let replacement_seq = compaction_seq.filter(|c| *c > stored.baseline_seq.0);

    if system.replacement_allowed {
        if let Some(c) = replacement_seq {
            return Ok(Prepared {
                baseline: system.new_baseline.clone(),
                baseline_seq: BaselineSeq(c),
                next: Some(StoredEpoch {
                    baseline: system.new_baseline.clone(),
                    snapshot: system.new_snapshot.clone(),
                    baseline_seq: BaselineSeq(c),
                }),
            });
        }
    }

    // Pas de remplacement possible : on avance seulement le snapshot. Le
    // baseline est conserve, donc la fenetre de contexte visible ne bouge pas.
    Ok(Prepared {
        baseline: stored.baseline.clone(),
        baseline_seq: stored.baseline_seq,
        next: Some(StoredEpoch {
            baseline: stored.baseline.clone(),
            snapshot: system.new_snapshot.clone(),
            baseline_seq: stored.baseline_seq,
        }),
    })
}

/// Apres application de `prepare` : le baseline effectif, et l'epoch a
/// persister s'il y en a une nouvelle.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub baseline: String,
    pub baseline_seq: BaselineSeq,
    /// `None` quand rien n'a change : ne rien ecrire en base.
    pub next: Option<StoredEpoch>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReconciliationError {
    #[error("le snapshot stocke est illisible : {0}")]
    InvalidSnapshot(String),
}

/// Reinitialise l'epoch : la prochaine lecture repartira de zero.
///
/// Equivalent de `reset` dans le TS, qui supprime la ligne.
pub fn reset() -> Option<StoredEpoch> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::session::history::Window;
    use crate::schema::session_message::{
        Compaction, CompactionReason, MessageBase, User, Prompt,
    };

    fn user(seq: i64, id: &str) -> Entry {
        Entry {
            seq,
            message: Message::User(User {
                base: MessageBase::new(id, seq),
                prompt: Prompt::default(),
            }),
        }
    }

    fn compaction(seq: i64, id: &str) -> Entry {
        Entry {
            seq,
            message: Message::Compaction(Compaction {
                base: MessageBase::new(id, seq),
                reason: CompactionReason::Auto,
                summary: "s".into(),
                recent: "r".into(),
            }),
        }
    }

    fn system_state(changed: bool, allowed: bool) -> SystemState {
        SystemState {
            changed,
            new_baseline: "BASE2".to_string(),
            new_snapshot: "SNAP2".to_string(),
            replacement_allowed: allowed,
        }
    }

    fn stored(seq: i64) -> StoredEpoch {
        StoredEpoch {
            baseline: "BASE1".to_string(),
            snapshot: "SNAP1".to_string(),
            baseline_seq: BaselineSeq(seq),
        }
    }

    #[test]
    fn une_session_neuve_part_du_dernier_rang() {
        let entries = vec![user(1, "msg_1"), user(2, "msg_2")];
        let p = prepare(None, &entries, &system_state(true, true)).unwrap();
        assert_eq!(p.baseline_seq, BaselineSeq(2));
        assert_eq!(p.baseline, "BASE2");
        assert!(p.next.is_some());
    }

    #[test]
    fn rien_ne_change_ne_ecrit_rien() {
        let entries = vec![user(1, "msg_1")];
        let p = prepare(Some(&stored(0)), &entries, &system_state(false, true)).unwrap();
        // Rien a persister : c'est le cas le plus frequent, il ne doit pas
        // declencher d'ecriture base.
        assert!(p.next.is_none());
        assert_eq!(p.baseline, "BASE1");
    }

    #[test]
    fn une_compaction_plus_recente_declenche_le_remplacement() {
        // baseline a 5, compaction a 9 : la compaction est plus recente.
        let entries = vec![user(1, "msg_1"), compaction(9, "msg_c")];
        let p = prepare(Some(&stored(5)), &entries, &system_state(true, true)).unwrap();
        // Le baseline saute sur la compaction.
        assert_eq!(p.baseline_seq, BaselineSeq(9));
        assert_eq!(p.baseline, "BASE2");
    }

    #[test]
    fn une_compaction_anterieure_ne_declenche_pas_le_remplacement() {
        // baseline a 10, compaction a 3 : elle est plus ancienne que le
        // baseline, elle ne peut pas le remplacer.
        let entries = vec![compaction(3, "msg_c"), user(11, "msg_u")];
        let p = prepare(Some(&stored(10)), &entries, &system_state(true, true)).unwrap();
        // Le baseline bouge pas, seul le snapshot avance.
        assert_eq!(p.baseline_seq, BaselineSeq(10));
        assert_eq!(p.baseline, "BASE1");
        let next = p.next.unwrap();
        assert_eq!(next.snapshot, "SNAP2");
    }

    #[test]
    fn remplacement_interdit_reagit_comme_une_simple_avance() {
        let entries = vec![compaction(9, "msg_c")];
        let p = prepare(Some(&stored(5)), &entries, &system_state(true, false)).unwrap();
        // Meme avec une compaction recente, l'interdiction impose l'avance seule.
        assert_eq!(p.baseline_seq, BaselineSeq(5));
    }

    #[test]
    fn le_remplacement_resserre_la_fenetre_de_contexte() {
        // Verification de bout en bout : apres remplacement, la fenetre visible
        // ne contient plus les anciens messages, ce qui est l'effet recherche.
        let entries = vec![
            user(1, "msg_1"),
            user(2, "msg_2"),
            compaction(9, "msg_c"),
            user(10, "msg_10"),
        ];
        let stored_epoch = stored(1);
        let p = prepare(Some(&stored_epoch), &entries, &system_state(true, true)).unwrap();

        let visible = crate::core::session::history::load(
            &entries,
            Window { compaction_seq: latest_compaction(&entries), baseline: Some(p.baseline_seq) },
        );
        // msg_1 et msg_2 sont hors fenetre.
        let ids: Vec<&str> = visible.iter().map(|m| m.id()).collect();
        assert!(!ids.contains(&"msg_1"));
        assert!(ids.contains(&"msg_10"));
    }
}
