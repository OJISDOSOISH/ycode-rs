//! Portage Rust de `opencode/packages/schema/src/session-message.ts`.
//!
//! Le modele de messages d'OpenCode. C'est la brique la plus fundamentale du
//! projet : session, outils, compaction, tout l.](https://docs.rs) repose dessus.
//!
//! Le TypeScript d'origine utilise `effect/Schema` avec des unions taggees
//! (`type: "user"`, `type: "assistant"`...). En Rust, ce pattern devient un
//! `enum` avec un discriminant, ce qui est plus strict : le compilateur impose
//! d couvrir chaque variante, la ou le TS laissait la possibilite d'oublier un
//! `case` et de laisser un message mal gere passer en silence.
//!
//! Les noms de champs restent identiques au TS d'origine, volontairement.
//! C'est ce qui permet de relire les deux versions cote a cote, et de verifier
//! qu'on n'a rien perdu en portant.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ------------------------------------------------------------------ identifiants

/// Identifiant de message, prefixe `msg_`.
///
/// En TS : `Schema.String.check(Schema.isStartsWith("msg_")).pipe(Schema.brand(...))`.
/// Le "brand" du TS est une verification de type a la compilation, absente en
/// Rust. On garde le prefixe comme invariant et on l'impose a la construction.
pub const MSG_ID_PREFIX: &str = "msg_";

pub fn is_valid_msg_id(id: &str) -> bool {
    id.starts_with(MSG_ID_PREFIX)
}

/// Horodatage en millisecondes depuis l'epoque Unix.
///
/// Le TS utilise `DateTimeUtcFromMillis`. On garde les millisecondes plutot que
/// de convertir en `SystemTime` : le stockage est en JSON, et l'aller-retour
/// JSON/millisecondes doit etre sans perte.
pub type Millis = i64;

// ------------------------------------------------------------------ erreurs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownError {
    #[serde(rename = "type")]
    pub kind: String, // toujours "unknown"
    pub message: String,
}

impl UnknownError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { kind: "unknown".to_string(), message: message.into() }
    }
}

// ------------------------------------------------------------------ metadonnees de provider

/// Metadonnees brutes renvoyees par un provider (OpenRouter, NVIDIA, ...).
///
/// Volontairement non type : ces structures changent constamme et varient d'un
/// provider a l'autre. Forcer un schema ici casserait a chaque nouvelle API.
pub type ProviderMetadata = Value;

// ------------------------------------------------------------------ prompt et fichiers

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAttachment {
    pub path: String,
    pub mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Prompt {
    pub text: String,
    pub files: Vec<FileAttachment>,
    pub agents: Vec<String>,
}

// ------------------------------------------------------------------ modele

/// Reference de modele (Model.Ref du TS) : utilisee quand la session change
/// de modele ou quand un assistant repond.
///
/// Forme canonique `{id, providerID, variant?}`, identique a celle de
/// core/session/schema.rs. L'ancienne forme `{provider, model}` n'existe
/// dans aucun fichier TS : elle est supprimee, pas migree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl ModelRef {
    pub fn new(id: impl Into<String>, provider_id: impl Into<String>) -> Self {
        Self { id: id.into(), provider_id: provider_id.into(), variant: None }
    }
}

// ------------------------------------------------------------------ contenu d'outil

/// Contenu produit par un appel d'outil : texte, ou image, ou donnees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    Text { text: String },
    Image { mime: String, data: String },
    Json { value: Value },
    Other { value: Value },
}

// ------------------------------------------------------------------ etat d'un appel d'outil

/// Etat d'avancement d'un appel d'outil.
///
/// Le TS definit quatre structs et les fusionne avec `toTaggedUnion("status")`.
/// Meme chose ici. `#[serde(tag = "status")]` produit exactement le meme JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ToolState {
    Pending {
        input: String,
    },
    Running {
        input: Value,
        structured: Value,
        content: Vec<ToolContent>,
    },
    Completed {
        input: Value,
        attachments: Option<Vec<FileAttachment>>,
        content: Vec<ToolContent>,
        output_paths: Option<Vec<String>>,
        structured: Value,
        result: Option<Value>,
    },
    Error {
        input: Value,
        content: Vec<ToolContent>,
        structured: Value,
        error: UnknownError,
        result: Option<Value>,
    },
}

impl ToolState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, ToolState::Completed { .. } | ToolState::Error { .. })
    }

    pub fn status_str(&self) -> &'static str {
        match self {
            ToolState::Pending { .. } => "pending",
            ToolState::Running { .. } => "running",
            ToolState::Completed { .. } => "completed",
            ToolState::Error { .. } => "error",
        }
    }
}

