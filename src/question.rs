//! Portage Rust de `opencode/packages/core/src/question.ts`.
//!
//! Service de questions interactives : creation, attente de reponse, rejet
//! et liste des requetes en attente. Utilise des canaux oneshot pour
//! l'attente asynchrone, equivalent aux Deferred d'Effect.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;

/// Identifiant de question, prefixe par "que_".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuestionId(pub String);

impl QuestionId {
    /// Genere un nouvel identifiant croissant (base sur UUID v7).
    pub fn create() -> Self {
        Self(format!("que_{}", Uuid::now_v7().as_u128()))
    }

    /// Cree un identifiant a partir d'une chaine existante.
    pub fn from_string(id: String) -> Self {
        Self(id)
    }
}

impl fmt::Display for QuestionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Option de reponse pour une question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Option_ {
    #[serde(rename = "label")]
    pub label: String,

    #[serde(rename = "description")]
    pub description: String,
}

/// Informations completement decrites pour poser une question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    #[serde(rename = "question")]
    pub question: String,

    #[serde(rename = "header")]
    pub header: String,

    #[serde(rename = "options")]
    pub options: Vec<Option_>,

    #[serde(rename = "multiple", skip_serializing_if = "std::option::Option::is_none")]
    pub multiple: Option<bool>,

    #[serde(rename = "custom", skip_serializing_if = "std::option::Option::is_none")]
    pub custom: Option<bool>,
}

/// Prompt minimal pour une question (sans champ custom).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prompt {
    #[serde(rename = "question")]
    pub question: String,

    #[serde(rename = "header")]
    pub header: String,

    #[serde(rename = "options")]
    pub options: Vec<Option_>,

    #[serde(rename = "multiple", skip_serializing_if = "std::option::Option::is_none")]
    pub multiple: Option<bool>,
}

/// Reference a un appel d'outil ayant declenche la question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "messageID")]
    pub message_id: String,

    #[serde(rename = "callID")]
    pub call_id: String,
}

/// Requete de question complete avec identifiant et session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    #[serde(rename = "id")]
    pub id: QuestionId,

    #[serde(rename = "sessionID")]
    pub session_id: String,

    #[serde(rename = "questions")]
    pub questions: Vec<Info>,

    #[serde(rename = "tool", skip_serializing_if = "std::option::Option::is_none")]
    pub tool: Option<Tool>,
}

/// Reponse utilisateur : tableau de labels selectionnes par question.
pub type Answer = Vec<String>;

/// Conteneur de reponses pour une requete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    #[serde(rename = "answers")]
    pub answers: Vec<Answer>,
}

/// Evenements emis par le service de questions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    #[serde(rename = "question.v2.asked")]
    Asked { request: Request },

    #[serde(rename = "question.v2.replied")]
    Replied {
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "requestID")]
        request_id: QuestionId,
        answers: Vec<Answer>,
    },

    #[serde(rename = "question.v2.rejected")]
    Rejected {
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "requestID")]
        request_id: QuestionId,
    },
}

/// Erreur : l'utilisateur a rejete la question.
#[derive(Debug, Clone, thiserror::Error)]
#[error("L'utilisateur a rejete cette question")]
pub struct RejectedError;

/// Erreur : requete introuvable.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("Requete introuvable : {request_id}")]
pub struct NotFoundError {
    #[serde(rename = "requestID")]
    pub request_id: QuestionId,
}

/// Entree pour poser une nouvelle question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskInput {
    #[serde(rename = "sessionID")]
    pub session_id: String,

    #[serde(rename = "questions")]
    pub questions: Vec<Info>,

    #[serde(rename = "tool", skip_serializing_if = "std::option::Option::is_none")]
    pub tool: Option<Tool>,
}

/// Entree pour repondre a une question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyInput {
    #[serde(rename = "requestID")]
    pub request_id: QuestionId,

    #[serde(rename = "answers")]
    pub answers: Vec<Answer>,
}

