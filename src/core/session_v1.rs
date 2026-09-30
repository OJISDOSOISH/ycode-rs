//! Portage Rust du contrat `opencode/packages/schema/src/v1/session.ts`.
//!
//! Ce fichier ne porte que le contrat de donnees : identifiants, erreurs
//! nommees, parts, messages, session et charges d evenements. Aucune logique
//! metier, aucun acces disque, aucun appel reseau.
//!
//! Conventions appliquees :
//! - `Schema.Struct` -> `struct` avec `Serialize, Deserialize`.
//! - unions discriminees TS -> `enum` avec `#[serde(tag = "...")]` et un
//!   `rename` explicite sur chaque variante.
//! - les structs portees par ces enums ne repetent PAS le discriminant
//!   (`type`, `status`, `role`, `name`) : c est l enum qui l ajoute a la
//!   serialisation. Seul ecart volontaire avec la source, sans lui serde
//!   refuserait la cle en double.
//! - tout champ `camelCase` porte un `rename` explicite, avec double
//!   verification des `ID` majuscules (`sessionID`, `messageID`, `partID`,
//!   `providerID`, `modelID`, `parentID`, `callID`, `projectID`,
//!   `workspaceID`, `clientName`, `retryCount`, `statusCode`, `isRetryable`,
//!   `responseHeaders`, `responseBody`). Seul `tail_start_id` est deja en
//!   snake_case dans la source et ne porte donc aucun `rename`.
//! - tout champ optionnel TS est un `Option` avec `skip_serializing_if`.
//! - `NonNegativeInt` -> `i64`, `Schema.Finite` -> `f64`,
//!   `Record<string, ...>` -> `BTreeMap` deterministe.
//! - les erreurs nommees TS (`{ name, data }`) sont portees en deux morceaux :
//!   un struct `...Data` pour `data`, et l enum `AssistantError` qui porte le
//!   tag `name`. Seule `ApiError` garde une struct d enveloppe `{ name, data }`
//!   car `RetryPart.error` l utilise hors union.
//! - les enveloppes d evenements (`define`, `inventory`, `aggregate`) sont du
//!   routage TS : seules les charges `schema` sont portees ici, sans le champ
//!   `type` de routage.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Identifiant de message, prefixe `msg` en TS.
pub type MessageId = String;
/// Identifiant de part, prefixe `prt` en TS.
pub type PartId = String;
/// Identifiant de session, prefixe `ses` en TS.
pub type SessionId = String;
/// Identifiant de projet en TS (`Project.ID`).
pub type ProjectId = String;
/// Identifiant de workspace, prefixe `wrk` en TS.
pub type WorkspaceId = String;
/// Identifiant de fournisseur en TS (`Provider.ID`).
pub type ProviderId = String;
/// Identifiant de modele en TS (`Model.ID`).
pub type ModelId = String;

// ---------------------------------------------------------------------------
// Erreurs nommees (forme TS : `{ name: Literal, data: { ... } }`)
// ---------------------------------------------------------------------------

/// Donnees de `MessageOutputLengthError`, objet vide en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputLengthData {}

/// Donnees de `ProviderAuthError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAuthData {
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    pub message: String,
}

/// Donnees de `MessageAbortedError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbortedData {
    pub message: String,
}

/// Donnees de `StructuredOutputError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredOutputData {
    pub message: String,
    pub retries: i64,
}

/// Donnees de `APIError`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiErrorData {
    pub message: String,
    #[serde(rename = "statusCode")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
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

/// Enveloppe `{ name, data }` de `APIError`, utilisee telle quelle par
/// `RetryPart.error` hors union.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub name: String,
    pub data: ApiErrorData,
}

impl ApiError {
    /// Construit l enveloppe avec le tag attendu par le TS.
    pub fn new(data: ApiErrorData) -> Self {
        Self {
            name: "APIError".to_string(),
            data,
        }
    }
}

/// Donnees de `ContextOverflowError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextOverflowData {
    pub message: String,
    #[serde(rename = "responseBody")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
}

/// Donnees de `ContentFilterError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFilterData {
    pub message: String,
}

/// Donnees de `UnknownError`. `ref` est un mot cle Rust, d ou `ref_` renomme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownErrorData {
    pub message: String,
    #[serde(rename = "ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
}

