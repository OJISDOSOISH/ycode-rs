//! Machine a etats de la boucle de tour, d'apres
//! `opencode/packages/core/src/session/runner/llm.ts`.
//!
//! L'original exprime les transitions par des exceptions (`TurnTransitionError`
//! avec `_tag: "ContinueAfterCompaction"`), levees au milieu d'effets en cours
//! puis rattrapees plus haut. C'est illisible en Rust : une exception traverse
//! les frontiers de fibres sans que le type ne l'autorise.
//!
//! Ici, les transitions sont des valeurs de retour explicites. Le compilateur
//! oblige a traiter chaque cas, ce qui est exactement le genre de garantie que
//! `Effect` apporte mais que la verite des exceptions ne donne pas.

use crate::core::session::compaction::should_compact;

/// Etat d'une tentative de tour.
#[derive(Debug, Clone, PartialEq)]
pub enum Transition {
    /// Le modele a produit sa reponse finale. Le tour est termine.
    Done(String),
    /// Le modele a demande des outils : on les execute et on rappelle le modele.
    NeedsTools(Vec<String>),
    /// Le contexte a deborde avant l'appel : il faut compacter et reessayer.
    Compacted { step: u32 },
    /// Le contexte a deborde *pendant* la generation, une fois le modele
    /// epuise. On compacte le contexte partiel et on le rappelle.
    CompactedAfterOverflow { step: u32 },
    /// Interruption : la session a change de repertoire ou d'agent, le tour en
    /// cours ne s'applique plus.
    Interrupted,
}

/// Configuration de l'agent pour un tour.
///
/// Le TS lit `agent.info?.steps` et `agent.info?.permissions`. Seuls les `steps`
/// sont traduits ici, parce que les permissions conditionnent l'existence des
/// outils, ce qui releve de la boucle d'appels et non de la machine d'etats.
#[derive(Debug, Clone, Copy)]
pub struct AgentConfig {
    /// Nombre de tours maximum pour cet agent. `None` signifie illimite, ce que
    /// l'original exprime par `steps === undefined`.
    pub steps: Option<u32>,
}

impl AgentConfig {
    /// Le tour courant est-il le dernier ?
    ///
    /// Au dernier tour, les outils sont retires : le modele est oblige de
    /// produire une reponse textuelle. Sans ca, un agent peut boucler en
    /// appelant des outils jusqu'a epuiser le budget, et la session se termine
    /// sans que l'utilisateur ait rien lu.
    pub fn is_last_step(&self, step: u32) -> bool {
        self.steps.is_some_and(|max| step >= max)
    }
}

/// Etat necessaire pour decider la transition.
#[derive(Debug, Clone, Copy)]
pub struct TurnState {
    pub step: u32,
    pub context_tokens: u64,
    pub context_limit: u64,
    pub output_reserve: u64,
    pub keep_tokens: u64,
}

impl TurnState {
    /// Faut-il compacter avant d'appeler le modele ?
    ///
    /// Distinct de `should_compact` : ici on se trouve *avant* l'appel, donc une
    /// compaction n'a pas encore eu lieu dans ce tour.
    pub fn needs_pre_compaction(&self) -> bool {
        should_compact(self.context_tokens, self.context_limit, self.output_reserve, self.keep_tokens)
    }
}

/// Ce qu'a produit l'appel au modele.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelOutput {
    /// Reponse finale, aucun outil demande.
    Final(String),
    /// Le modele a demande des appels d'outils.
    ToolCalls(Vec<String>),
    /// Le modele a epuise ses tokens avant de finir. C'est le cas le plus
    ///Vicieux : le raisonnement est interrompu en plein, et le contexte produit
    /// est a moitie inacheve.
    Overflow { partial: String },
}

/// Decide de la transition, a partir de l'etat et de ce que le modele a renvoye.
///
/// La fonction est pure : aucune base, aucun effet. C'est ce qui la rend
/// testable, contrairement a la version TypeScript ou la decision est noyee dans
/// une chaine d'effets.
pub fn transition(
    config: &AgentConfig,
    state: &TurnState,
    output: &ModelOutput,
    compacted_already: bool,
) -> Transition {
    // Le modele a epuise ses tokens. On compacte, mais une seule fois par tour :
    // compacter en boucle sur un overflow qui ne se resoudra pas consommerait le
    // contexte pour rien.
    if let ModelOutput::Overflow { partial } = output {
        if !compacted_already {
            return Transition::CompactedAfterOverflow { step: state.step };
        }
        // Deuxieme overflow : on garde ce qui a ete produit plutot que de
        // boucler indefiniment.
        return Transition::Done(partial.clone());
    }

    match output {
        ModelOutput::Final(text) => {
            if config.is_last_step(state.step) {
                // Au dernier tour, une reponse sans outils met fin au tour.
                Transition::Done(text.clone())
            } else {
                Transition::Done(text.clone())
            }
        }
        ModelOutput::ToolCalls(calls) => {
            // Au dernier tour, les outils sont retires. Si le modele en demande
            // quand meme, on ne les execute pas et on lui demande une reponse :
            // c'est la seule facon de sortir du tour avec une vraie reponse.
            if config.is_last_step(state.step) {
                return Transition::Done(
                    "Le modele a demande des outils au dernier tour, qui n'en accepte pas.".to_string(),
                );
            }
            Transition::NeedsTools(calls.clone())
        }
        ModelOutput::Overflow { partial } => Transition::Done(partial.clone()),
    }
}