// ------------------------------------------------------------------ champs de base

/// Champs presents sur tous les messages.
///
/// `time` est `Option` parce que le TypeScript d'origine redefinit `time` dans
/// certains messages : `Assistant` a un `time { created, completed }` qui
/// **remplace** le `time { created }` herite de `Base`. En JavaScript, un objet
/// litteral avec deux cles `time` conserve la derniere. Rust, lui, refuse deux
/// champs identiques au meme niveau quand on `flatten`, d'ou le `Option` : le
/// message Assistant porte son `time` riche a part, et on laisse `base.time` a
/// `None` pour lui.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageBase {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<MessageTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageTime {
    pub created: Millis,
}

impl MessageBase {
    pub fn new(id: impl Into<String>, created: Millis) -> Self {
        Self { id: id.into(), metadata: None, time: Some(MessageTime { created }) }
    }

    /// Base sans `time`, pour les messages qui en red definissent un plus riche
    /// (voir `Assistant`).
    pub fn without_time(id: impl Into<String>) -> Self {
        Self { id: id.into(), metadata: None, time: None }
    }
}

// ------------------------------------------------------------------ messages

/// Changement d'agent en cours de session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSwitched {
    #[serde(flatten)]
    pub base: MessageBase,
    pub agent: String,
}


/// Changement de modele en cours de session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSwitched {
    #[serde(flatten)]
    pub base: MessageBase,
    pub model: ModelRef,
}


/// Message de l'utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    #[serde(flatten)]
    pub base: MessageBase,
    #[serde(flatten)]
    pub prompt: Prompt,
}


/// Message genere par le systeme, pas par un humain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Synthetic {
    #[serde(flatten)]
    pub base: MessageBase,
    /// Le TS nomme ce champ `sessionID` (camelCase). Sans le `rename`, Serde
    /// emettrait `session_id` et une session ecrite par le TS serait illisible.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub text: String,
}


/// Message de niveau systeme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct System {
    #[serde(flatten)]
    pub base: MessageBase,
    pub text: String,
}


/// Resultat d'une commande shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shell {
    #[serde(flatten)]
    pub base: MessageBase,
    /// `callID` dans le TS, pas `call_id`. Meme raison que `Synthetic.session_id`.
    #[serde(rename = "callID")]
    pub call_id: String,
    pub command: String,
    pub output: String,
    /// Redefinit le `time` herite de `base`, comme dans le TS. Le `base.time`
    /// doit donc rester `None` ici, sinon deux `time` se serialisent.
    pub time: ShellTime,
}