/// Union des erreurs d un message assistant, taggee sur `name` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name")]
pub enum AssistantError {
    #[serde(rename = "ProviderAuthError")]
    ProviderAuth(ProviderAuthData),
    #[serde(rename = "UnknownError")]
    Unknown(UnknownErrorData),
    #[serde(rename = "MessageOutputLengthError")]
    OutputLength(OutputLengthData),
    #[serde(rename = "MessageAbortedError")]
    Aborted(AbortedData),
    #[serde(rename = "StructuredOutputError")]
    StructuredOutput(StructuredOutputData),
    #[serde(rename = "ContextOverflowError")]
    ContextOverflow(ContextOverflowData),
    #[serde(rename = "ContentFilterError")]
    ContentFilter(ContentFilterData),
    #[serde(rename = "APIError")]
    Api(ApiErrorData),
}

// ---------------------------------------------------------------------------
// Format de sortie demande a l assistant
// ---------------------------------------------------------------------------

/// Variante texte, objet a un seul discriminant en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputFormatText {}

fn default_retry_count() -> i64 {
    2
}

/// Variante schema JSON, avec 2 essais par defaut en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputFormatJsonSchema {
    pub schema: BTreeMap<String, serde_json::Value>,
    #[serde(rename = "retryCount", default = "default_retry_count")]
    pub retry_count: i64,
}

/// Format de sortie, union taggee sur `type` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputFormat {
    #[serde(rename = "text")]
    Text(OutputFormatText),
    #[serde(rename = "json_schema")]
    JsonSchema(OutputFormatJsonSchema),
}

// ---------------------------------------------------------------------------
// Petits blocs partages
// ---------------------------------------------------------------------------

/// Plage de temps `{ start, end? }`, entiers en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

/// Position texte d une source de fichier, flottants en TS (`Schema.Finite`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSourceText {
    pub value: String,
    pub start: f64,
    pub end: f64,
}

/// Source d une part agent, entiers en TS (`NonNegativeInt`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSource {
    pub value: String,
    pub start: i64,
    pub end: i64,
}

/// Un point `{ line, character }` d une plage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangePoint {
    pub line: i64,
    pub character: i64,
}

/// Plage ligne / caractere d une source symbole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: RangePoint,
    pub end: RangePoint,
}

/// Compteurs de tokens, flottants en TS (`Schema.Finite`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenCache {
    pub read: f64,
    pub write: f64,
}

/// Bloc `tokens` commun a `StepFinishPart` et au message assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    pub input: f64,
    pub output: f64,
    pub reasoning: f64,
    pub cache: TokenCache,
}

/// Temps de demarrage seul (etat `running`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartTime {
    pub start: i64,
}

/// Temps de debut et de fin (etats `completed` et `error`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartEndTime {
    pub start: i64,
    pub end: i64,
}

/// Temps d une part de nouvel essai.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryTime {
    pub created: i64,
}

/// Modele vise par une sous-tache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtaskModel {
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    #[serde(rename = "modelID")]
    pub model_id: ModelId,
}

// ---------------------------------------------------------------------------
// Sources de parts fichier
// ---------------------------------------------------------------------------

/// Source fichier brut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSource {
    pub text: FileSourceText,
    pub path: String,
}

/// Source symbole de code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolSource {
    pub text: FileSourceText,
    pub path: String,
    pub range: Range,
    pub name: String,
    pub kind: i64,
}

/// Source ressource externe (MCP ou autre).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSource {
    pub text: FileSourceText,
    #[serde(rename = "clientName")]
    pub client_name: String,
    pub uri: String,
}

/// Source d une part fichier, union taggee sur `type` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FilePartSource {
    #[serde(rename = "file")]
    File(FileSource),
    #[serde(rename = "symbol")]
    Symbol(SymbolSource),
    #[serde(rename = "resource")]
    Resource(ResourceSource),
}

// ---------------------------------------------------------------------------
// Parts (base TS : `{ id, sessionID, messageID }`, discriminant `type`)
// ---------------------------------------------------------------------------

/// Part capture d etat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub snapshot: String,
}

