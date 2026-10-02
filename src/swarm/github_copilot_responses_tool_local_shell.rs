//! Portage de `github-copilot/responses/tool/local-shell.ts`.
//!
//! La source fait 64 lignes : schemas zod de l entree (`action.exec`,
//! `command`, `timeoutMs`, `user`, `workingDirectory`, `env`) et de la sortie
//! (`output`), plus la fabrique d outil id `"openai.local_shell"`.
//!
//! Seule la partie pure est portee ici, sans `@ai-sdk/provider-utils` :
//! types d action et de sortie avec leurs cles exactes.
//!
//! Volontairement non porte : `createProviderToolFactoryWithOutputSchema`,
//! la validation zod et l execution de la commande.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifiant de l outil tel qu enregistre par la fabrique.
pub const ID_OUTIL: &str = "openai.local_shell";

/// Action `exec` : commande + options, cles camelCase cote entree outil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionExec {
    #[serde(rename = "type")]
    pub kind: String,
    pub command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "timeoutMs")]
    pub delai_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "workingDirectory")]
    pub repertoire: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
}

impl ActionExec {
    pub fn nouvelle(commande: Vec<String>) -> Self {
        ActionExec {
            kind: "exec".to_string(),
            command: commande,
            delai_ms: None,
            user: None,
            repertoire: None,
            env: None,
        }
    }
}

/// Entree de l outil : `{ action }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntreeShell {
    pub action: ActionExec,
}

/// Sortie de l outil : `{ output }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortieShell {
    pub output: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_identifiant_est_celui_de_la_source() {
        assert_eq!(ID_OUTIL, "openai.local_shell");
    }

    #[test]
    fn le_type_exec_est_constant() {
        let action = ActionExec::nouvelle(vec!["ls".to_string()]);
        assert_eq!(action.kind, "exec");
        let valeur = serde_json::to_value(&action).unwrap();
        assert_eq!(valeur.get("type").and_then(|v| v.as_str()), Some("exec"));
    }

    #[test]
    fn les_cles_optionnelles_gardent_leur_camel_case() {
        let mut action = ActionExec::nouvelle(vec!["echo".to_string(), "bonjour".to_string()]);
        action.delai_ms = Some(1_000.0);
        action.repertoire = Some("/tmp".to_string());
        let valeur = serde_json::to_value(&action).unwrap();
        assert!(valeur.get("timeoutMs").is_some());
        assert!(valeur.get("workingDirectory").is_some());
        assert!(valeur.get("timeout_ms").is_none());
        assert!(valeur.get("working_directory").is_none());
    }

    #[test]
    fn une_action_minimale_ne_serialise_que_type_et_commande() {
        let action = ActionExec::nouvelle(vec!["ls".to_string()]);
        let valeur = serde_json::to_value(&action).unwrap();
        assert!(valeur.get("user").is_none());
        assert!(valeur.get("env").is_none());
        let relue: ActionExec = serde_json::from_value(valeur).unwrap();
        assert_eq!(relue, action);
    }

    #[test]
    fn la_sortie_porte_le_texte_de_sortie() {
        let sortie = SortieShell {
            output: "ok".to_string(),
        };
        let valeur = serde_json::to_value(&sortie).unwrap();
        assert_eq!(valeur.get("output").and_then(|v| v.as_str()), Some("ok"));
    }
}
