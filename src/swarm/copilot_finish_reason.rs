//! Portage de `github-copilot/chat/map-openai-compatible-finish-reason.ts`.
//!
//! La source est une fonction pure de 19 lignes : elle traduit la raison de fin
//! de reponse d'une API compatible OpenAI vers la valeur unifiee du SDK.
//!
//! Points de fidelite importants :
//!
//! - L'entree est `string | null | undefined`, ce qui devient `Option<&str>`.
//!   Le `switch` de la source ne comporte **aucun** `case null` ni
//!   `case undefined` : ces deux valeurs distinctes tombent donc toutes les
//!   deux dans le `default` commun. En Rust elles fusionnent en un seul `None`,
//!   ce qui est la seule difference entre les deux langages, et elle n'a aucune
//!   consequence puisque le `default` est deja commun aux deux.
//! - Le `default` du `switch` renvoie la **litterale** `"other"`. Il ne
//!   renvoie jamais la valeur d'entree. Une raison inconnue, une chaine vide
//!   ou une absence de raison donnent donc tous `"other"`, et non une
//!   recopie de ce qui est recu.
//! - Le `switch` JavaScript compare par egalite stricte : la casse compte.
//!   `"STOP"` n'est pas `"stop"` et bascule donc vers `"other"`. Le `match`
//!   Rust se comporte de la meme facon, les motifs de chaine etant sensibles
//!   a la casse.
//! - `"content_filter"` et `tool_calls` portent un **tiret** dans la valeur
//!   unifiee alors que la source OpenAI utilise un tiret bas ou rien du tout.
//!   Ces tirets sont des caracteres d'echanges, ils sont figes ici.
//!
//! L'union `LanguageModelV3FinishReason["unified"]` comporte **six** membres et
//! non cinq : `stop`, `length`, `content-filter`, `tool-calls`, `error` et
//! `other`. Le paquet `@ai-sdk/provider` n'est pas installe sur ce poste, mais la
//! presence de `"error"` se deduit du depot lui-meme :
//! `openai-compatible-chat-language-model.ts` type `finishReason.unified` par
//! `ReturnType<typeof mapOpenAICompatibleFinishReason>` (ligne 342) puis lui
//! affecte `unified: "error"` (lignes 396 et 409). La variante `Error` est donc
//! declaree pour que le type reste complet sur le fil, **mais aucune branche du
//! `match` ne la produit** : la fonction est pure, un `switch` sans effet de
//! bord ne peut pas forger une erreur. Un test verrouille cette separation.
//!
//! Corollaire : la liste des cinq valeurs presentees dans le `match` est
//! exhaustive pour la *fonction*, pas pour le *type*. C'est exactement la
//! distinction que le `default` exerce, et c'est pourquoi `Error` est portee par
//! l'enum sans etre atteignable par la fonction.

use serde::{Deserialize, Serialize};

/// Les raisons de fin unifiees, avec les chaines exactes du SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnifiedFinishReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "content-filter")]
    ContentFilter,
    #[serde(rename = "tool-calls")]
    ToolCalls,
    /// Membre de l'union du SDK que cette fonction ne produit jamais.
    ///
    /// Il figure ici pour que le type reste complet sur le fil : le modele de
    /// chat voisin emet bien `unified: "error"`. Aucun bras du `match` n'y
    /// aboutit, et `map_openai_compatible_finish_reason` le renvoie donc jamais.
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "other")]
    Other,
}

impl UnifiedFinishReason {
    /// La chaine unifiee telle qu'elle circule sur le reseau.
    pub fn as_str(&self) -> &'static str {
        match self {
            UnifiedFinishReason::Stop => "stop",
            UnifiedFinishReason::Length => "length",
            UnifiedFinishReason::ContentFilter => "content-filter",
            UnifiedFinishReason::ToolCalls => "tool-calls",
            UnifiedFinishReason::Error => "error",
            UnifiedFinishReason::Other => "other",
        }
    }
}