/// Part patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub hash: String,
    pub files: Vec<String>,
}

/// Part texte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthetic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
}

/// Part raisonnement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
    pub time: TimeRange,
}

/// Part fichier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<FilePartSource>,
}

/// Part agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSource>,
}

/// Part compaction. `tail_start_id` est deja en snake_case en source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub auto: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_start_id: Option<MessageId>,
}

/// Part sous-tache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtaskPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub prompt: String,
    pub description: String,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SubtaskModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// Part nouvel essai apres une erreur d API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub attempt: i64,
    pub error: ApiError,
    pub time: RetryTime,
}

/// Part debut d etape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepStartPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

/// Part fin d etape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepFinishPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    pub cost: f64,
    pub tokens: StepTokens,
}

// ---------------------------------------------------------------------------
// Etat d outil, union taggee sur `status` en TS
// ---------------------------------------------------------------------------

/// Outil en attente d execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatePending {
    pub input: BTreeMap<String, serde_json::Value>,
    pub raw: String,
}

/// Outil en cours d execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolStateRunning {
    pub input: BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
    pub time: StartTime,
}

/// Outil termine. `metadata` est obligatoire en TS, pas d `Option` ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolStateCompleted {
    pub input: BTreeMap<String, serde_json::Value>,
    pub output: String,
    pub title: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub time: ToolCompletedTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<FilePart>>,
}

/// Temps d un outil termine, avec instant de compaction optionnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCompletedTime {
    pub start: i64,
    pub end: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compacted: Option<i64>,
}

/// Outil en erreur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolStateError {
    pub input: BTreeMap<String, serde_json::Value>,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
    pub time: StartEndTime,
}

/// Etat d un appel d outil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ToolState {
    #[serde(rename = "pending")]
    Pending(ToolStatePending),
    #[serde(rename = "running")]
    Running(ToolStateRunning),
    #[serde(rename = "completed")]
    Completed(ToolStateCompleted),
    #[serde(rename = "error")]
    Error(ToolStateError),
}

/// Part appel d outil. `callID` garde ses deux majuscules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPart {
    pub id: PartId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "callID")]
    pub call_id: String,
    pub tool: String,
    pub state: ToolState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
}

/// Union de toutes les parts, taggee sur `type` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Part {
    #[serde(rename = "text")]
    Text(TextPart),
    #[serde(rename = "subtask")]
    Subtask(SubtaskPart),
    #[serde(rename = "reasoning")]
    Reasoning(ReasoningPart),
    #[serde(rename = "file")]
    File(FilePart),
    #[serde(rename = "tool")]
    Tool(ToolPart),
    #[serde(rename = "step-start")]
    StepStart(StepStartPart),
    #[serde(rename = "step-finish")]
    StepFinish(StepFinishPart),
    #[serde(rename = "snapshot")]
    Snapshot(SnapshotPart),
    #[serde(rename = "patch")]
    Patch(PatchPart),
    #[serde(rename = "agent")]
    Agent(AgentPart),
    #[serde(rename = "retry")]
    Retry(RetryPart),
    #[serde(rename = "compaction")]
    Compaction(CompactionPart),
}

// ---------------------------------------------------------------------------
// Diff de fichier et resume utilisateur
// ---------------------------------------------------------------------------

/// Statut d un diff, union de chaines en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileDiffStatus {
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "modified")]
    Modified,
}

/// Diff de fichier (`FileDiff.Info` en TS).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDiffInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    pub additions: f64,
    pub deletions: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<FileDiffStatus>,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Temps d un message utilisateur. Flottant car `Timestamp` est un
/// `Schema.Finite` en TS, contrairement au message assistant en entiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserTime {
    pub created: f64,
}

/// Resume joint a un message utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub diffs: Vec<FileDiffInfo>,
}

/// Modele vise par un message utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserModel {
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    #[serde(rename = "modelID")]
    pub model_id: ModelId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Message utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMessage {
    pub id: MessageId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub time: UserTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<UserSummary>,
    pub agent: String,
    pub model: UserModel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<BTreeMap<String, bool>>,
}

/// Temps d un message assistant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantTime {
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<i64>,
}

/// Chemins d execution d un message assistant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantPath {
    pub cwd: String,
    pub root: String,
}