/// Interface du service de questions.
#[async_trait]
pub trait QuestionService: Send + Sync {
    /// Pose une question et attend la reponse.
    async fn ask(&self, input: AskInput) -> Result<Vec<Answer>, RejectedError>;

    /// Repond a une question en attente.
    async fn reply(&self, input: ReplyInput) -> Result<(), NotFoundError>;

    /// Rejete une question en attente.
    async fn reject(&self, request_id: QuestionId) -> Result<(), NotFoundError>;

    /// Liste toutes les requetes en attente.
    async fn list(&self) -> Vec<Request>;
}

/// Requete en attente avec son canal de reponse.
struct Pending {
    request: Request,
    sender: oneshot::Sender<Result<Vec<Answer>, RejectedError>>,
}

/// Implementation concrete du service de questions.
#[derive(Clone)]
pub struct QuestionServiceImpl {
    pending: Arc<Mutex<HashMap<QuestionId, Pending>>>,
    event_sender: tokio::sync::broadcast::Sender<Event>,
}

impl QuestionServiceImpl {
    /// Cree un nouveau service avec un canal d'evenements.
    pub fn new(event_sender: tokio::sync::broadcast::Sender<Event>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
        }
    }

    /// Envoie un evenement si des abonnes existent.
    async fn publish(&self, event: Event) {
        let _ = self.event_sender.send(event);
    }
}

