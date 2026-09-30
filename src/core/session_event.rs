//! Portage Rust de `opencode/packages/schema/src/session-event.ts`.
//!
//! Contrat seul, sans logique : chaque `Event.define` du TS devient une
//! struct de donnees `XxxData`, plus une enveloppe generique
//! `EventEnvelope<D>` qui porte les champs communs (`id`, `metadata`,
//! `durable`, `location`, `data`). Les unions `Durable` et `All` du TS
//! deviennent deux enums tagges sur `type` avec un `rename` explicite par
//! variante, comme `Schema.toTaggedUnion("type")`.
//!
//! Conventions appliquees :
//! - tout champ `camelCase` porte un `rename` explicite (`sessionID`,
//!   `messageID`, `assistantMessageID`, `callID`, `textID`, `reasoningID`,
//!   `aggregateID`, `providerID`, `workspaceID`, `partID`, `isRetryable`,
//!   `statusCode`, `responseHeaders`, `responseBody`, `outputPaths`,
//!   `providerMetadata`). Verifie deux fois : `ID` reste en majuscules,
//!   jamais `Id`.
//! - tout champ optionnel du TS est un `Option` avec `skip_serializing_if`.
//! - `DateTimeUtcFromMillis` vaut `i64`, millisecondes depuis l epoch.
//! - `Schema.Finite` vaut `f64`, `NonNegativeInt` vaut `u64`.
//! - `Record(String, Unknown)` vaut `BTreeMap<String, serde_json::Value>`.
//! - les fragments live seuls (`Text.Delta`, `Reasoning.Delta`,
//!   `Tool.Input.Delta`, `Compaction.Delta`) sont dans `SessionEvent`
//!   mais pas dans `SessionDurableEvent`, comme dans le TS.
//! - `Source` local (start/end en `u64`) ne doit pas etre confondu avec
//!   `Prompt.Source` (start/end en `f64`) : ici `EventSource` et
//!   `PromptSource` sont deux structs distinctes.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Identifiant de session (`ses_...` en TS).
pub type SessionId = String;
/// Identifiant de message (`msg_...` en TS).
pub type MessageId = String;
/// Identifiant d evenement (`evt_...` en TS).
pub type EventId = String;
/// Chemin relatif, marque opaque en TS.
pub type RelativePath = String;

/// Plage de texte propre a ce fichier (`session.next.event.source`).
///
/// Attention : `start` et `end` sont des entiers non negatifs ici,
/// pas des `f64` comme dans `Prompt.Source`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSource {
    pub start: u64,
    pub end: u64,
    pub text: String,
}

/// Reference compacte vers un modele, miroir de `Model.Ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Reference de localisation, miroir de `Location.Ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationRef {
    pub directory: String,
    #[serde(rename = "workspaceID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// Plage source d une piece jointe, miroir de `Prompt.Source` (en `f64`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSource {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Piece jointe de fichier, miroir de `Prompt.FileAttachment`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAttachment {
    pub uri: String,
    pub mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PromptSource>,
}

/// Piece jointe d agent, miroir de `Prompt.AgentAttachment`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentAttachment {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PromptSource>,
}

/// Invite utilisateur, miroir de `Prompt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<AgentAttachment>>,
}

/// Metadonnees fournisseur : double dictionnaire, miroir de `ProviderMetadata`.
pub type ProviderMetadata = BTreeMap<String, BTreeMap<String, serde_json::Value>>;

/// Contenu d outil, miroir de `ToolContent` (union taggee sur `type`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "file")]
    File {
        uri: String,
        mime: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

/// Erreur inconnue, miroir de `SessionMessage.UnknownError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Fin d info durable d enveloppe (`aggregateID`, `seq`, `version`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDurableInfo {
    #[serde(rename = "aggregateID")]
    pub aggregate_id: String,
    pub seq: i64,
    pub version: i64,
}