/// Message assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: MessageId,
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub time: AssistantTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AssistantError>,
    #[serde(rename = "parentID")]
    pub parent_id: MessageId,
    #[serde(rename = "modelID")]
    pub model_id: ModelId,
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    pub mode: String,
    pub agent: String,
    pub path: AssistantPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<bool>,
    pub cost: f64,
    pub tokens: StepTokens,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
}

/// Message, union taggee sur `role` en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum MessageInfo {
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
}

/// Message accompagne de ses parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageWithParts {
    pub info: MessageInfo,
    pub parts: Vec<Part>,
}

// ---------------------------------------------------------------------------
// Parts d entree (creation, sans `sessionID` ni `messageID`)
// ---------------------------------------------------------------------------

/// Part texte a creer. Le discriminant `type` est garde tel quel car ces
/// structs ne sont portees par aucun enum ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextPartInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<PartId>,
    #[serde(rename = "type")]
    pub part_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthetic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
}

/// Part fichier a creer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePartInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<PartId>,
    #[serde(rename = "type")]
    pub part_type: String,
    pub mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<FilePartSource>,
}

/// Part agent a creer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPartInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<PartId>,
    #[serde(rename = "type")]
    pub part_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSource>,
}

/// Part sous-tache a creer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtaskPartInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<PartId>,
    #[serde(rename = "type")]
    pub part_type: String,
    pub prompt: String,
    pub description: String,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SubtaskModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Resume statistique d une session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub additions: f64,
    pub deletions: f64,
    pub files: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diffs: Option<Vec<FileDiffInfo>>,
}

/// Compteurs de tokens d une session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionTokens {
    pub input: f64,
    pub output: f64,
    pub reasoning: f64,
    pub cache: TokenCache,
}

/// Partage d une session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionShare {
    pub url: String,
}

/// Point de restauration d une session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRevert {
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "partID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_id: Option<PartId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// Modele courant d une session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionModel {
    pub id: ModelId,
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Action de permission (`PermissionV1.Action` en TS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionAction {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "ask")]
    Ask,
}

/// Regle de permission (`PermissionV1.Rule` en TS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: String,
    pub pattern: String,
    pub action: PermissionAction,
}

/// Horodatages d une session. `archived` est flottant en TS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionTime {
    pub created: i64,
    pub updated: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compacting: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<f64>,
}

/// Fiche session complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub slug: String,
    #[serde(rename = "projectID")]
    pub project_id: ProjectId,
    #[serde(rename = "workspaceID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<WorkspaceId>,
    pub directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "parentID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SessionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<SessionTokens>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share: Option<SessionShare>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SessionModel>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
    pub time: SessionTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<Vec<PermissionRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revert: Option<SessionRevert>,
}

// ---------------------------------------------------------------------------
// Charges d evenements (champs `schema` des `define` TS)
// ---------------------------------------------------------------------------

/// Charge commune a `session.created`, `session.updated`, `session.deleted`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEventPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub info: SessionInfo,
}

/// Charge de `message.updated`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageUpdatedPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub info: MessageInfo,
}

/// Charge de `message.removed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRemovedPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
}

/// Charge de `message.part.updated`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartUpdatedPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub part: Part,
    pub time: f64,
}

/// Charge de `message.part.removed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartRemovedPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "partID")]
    pub part_id: PartId,
}

/// Charge de `message.part.delta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartDeltaPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "partID")]
    pub part_id: PartId,
    pub field: String,
    pub delta: String,
}

/// Charge de `session.diff`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDiffPayload {
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    pub diff: Vec<FileDiffInfo>,
}