#[async_trait]
impl QuestionService for QuestionServiceImpl {
    async fn ask(&self, input: AskInput) -> Result<Vec<Answer>, RejectedError> {
        let id = QuestionId::create();
        let (tx, rx) = oneshot::channel();

        let request = Request {
            id: id.clone(),
            session_id: input.session_id.clone(),
            questions: input.questions,
            tool: input.tool,
        };

        {
            let mut pending = self.pending.lock().await;
            pending.insert(id.clone(), Pending { request: request.clone(), sender: tx });
        }

        self.publish(Event::Asked { request }).await;

        // Attend la reponse ou le rejet.
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(RejectedError),
        }
    }

    async fn reply(&self, input: ReplyInput) -> Result<(), NotFoundError> {
        let pending = {
            let mut pending = self.pending.lock().await;
            pending.remove(&input.request_id)
        };

        let Some(pending_item) = pending else {
            return Err(NotFoundError { request_id: input.request_id });
        };

        self.publish(Event::Replied {
            session_id: pending_item.request.session_id.clone(),
            request_id: pending_item.request.id.clone(),
            answers: input.answers.clone(),
        })
        .await;

        let _ = pending_item.sender.send(Ok(input.answers));
        Ok(())
    }

    async fn reject(&self, request_id: QuestionId) -> Result<(), NotFoundError> {
        let pending = {
            let mut pending = self.pending.lock().await;
            pending.remove(&request_id)
        };

        let Some(pending_item) = pending else {
            return Err(NotFoundError { request_id });
        };

        self.publish(Event::Rejected {
            session_id: pending_item.request.session_id.clone(),
            request_id: pending_item.request.id.clone(),
        })
        .await;

        let _ = pending_item.sender.send(Err(RejectedError));
        Ok(())
    }

    async fn list(&self) -> Vec<Request> {
        let pending = self.pending.lock().await;
        pending.values().map(|p| p.request.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info() -> Info {
        Info {
            question: "Choisissez une option".to_string(),
            header: "Options".to_string(),
            options: vec![
                Option_ { label: "A".to_string(), description: "Premier choix".to_string() },
                Option_ { label: "B".to_string(), description: "Second choix".to_string() },
            ],
            multiple: Some(false),
            custom: Some(true),
        }
    }

    fn make_ask_input() -> AskInput {
        AskInput {
            session_id: "ses_test".to_string(),
            questions: vec![make_info()],
            tool: None,
        }
    }

    #[tokio::test]
    async fn liste_vide_au_demarrage() {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let service = QuestionServiceImpl::new(tx);
        let list = service.list().await;
        assert!(list.is_empty(), "La liste doit etre vide au demarrage");
    }

    #[tokio::test]
    async fn une_seule_question_ajoutee_est_listee() {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let service = QuestionServiceImpl::new(tx.clone());

        let input = make_ask_input();
        let service_clone = service.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let requests = service_clone.list().await;
            if let Some(req) = requests.first() {
                let _ = service_clone.reply(ReplyInput {
                    request_id: req.id.clone(),
                    answers: vec![vec!["A".to_string()]],
                }).await;
            }
        });

        let _ = service.ask(input).await;
        let list = service.list().await;
        assert!(list.is_empty(), "La liste doit etre vide apres reponse");
    }

    #[tokio::test]
    async fn rejet_retire_la_requete_de_la_liste() {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let service = QuestionServiceImpl::new(tx.clone());

        let input = make_ask_input();
        let service_clone = service.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let requests = service_clone.list().await;
            if let Some(req) = requests.first() {
                let _ = service_clone.reject(req.id.clone()).await;
            }
        });

        let result = service.ask(input).await;
        assert!(result.is_err(), "Le rejet doit retourner une erreur");
        let list = service.list().await;
        assert!(list.is_empty(), "La liste doit etre vide apres rejet");
    }

    #[tokio::test]
    async fn reponses_multiples_ordre_conserve() {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let service = QuestionServiceImpl::new(tx.clone());

        let mut input = make_ask_input();
        input.questions = vec![
            make_info(),
            Info {
                question: "Deuxieme question".to_string(),
                header: "Q2".to_string(),
                options: vec![Option_ { label: "X".to_string(), description: "Choix X".to_string() }],
                multiple: Some(true),
                custom: Some(false),
            },
        ];

        let service_clone = service.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let requests = service_clone.list().await;
            if let Some(req) = requests.first() {
                let _ = service_clone.reply(ReplyInput {
                    request_id: req.id.clone(),
                    answers: vec![vec!["A".to_string()], vec!["X".to_string()]],
                }).await;
            }
        });

        let answers = service.ask(input).await.unwrap();
        assert_eq!(answers.len(), 2, "Deux reponses attendues");
        assert_eq!(answers[0], vec!["A"]);
        assert_eq!(answers[1], vec!["X"]);
    }

    #[tokio::test]
    async fn reponse_a_requete_inexistante_retourne_erreur() {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let service = QuestionServiceImpl::new(tx);

        let result = service.reply(ReplyInput {
            request_id: QuestionId::from_string("que_inexistant".to_string()),
            answers: vec![vec!["A".to_string()]],
        }).await;

        assert!(result.is_err(), "Reponse a ID inconnu doit echouer");
    }

    #[tokio::test]
    async fn rejet_requete_inexistante_retourne_erreur() {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let service = QuestionServiceImpl::new(tx);

        let result = service.reject(QuestionId::from_string("que_inexistant".to_string())).await;

        assert!(result.is_err(), "Rejet d'ID inconnu doit echouer");
    }

    #[test]
    fn serialisation_option_conserve_camelcase() {
        let opt = Option_ { label: "Test".to_string(), description: "Desc".to_string() };
        let json = serde_json::to_string(&opt).unwrap();
        assert!(json.contains("\"label\""));
        assert!(json.contains("\"description\""));
    }

    #[test]
    fn serialisation_info_ignore_champs_none() {
        let info = Info {
            question: "Q".to_string(),
            header: "H".to_string(),
            options: vec![],
            multiple: None,
            custom: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("multiple"));
        assert!(!json.contains("custom"));
    }

    #[test]
    fn serialisation_event_tag_explicite() {
        let event = Event::Asked { request: Request {
            id: QuestionId::create(),
            session_id: "ses".to_string(),
            questions: vec![],
            tool: None,
        }};
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"question.v2.asked\""));
    }
}