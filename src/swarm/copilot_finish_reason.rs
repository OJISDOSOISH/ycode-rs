//! Portage de `github-copilot/chat/map-openai-compatible-finish-reason.ts`.
//!
//! La source est une fonction pure de 19 lignes : elle traduit la raison de fin
//! de reponse d'une API compatible OpenAI vers la valeur unifiee du SDK.
//!
//! Points de fidelite importants :
//!
//! - L'entree est `string | null | undefined`, ce qui devient `Option<&str>`.
//!   En TypeScript `null` et `undefined` sont deux cas distincts du `switch`,
//!   mais ils tombent tous les deux dans le meme `default`. En Rust ils
//!   fusionnent en un seul `None` : c'est la seule difference, et elle n'a
//!   aucune consequence car le `default` est commun aux deux.
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
//! L'union `LanguageModelV3FinishReason["unified"]` du SDK contient aussi la
//! valeur `"error"`. Cette fonction ne la produit jamais, donc la variante
//! correspondante est volontairement absente plutot que d'etre inventee.

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
            serde_json::to_string(&UnifiedFinishReason::Other).unwrap(),
            "\"other\""
        );
    }

    #[test]
    fn chaque_variante_se_relit_depuis_sa_chaine() {
        let relues: Vec<UnifiedFinishReason> = serde_json::from_str(
            "[\"stop\",\"length\",\"content-filter\",\"tool-calls\",\"other\"]",
        )
        .unwrap();
        assert_eq!(
            relues,
            vec![
                UnifiedFinishReason::Stop,
                UnifiedFinishReason::Length,
                UnifiedFinishReason::ContentFilter,
                UnifiedFinishReason::ToolCalls,
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
}
