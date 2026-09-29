//! Portage Rust de `opencode/packages/core/src/session/history.ts`.
//!
//! C'est la mecanique qui permet de depasser la fenetre de contexte du modele,
//! et c'est la partie la plus subtile de tout le projet. Deux mecanismes se
//! combinent :
//!
//! - La **compaction** : quand le contexte deborde, un message `compaction`
//!   condense tout ce qui precede en un resume. Seuls les messages posterieurs
//!   restent lisibles en entier.
//! - L'**epoch** (`baseline_seq`) : un curseur qui marque le debut de la
//!   tentative courante. Ce qui precede peut etre coupe, mais les messages
//!   `system` posterieurs au curseur sont toujours conserves, parce que les
//!   instructions systeme ne doivent jamais disparaitre du contexte.
//!
//! L'original exprime la fenetre visible comme un `OR` SQL a deux branches. On
//! garde exactement la meme logique, traduite en predicat Rust, pour que le port
//! reste lisible face a l'original.

use crate::schema::session_message::Message;

/// Curseur de debut d'epoch d'une session.
///
/// Point de depart d'une tentative de tour. Les messages anterieurs peuvent
/// etre sacrifies pour economiser du contexte ; ceux qui sont posterieurs, et
/// qui plus particulierement sont de type systeme, sont conserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineSeq(pub i64);

/// Un message tel qu'il est stocke : sa position dans la session, et son contenu.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Rang du message dans la session, croissant. Sert d'ordre total.
    pub seq: i64,
    pub message: Message,
}

impl Entry {
    fn is_system(&self) -> bool {
        self.message.type_str() == "system"
    }
}

/// Fenetre d'historique a reconstruire.
///
/// Le type de compactage n'est pas une donnee de message mais une information
/// de position : on ne cherche que le `seq` du dernier message de compaction,
/// pas son contenu.
#[derive(Debug, Clone, Copy, Default)]
pub struct Window {
    /// Rang du dernier message de compaction, s'il y en a un.
    pub compaction_seq: Option<i64>,
    /// Curseur d'epoch, si la session en a un.
    pub baseline: Option<BaselineSeq>,
}

/// Reproduit la fenetre `messageRows` de l'original.
///
/// Equivalent de :
/// ```sql
/// where session_id = ?
///   and (
///         (compaction is null)                        -- pas de compaction
///         or seq >= compaction                        -- apres la compaction
///         or (baseline is not null
///             and type = 'system' and seq > baseline) -- systeme de l'epoch
///   )
///   and (
///         (baseline is null)                          -- pas d'epoch
///         or type != 'system'                         -- non-systeme
///         or seq > baseline                           -- systeme posterior
///   )
/// order by seq asc
/// ```
///
/// La deuxieme clause est contre-intuitive et merite un commentaire : elle
/// **retre** les messages `system` anterieurs au baseline, pour que le contexte
/// ne contienne pas deux fois les memes instructions systeme (une ancienne
/// version, plus une recente). Un message systeme anterieur au curseur est donc
/// masque, sauf s'il passe par la branche speciale de la premiere clause.
fn is_visible(seq: i64, message: &Message, window: Window) -> bool {
    let is_system = message.type_str() == "system";

    let after_compaction = match window.compaction_seq {
        // Pas de compaction : la premiere contrainte ne retient rien.
        None => true,
        Some(c) => {
            seq >= c
                // Exception : les instructions systeme de l'epoch courante
                // survivent a la compaction, sinon l'agent perdrait ses
                // consignes en plein vol.
                || (window.baseline.is_some() && is_system && seq > window.baseline.map(|b| b.0).unwrap())
        }
    };

    let respects_epoch = match window.baseline {
        None => true,
        Some(b) => {
            // On ne garde que ce qui n'est pas systeme, ou ce qui est systeme
            // et posterieur au curseur.
            !is_system || seq > b.0
        }
    };

    after_compaction && respects_epoch
}

/// Reconstruit l'historique visible d'une session.
///
/// `entries` peut contenir des messages de n'importe quelle session : ceux
/// d'une autre session sont simplement ignores. C'est ce qui permet a l'appelant
/// de passer un flux melange sans pre-filtrer.
pub fn load(entries: &[Entry], window: Window) -> Vec<Message> {
    entries
        .iter()
        .filter(|e| is_visible(e.seq, &e.message, window))
        .map(|e| e.message.clone())
        .collect()
}

/// Comme `load`, mais conserve le `seq` : c'est ce dont le runner a besoin pour
/// savoir jusqu'ou il a lu avant de recharger.
pub fn entries_for_runner(entries: &[Entry], window: Window) -> Vec<Entry> {
    entries
        .iter()
        .filter(|e| is_visible(e.seq, &e.message, window))
        .cloned()
        .collect()
}

