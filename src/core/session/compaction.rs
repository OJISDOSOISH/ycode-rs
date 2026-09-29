//! Portage Rust de la politique de `opencode/packages/core/src/session/compaction.ts`.
//!
//! Decider *quand* compacter est aussi important que la compaction elle-meme.
//! Deux regulation evitent que la fenetre de contexte n'explose :
//!
//! - La **troncature des sorties d'outils** a 2 000 caracteres. Un agent qui
//!   affiche un fichier de 5 000 lignes n'ecrit pas 5 000 lignes dans son
//!   contexte. C'est la regulation la plus rentable du systeme.
//! - Le **maintien d'un tampon** de 20 000 tokens avant de compacter, tout en
//!   gardant 8 000 tokens de contexte recent intact. On ne compacte pas trop
//!   tot : si on l'a fait trop tot, le modele a perdu le fil et ne peut plus
//!   travailler utilement.

/// Nombre de tokens qu'on garde sous la main apres compaction.
///
/// Ce n'est pas du contexte perdu : c'est l'historique recent conserve verbatim,
/// pour que le modele garde le fil immediat de la conversation.
pub const DEFAULT_KEEP_TOKENS: u64 = 8_000;

/// Marge de securite avant de declencher une compaction.
///
/// On compacte quand le contexte remplit la fenetre moins cette marge. Sans
/// elle, on detecte le debordement trop tard, et la reponse du modele est deja
/// coupee.
pub const DEFAULT_BUFFER: u64 = 20_000;

/// Longueur maximale d'une sortie d'outil conservee.
///
/// La regulation la plus rentable : un agent qui lit un gros fichier n'injecte
/// que les 2 000 premiers caracteres dans son contexte.
pub const TOOL_OUTPUT_MAX_CHARS: usize = 2_000;

/// Tokens reserves au resume genere lors d'une compaction.
pub const SUMMARY_OUTPUT_TOKENS: u64 = 4_096;

/// Estimation du nombre de tokens d'un texte.
///
/// Heuristique volontairement simple : environ 4 caracteres par token. C'est
/// l'estimation standard pour les modeles a tokenizer de type GPT/BPE en anglais
/// et en code. Elle est grossiere, mais une estimation grossiere qui ne coute
/// rien vaut mieux qu'un vrai comptage, qui obligerait a charger le tokenizer du
/// modele a chaque boucle de decision.
///
/// Le facteur est volontairement en `f64` et non entier : a l'echelle d'un
/// contexte de 200 000 tokens, l'arrondi au token entier n'a aucun sens.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as f64 / 4.0).ceil() as u64
}

/// Estime les tokens d'une structure serialisable.
///
/// Le JSON est produit avant comptage, donc la memoire de sa structure est
/// comptee comme du texte : c'est ce que le modele verra reellement.
pub fn estimate_value<T: serde::Serialize>(value: &T) -> u64 {
    match serde_json::to_string(value) {
        Ok(json) => estimate_tokens(&json),
        // Serialisation impossible : on retient une estimation large plutot que
        // de sous-estimer, qui ferait deborder le contexte en silence.
        Err(_) => 4_096,
    }
}

/// Tronque une sortie d'outil a la limite reglementaire.
///
/// La coupure se fait sur un frontiere de caractere UTF-8 valide, sinon on
/// paniquerait sur une chaine de fin de ligne multi-octets. Le marqueur indique
/// qu'il y a eu une coupe, pour que le modele sache qu'il ne voit pas tout.
pub fn truncate_tool_output(text: &str) -> String {
    if text.chars().count() <= TOOL_OUTPUT_MAX_CHARS {
        return text.to_string();
    }

    let cut: String = text.chars().take(TOOL_OUTPUT_MAX_CHARS).collect();
    let omitted = text.chars().count() - TOOL_OUTPUT_MAX_CHARS;
    format!("{cut}\n[... {omitted} caracteres tronques ...]")
}

/// Faut-il compacter maintenant ?
///
/// Le calcul de l'original :
/// `context_tokens + output_reserve > context_limit - buffer`
///
/// avec, en plus, la contrainte de garder `keep_tokens` d'historique recent.
/// Les deux conditions doivent etre vraies : sans la premiere, on ne compacte
/// pas a temps ; sans la seconde, on compacte trop tot et le modele perd le fil.
pub fn should_compact(context_tokens: u64, context_limit: u64, output_reserve: u64, keep_tokens: u64) -> bool {
    if context_limit == 0 {
        // Pas de limite connue : on ne compacte pas, sinon on mutilerait
        // l'historique sans raison.
        return false;
    }
    let headroom = context_limit.saturating_sub(DEFAULT_BUFFER);
    let pressure = context_tokens.saturating_add(output_reserve);
    pressure > headroom && context_tokens > keep_tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_estimation_compte_quatre_caracteres_par_token() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn l_estimation_gere_les_accents_sans_paniquer() {
        // Un caractere multi-octets compte pour 1, pas pour sa taille en octets.
        assert_eq!(estimate_tokens("éééé"), 1);
    }

    #[test]
    fn une_sortie_courte_n_est_pas_tronquee() {
        let s = "court";
        assert_eq!(truncate_tool_output(s), s);
    }

    #[test]
    fn une_sortie_longue_est_tronquee_et_annoncee() {
        let s = "x".repeat(TOOL_OUTPUT_MAX_CHARS + 500);
        let out = truncate_tool_output(&s);
        assert!(out.contains("tronques"));
        assert!(out.contains("500 caracteres"));
        // Le contenu utile ne doit pas depasser la limite de facon absurde.
        assert!(out.len() < TOOL_OUTPUT_MAX_CHARS + 200);
    }

    #[test]
    fn la_troncature_ne_coupe_pas_un_utf8() {
        // Un emoji fait 4 octets : couper au milieu produirait du JSON invalide
        // et un panic Rust.
        let s = "🎉".repeat(TOOL_OUTPUT_MAX_CHARS);
        let out = truncate_tool_output(&s);
        assert!(out.is_char_boundary(0));
    }

    #[test]
    fn on_necompacte_pas_tant_quil_reste_de_la_place() {
        // 30 000 tokens utilises sur 200 000 : tres large marge.
        assert!(!should_compact(30_000, 200_000, 8_000, 8_000));
    }

    #[test]
    fn oncompacte_quand_le_tampon_est_mange() {
        // 200 000 de limite, 20 000 de tampon => on compacte au-dela de 180 000.
        assert!(!should_compact(170_000, 200_000, 8_000, 8_000));
        assert!(should_compact(185_000, 200_000, 8_000, 8_000));
    }

    #[test]
    fn la_reserve_de_sortie_compte_dans_la_pression() {
        // Meme contexte, mais on demande 30 000 tokens de sortie : la pression
        // effective depasse le seuil, il faut compacter.
        assert!(!should_compact(150_000, 200_000, 8_000, 8_000));
        assert!(should_compact(150_000, 200_000, 40_000, 8_000));
    }

    #[test]
    fn on_necompacte_pas_un_contexte_deja_minuscule() {
        // Garder 8 000 tokens recent : inutile de compacter un historique de
        // 2 000 tokens, on perdrait plus qu'on ne gagne.
        assert!(!should_compact(2_000, 200_000, 8_000, 8_000));
    }

    #[test]
    fn une_limite_inconnue_desactive_la_compaction() {
        assert!(!should_compact(999_999, 0, 0, 8_000));
    }
}