/// Enveloppe commune a tous les evenements, miroir de `Event.Payload`.
///
/// Le champ `type` n est pas ici : il est porte par les enums `SessionEvent`
/// et `SessionDurableEvent` via `#[serde(tag = "type")]`, ce qui reproduit
/// le `toTaggedUnion("type")` du TS a la serialisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope<D> {
    pub id: EventId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durable: Option<EventDurableInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationRef>,
    pub data: D,
}

/// Mode de livraison de l invite, miroir de `Delivery`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delivery {
    #[serde(rename = "steer")]
    Steer,
    #[serde(rename = "queue")]
    Queue,
}

/// Raison de compaction, litteraux `"auto"` et `"manual"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionReason {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "manual")]
    Manual,
}

/// Donnees de `session.next.agent.switched`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSwitchedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub agent: String,
}

/// Donnees de `session.next.model.switched`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSwitchedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub model: ModelRef,
}

/// Donnees de `session.next.moved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub location: LocationRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdirectory: Option<RelativePath>,
}

/// Champs communs a `Prompted` et `PromptAdmitted`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptFieldsData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub prompt: Prompt,
    pub delivery: Delivery,
}

/// Donnees de `session.next.prompted` (meme schema que `PromptAdmitted`).
pub type PromptedData = PromptFieldsData;
/// Donnees de `session.next.prompt.admitted` (meme schema que `Prompted`).
pub type PromptAdmittedData = PromptFieldsData;

/// Donnees de `session.next.context.updated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUpdatedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub text: String,
}

/// Donnees de `session.next.synthetic` (meme schema que `ContextUpdated`).
pub type SyntheticData = ContextUpdatedData;

/// Donnees de `session.next.shell.started`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellStartedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub command: String,
}

/// Donnees de `session.next.shell.ended`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellEndedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub output: String,
}

/// Donnees de `session.next.step.started`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepStartedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    pub agent: String,
    pub model: ModelRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

/// Cache de tokens d un pas termine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepTokensCache {
    pub read: f64,
    pub write: f64,
}

/// Compteurs de tokens d un pas termine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepTokens {
    pub input: f64,
    pub output: f64,
    pub reasoning: f64,
    pub cache: StepTokensCache,
}

/// Donnees de `session.next.step.ended` (version durable 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepEndedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    pub finish: String,
    pub cost: f64,
    pub tokens: StepTokens,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<RelativePath>>,
}

/// Donnees de `session.next.step.failed` (version durable 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepFailedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    pub error: UnknownError,
}

/// Donnees de `session.next.text.started`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStartedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "textID")]
    pub text_id: String,
}

/// Donnees de `session.next.text.delta` (live seul, non durable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextDeltaData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "textID")]
    pub text_id: String,
    pub delta: String,
}

/// Donnees de `session.next.text.ended`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEndedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "textID")]
    pub text_id: String,
    pub text: String,
}

/// Donnees de `session.next.reasoning.started`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningStartedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "reasoningID")]
    pub reasoning_id: String,
    #[serde(rename = "providerMetadata")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<ProviderMetadata>,
}

/// Donnees de `session.next.reasoning.delta` (live seul, non durable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningDeltaData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "reasoningID")]
    pub reasoning_id: String,
    pub delta: String,
}

/// Donnees de `session.next.reasoning.ended`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningEndedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "reasoningID")]
    pub reasoning_id: String,
    pub text: String,
    #[serde(rename = "providerMetadata")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<ProviderMetadata>,
}

/// Donnees de `session.next.tool.input.started`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInputStartedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub name: String,
}

/// Donnees de `session.next.tool.input.delta` (live seul, non durable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInputDeltaData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub delta: String,
}

/// Donnees de `session.next.tool.input.ended`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInputEndedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub text: String,
}

/// Bloc fournisseur d un appel d outil (`executed` plus `metadata`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolProviderInfo {
    pub executed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProviderMetadata>,
}

/// Donnees de `session.next.tool.called`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCalledData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub tool: String,
    pub input: BTreeMap<String, serde_json::Value>,
    pub provider: ToolProviderInfo,
}

/// Donnees de `session.next.tool.progress`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolProgressData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub structured: BTreeMap<String, serde_json::Value>,
    pub content: Vec<ToolContent>,
}