/// Charge de `session.error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionErrorPayload {
    #[serde(rename = "sessionID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AssistantError>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn texte_part() -> Part {
        Part::Text(TextPart {
            id: "prt_1".to_string(),
            session_id: "ses_1".to_string(),
            message_id: "msg_1".to_string(),
            text: "bonjour".to_string(),
            synthetic: None,
            ignored: None,
            time: None,
            metadata: None,
        })
    }

    #[test]
    fn un_texte_part_se_serialise_avec_les_bons_noms() {
        let v = serde_json::to_value(texte_part()).unwrap();
        assert_eq!(v["type"], "text");
        assert_eq!(v["sessionID"], "ses_1");
        assert_eq!(v["messageID"], "msg_1");
        assert!(v.get("synthetic").is_none());
        assert!(v.get("session_id").is_none());
        assert!(v.get("messageId").is_none());
    }

    #[test]
    fn un_etat_outil_termine_fait_l_aller_retour() {
        let etat = ToolState::Completed(ToolStateCompleted {
            input: BTreeMap::new(),
            output: "ok".to_string(),
            title: "lecture".to_string(),
            metadata: BTreeMap::new(),
            time: ToolCompletedTime {
                start: 1,
                end: 2,
                compacted: None,
            },
            attachments: None,
        });
        let v = serde_json::to_value(&etat).unwrap();
        assert_eq!(v["status"], "completed");
        let retour: ToolState = serde_json::from_value(v).unwrap();
        assert_eq!(retour, etat);
    }

    #[test]
    fn une_part_compaction_garde_le_snake_case() {
        let part = Part::Compaction(CompactionPart {
            id: "prt_2".to_string(),
            session_id: "ses_1".to_string(),
            message_id: "msg_1".to_string(),
            auto: true,
            overflow: None,
            tail_start_id: Some("msg_0".to_string()),
        });
        let v = serde_json::to_value(&part).unwrap();
        assert_eq!(v["tail_start_id"], "msg_0");
        assert_eq!(v["type"], "compaction");
    }

    #[test]
    fn une_erreur_api_garde_son_tag_name() {
        let erreur = AssistantError::Api(ApiErrorData {
            message: "panne".to_string(),
            status_code: Some(500),
            is_retryable: true,
            response_headers: None,
            response_body: None,
            metadata: None,
        });
        let v = serde_json::to_value(&erreur).unwrap();
        assert_eq!(v["name"], "APIError");
        assert_eq!(v["statusCode"], 500);
        assert_eq!(v["isRetryable"], true);
        let retour: AssistantError = serde_json::from_value(v).unwrap();
        assert_eq!(retour, erreur);
    }

    #[test]
    fn une_session_info_renomme_les_id() {
        let json = serde_json::json!({
            "id": "ses_1",
            "slug": "test",
            "projectID": "prj_1",
            "workspaceID": "wrk_1",
            "directory": "/tmp",
            "parentID": "ses_0",
            "title": "t",
            "version": "1",
            "time": { "created": 1, "updated": 2 }
        });
        let info: SessionInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.project_id, "prj_1");
        assert_eq!(info.workspace_id, Some("wrk_1".to_string()));
        assert_eq!(info.parent_id, Some("ses_0".to_string()));
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["projectID"], "prj_1");
        assert!(v.get("project_id").is_none());
        assert!(v.get("projectId").is_none());
    }

    #[test]
    fn un_format_json_sans_retry_vaut_deux() {
        let v = serde_json::json!({ "type": "json_schema", "schema": {} });
        let format: OutputFormat = serde_json::from_value(v).unwrap();
        match format {
            OutputFormat::JsonSchema(s) => assert_eq!(s.retry_count, 2),
            _ => panic!("mauvais format"),
        }
    }

    #[test]
    fn une_source_ressource_renomme_client_name() {
        let source = FilePartSource::Resource(ResourceSource {
            text: FileSourceText {
                value: "x".to_string(),
                start: 0.0,
                end: 1.0,
            },
            client_name: "mcp".to_string(),
            uri: "uri://truc".to_string(),
        });
        let v = serde_json::to_value(&source).unwrap();
        assert_eq!(v["type"], "resource");
        assert_eq!(v["clientName"], "mcp");
        assert!(v.get("client_name").is_none());
    }

    #[test]
    fn un_input_part_sans_id_ne_serialise_pas_id() {
        let entree = TextPartInput {
            id: None,
            part_type: "text".to_string(),
            text: "salut".to_string(),
            synthetic: None,
            ignored: None,
            time: None,
            metadata: None,
        };
        let v = serde_json::to_value(&entree).unwrap();
        assert_eq!(v["type"], "text");
        assert!(v.get("id").is_none());
    }
}
