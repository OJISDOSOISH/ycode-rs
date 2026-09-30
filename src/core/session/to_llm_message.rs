//! Conversion des messages de session vers le format attendu par un provider,
//! d'apres `opencode/packages/core/src/session/runner/to-llm-message.ts`.
//!
//! C'est l'adaptateur entre le modele de donnees riche d'OpenCode et le format
//! simple qu'un provider comprend. Deux subtilites la rendent delicate.
//!
//! **Les metadonnees de provider ne sont reutilisees que pour le meme modele.**
//! Elles servent au cache et a la reprise de raisonnement chez un provider
//! donne. Les renvoyer a un autre provider produit des erreurs de cache
//! silencieuses, qui apparaissent bien plus tard, sans lien apparent avec leur
//! cause. D'ou la comparaison `sameModel`.
//!
//! **Le raisonnement d'un autre modele devient du texte.** On ne peut pas
//! renvoyer un raisonnement etrange chez un provider qui ne l'a pas produit :
//! il degrade le texte ordinaire, ce qui est la seule chose que le nouveau modele
//! peut comprendre.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::schema::session_message::{
    AssistantContent, AssistantTool, Message, ModelRef, ToolState, User,
};

/// Partie de contenu telle qu'un provider la comprend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    Text {
        text: String,
    },
    Media {
        #[serde(rename = "mediaType")]
        media_type: Option<String>,
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
    Reasoning {
        text: String,
        #[serde(rename = "providerMetadata", skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    ToolCall {
        id: String,
        name: String,
        input: Value,
        #[serde(rename = "providerExecuted", skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
        #[serde(rename = "providerMetadata", skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
    ToolResult {
        id: String,
        name: String,
        result: Value,
        #[serde(rename = "resultType", skip_serializing_if = "Option::is_none")]
        result_type: Option<String>,
        #[serde(rename = "providerExecuted", skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
        #[serde(rename = "providerMetadata", skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<Value>,
    },
}

/// Role d'un message envoye au provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

/// Message envoye au provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmMessage {
    pub id: String,
    pub role: Role,
    pub content: Vec<ContentPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Fichier joint a un message utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaFile {
    #[serde(rename = "mediaType")]
    pub mime: Option<String>,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl MediaFile {
    fn to_part(&self) -> ContentPart {
        ContentPart::Media {
            media_type: self.mime.clone(),
            data: self.data.clone(),
            filename: self.filename.clone(),
        }
    }
}

/// Le provider cible, pour la comparaison de modele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetModel {
    pub provider: String,
    pub id: String,
}

/// Parametres d'entree de l'appel de l'outil.
///
/// L'original parse le JSON stocke en `pending` et le renvoie tel quel en cas
/// d'echec. On reproduit : un JSON invalide n'est pas une raison de perdre
/// l'appel, il faut le transmettre brut pour que le modele puisse le corriger.
pub fn tool_input(tool: &AssistantTool) -> Value {
    match &tool.state {
        ToolState::Pending { input } => serde_json::from_str(input).unwrap_or_else(|_| Value::String(input.clone())),
        ToolState::Running { input, .. }
        | ToolState::Completed { input, .. }
        | ToolState::Error { input, .. } => input.clone(),
    }
}

/// Partie d'appel d'outil.
pub fn tool_call(tool: &AssistantTool, provider_metadata: Option<Value>) -> ContentPart {
    ContentPart::ToolCall {
        id: tool.id.clone(),
        name: tool.name.clone(),
        input: tool_input(tool),
        provider_executed: tool.provider.as_ref().map(|p| p.executed),
        provider_metadata,
    }
}

/// Partie de resultat d'un outil.
///
/// `None` si l'appel n'est pas encore termine : un appel en cours n'a pas de
/// resultat, et en envoyer un vaudrait `null` que le modele interpreterait
/// comme un echec.
pub fn tool_result(tool: &AssistantTool, provider_metadata: Option<Value>) -> Option<ContentPart> {
    let executed = tool.provider.as_ref().map(|p| p.executed).unwrap_or(false);

    match &tool.state {
        ToolState::Completed { structured, content, result, .. } => {
            // Un outil execute par le provider a deja produit son resultat
            // canonique : on ne le recalcule pas.
            let value = if executed {
                result.clone().unwrap_or(Value::Null)
            } else {
                result_value(structured, content)
            };
            Some(ContentPart::ToolResult {
                id: tool.id.clone(),
                name: tool.name.clone(),
                result: value,
                result_type: None,
                provider_executed: Some(executed),
                provider_metadata,
            })
        }
        ToolState::Error { structured, content, error, result, .. } => {
            let value = if executed {
                result.clone().unwrap_or(Value::Null)
            } else {
                serde_json::json!({
                    "error": error,
                    "content": content,
                    "structured": structured,
                })
            };
            Some(ContentPart::ToolResult {
                id: tool.id.clone(),
                name: tool.name.clone(),
                result: value,
                result_type: Some("error".to_string()),
                provider_executed: Some(executed),
                provider_metadata,
            })
        }
        // En attente ou en cours : pas de resultat.
        ToolState::Pending { .. } | ToolState::Running { .. } => None,
    }
}

/// Valeur de resultat d'un outil execute localement.
fn result_value(structured: &Value, content: &[crate::schema::session_message::ToolContent]) -> Value {
    if !structured.is_null() {
        return structured.clone();
    }
    let texts: Vec<String> = content
        .iter()
        .filter_map(|c| match c {
            crate::schema::session_message::ToolContent::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    if texts.len() == 1 {
        Value::String(texts.into_iter().next().unwrap())
    } else {
        Value::Array(texts.into_iter().map(Value::String).collect())
    }
}

/// Une partie est-elle porteuse de sens ?
///
/// Le filtrage evite d'envoyer un raisonnement vide ou un texte vide : un
/// provider peut les refuser, ou pire les compter comme du contexte inutile.
fn is_meaningful(part: &ContentPart) -> bool {
    match part {
        ContentPart::Text { text } => !text.is_empty(),
        ContentPart::Reasoning { text, provider_metadata } => {
            // `as_ref` : on inspecte la reference, on ne la consomme pas.
            !text.is_empty()
                || provider_metadata
                    .as_ref()
                    .and_then(|m| m.as_object())
                    .is_some_and(|o| !o.is_empty())
        }
        _ => true,
    }
}

/// Convertit un message de session en messages pour le provider.
///
/// Un message produit **zero, un ou plusieurs** messages : un message assistant
/// suivi de resultats d'outils donne un message principal plus un message par
/// resultat. Une fonction qui renvoyait un seul message perdrait ces resultats.
pub fn to_llm_messages(message: &Message, target: &TargetModel) -> Vec<LlmMessage> {
    match message {
        // Un changement d'agent ou de modele n'est pas un message : c'est une
        // metadonnee de session. Le renvoyer au provider le ferait halluciner
        // un tour.
        Message::AgentSwitched(_) | Message::ModelSwitched(_) => vec![],

        Message::User(User { base, prompt, .. }) => {
            let mut content = vec![ContentPart::Text { text: prompt.text.clone() }];
            for file in &prompt.files {
                content.push(MediaFile {
                    mime: file.mime.clone(),
                    data: String::new(),
                    filename: None,
                    description: None,
                }
                .to_part());
            }
            let mut metadata = base.metadata.clone();
            if !prompt.agents.is_empty() {
                let mut m = metadata.unwrap_or_else(|| Value::Object(Default::default()));
                if let Some(obj) = m.as_object_mut() {
                    obj.insert("agents".to_string(), serde_json::json!(prompt.agents));
                }
                metadata = Some(m);
            }
            vec![LlmMessage { id: base.id.clone(), role: Role::User, content, metadata }]
        }

        Message::Synthetic(s) => vec![LlmMessage {
            id: s.base.id.clone(),
            role: Role::User,
            content: vec![ContentPart::Text { text: s.text.clone() }],
            metadata: s.base.metadata.clone(),
        }],

        Message::System(s) => vec![LlmMessage {
            id: s.base.id.clone(),
            role: Role::System,
            content: vec![ContentPart::Text { text: s.text.clone() }],
            metadata: s.base.metadata.clone(),
        }],

        Message::Assistant(a) => {
            // `a.model` est un `ModelRef` canonique `{id, providerID, variant?}`.
            // `target` est le struct local `TargetModel { provider, id }`, dont les
            // noms n'ont pas ete migres. D'ou le melange apparent des deux formes.
            let same_model = a.model.provider_id == target.provider && a.model.id == target.id;
            // Une erreur invalide les metadonnees : elles decrivent un appel qui
            // n'a pas abouti, les reutiliser ferait Echouer la reprise.
            let reuse = same_model && a.error.is_none();

            let mut content = Vec::new();
            for item in &a.content {
                match item {
                    AssistantContent::Text { text, .. } => content.push(ContentPart::Text { text: text.clone() }),
                    AssistantContent::Reasoning { text, provider_metadata, .. } => {
                        if same_model {
                            content.push(ContentPart::Reasoning {
                                text: text.clone(),
                                provider_metadata: if reuse { provider_metadata.clone() } else { None },
                            });
                        } else if !text.is_empty() {
                            // Un raisonnement produit ailleurs devient du texte
                            // ordinaire : c'est la seule forme que l autre modele
                            // sait traiter.
                            content.push(ContentPart::Text { text: text.clone() });
                        }
                    }
                    AssistantContent::Tool(t) => {
                        let executed = t.provider.as_ref().map(|p| p.executed).unwrap_or(false);
                        let meta = if reuse { t.provider.as_ref().and_then(|p| p.metadata.clone()) } else { None };
                        content.push(tool_call(t, meta));
                        if executed {
                            let result_meta = if reuse {
                                t.provider.as_ref().and_then(|p| p.result_metadata.clone().or_else(|| p.metadata.clone()))
                            } else {
                                None
                            };
                            if let Some(r) = tool_result(t, result_meta) {
                                content.push(r);
                            }
                        }
                    }
                }
            }

            let meaningful: Vec<ContentPart> = content.into_iter().filter(is_meaningful).collect();

            // Les resultats d'outils **non** executes par le provider deviennent
            // des messages distincts : le provider n'a pas d'entree pour un
            // resultat qu'il n'a pas produit lui-meme.
            let results: Vec<LlmMessage> = a
                .content
                .iter()
                .filter_map(|item| match item {
                    AssistantContent::Tool(t) if t.provider.as_ref().map(|p| p.executed) != Some(true) => {
                        let meta = if reuse {
                            t.provider.as_ref().and_then(|p| p.result_metadata.clone().or_else(|| p.metadata.clone()))
                        } else {
                            None
                        };
                        tool_result(t, meta).map(|part| LlmMessage {
                            id: t.id.clone(),
                            role: Role::Tool,
                            content: vec![part],
                            metadata: None,
                        })
                    }
                    _ => None,
                })
                .collect();

            if meaningful.is_empty() {
                return results;
            }

            let mut out = vec![LlmMessage {
                id: a.base.id.clone(),
                role: Role::Assistant,
                content: meaningful,
                metadata: a.base.metadata.clone(),
            }];
            out.extend(results);
            out
        }

        // Le resultat d'une commande shell est deja passe par un message outil
        // ou par le texte de l'assistant : le renvoyer double le contexte.
        Message::Shell(_) => vec![],

        // La compaction remplace l'historique qu'elle resume, elle ne s y ajoute
        // pas.
        Message::Compaction(c) => vec![LlmMessage {
            id: c.base.id.clone(),
            role: Role::User,
            content: vec![ContentPart::Text { text: c.summary.clone() }],
            metadata: None,
        }],
    }
}

/// Convertit tout un historique.
pub fn to_llm_history(messages: &[Message], target: &TargetModel) -> Vec<LlmMessage> {
    messages.iter().flat_map(|m| to_llm_messages(m, target)).collect()
}

/// Le modele d'un message assistant, pour comparaison.
pub fn model_of(message: &Message) -> Option<ModelRef> {
    match message {
        Message::Assistant(a) => Some(a.model.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::session_message::{Assistant, MessageBase, Prompt, System};

    fn target() -> TargetModel {
        TargetModel { provider: "openai".into(), id: "gpt-4".into() }
    }

    fn assistant_msg(provider: &str, id: &str) -> Message {
        Message::Assistant(Assistant::new("msg_1", 1, "build", ModelRef::new(id, provider)))
    }

    fn with_content(msg: Message, content: Vec<AssistantContent>) -> Message {
        let Message::Assistant(mut a) = msg else { panic!("assistant attendu") };
        a.content = content;
        Message::Assistant(a)
    }

    #[test]
    fn un_changement_de_modele_ne_produit_aucun_message() {
        // L'envoyer ferait compter un tour fantome au provider.
        let msg = Message::ModelSwitched(crate::schema::session_message::ModelSwitched {
            base: MessageBase::new("msg_1", 1),
            model: ModelRef::new("claude", "anthropic"),
        });
        assert!(to_llm_messages(&msg, &target()).is_empty());
    }

    #[test]
    fn le_raisonnement_passe_a_un_autre_modele_devient_du_texte() {
        // Un raisonnement etrange chez un autre provider provoque des erreurs
        // de cache sans cause apparente.
        let msg = with_content(
            assistant_msg("anthropic", "claude"),
            vec![AssistantContent::Reasoning {
                id: "r1".into(),
                text: "je reflechis".into(),
                provider_metadata: Some(serde_json::json!({"cache": "x"})),
                time: None,
            }],
        );
        let out = to_llm_messages(&msg, &target());
        assert!(matches!(out[0].content[0], ContentPart::Text { .. }), "degrade en texte");
    }

    #[test]
    fn le_raisonnement_du_meme_modele_garde_ses_metadonnees() {
        let meta = serde_json::json!({"cache": "x"});
        let msg = with_content(
            assistant_msg("openai", "gpt-4"),
            vec![AssistantContent::Reasoning {
                id: "r1".into(),
                text: "je reflechis".into(),
                provider_metadata: Some(meta.clone()),
                time: None,
            }],
        );
        let out = to_llm_messages(&msg, &target());
        match &out[0].content[0] {
            ContentPart::Reasoning { provider_metadata, .. } => {
                assert_eq!(provider_metadata, &Some(meta), "les metadonnees sont conservees");
            }
            other => panic!("attendu Reasoning, obtenu {other:?}"),
        }
    }

    #[test]
    fn une_erreur_invalide_la_reutilisation_des_metadonnees() {
        let meta = serde_json::json!({"cache": "x"});
        let mut a = Assistant::new("msg_1", 1, "build", ModelRef::new("openai", "gpt-4"));
        a.error = Some(crate::schema::session_message::UnknownError::new("boom"));
        a.content.push(AssistantContent::Reasoning {
            id: "r1".into(),
            text: "partiel".into(),
            provider_metadata: Some(meta),
            time: None,
        });
        let out = to_llm_messages(&Message::Assistant(a), &target());
        match &out[0].content[0] {
            ContentPart::Reasoning { provider_metadata, .. } => {
                assert!(provider_metadata.is_none(), "un appel en echec ne se reprend pas");
            }
            other => panic!("attendu Reasoning, obtenu {other:?}"),
        }
    }

    #[test]
    fn un_raisonnement_vide_est_filtre() {
        let msg = with_content(
            assistant_msg("openai", "gpt-4"),
            vec![AssistantContent::Reasoning { id: "r1".into(), text: String::new(), provider_metadata: None, time: None }],
        );
        assert!(to_llm_messages(&msg, &target()).is_empty());
    }

    #[test]
    fn un_shell_ne_produit_aucun_message() {
        let msg = Message::Shell(crate::schema::session_message::Shell::new("msg_1", "c1", "ls", "a", 1));
        assert!(to_llm_messages(&msg, &target()).is_empty());
    }

    #[test]
    fn un_message_utilisateur_porte_son_texte_et_ses_fichiers() {
        let msg = Message::User(User {
            base: MessageBase::new("msg_1", 1),
            prompt: Prompt {
                text: "que vois-tu ?".into(),
                files: vec![crate::schema::session_message::FileAttachment { path: "a.png".into(), mime: Some("image/png".into()) }],
                agents: vec![],
            },
        });
        let out = to_llm_messages(&msg, &target());
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0].content[0], ContentPart::Text { text } if text == "que vois-tu ?"));
        assert!(matches!(&out[0].content[1], ContentPart::Media { media_type, .. } if media_type.as_deref() == Some("image/png")));
    }

    #[test]
    fn un_systeme_passe_en_role_systeme() {
        let msg = Message::System(System { base: MessageBase::new("msg_1", 1), text: "regle".into() });
        let out = to_llm_messages(&msg, &target());
        assert_eq!(out[0].role, Role::System);
    }

    #[test]
    fn un_outil_en_attente_na_pas_de_resultat() {
        let mut a = Assistant::new("msg_1", 1, "build", ModelRef::new("openai", "gpt-4"));
        a.content.push(AssistantContent::Tool(AssistantTool {
            id: "call_1".into(),
            name: "read".into(),
            provider: None,
            state: ToolState::Pending { input: "{\"path\":\"a.rs\"}".into() },
            time: crate::schema::session_message::AssistantToolTime {
                created: 1,
                ran: None,
                completed: None,
                pruned: None,
            },
        }));
        let out = to_llm_messages(&Message::Assistant(a), &target());
        // Un seul message : l'appel, sans resultat.
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0].content[0], ContentPart::ToolCall { .. }));
    }

    #[test]
    fn un_outil_local_genere_un_message_outil_separe() {
        let mut a = Assistant::new("msg_1", 1, "build", ModelRef::new("openai", "gpt-4"));
        a.content.push(AssistantContent::Tool(AssistantTool {
            id: "call_1".into(),
            name: "read".into(),
            provider: None,
            state: ToolState::Completed {
                input: serde_json::json!({}),
                attachments: None,
                content: vec![crate::schema::session_message::ToolContent::Text { text: "contenu".into() }],
                output_paths: None,
                structured: Value::Null,
                result: None,
            },
            time: crate::schema::session_message::AssistantToolTime {
                created: 1,
                ran: None,
                completed: Some(2),
                pruned: None,
            },
        }));
        let out = to_llm_messages(&Message::Assistant(a), &target());
        // Le message assistant + un message outil distinct.
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].role, Role::Tool);
    }

    #[test]
    fn un_entree_json_invalide_est_transmise_brute() {
        // Perdre l'appel sur un JSON mal forme empecherait le modele de le
        // corriger lui-meme.
        let tool = AssistantTool {
            id: "call_1".into(),
            name: "read".into(),
            provider: None,
            state: ToolState::Pending { input: "pas du json".into() },
            time: crate::schema::session_message::AssistantToolTime {
                created: 1,
                ran: None,
                completed: None,
                pruned: None,
            },
        };
        assert_eq!(tool_input(&tool), Value::String("pas du json".into()));
    }

    #[test]
    fn une_erreur_d_outil_est_typee_comme_telle() {
        let tool = AssistantTool {
            id: "call_1".into(),
            name: "read".into(),
            provider: None,
            state: ToolState::Error {
                input: serde_json::json!({}),
                content: vec![],
                structured: Value::Null,
                error: crate::schema::session_message::UnknownError::new("fichier introuvable"),
                result: None,
            },
            time: crate::schema::session_message::AssistantToolTime {
                created: 1,
                ran: None,
                completed: Some(2),
                pruned: None,
            },
        };
        let r = tool_result(&tool, None).unwrap();
        match r {
            ContentPart::ToolResult { result_type, .. } => assert_eq!(result_type.as_deref(), Some("error")),
            other => panic!("attendu ToolResult, obtenu {other:?}"),
        }
    }
}