/// Cle de cache de prompt pour une session.
///
/// L'original derive la cle de l'ID de session pour que le provider reutilise le
/// prefixe de contexte d'un tour a l'autre, ce qui reduit le cout et la latence.
/// On reproduit exactement la regle, y compris le cas des identifiants de 64
/// caracteres hexadecimaux, ou le prefixe `ses_` est retire.
pub fn prompt_cache_key(session_id: &str) -> String {
    if let Some(hex) = session_id.strip_prefix("ses_") {
        if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
            return hex.to_string();
        }
    }
    session_id.to_string()
}

/// En-tete d'affinite de session.
///
/// C'est ce qui permet au serveur de route la requete vers le meme hote, et donc
/// de conserver le cache de prompt. L'identifiant de session part dans l'en-tete
/// et non dans l'URL : il n'est pas incredite, mais il ne doit pas finir dans les
/// journaux d'acces.
pub fn session_affinity_header(session_id: &str) -> (&'static str, String) {
    ("x-session-affinity", session_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(step: u32) -> TurnState {
        TurnState { step, context_tokens: 10_000, context_limit: 200_000, output_reserve: 8_000, keep_tokens: 8_000 }
    }

    fn unlimited() -> AgentConfig {
        AgentConfig { steps: None }
    }

    fn capped(n: u32) -> AgentConfig {
        AgentConfig { steps: Some(n) }
    }

    #[test]
    fn une_reponse_finale_termine_le_tour() {
        let t = transition(&unlimited(), &state(1), &ModelOutput::Final("fait".into()), false);
        assert_eq!(t, Transition::Done("fait".into()));
    }

    #[test]
    fn des_appels_d_outils_demandent_un_tour_supplementaire() {
        let t = transition(&unlimited(), &state(1), &ModelOutput::ToolCalls(vec!["read".into()]), false);
        assert_eq!(t, Transition::NeedsTools(vec!["read".into()]));
    }

    #[test]
    fn au_dernier_tour_les_outils_sont_refuses() {
        // C'est le garde-fou qui garantit une reponse finale pour l'utilisateur.
        let t = transition(&capped(2), &state(2), &ModelOutput::ToolCalls(vec!["write".into()]), false);
        match t {
            Transition::Done(msg) => assert!(msg.contains("dernier tour")),
            other => panic!("attendu Done, obtenu {other:?}"),
        }
    }

    #[test]
    fn un_debordement_declenche_une_compaction_unique() {
        let t = transition(&unlimited(), &state(1), &ModelOutput::Overflow { partial: "a moitie".into() }, false);
        assert_eq!(t, Transition::CompactedAfterOverflow { step: 1 });
    }

    #[test]
    fn un_second_debordement_ne_boucle_pas() {
        // Si l'on recompactionsans arret, on consommerait le contexte pour rien.
        let t = transition(&unlimited(), &state(1), &ModelOutput::Overflow { partial: "partiel".into() }, true);
        assert_eq!(t, Transition::Done("partiel".into()));
    }

    #[test]
    fn la_detection_du_dernier_tour_respecte_l_absence_de_plafond() {
        let c = unlimited();
        assert!(!c.is_last_step(9999));
        assert!(capped(3).is_last_step(3));
        assert!(!capped(3).is_last_step(2));
    }

    #[test]
    fn un_contexte_serre_demande_une_compaction_avant_l_appel() {
        let s = TurnState { step: 1, context_tokens: 190_000, context_limit: 200_000, output_reserve: 8_000, keep_tokens: 8_000 };
        assert!(s.needs_pre_compaction());
        let s2 = TurnState { context_tokens: 1_000, ..s };
        assert!(!s2.needs_pre_compaction());
    }

    #[test]
    fn la_cle_de_cache_respecte_les_deux_formes_d_identifiant() {
        // Forme courte ou non-hexadecimale : on garde tel quel.
        assert_eq!(prompt_cache_key("ses_abc"), "ses_abc");
        // Forme longue hexadecimale : on retire le prefixe.
        let hex = "a".repeat(64);
        assert_eq!(prompt_cache_key(&format!("ses_{hex}")), hex);
        // Mauvaise longueur : pas de retrait.
        assert_eq!(prompt_cache_key(&format!("ses_{}", "a".repeat(63))), format!("ses_{}", "a".repeat(63)));
    }

    #[test]
    fn la_cle_de_cache_refuse_les_majuscules() {
        // La regex de l'original est `/^ses_[0-9a-f]{64}$/`, sans le drapeau
        // `i` : les majuscules ne sont donc pas acceptees, et l'identifiant
        // reste intact.
        let upper = format!("ses_{}", "A".repeat(64));
        assert_eq!(prompt_cache_key(&upper), upper);
    }

    #[test]
    fn l_en_tete_d_affinite_transporte_la_session() {
        let (k, v) = session_affinity_header("ses_123");
        assert_eq!(k, "x-session-affinity");
        assert_eq!(v, "ses_123");
    }
}