impl Shell {
    /// `base.time` est laisse a `None` : le `time` riche du shell le remplace.
    pub fn new(
        id: impl Into<String>,
        call_id: impl Into<String>,
        command: impl Into<String>,
        output: impl Into<String>,
        created: Millis,
    ) -> Self {
        Self {
            base: MessageBase::without_time(id),
            call_id: call_id.into(),
            command: command.into(),
            output: output.into(),
            time: ShellTime { created, completed: None },
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellTime {
    pub created: Millis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Millis>,
}

// ------------------------------------------------------------------ contenu assistant

/// Contenu produit par le modele : texte, raisonnement, ou appel d'outil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AssistantContent {
    Text {
        id: String,
        text: String,
    },
    Reasoning {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<ProviderMetadata>,
        #[serde(skip_serializing_if = "Option::is_none")]
        time: Option<AssistantContentTime>,
    },
    Tool(AssistantTool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantContentTime {
    pub created: Millis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Millis>,
}

/// Appel d'outil emis par le modele.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantTool {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AssistantToolProvider>,
    pub state: ToolState,
    pub time: AssistantToolTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantToolProvider {
    pub executed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProviderMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_metadata: Option<ProviderMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantToolTime {
    pub created: Millis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ran: Option<Millis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Millis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pruned: Option<Millis>,
}

// ------------------------------------------------------------------ message assistant

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assistant {
    #[serde(flatten)]
    pub base: MessageBase,
    pub agent: String,
    pub model: ModelRef,
    pub content: Vec<AssistantContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Snapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UnknownError>,
    pub time: AssistantTime,
}

impl Assistant {
    /// Construit un message assistant. `base.time` est laisse a `None` : le
    /// `time` riche de l'assistant le remplace, comme dans le TS.
    pub fn new(id: impl Into<String>, created: Millis, agent: impl Into<String>, model: ModelRef) -> Self {
        Self {
            base: MessageBase::without_time(id),
            agent: agent.into(),
            model,
            content: vec![],
            snapshot: None,
            finish: None,
            cost: None,
            tokens: None,
            error: None,
            time: AssistantTime { created, completed: None },
        }
    }
}


/// Bornes du snapshot de fichiers captures avant/apres une modification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: f64,
    pub output: f64,
    pub reasoning: f64,
    pub cache: TokenCache,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenCache {
    pub read: f64,
    pub write: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantTime {
    pub created: Millis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Millis>,
}

// ------------------------------------------------------------------ compaction

/// Resume de contexte. C'est ce qui permet de depasser la fenetre de contexte
/// du modele : quand ca overflow, on remplace l'historique par un resume.
///
/// `recent` garde les derniers echanges verbatim pour que le modele ne perde
/// pas le fil immediat, quand `summary` porte le reste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Compaction {
    #[serde(flatten)]
    pub base: MessageBase,
    pub reason: CompactionReason,
    pub summary: String,
    pub recent: String,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactionReason {
    /// Declenchee automatiquement par depassement de contexte.
    Auto,
    /// Declenchee a la demande de l'utilisateur.
    Manual,
}

// ------------------------------------------------------------------ union principale

/// Tout message de session.
///
/// Le TS : `Schema.Union([...]).pipe(Schema.toTaggedUnion("type"))`.
/// Chaque struct porte son `kind`, et cet enum les regroupe. Le nom de champ
/// TS `type` devient le nom de champ `kind` ici, parce que `type` est un mot
/// cle en Rust et Serde ne peut pas le nommer directement a l'interieur d'une
/// struct ; le `#[serde(rename = "type")]` sur chaque variante restitue le
/// format d'origine au JSON.
///
/// Chaque variante porte un `rename` explicite, sa valeur de tag, parce que
/// l'enum externe `tag = "type"` ne peut pas deduire un tag kebab-case d'un nom
/// de variante PascalCase. Sans ca, Serde attendrait `"Assistant"` et non
/// `"assistant"`, et les sessions deja serialisees par le TS seraient
/// illisibles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "agent-switched")]
    AgentSwitched(AgentSwitched),
    #[serde(rename = "model-switched")]
    ModelSwitched(ModelSwitched),
    #[serde(rename = "user")]
    User(User),
    #[serde(rename = "synthetic")]
    Synthetic(Synthetic),
    #[serde(rename = "system")]
    System(System),
    #[serde(rename = "shell")]
    Shell(Shell),
    #[serde(rename = "assistant")]
    Assistant(Assistant),
    #[serde(rename = "compaction")]
    Compaction(Compaction),
}

impl Message {
    pub fn base(&self) -> &MessageBase {
        match self {
            Message::AgentSwitched(m) => &m.base,
            Message::ModelSwitched(m) => &m.base,
            Message::User(m) => &m.base,
            Message::Synthetic(m) => &m.base,
            Message::System(m) => &m.base,
            Message::Shell(m) => &m.base,
            Message::Assistant(m) => &m.base,
            Message::Compaction(m) => &m.base,
        }
    }

    pub fn id(&self) -> &str {
        &self.base().id
    }

    /// Le discriminant, tel qu'il apparait dans le JSON.
    pub fn type_str(&self) -> &'static str {
        match self {
            Message::AgentSwitched(_) => "agent-switched",
            Message::ModelSwitched(_) => "model-switched",
            Message::User(_) => "user",
            Message::Synthetic(_) => "synthetic",
            Message::System(_) => "system",
            Message::Shell(_) => "shell",
            Message::Assistant(_) => "assistant",
            Message::Compaction(_) => "compaction",
        }
    }

    /// Un message ajoute-t-il du contexte a la fenetre du modele ?
    ///
    /// Non pour `Shell` : la sortie d'une commande a deja ete injectee dans le
    /// contexte d'un message `Tool`, la compter deux fois ferait exploser
    /// l'utilisation de contexte pour rien. Non plus pour `Compaction`, qui par
    /// definition *reduit* ce qui precede.
    pub fn counts_against_context(&self) -> bool {
        !matches!(self, Message::Shell(_) | Message::Compaction(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> MessageBase {
        MessageBase::new("msg_test0001", 1_700_000_000_000)
    }

    #[test]
    fn un_message_user_se_serialise_comme_dans_le_ts() {
        let msg = Message::User(User {
            base: base(),
            prompt: Prompt {
                text: "bonjour".to_string(),
                files: vec![],
                agents: vec![],
            },
        });

        let json = serde_json::to_value(&msg).unwrap();

        // Le discriminant doit s'appeler "type" comme en TypeScript.
        assert_eq!(json["type"], "user");
        assert_eq!(json["text"], "bonjour");
        assert_eq!(json["id"], "msg_test0001");
        assert_eq!(json["time"]["created"], 1_700_000_000_000i64);
    }

    #[test]
    fn le_json_se_deserialise_vers_la_bonne_variante() {
        let json = serde_json::json!({
            "type": "assistant",
            "id": "msg_test0002",
            "time": { "created": 100, "completed": 200 },
            "agent": "build",
            "model": { "id": "claude-sonnet-4-5", "providerID": "opencode" },
            "content": []
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match &msg {
            Message::Assistant(a) => {
                assert_eq!(a.agent, "build");
                assert_eq!(a.model.id, "claude-sonnet-4-5");
                assert_eq!(a.time.created, 100);
                assert_eq!(a.time.completed, Some(200));
                // Le `time` de base est laisse vide : l'assistant porte le sien.
                assert!(a.base.time.is_none());
            }
            other => panic!("mauvaise variante : {}", other.type_str()),
        }
    }

    #[test]
    fn un_assistant_ne_serialise_qu_un_seul_time() {
        // Regression : le TS a deux cles `time`, JS garde la derniere. Si on
        // laissait les deux se serialiser, le JSON produit aurait un `time` en
        // double et ne serait plus relisible par le TS d'origine.
        let msg = Message::Assistant(Assistant::new(
            "msg_test0003",
            10,
            "build",
            ModelRef::new("claude-sonnet-4-5", "opencode"),
        ));
        let v = serde_json::to_value(&msg).unwrap();

        // On recompte la cle pour detecter un doublon.
        let time_keys = msg.base().time.is_some() as u8 + 1;
        assert_eq!(time_keys, 1, "l'assistant doit n'exposer qu'un seul time");
        assert_eq!(v["time"]["created"], 10);
        assert!(v.get("time").is_some());
    }

    #[test]
    fn shell_ne_compte_pas_contre_le_contexte() {
        let shell = Message::Shell(Shell::new("msg_s", "call_1", "ls", "a", 1));
        assert!(!shell.counts_against_context());

        let user = Message::User(User {
            base: base(),
            prompt: Prompt::default(),
        });
        assert!(user.counts_against_context());
    }

    #[test]
    fn un_etat_d_outil_termine_est_reconnu() {
        let pending = ToolState::Pending { input: "{}".to_string() };
        assert!(!pending.is_terminal());
        assert_eq!(pending.status_str(), "pending");

        let done = ToolState::Completed {
            input: serde_json::json!({}),
            attachments: None,
            content: vec![ToolContent::Text { text: "ok".into() }],
            output_paths: None,
            structured: serde_json::json!({}),
            result: None,
        };
        assert!(done.is_terminal());
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Regression : Serde emettait `session_id` et `call_id` en snake_case,
        // alors que le TS ecrit `sessionID` et `callID`. Une session produite
        // par l'un n'etait donc pas relisible par l'autre.
        let syn = Message::Synthetic(Synthetic {
            base: MessageBase::new("msg_s1", 1),
            session_id: "ses_1".to_string(),
            text: "x".to_string(),
        });
        let v = serde_json::to_value(&syn).unwrap();
        assert_eq!(v["sessionID"], "ses_1", "le champ doit s'appeler sessionID");
        assert!(v.get("session_id").is_none(), "snake_case interdit");

        let sh = Message::Shell(Shell::new("msg_h1", "call_9", "ls", "a", 5));
        let v = serde_json::to_value(&sh).unwrap();
        assert_eq!(v["callID"], "call_9", "le champ doit s'appeler callID");
        assert!(v.get("call_id").is_none(), "snake_case interdit");
    }

    #[test]
    fn un_shell_ne_serialise_qu_un_seul_time() {
        // Comme l'assistant, le shell redéfinit `time`. Si `base.time` restait
        // renseigne, le JSON aurait deux cles `time`.
        let sh = Message::Shell(Shell::new("msg_h2", "call_10", "ls", "a", 7));
        assert!(sh.base().time.is_none());
        let json = serde_json::to_string(&sh).unwrap();
        assert_eq!(json.matches("\"time\"").count(), 1, "un seul time attendu");
    }

    #[test]
    fn un_shell_se_deserialise_depuis_le_json_du_typescript() {
        // Le vrai test de compatibilite : du JSON ecrit a la main comme le TS
        // le ferait, relu par le portage Rust.
        let json = serde_json::json!({
            "type": "shell",
            "id": "msg_h3",
            "callID": "call_abc",
            "command": "cargo test",
            "output": "ok",
            "time": { "created": 42 }
        });
        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Shell(s) => {
                assert_eq!(s.call_id, "call_abc");
                assert_eq!(s.command, "cargo test");
                assert_eq!(s.time.created, 42);
            }
            other => panic!("mauvaise variante : {}", other.type_str()),
        }
    }
}