/// Traduit une raison de fin compatible OpenAI vers sa valeur unifiee.
///
/// Le parametre `None` couvre `null` et `undefined`.
pub fn map_openai_compatible_finish_reason(
    finish_reason: Option<&str>,
) -> UnifiedFinishReason {
    match finish_reason {
        Some("stop") => UnifiedFinishReason::Stop,
        Some("length") => UnifiedFinishReason::Length,
        Some("content_filter") => UnifiedFinishReason::ContentFilter,
        Some("function_call") | Some("tool_calls") => UnifiedFinishReason::ToolCalls,
        // Tout le reste, y compris None et la chaine vide : la valeur de retour
        // est la litterale "other", jamais la valeur d'entree.
        _ => UnifiedFinishReason::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_raison_stop_donne_stop() {
        assert_eq!(
            map_openai_compatible_finish_reason(Some("stop")).as_str(),
            "stop"
        );
    }

    #[test]
    fn une_raison_length_donne_length() {
        assert_eq!(
            map_openai_compatible_finish_reason(Some("length")).as_str(),
            "length"
        );
    }

    #[test]
    fn content_filter_devient_content_filter_avec_un_tiret() {
        assert_eq!(
            map_openai_compatible_finish_reason(Some("content_filter")).as_str(),
            "content-filter"
        );
    }

    #[test]
    fn function_call_et_tool_calls_donnent_la_meme_valeur() {
        assert_eq!(
            map_openai_compatible_finish_reason(Some("function_call")),
            UnifiedFinishReason::ToolCalls
        );
        assert_eq!(
            map_openai_compatible_finish_reason(Some("tool_calls")),
            UnifiedFinishReason::ToolCalls
        );
        assert_eq!(
            map_openai_compatible_finish_reason(Some("tool_calls")).as_str(),
            "tool-calls"
        );
    }

    #[test]
    fn une_raison_inconnue_devient_other_sans_la_recopier() {
        assert_eq!(
            map_openai_compatible_finish_reason(Some("banane")).as_str(),
            "other"
        );
        assert_eq!(
            map_openai_compatible_finish_reason(Some("not_a_real_reason")).as_str(),
            "other"
        );
        // La valeur entrante n'est jamais ressortie telle quelle.
        assert_ne!(
            map_openai_compatible_finish_reason(Some("banane")).as_str(),
            "banane"
        );
    }

    #[test]
    fn une_raison_absente_devient_other() {
        assert_eq!(
            map_openai_compatible_finish_reason(None).as_str(),
            "other"
        );
    }

    #[test]
    fn une_chaine_vide_devient_other() {
        // Une chaine vide est un `Some("")` en Rust, pas un `None`. Elle ne
        // correspond a aucun motif et doit finir sur la branche de repli.
        assert_eq!(
            map_openai_compatible_finish_reason(Some("")).as_str(),
            "other"
        );
    }

    #[test]
    fn la_casse_compte_comme_en_javascript() {
        assert_eq!(
            map_openai_compatible_finish_reason(Some("STOP")).as_str(),
            "other"
        );
        assert_eq!(
            map_openai_compatible_finish_reason(Some("Content_Filter")).as_str(),
            "other"
        );
    }

    #[test]
    fn une_variante_sans_espace_ne_devient_pas_stop() {
        // Piege de veracite : " stop" n'est pas "stop".
        assert_eq!(
            map_openai_compatible_finish_reason(Some(" stop")).as_str(),
            "other"
        );
    }

    #[test]
    fn les_chaines_emises_sont_bien_celles_du_sdk() {
        // Forme BARE STRING, pas un objet balise : le `switch` renvoie une
        // litterale, donc la variante se serialise seule, jamais {"Stop": null}.
        assert_eq!(
            serde_json::to_string(&UnifiedFinishReason::Stop).unwrap(),
            "\"stop\""
        );
        assert_eq!(
            serde_json::to_string(&UnifiedFinishReason::Length).unwrap(),
            "\"length\""
        );
        assert_eq!(
            serde_json::to_string(&UnifiedFinishReason::ContentFilter).unwrap(),
            "\"content-filter\""
        );
        assert_eq!(
            serde_json::to_string(&UnifiedFinishReason::ToolCalls).unwrap(),
            "\"tool-calls\""
        );
        assert_eq!(
            serde_json::to_string(&UnifiedFinishReason::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&UnifiedFinishReason::Other).unwrap(),
            "\"other\""
        );
    }

    #[test]
    fn chaque_variante_se_relit_depuis_sa_chaine() {
        let relues: Vec<UnifiedFinishReason> = serde_json::from_str(
            "[\"stop\",\"length\",\"content-filter\",\"tool-calls\",\"error\",\"other\"]",
        )
        .unwrap();
        assert_eq!(
            relues,
            vec![
                UnifiedFinishReason::Stop,
                UnifiedFinishReason::Length,
                UnifiedFinishReason::ContentFilter,
                UnifiedFinishReason::ToolCalls,
                UnifiedFinishReason::Error,
                UnifiedFinishReason::Other,
            ]
        );
        for relue in &relues {
            assert_eq!(
                serde_json::to_string(relue).unwrap(),
                format!("\"{}\"", relue.as_str())
            );
        }
    }

    #[test]
    fn error_est_dans_le_type_mais_jamais_produit_par_la_fonction() {
        // Le modele de chat voisin emet `unified: "error"` (lignes 396 et 409 de
        // openai-compatible-chat-language-model.ts), donc la chaine se relit.
        assert_eq!(
            serde_json::from_str::<UnifiedFinishReason>("\"error\"").unwrap(),
            UnifiedFinishReason::Error
        );
        // Mais la fonction est pure : aucune entree ne peut la produire. C'est la
        // separation type / fonction qui garantit que le `default` ne fabrique
        // jamais une erreur non plus.
        for entree in [
            None,
            Some(""),
            Some("error"),
            Some("Error"),
            Some("stop"),
            Some("length"),
            Some("content_filter"),
            Some("function_call"),
            Some("tool_calls"),
            Some("banane"),
            Some("autre chose"),
        ] {
            assert_ne!(
                map_openai_compatible_finish_reason(entree),
                UnifiedFinishReason::Error,
                "la fonction ne doit jamais produire Error, entree {:?}",
                entree
            );
        }
    }

    #[test]
    fn toute_entree_retombe_sur_une_valeur_unifiee_et_jamais_sur_sa_forme_brute() {
        // Test de totalite : le `default` renvoie la litterale "other", donc
        // l'image de la fonction est toujours une valeur du jeu unifie. Aucune
        // entree ne peut se retrouver resortie telle quelle.
        let autorisees = [
            "stop",
            "length",
            "content-filter",
            "tool-calls",
            "error",
            "other",
        ];
        let entrees = [
            None,
            Some(""),
            Some(" "),
            Some("stop"),
            Some("stop "),
            Some(" stop"),
            Some("STOP"),
            Some("Stop"),
            Some("length"),
            Some("Length"),
            Some("max_tokens"),
            Some("max_output_tokens"),
            Some("content_filter"),
            Some("Content_Filter"),
            Some("content-filter"),
            Some("function_call"),
            Some("functionCall"),
            Some("tool_calls"),
            Some("tool-calls"),
            Some("other"),
            Some("error"),
            Some("banane"),
            Some("not_a_real_reason"),
            Some("tool call"),
            Some("stop\n"),
            Some("\u{0}"),
        ];
        for entree in entrees {
            let obtenue_str = map_openai_compatible_finish_reason(entree).as_str();
            // Cette assertion suffit a interdire la recopie : si la sortie
            // appartient toujours au jeu unifie, une entree comme "banane" ne
            // peut structurellement pas ressortir inchangee.
            assert!(
                autorisees.contains(&obtenue_str),
                "entree {:?} a produit {:?}, hors du jeu unifie",
                entree,
                obtenue_str
            );
        }
    }
}