/// Donnees de `session.next.tool.success`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSuccessData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub structured: BTreeMap<String, serde_json::Value>,
    pub content: Vec<ToolContent>,
    #[serde(rename = "outputPaths")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    pub provider: ToolProviderInfo,
}

/// Donnees de `session.next.tool.failed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolFailedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub error: UnknownError,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    pub provider: ToolProviderInfo,
}

/// Erreur rejouable avec contexte HTTP, miroir de `RetryError`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryError {
    pub message: String,
    #[serde(rename = "statusCode")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<f64>,
    #[serde(rename = "isRetryable")]
    pub is_retryable: bool,
    #[serde(rename = "responseHeaders")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<BTreeMap<String, String>>,
    #[serde(rename = "responseBody")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Donnees de `session.next.retried`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetriedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub attempt: f64,
    pub error: RetryError,
}

/// Donnees de `session.next.compaction.started`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionStartedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub reason: CompactionReason,
}

/// Donnees de `session.next.compaction.delta` (live seul, non durable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionDeltaData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub text: String,
}

/// Donnees de `session.next.compaction.ended`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionEndedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub reason: CompactionReason,
    pub text: String,
    pub recent: String,
}

/// Statut d un diff de fichier, miroir de `Revert.FileDiff.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileDiffStatus {
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "modified")]
    Modified,
    #[serde(rename = "deleted")]
    Deleted,
}

/// Diff de fichier, miroir de `Revert.FileDiff`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: RelativePath,
    pub status: FileDiffStatus,
    pub additions: u64,
    pub deletions: u64,
    pub patch: String,
}

/// Etat de retour en arriere, miroir de `Revert.State`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertState {
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "partID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileDiff>>,
}

/// Donnees de `session.next.revert.staged`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertStagedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub revert: RevertState,
}

/// Donnees de `session.next.revert.cleared`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertClearedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
}

/// Donnees de `session.next.revert.committed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertCommittedData {
    pub timestamp: i64,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
}

/// Tous les evenements, miroir de `All` (durable + fragments live).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEvent {
    #[serde(rename = "session.next.agent.switched")]
    AgentSwitched(EventEnvelope<AgentSwitchedData>),
    #[serde(rename = "session.next.model.switched")]
    ModelSwitched(EventEnvelope<ModelSwitchedData>),
    #[serde(rename = "session.next.moved")]
    Moved(EventEnvelope<MovedData>),
    #[serde(rename = "session.next.prompted")]
    Prompted(EventEnvelope<PromptedData>),
    #[serde(rename = "session.next.prompt.admitted")]
    PromptAdmitted(EventEnvelope<PromptAdmittedData>),
    #[serde(rename = "session.next.context.updated")]
    ContextUpdated(EventEnvelope<ContextUpdatedData>),
    #[serde(rename = "session.next.synthetic")]
    Synthetic(EventEnvelope<SyntheticData>),
    #[serde(rename = "session.next.shell.started")]
    ShellStarted(EventEnvelope<ShellStartedData>),
    #[serde(rename = "session.next.shell.ended")]
    ShellEnded(EventEnvelope<ShellEndedData>),
    #[serde(rename = "session.next.step.started")]
    StepStarted(EventEnvelope<StepStartedData>),
    #[serde(rename = "session.next.step.ended")]
    StepEnded(EventEnvelope<StepEndedData>),
    #[serde(rename = "session.next.step.failed")]
    StepFailed(EventEnvelope<StepFailedData>),
    #[serde(rename = "session.next.text.started")]
    TextStarted(EventEnvelope<TextStartedData>),
    #[serde(rename = "session.next.text.delta")]
    TextDelta(EventEnvelope<TextDeltaData>),
    #[serde(rename = "session.next.text.ended")]
    TextEnded(EventEnvelope<TextEndedData>),
    #[serde(rename = "session.next.reasoning.started")]
    ReasoningStarted(EventEnvelope<ReasoningStartedData>),
    #[serde(rename = "session.next.reasoning.delta")]
    ReasoningDelta(EventEnvelope<ReasoningDeltaData>),
    #[serde(rename = "session.next.reasoning.ended")]
    ReasoningEnded(EventEnvelope<ReasoningEndedData>),
    #[serde(rename = "session.next.tool.input.started")]
    ToolInputStarted(EventEnvelope<ToolInputStartedData>),
    #[serde(rename = "session.next.tool.input.delta")]
    ToolInputDelta(EventEnvelope<ToolInputDeltaData>),
    #[serde(rename = "session.next.tool.input.ended")]
    ToolInputEnded(EventEnvelope<ToolInputEndedData>),
    #[serde(rename = "session.next.tool.called")]
    ToolCalled(EventEnvelope<ToolCalledData>),
    #[serde(rename = "session.next.tool.progress")]
    ToolProgress(EventEnvelope<ToolProgressData>),
    #[serde(rename = "session.next.tool.success")]
    ToolSuccess(EventEnvelope<ToolSuccessData>),
    #[serde(rename = "session.next.tool.failed")]
    ToolFailed(EventEnvelope<ToolFailedData>),
    #[serde(rename = "session.next.retried")]
    Retried(EventEnvelope<RetriedData>),
    #[serde(rename = "session.next.compaction.started")]
    CompactionStarted(EventEnvelope<CompactionStartedData>),
    #[serde(rename = "session.next.compaction.delta")]
    CompactionDelta(EventEnvelope<CompactionDeltaData>),
    #[serde(rename = "session.next.compaction.ended")]
    CompactionEnded(EventEnvelope<CompactionEndedData>),
    #[serde(rename = "session.next.revert.staged")]
    RevertStaged(EventEnvelope<RevertStagedData>),
    #[serde(rename = "session.next.revert.cleared")]
    RevertCleared(EventEnvelope<RevertClearedData>),
    #[serde(rename = "session.next.revert.committed")]
    RevertCommitted(EventEnvelope<RevertCommittedData>),
}