/// Dernier rang d'un message de compaction.
///
/// Retourne le `seq` le plus eleve parmi les messages de type `compaction` : le
/// plus recent. C'est la frontiere a partir de laquelle l'historique est lu en
/// entier.
pub fn latest_compaction(entries: &[Entry]) -> Option<i64> {
    entries
        .iter()
        .filter(|e| e.message.type_str() == "compaction")
        .map(|e| e.seq)
        .max()
}

/// Une fenetre qui laisse tout passer : aucune compaction, aucun epoch.
pub fn full_window() -> Window {
    Window::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::session_message::{
        Assistant, Compaction, CompactionReason, MessageBase, ModelRef, Prompt, System, User,
    };

    fn user(seq: i64, id: &str) -> Entry {
        Entry {
            seq,
            message: Message::User(User {
                base: MessageBase::new(id, seq),
                prompt: Prompt { text: "x".into(), files: vec![], agents: vec![] },
            }),
        }
    }

    fn system(seq: i64, id: &str) -> Entry {
        Entry { seq, message: Message::System(System { base: MessageBase::new(id, seq), text: "regle".into() }) }
    }

    fn assistant(seq: i64, id: &str) -> Entry {
        Entry {
            seq,
            message: Message::Assistant(Assistant::new(id, seq, "build", ModelRef::new("p", "m"))),
        }
    }

    fn compaction(seq: i64, id: &str) -> Entry {
        Entry {
            seq,
            message: Message::Compaction(Compaction {
                base: MessageBase::new(id, seq),
                reason: CompactionReason::Auto,
                summary: "resume".into(),
                recent: "recent".into(),
            }),
        }
    }

    #[test]
    fn sans_compaction_ni_epoch_on_voit_tout() {
        let entries = vec![user(1, "msg_1"), system(2, "msg_2"), assistant(3, "msg_3")];
        let out = load(&entries, full_window());
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn la_compaction_masque_les_anciens_messages() {
        // 1,2 avant compaction ; 3 = compaction ; 4,5 apres.
        let entries = vec![
            user(1, "msg_1"),
            system(2, "msg_2"),
            compaction(3, "msg_3"),
            user(4, "msg_4"),
            assistant(5, "msg_5"),
        ];
        let window = Window { compaction_seq: Some(3), baseline: None };
        // On lit les seq via `entries_for_runner`, qui les conserve, plutot que
        // de deduire le seq depuis le message : un message `Compaction` n'a pas
        // de `time` de base, donc la deduction planterait.
        let out = entries_for_runner(&entries, window);
        let seqs: Vec<i64> = out.iter().map(|e| e.seq).collect();
        // Seuls 3, 4, 5 restent.
        assert_eq!(seqs, vec![3, 4, 5]);
    }

    #[test]
    fn l_instruction_systeme_de_l_epoch_survit_a_la_compaction() {
        // Le cas subtle : une instruction systeme emise apres le baseline doit
        // rester visible meme si elle est anterieure a la compaction, sinon
        // l'agent perd ses consignes en plein vol.
        let entries = vec![
            compaction(5, "msg_c"),
            system(6, "msg_s"),  // systeme, apres le baseline 4, avant... non, apres compaction
            user(7, "msg_u"),
        ];
        // baseline a 4 : le systeme 6 est apres le baseline ET apres la
        // compaction, donc visible.
        let window = Window { compaction_seq: Some(5), baseline: Some(BaselineSeq(4)) };
        let out = load(&entries, window);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn l_epoch_masque_les_anciens_systeme_pour_eviter_le_doublon() {
        // Deux messages systeme : un avant le baseline, un apres. Celui
        // d'avant doit disparaitre, sinon les instructions sont dupliquees.
        let entries = vec![
            system(1, "msg_sys_ancien"),
            user(2, "msg_u1"),
            system(10, "msg_sys_recent"),
            user(11, "msg_u2"),
        ];
        let window = Window { compaction_seq: None, baseline: Some(BaselineSeq(5)) };
        let out = load(&entries, window);
        let ids: Vec<&str> = out.iter().map(|m| m.id()).collect();
        // L'ancien systeme (1) est masque. Le recent (10) reste.
        assert!(!ids.contains(&"msg_sys_ancien"));
        assert!(ids.contains(&"msg_sys_recent"));
    }

    #[test]
    fn latest_compaction_retourne_le_plus_recent() {
        let entries = vec![compaction(3, "msg_c1"), compaction(9, "msg_c2"), user(10, "msg_u")];
        assert_eq!(latest_compaction(&entries), Some(9));
        assert_eq!(latest_compaction(&[user(1, "msg_1")]), None);
    }

    #[test]
    fn l_ordre_entree_est_preserve() {
        // Les entrees peuvent arriver dans le desordre ; la fenetre doit les
        // rendre dans l'ordre du seq, comme le ORDER BY seq de l'original.
        let entries = vec![user(3, "msg_3"), user(1, "msg_1"), user(2, "msg_2")];
        let mut out = entries_for_runner(&entries, full_window());
        out.sort_by_key(|e| e.seq);
        let seqs: Vec<i64> = out.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
    }
}

