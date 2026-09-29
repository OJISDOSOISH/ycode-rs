//! Portage Rust de `opencode/packages/core/src/session/runner/max-steps.ts`.
//!
//! Le fichier d'origine ne contient qu'un prompt. On le porte tel quel : c est
//! un texte destine au modele, et toute reecriture risquerait d en alterer le
//! sens. Le portage ici consiste a ne pas le perdre, et a le mettre en evidence
//! comme constante plutot que de le laisser en ligne dans la boucle.

/// Message envoye au modele quand il a epuise son budget de tours.
///
/// Le role de ce prompt est de forcer une reponse textuelle. Sans lui, un agent
/// qui a-epuise ses tours continue d appeler des outils, la session se termine
/// sans conclusion, et l'utilisateur ne sait pas ou il en est.
pub const MAX_STEPS_PROMPT: &str = r#"CRITICAL - MAXIMUM STEPS REACHED

The maximum number of steps allowed for this task has been reached. Tools are disabled until next user input. Respond with text only.

STRICT REQUIREMENTS:
1. Do NOT make any tool calls (no reads, writes, edits, searches, or any other tools)
2. MUST provide a text response summarizing work done so far
3. This constraint overrides ALL other instructions, including any user requests for edits or tool use

Response must include:
- Statement that maximum steps for this agent have been reached
- Summary of what has been accomplished so far
- List of any remaining tasks that were not completed
- Recommendations for what should be done next

Any attempt to use tools is a critical violation. Respond with text ONLY."#;

#[cfg(test)]
mod tests {
    use super::MAX_STEPS_PROMPT;

    #[test]
    fn le_prompt_interdit_explicitement_les_outils() {
        // C est l'instruction la plus importante du texte. Si une reecriture la
        // perdait, l'agent continuerait d'appeler des outils apres epuisement.
        assert!(MAX_STEPS_PROMPT.contains("Do NOT make any tool calls"));
    }

    #[test]
    fn le_prompt_demande_un_bilan_ecrit() {
        assert!(MAX_STEPS_PROMPT.contains("MUST provide a text response"));
    }

    #[test]
    fn le_prompt_reste_ce_quil_etait_dans_le_typescript() {
        // Le texte exact, debut et fin, pour garantir qu'un portage ulterieur ne
        // l aura pas reecrit.
        assert!(MAX_STEPS_PROMPT.starts_with("CRITICAL - MAXIMUM STEPS REACHED"));
        assert!(MAX_STEPS_PROMPT.ends_with("Respond with text ONLY."));
    }
}