/// Evenements rejouables seuls, miroir de `Durable` (sans les 4 deltas live).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionDurableEvent {
    #[serde(rename = "session.next.agent.switched")]
    AgentSwitched(EventEnvelope<AgentSwitchedData>),
    #[serde(rename = "session.next.model.switched")]
    ModelSwitched(EventEnvelope<ModelSwitchedData>),
    #[serde(rename = "session.next.moved")]
    Moved(EventEnvelope<MovedData>),
    #[serde(rename = "session.next.prompted")]
    Prompted(EventEnvelope<PromptedData>),
    #[serde(rename = "session.next.prompt.admitted")]
    PromptAdmitted(EventEnvelope<PromptAdmittedData>),
    #[serde(rename = "session.next.context.updated")]
    ContextUpdated(EventEnvelope<ContextUpdatedData>),
    #[serde(rename = "session.next.synthetic")]
    Synthetic(EventEnvelope<SyntheticData>),
    #[serde(rename = "session.next.shell.started")]
    ShellStarted(EventEnvelope<ShellStartedData>),
    #[serde(rename = "session.next.shell.ended")]
    ShellEnded(EventEnvelope<ShellEndedData>),
    #[serde(rename = "session.next.step.started")]
    StepStarted(EventEnvelope<StepStartedData>),
    #[serde(rename = "session.next.step.ended")]
    StepEnded(EventEnvelope<StepEndedData>),
    #[serde(rename = "session.next.step.failed")]
    StepFailed(EventEnvelope<StepFailedData>),
    #[serde(rename = "session.next.text.started")]
    TextStarted(EventEnvelope<TextStartedData>),
    #[serde(rename = "session.next.text.ended")]
    TextEnded(EventEnvelope<TextEndedData>),
    #[serde(rename = "session.next.reasoning.started")]
    ReasoningStarted(EventEnvelope<ReasoningStartedData>),
    #[serde(rename = "session.next.reasoning.ended")]
    ReasoningEnded(EventEnvelope<ReasoningEndedData>),
    #[serde(rename = "session.next.tool.input.started")]
    ToolInputStarted(EventEnvelope<ToolInputStartedData>),
    #[serde(rename = "session.next.tool.input.ended")]
    ToolInputEnded(EventEnvelope<ToolInputEndedData>),
    #[serde(rename = "session.next.tool.called")]
    ToolCalled(EventEnvelope<ToolCalledData>),
    #[serde(rename = "session.next.tool.progress")]
    ToolProgress(EventEnvelope<ToolProgressData>),
    #[serde(rename = "session.next.tool.success")]
    ToolSuccess(EventEnvelope<ToolSuccessData>),
    #[serde(rename = "session.next.tool.failed")]
    ToolFailed(EventEnvelope<ToolFailedData>),
    #[serde(rename = "session.next.retried")]
    Retried(EventEnvelope<RetriedData>),
    #[serde(rename = "session.next.compaction.started")]
    CompactionStarted(EventEnvelope<CompactionStartedData>),
    #[serde(rename = "session.next.compaction.ended")]
    CompactionEnded(EventEnvelope<CompactionEndedData>),
    #[serde(rename = "session.next.revert.staged")]
    RevertStaged(EventEnvelope<RevertStagedData>),
    #[serde(rename = "session.next.revert.cleared")]
    RevertCleared(EventEnvelope<RevertClearedData>),
    #[serde(rename = "session.next.revert.committed")]
    RevertCommitted(EventEnvelope<RevertCommittedData>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enveloppe_agent() -> EventEnvelope<AgentSwitchedData> {
        EventEnvelope {
            id: "evt_1".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data: AgentSwitchedData {
                timestamp: 1_700_000_000_000,
                session_id: "ses_abc".to_string(),
                message_id: "msg_1".to_string(),
                agent: "plan".to_string(),
            },
        }
    }

    #[test]
    fn un_agent_switched_garde_session_id_et_message_id() {
        let ev = SessionEvent::AgentSwitched(enveloppe_agent());
        let json = serde_json::to_value(&ev).expect("serialisation JSON");
        assert_eq!(json["type"], serde_json::json!("session.next.agent.switched"));
        assert_eq!(json["data"]["sessionID"], serde_json::json!("ses_abc"));
        assert_eq!(json["data"]["messageID"], serde_json::json!("msg_1"));
        assert!(json["data"].get("session_id").is_none());
        assert!(json["data"].get("messageId").is_none());
        let back: SessionEvent = serde_json::from_value(json).expect("retour JSON");
        assert_eq!(back, ev);
    }

    #[test]
    fn un_champ_optionnel_absent_ne_sort_pas_en_json() {
        let data = MovedData {
            timestamp: 1_700_000_000_000,
            session_id: "ses_abc".to_string(),
            location: LocationRef {
                directory: "/tmp/proj".to_string(),
                workspace_id: None,
            },
            subdirectory: None,
        };
        let json = serde_json::to_value(&data).expect("serialisation JSON");
        assert!(json.get("subdirectory").is_none());
        assert!(json["location"].get("workspaceID").is_none());
        let env = EventEnvelope {
            id: "evt_2".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data,
        };
        let top = serde_json::to_value(&env).expect("serialisation enveloppe");
        assert!(top.get("metadata").is_none());
        assert!(top.get("durable").is_none());
        assert!(top.get("location").is_none());
    }

    #[test]
    fn le_tag_delta_texte_choisit_la_bonne_variante() {
        let json = serde_json::json!({
            "id": "evt_3",
            "type": "session.next.text.delta",
            "data": {
                "timestamp": 1_700_000_000_000i64,
                "sessionID": "ses_abc",
                "assistantMessageID": "msg_a",
                "textID": "txt_1",
                "delta": "bonjour"
            }
        });
        let ev: SessionEvent = serde_json::from_value(json).expect("parse delta texte");
        match ev {
            SessionEvent::TextDelta(env) => {
                assert_eq!(env.data.delta, "bonjour");
                assert_eq!(env.data.text_id, "txt_1");
            }
            autre => panic!("mauvaise variante : {:?}", autre),
        }
    }

    #[test]
    fn un_delta_live_passe_en_all_mais_pas_en_durable() {
        let json = serde_json::json!({
            "id": "evt_4",
            "type": "session.next.tool.input.delta",
            "data": {
                "timestamp": 1_700_000_000_000i64,
                "sessionID": "ses_abc",
                "assistantMessageID": "msg_a",
                "callID": "call_1",
                "delta": "{"
            }
        });
        let all: SessionEvent = serde_json::from_value(json.clone()).expect("delta accepte en All");
        match all {
            SessionEvent::ToolInputDelta(_) => {}
            autre => panic!("mauvaise variante : {:?}", autre),
        }
        assert!(serde_json::from_value::<SessionDurableEvent>(json).is_err());
        let fini = serde_json::json!({
            "id": "evt_5",
            "type": "session.next.compaction.ended",
            "data": {
                "timestamp": 1_700_000_000_000i64,
                "sessionID": "ses_abc",
                "messageID": "msg_1",
                "reason": "manual",
                "text": "resume",
                "recent": "fin"
            }
        });
        let durable: SessionDurableEvent = serde_json::from_value(fini).expect("compaction durable");
        match durable {
            SessionDurableEvent::CompactionEnded(env) => assert_eq!(env.data.reason, CompactionReason::Manual),
            autre => panic!("mauvaise variante : {:?}", autre),
        }
    }

    #[test]
    fn une_erreur_rejouable_garde_is_retryable_en_camel_case() {
        let data = RetriedData {
            timestamp: 1_700_000_000_000,
            session_id: "ses_abc".to_string(),
            attempt: 2.0,
            error: RetryError {
                message: " Limite atteinte ".to_string(),
                status_code: Some(429.0),
                is_retryable: true,
                response_headers: None,
                response_body: None,
                metadata: None,
            },
        };
        let json = serde_json::to_value(&data).expect("serialisation JSON");
        assert_eq!(json["error"]["isRetryable"], serde_json::json!(true));
        assert_eq!(json["error"]["statusCode"], serde_json::json!(429.0));
        assert!(json["error"].get("is_retryable").is_none());
        assert!(json["error"].get("responseHeaders").is_none());
        let back: RetriedData = serde_json::from_value(json).expect("retour JSON");
        assert_eq!(back, data);
    }

    #[test]
    fn un_succes_d_outil_garde_output_paths_en_camel_case() {
        let data = ToolSuccessData {
            timestamp: 1_700_000_000_000,
            session_id: "ses_abc".to_string(),
            assistant_message_id: "msg_a".to_string(),
            call_id: "call_9".to_string(),
            structured: BTreeMap::new(),
            content: vec![ToolContent::Text { text: "ok".to_string() }],
            output_paths: Some(vec!["out/log.txt".to_string()]),
            result: None,
            provider: ToolProviderInfo { executed: true, metadata: None },
        };
        let json = serde_json::to_value(&data).expect("serialisation JSON");
        assert_eq!(json["callID"], serde_json::json!("call_9"));
        assert_eq!(json["assistantMessageID"], serde_json::json!("msg_a"));
        assert_eq!(json["outputPaths"], serde_json::json!(["out/log.txt"]));
        assert!(json.get("output_paths").is_none());
        assert!(json.get("callId").is_none());
        assert!(json.get("result").is_none());
        let back: ToolSuccessData = serde_json::from_value(json).expect("retour JSON");
        assert_eq!(back, data);
    }

    #[test]
    fn un_shell_garde_call_id_avec_id_en_majuscules() {
        let data = ShellStartedData {
            timestamp: 1_700_000_000_000,
            session_id: "ses_abc".to_string(),
            message_id: "msg_1".to_string(),
            call_id: "call_3".to_string(),
            command: "ls".to_string(),
        };
        let json = serde_json::to_value(&data).expect("serialisation JSON");
        assert_eq!(json["callID"], serde_json::json!("call_3"));
        assert!(json.get("callId").is_none());
        assert!(json.get("call_ID").is_none());
        let back: ShellStartedData = serde_json::from_value(json).expect("retour JSON");
        assert_eq!(back, data);
    }
}
