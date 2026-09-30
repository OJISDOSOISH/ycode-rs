//! Portage Rust de `opencode/packages/core/src/github-copilot/responses/openai-responses-language-model.ts`.
//!
//! Logique metier pure uniquement : schemas de la Responses API, configuration
//! par modele, resolution des options provider, avertissements d'options non
//! prises en charge, gardes de chunks de stream et petits calculs d'usage.
//!
//! N'est PAS porte ici (runtime d'effets, voir POINT FAIBLE du rapport) :
//! `getArgs` async, `doGenerate` / `doStream` HTTP, `TransformStream`, `fetch`,
//! `convertToOpenAIResponsesInput`, `prepareResponsesTools`,
//! `mapOpenAIResponseFinishReason`, `postJsonToApi` et les handlers de reponse.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Borne haute de `top_logprobs` acceptes par l'API (entier entre 0 et 20).
///
/// Documente dans la source avec un lien vers la reference de l'API Responses.
pub const TOP_LOGPROBS_MAX: u64 = 20;

/// Motif d'URL accepte pour les pieces jointes image et PDF.
///
/// L'original utilise des `RegExp` `/^https?:\/\/.*$/`. Sans moteur de regex
/// dans ce module pur, on conserve le motif sous forme de chaine et on le
/// documente comme tel.
pub const MOTIF_URL_HTTP: &str = "^https?://.*$";

/// Valeur d'`include` ajoutee quand des logprobs sont demandees.
pub const INCLUDE_LOGPROBS: &str = "message.output_text.logprobs";

/// Valeur d'`include` ajoutee quand un outil de recherche web est present.
pub const INCLUDE_SOURCES_RECHERCHE: &str = "web_search_call.action.sources";

/// Valeur d'`include` ajoutee quand un outil d'interpretation de code est present.
pub const INCLUDE_SORTIES_INTERPRETE: &str = "code_interpreter_call.outputs";

/// Nom d'outil de repli quand aucun nom de recherche web n'est configure.
///
/// Correspond a `webSearchToolName ?? "web_search"` de `doGenerate`.
pub const NOM_RECHERCHE_DEFAUT: &str = "web_search";

/// Statut de repli pour un appel ordinateur sans statut.
///
/// Correspond a `part.status || "completed"` : test de veracite, pas de
/// nullite (voir `statut_computer_ou_termine`).
pub const STATUT_COMPUTER_DEFAUT: &str = "completed";

// ---------------------------------------------------------------------------
// Items de sortie (reponse non streamee)
// ---------------------------------------------------------------------------

/// Action d'un appel de recherche web (forme complete de `doGenerate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionRecherche {
    /// Recherche par requete textuelle.
    #[serde(rename = "search")]
    Recherche {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
    },
    /// Ouverture d'une page.
    #[serde(rename = "open_page")]
    OuvrirPage { url: String },
    /// Recherche d'un motif dans une page.
    #[serde(rename = "find")]
    Trouver { url: String, pattern: String },
}

/// Item `web_search_call` de la reponse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelRechercheWeb {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionRecherche>,
}

/// Un resultat de recherche de fichiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatFichier {
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub file_id: String,
    pub filename: String,
    pub score: f64,
    pub text: String,
}

/// Item `file_search_call` de la reponse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelRechercheFichiers {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub queries: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<ResultatFichier>>,
}

/// Une sortie d'interprete de code : journaux ou image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SortieInterprete {
    /// Sortie texte de l'interprete.
    #[serde(rename = "logs")]
    Journaux { logs: String },
    /// Sortie image de l'interprete.
    #[serde(rename = "image")]
    Image { url: String },
}

/// Item `code_interpreter_call` de la reponse.
///
/// `code` et `outputs` sont nullables dans la source, donc `Option` ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelInterprete {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub container_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<SortieInterprete>>,
}

/// Action `exec` d'un appel shell local (cles snake_case de l'API).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionShell {
    #[serde(rename = "type")]
    pub kind: String,
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
}

/// Item `local_shell_call` de la reponse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelShellLocal {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub call_id: String,
    pub action: ActionShell,
}

/// Item `image_generation_call` de la reponse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelGenerationImage {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub result: String,
}

/// Une entree de logprobs avec son top de tokens probables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntreeLogprobs {
    pub token: String,
    pub logprob: f64,
    pub top_logprobs: Vec<TokenProbable>,
}

/// Un token probable associe a sa logprobabilite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenProbable {
    pub token: String,
    pub logprob: f64,
}

/// Annotation portee par un segment de texte de reponse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Annotation {
    /// Citation d'URL avec ses offsets.
    #[serde(rename = "url_citation")]
    CitationUrl {
        start_index: i64,
        end_index: i64,
        url: String,
        title: String,
    },
    /// Citation de fichier avec ses champs optionnels.
    #[serde(rename = "file_citation")]
    CitationFichier {
        file_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start_index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quote: Option<String>,
    },
    /// Citation de fichier de conteneur (aucun champ dans la source).
    ///
    /// Doute de portage : variante vide d'enum a tag interne, sans preuve
    /// locale de la serialisation exacte (voir POINT FAIBLE).
    #[serde(rename = "container_file_citation")]
    CitationConteneur {},
}

/// Un segment `output_text` d'un message assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentTexte {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<EntreeLogprobs>>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

/// Une partie de resume de raisonnement (`summary_text`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartieResume {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

/// Un element du tableau `output` de la reponse.
///
/// Union discriminee sur `type`, comme le `discriminatedUnion` de la source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ElementSortie {
    /// Message assistant avec ses segments de texte.
    #[serde(rename = "message")]
    Message {
        role: String,
        id: String,
        content: Vec<SegmentTexte>,
    },
    /// Appel de recherche web execute par le provider.
    #[serde(rename = "web_search_call")]
    RechercheWeb(AppelRechercheWebSansType),
    /// Appel de recherche de fichiers execute par le provider.
    #[serde(rename = "file_search_call")]
    RechercheFichiers(AppelRechercheFichiersSansType),
    /// Appel d'interprete de code execute par le provider.
    #[serde(rename = "code_interpreter_call")]
    Interprete(AppelInterpreteSansType),
    /// Appel de generation d'image execute par le provider.
    #[serde(rename = "image_generation_call")]
    GenerationImage(AppelGenerationImageSansType),
    /// Appel shell local.
    #[serde(rename = "local_shell_call")]
    ShellLocal(AppelShellLocalSansType),
    /// Appel de fonction a executer cote client.
    #[serde(rename = "function_call")]
    AppelFonction {
        call_id: String,
        name: String,
        arguments: String,
        id: String,
    },
    /// Appel ordinateur execute par le provider.
    #[serde(rename = "computer_call")]
    AppelComputer {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Bloc de raisonnement avec son resume.
    #[serde(rename = "reasoning")]
    Raisonnement {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encrypted_content: Option<String>,
        summary: Vec<PartieResume>,
    },
}

/// Contenu d'un `web_search_call` sans le tag (le tag vit sur l'enum).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelRechercheWebSansType {
    pub id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionRecherche>,
}

/// Contenu d'un `file_search_call` sans le tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelRechercheFichiersSansType {
    pub id: String,
    pub queries: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<ResultatFichier>>,
}

/// Contenu d'un `code_interpreter_call` sans le tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelInterpreteSansType {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub container_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<SortieInterprete>>,
}

/// Contenu d'un `image_generation_call` sans le tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelGenerationImageSansType {
    pub id: String,
    pub result: String,
}

/// Contenu d'un `local_shell_call` sans le tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelShellLocalSansType {
    pub id: String,
    pub call_id: String,
    pub action: ActionShell,
}

// ---------------------------------------------------------------------------
// Usage et reponse
// ---------------------------------------------------------------------------

/// Detail des tokens d'entree (tokens en cache).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailsTokensEntree {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
}

/// Detail des tokens de sortie (tokens de raisonnement).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailsTokensSortie {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
}

/// Bloc `usage` de la reponse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens_details: Option<DetailsTokensEntree>,
    pub output_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens_details: Option<DetailsTokensSortie>,
}

/// Erreur applicative portee par une reponse 200 (`response.error`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErreurReponse {
    pub code: String,
    pub message: String,
}

/// Raison d'incompletude (`incomplete_details.reason`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailIncomplet {
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Resultats d'outils construits dans le mapping de `doGenerate`
// ---------------------------------------------------------------------------

/// Resultat `computer_use` construit cote client.
///
/// Le statut suit la regle falsy `part.status || "completed"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatComputer {
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
}

/// Une ligne de resultat de recherche de fichiers, cles camelCase.
///
/// La source convertit `file_id` en `fileId` dans l'objet resultat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LigneResultatFichiers {
    pub attributes: BTreeMap<String, serde_json::Value>,
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub filename: String,
    pub score: f64,
    pub text: String,
}

/// Resultat `file_search` avec `results` nullable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatRechercheFichiers {
    pub queries: Vec<String>,
    pub results: Option<Vec<LigneResultatFichiers>>,
}

/// Resultat `code_interpreter` avec `outputs` nullable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatInterprete {
    pub outputs: Option<Vec<SortieInterprete>>,
}

/// Resultat `image_generation` (base64 ou chaine resultat).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatImage {
    pub result: String,
}

// ---------------------------------------------------------------------------
// Chunks de stream
// ---------------------------------------------------------------------------

/// Chunk `response.output_text.delta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkDeltaTexte {
    pub item_id: String,
    pub delta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<EntreeLogprobs>>,
}

/// Chunk `error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkErreur {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    pub sequence_number: i64,
}

/// Corps commun des chunks `response.completed` / `response.incomplete`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpsFin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incomplete_details: Option<DetailIncomplet>,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

/// Corps du chunk `response.created`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpsCreation {
    pub id: String,
    pub created_at: f64,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

/// Action de recherche reduite du chunk `output_item.added`.
///
/// La source n'autorise ici que la variante `search` avec requete optionnelle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionRechercheAjoutee {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// Item leger du chunk `response.output_item.added`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ElementAjoute {
    /// Debut d'un message assistant.
    #[serde(rename = "message")]
    Message { id: String },
    /// Debut d'un bloc de raisonnement.
    #[serde(rename = "reasoning")]
    Raisonnement {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encrypted_content: Option<String>,
    },
    /// Debut d'un appel de fonction.
    #[serde(rename = "function_call")]
    AppelFonction {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
    },
    /// Debut d'une recherche web.
    #[serde(rename = "web_search_call")]
    RechercheWeb {
        id: String,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<ActionRechercheAjoutee>,
    },
    /// Debut d'un appel ordinateur.
    #[serde(rename = "computer_call")]
    AppelComputer { id: String, status: String },
    /// Debut d'une recherche de fichiers.
    #[serde(rename = "file_search_call")]
    RechercheFichiers { id: String },
    /// Debut d'une generation d'image.
    #[serde(rename = "image_generation_call")]
    GenerationImage { id: String },
    /// Debut d'un interprete de code.
    #[serde(rename = "code_interpreter_call")]
    Interprete {
        id: String,
        container_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outputs: Option<Vec<SortieInterprete>>,
        status: String,
    },
}

/// Item complet du chunk `response.output_item.done`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ElementTermine {
    /// Fin d'un message assistant.
    #[serde(rename = "message")]
    Message { id: String },
    /// Fin d'un bloc de raisonnement.
    #[serde(rename = "reasoning")]
    Raisonnement {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encrypted_content: Option<String>,
    },
    /// Fin d'un appel de fonction (statut `completed` dans la source).
    #[serde(rename = "function_call")]
    AppelFonction {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: String,
    },
    /// Fin d'un interprete de code (forme complete).
    #[serde(rename = "code_interpreter_call")]
    Interprete(AppelInterpreteSansType),
    /// Fin d'une generation d'image (forme complete).
    #[serde(rename = "image_generation_call")]
    GenerationImage(AppelGenerationImageSansType),
    /// Fin d'une recherche web (forme complete).
    #[serde(rename = "web_search_call")]
    RechercheWeb(AppelRechercheWebSansType),
    /// Fin d'une recherche de fichiers (forme complete).
    #[serde(rename = "file_search_call")]
    RechercheFichiers(AppelRechercheFichiersSansType),
    /// Fin d'un appel shell local (forme complete).
    #[serde(rename = "local_shell_call")]
    ShellLocal(AppelShellLocalSansType),
    /// Fin d'un appel ordinateur (statut `completed` dans la source).
    #[serde(rename = "computer_call")]
    AppelComputer { id: String, status: String },
}

/// Chunk `response.function_call_arguments.delta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkDeltaArguments {
    pub item_id: String,
    pub output_index: i64,
    pub delta: String,
}

/// Chunk `response.image_generation_call.partial_image`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkImagePartielle {
    pub item_id: String,
    pub output_index: i64,
    pub partial_image_b64: String,
}

/// Chunk `response.code_interpreter_call_code.delta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkDeltaCode {
    pub item_id: String,
    pub output_index: i64,
    pub delta: String,
}

/// Chunk `response.code_interpreter_call_code.done`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkCodeTermine {
    pub item_id: String,
    pub output_index: i64,
    pub code: String,
}

/// Annotation reduite du chunk `response.output_text.annotation.added`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnnotationAjoutee {
    /// Citation d'URL ajoutee en stream.
    #[serde(rename = "url_citation")]
    CitationUrl { url: String, title: String },
    /// Citation de fichier ajoutee en stream.
    #[serde(rename = "file_citation")]
    CitationFichier {
        file_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start_index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_index: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quote: Option<String>,
    },
}

/// Chunk `response.output_text.annotation.added`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkAnnotationAjoutee {
    pub annotation: AnnotationAjoutee,
}

/// Chunk `response.reasoning_summary_part.added`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkPartieResume {
    pub item_id: String,
    pub summary_index: i64,
}

/// Chunk `response.reasoning_summary_text.delta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkDeltaResume {
    pub item_id: String,
    pub summary_index: i64,
    pub delta: String,
}

/// Union des chunks de stream, discriminee sur `type`.
///
/// La source ajoute un repli `z.object({ type: z.string() }).loose()` pour les
/// chunks inconnus. Ce repli de validation souple n'est pas portable en pur :
/// un type inconnu echoue a la deserialisation et suit le chemin d'erreur du
/// stream (voir POINT FAIBLE). `est_type_chunk_connu` documente la liste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChunkReponses {
    /// Delta de texte assistant.
    #[serde(rename = "response.output_text.delta")]
    DeltaTexte(ChunkDeltaTexte),
    /// Reponse terminee avec succes.
    #[serde(rename = "response.completed")]
    Terminee(CorpsFinAvecReponse),
    /// Reponse interrompue.
    #[serde(rename = "response.incomplete")]
    Inincomplete(CorpsFinAvecReponse),
    /// Creation de la reponse (id, date, modele).
    #[serde(rename = "response.created")]
    Creee(CorpsCreationAvecReponse),
    /// Ajout d'un item de sortie.
    #[serde(rename = "response.output_item.added")]
    ElementAjoute(ChunkElementAjoute),
    /// Fin d'un item de sortie.
    #[serde(rename = "response.output_item.done")]
    ElementTermine(ChunkElementTermine),
    /// Delta d'arguments d'appel de fonction.
    #[serde(rename = "response.function_call_arguments.delta")]
    DeltaArguments(ChunkDeltaArguments),
    /// Image partielle de generation.
    #[serde(rename = "response.image_generation_call.partial_image")]
    ImagePartielle(ChunkImagePartielle),
    /// Delta de code d'interprete.
    #[serde(rename = "response.code_interpreter_call_code.delta")]
    DeltaCode(ChunkDeltaCode),
    /// Code d'interprete termine.
    #[serde(rename = "response.code_interpreter_call_code.done")]
    CodeTermine(ChunkCodeTermine),
    /// Annotation ajoutee en stream.
    #[serde(rename = "response.output_text.annotation.added")]
    AnnotationAjoutee(ChunkAnnotationAjoutee),
    /// Partie de resume de raisonnement ajoutee.
    #[serde(rename = "response.reasoning_summary_part.added")]
    PartieResume(ChunkPartieResume),
    /// Delta de texte de resume de raisonnement.
    #[serde(rename = "response.reasoning_summary_text.delta")]
    DeltaResume(ChunkDeltaResume),
    /// Chunk d'erreur.
    #[serde(rename = "error")]
    Erreur(ChunkErreur),
}

/// Enveloppe `response` des chunks de fin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpsFinAvecReponse {
    pub response: CorpsFin,
}

/// Enveloppe `response` du chunk de creation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpsCreationAvecReponse {
    pub response: CorpsCreation,
}

/// Enveloppe d'ajout d'item (`output_index` + item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkElementAjoute {
    pub output_index: i64,
    pub item: ElementAjoute,
}

/// Enveloppe de fin d'item (`output_index` + item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkElementTermine {
    pub output_index: i64,
    pub item: ElementTermine,
}

// ---------------------------------------------------------------------------
// Configuration par modele (portage pur de `getResponsesModelConfig`)
// ---------------------------------------------------------------------------

/// Mode du message systeme selon le modele.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModeMessageSysteme {
    /// Message systeme supprime (o1-mini, o1-preview).
    #[serde(rename = "remove")]
    Supprime,
    /// Message systeme classique (modeles non raisonnants).
    #[serde(rename = "system")]
    Systeme,
    /// Message systeme en role developpeur (raisonnants recents).
    #[serde(rename = "developer")]
    Developpeur,
}

/// Configuration derivee de l'id de modele.
///
/// Champs en snake_case avec renoms vers le camelCase d'origine, qui ne sert
/// qu'a la relecture croisee (cette structure ne traverse pas l'API).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigModele {
    /// Vrai pour les modeles a raisonnement (serie o, gpt-5, codex, ...).
    #[serde(rename = "isReasoningModel")]
    pub est_raisonnement: bool,
    /// Traitement du message systeme.
    #[serde(rename = "systemMessageMode")]
    pub mode_systeme: ModeMessageSysteme,
    /// Troncature auto obligatoire.
    #[serde(rename = "requiredAutoTruncation")]
    pub troncature_auto_requise: bool,
    /// Eligibilite au palier `flex`.
    #[serde(rename = "supportsFlexProcessing")]
    pub prend_flex: bool,
    /// Eligibilite au palier `priority`.
    #[serde(rename = "supportsPriorityProcessing")]
    pub prend_priorite: bool,
}

/// Dit si un id de modele prend en charge le traitement `flex`.
///
/// Regle d'origine : `o3*`, `o4-mini*`, ou `gpt-5*` hors `gpt-5-chat*`.
pub fn prend_en_charge_flex(model_id: &str) -> bool {
    model_id.starts_with("o3")
        || model_id.starts_with("o4-mini")
        || (model_id.starts_with("gpt-5") && !model_id.starts_with("gpt-5-chat"))
}

/// Dit si un id de modele prend en charge le traitement `priority`.
///
/// Regle d'origine : `gpt-4*`, `gpt-5-mini*`, `gpt-5*` hors `gpt-5-nano*` et
/// hors `gpt-5-chat*`, `o3*`, `o4-mini*`.
pub fn prend_en_charge_priorite(model_id: &str) -> bool {
    model_id.starts_with("gpt-4")
        || model_id.starts_with("gpt-5-mini")
        || (model_id.starts_with("gpt-5")
            && !model_id.starts_with("gpt-5-nano")
            && !model_id.starts_with("gpt-5-chat"))
        || model_id.starts_with("o3")
        || model_id.starts_with("o4-mini")
}

/// Calcule la configuration d'un modele depuis son id.
///
/// Portage fidele de `getResponsesModelConfig`, y compris l'ordre des
/// branches : `gpt-5-chat*` d'abord (non raisonnant), puis serie o / gpt-5 /
/// codex / computer-use (raisonnants, avec cas `o1-mini` / `o1-preview` en
/// mode `remove`), puis defaut non raisonnant en mode `system`.
pub fn config_modele_reponses(model_id: &str) -> ConfigModele {
    let base_flex = prend_en_charge_flex(model_id);
    let base_priorite = prend_en_charge_priorite(model_id);

    // Les modeles gpt-5-chat sont non raisonnants.
    if model_id.starts_with("gpt-5-chat") {
        return ConfigModele {
            est_raisonnement: false,
            mode_systeme: ModeMessageSysteme::Systeme,
            troncature_auto_requise: false,
            prend_flex: base_flex,
            prend_priorite: base_priorite,
        };
    }

    // Modeles a raisonnement de la serie o et associes.
    if model_id.starts_with("o")
        || model_id.starts_with("gpt-5")
        || model_id.starts_with("codex-")
        || model_id.starts_with("computer-use")
    {
        if model_id.starts_with("o1-mini") || model_id.starts_with("o1-preview") {
            return ConfigModele {
                est_raisonnement: true,
                mode_systeme: ModeMessageSysteme::Supprime,
                troncature_auto_requise: false,
                prend_flex: base_flex,
                prend_priorite: base_priorite,
            };
        }
        return ConfigModele {
            est_raisonnement: true,
            mode_systeme: ModeMessageSysteme::Developpeur,
            troncature_auto_requise: false,
            prend_flex: base_flex,
            prend_priorite: base_priorite,
        };
    }

    // Modeles gpt classiques.
    ConfigModele {
        est_raisonnement: false,
        mode_systeme: ModeMessageSysteme::Systeme,
        troncature_auto_requise: false,
        prend_flex: base_flex,
        prend_priorite: base_priorite,
    }
}

// ---------------------------------------------------------------------------
// Options provider (portage pur de `openaiResponsesProviderOptionsSchema`)
// ---------------------------------------------------------------------------

/// Palier de service demande (`serviceTier`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PalierService {
    /// Palier automatique.
    #[serde(rename = "auto")]
    Auto,
    /// Traitement flexible (modeles eligibles uniquement).
    #[serde(rename = "flex")]
    Flex,
    /// Traitement prioritaire (acces Entreprise requis).
    #[serde(rename = "priority")]
    Priorite,
}

/// Verbosite du texte (`textVerbosity`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerbositeTexte {
    /// Reponse courte.
    #[serde(rename = "low")]
    Basse,
    /// Reponse moyenne.
    #[serde(rename = "medium")]
    Moyenne,
    /// Reponse detaillee.
    #[serde(rename = "high")]
    Haute,
}

/// Option `logprobs` : drapeau ou nombre entre 1 et `TOP_LOGPROBS_MAX`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OptionLogprobs {
    /// `true` demande le maximum, `false` desactive.
    Drapeau(bool),
    /// Nombre de tokens probables demandes.
    Nombre(u64),
}

/// Options provider `copilot` pour la Responses API.
///
/// Champs camelCase d'origine avec renoms explicites. `include` reste un
/// `Vec<String>` car `getArgs` y ajoute des valeurs hors schema
/// (`web_search_call.action.sources`, `code_interpreter_call.outputs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionsProviderReponses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<OptionLogprobs>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "maxToolCalls")]
    pub max_tool_calls: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "parallelToolCalls")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "previousResponseId")]
    pub previous_response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "promptCacheKey")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "reasoningSummary")]
    pub reasoning_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "safetyIdentifier")]
    pub safety_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "serviceTier")]
    pub service_tier: Option<PalierService>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "strictJsonSchema")]
    pub strict_json_schema: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "textVerbosity")]
    pub text_verbosity: Option<VerbositeTexte>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Resout l'option `logprobs` en nombre de `top_logprobs`.
///
/// Regle d'origine : un nombre est garde tel quel, `true` donne
/// `TOP_LOGPROBS_MAX`, `false` ou absent donne rien.
pub fn resoudre_top_logprobs(option: &Option<OptionLogprobs>) -> Option<u64> {
    match option {
        None => None,
        Some(OptionLogprobs::Nombre(n)) => Some(*n),
        Some(OptionLogprobs::Drapeau(true)) => Some(TOP_LOGPROBS_MAX),
        Some(OptionLogprobs::Drapeau(false)) => None,
    }
}

// ---------------------------------------------------------------------------
// Avertissements et regles de `getArgs` (partie pure uniquement)
// ---------------------------------------------------------------------------

/// Un avertissement `unsupported` emis pendant la construction des arguments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Avertissement {
    #[serde(rename = "type")]
    pub kind: String,
    pub feature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Construit un avertissement `unsupported` pour une option donnee.
pub fn avertissement_non_pris_en_charge(feature: &str, details: Option<&str>) -> Avertissement {
    Avertissement {
        kind: "unsupported".to_string(),
        feature: feature.to_string(),
        details: details.map(|d| d.to_string()),
    }
}

/// Avertissements pour les options jamais supportees par la Responses API.
///
/// Chaque drapeau vaut vrai quand l'option est renseignee (`!= null`).
pub fn avertissements_base(
    top_k: bool,
    seed: bool,
    presence_penalty: bool,
    frequency_penalty: bool,
    stop_sequences: bool,
) -> Vec<Avertissement> {
    let mut out = Vec::new();
    if top_k {
        out.push(avertissement_non_pris_en_charge("topK", None));
    }
    if seed {
        out.push(avertissement_non_pris_en_charge("seed", None));
    }
    if presence_penalty {
        out.push(avertissement_non_pris_en_charge("presencePenalty", None));
    }
    if frequency_penalty {
        out.push(avertissement_non_pris_en_charge("frequencyPenalty", None));
    }
    if stop_sequences {
        out.push(avertissement_non_pris_en_charge("stopSequences", None));
    }
    out
}

/// Applique les regles temperature / top_p / raisonnement de `getArgs`.
///
/// Pour un modele raisonnant, `temperature` et `top_p` sont retires (rendus
/// `None`) avec avertissement ; pour un modele classique, `reasoningEffort`
/// et `reasoningSummary` renseignes donnent un avertissement chacun.
/// Retourne `(temperature, top_p, avertissements)`.
pub fn appliquer_regles_raisonnement(
    est_raisonnement: bool,
    temperature: Option<f64>,
    top_p: Option<f64>,
    effort_renseigne: bool,
    resume_renseigne: bool,
) -> (Option<f64>, Option<f64>, Vec<Avertissement>) {
    let mut avertissements = Vec::new();
    if est_raisonnement {
        let mut temperature_finale = temperature;
        let mut top_p_final = top_p;
        if temperature_finale.is_some() {
            temperature_finale = None;
            avertissements.push(avertissement_non_pris_en_charge(
                "temperature",
                Some("temperature is not supported for reasoning models"),
            ));
        }
        if top_p_final.is_some() {
            top_p_final = None;
            avertissements.push(avertissement_non_pris_en_charge(
                "topP",
                Some("topP is not supported for reasoning models"),
            ));
        }
        (temperature_finale, top_p_final, avertissements)
    } else {
        if effort_renseigne {
            avertissements.push(avertissement_non_pris_en_charge(
                "reasoningEffort",
                Some("reasoningEffort is not supported for non-reasoning models"),
            ));
        }
        if resume_renseigne {
            avertissements.push(avertissement_non_pris_en_charge(
                "reasoningSummary",
                Some("reasoningSummary is not supported for non-reasoning models"),
            ));
        }
        (temperature, top_p, avertissements)
    }
}

/// Valide le palier de service demande contre l'eligibilite du modele.
///
/// Un palier `flex` ou `priority` non pris en charge est retire (rendu
/// `None`) avec avertissement. Les chaines de detail reprennent la source
/// a la lettre.
pub fn valider_palier_service(
    palier: Option<PalierService>,
    flex_ok: bool,
    priorite_ok: bool,
) -> (Option<PalierService>, Vec<Avertissement>) {
    let mut avertissements = Vec::new();
    match palier {
        Some(PalierService::Flex) if !flex_ok => {
            avertissements.push(avertissement_non_pris_en_charge(
                "serviceTier",
                Some("flex processing is only available for o3, o4-mini, and gpt-5 models"),
            ));
            (None, avertissements)
        }
        Some(PalierService::Priorite) if !priorite_ok => {
            avertissements.push(avertissement_non_pris_en_charge(
                "serviceTier",
                Some("priority processing is only available for supported models (gpt-4, gpt-5, gpt-5-mini, o3, o4-mini) and requires Enterprise access. gpt-5-nano is not supported"),
            ));
            (None, avertissements)
        }
        autre => (autre, avertissements),
    }
}

/// Ajoute une cle a la liste `include` (`addInclude` de `getArgs`).
///
/// La source concatene sans dedupliquer : cette fonction pousse toujours,
/// meme en cas de doublon.
pub fn ajouter_include(inclus: &mut Vec<String>, cle: &str) {
    inclus.push(cle.to_string());
}

/// Calcule les valeurs d'`include` automatiques de `getArgs`.
///
/// `top_logprobs` renseigne ajoute les logprobs, `a_recherche_web` ajoute les
/// sources, `a_interprete` ajoute les sorties d'interprete.
pub fn includes_automatiques(
    top_logprobs: Option<u64>,
    a_recherche_web: bool,
    a_interprete: bool,
) -> Vec<String> {
    let mut inclus = Vec::new();
    if top_logprobs.is_some() {
        ajouter_include(&mut inclus, INCLUDE_LOGPROBS);
    }
    if a_recherche_web {
        ajouter_include(&mut inclus, INCLUDE_SOURCES_RECHERCHE);
    }
    if a_interprete {
        ajouter_include(&mut inclus, INCLUDE_SORTIES_INTERPRETE);
    }
    inclus
}

// ---------------------------------------------------------------------------
// Gardes de chunks (portage pur des fonctions `is*Chunk`)
// ---------------------------------------------------------------------------

/// Vrai si le chunk est un delta de texte (`response.output_text.delta`).
pub fn est_delta_texte(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::DeltaTexte(_))
}

/// Vrai si le chunk est une fin d'item (`response.output_item.done`).
pub fn est_element_termine(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::ElementTermine(_))
}

/// Vrai si le chunk termine un bloc de raisonnement.
pub fn est_raisonnement_termine(chunk: &ChunkReponses) -> bool {
    match chunk {
        ChunkReponses::ElementTermine(env) => matches!(env.item, ElementTermine::Raisonnement { .. }),
        _ => false,
    }
}

/// Vrai si le chunk clot la reponse (`completed` ou `incomplete`).
pub fn est_chunk_fin(chunk: &ChunkReponses) -> bool {
    matches!(
        chunk,
        ChunkReponses::Terminee(_) | ChunkReponses::Inincomplete(_)
    )
}

/// Vrai si le chunk cree la reponse (`response.created`).
pub fn est_chunk_cree(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::Creee(_))
}

/// Vrai si le chunk est un delta d'arguments d'appel de fonction.
pub fn est_delta_arguments_fonction(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::DeltaArguments(_))
}

/// Vrai si le chunk est une image partielle de generation.
pub fn est_image_partielle(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::ImagePartielle(_))
}

/// Vrai si le chunk est un delta de code d'interprete.
pub fn est_delta_code_interprete(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::DeltaCode(_))
}

/// Vrai si le chunk termine le code d'interprete.
pub fn est_code_interprete_termine(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::CodeTermine(_))
}

/// Vrai si le chunk ajoute un item (`response.output_item.added`).
pub fn est_element_ajoute(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::ElementAjoute(_))
}

/// Vrai si le chunk ajoute demarre un bloc de raisonnement.
pub fn est_raisonnement_ajoute(chunk: &ChunkReponses) -> bool {
    match chunk {
        ChunkReponses::ElementAjoute(env) => matches!(env.item, ElementAjoute::Raisonnement { .. }),
        _ => false,
    }
}

/// Vrai si le chunk ajoute une annotation de texte.
pub fn est_annotation_ajoutee(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::AnnotationAjoutee(_))
}

/// Vrai si le chunk ajoute une partie de resume de raisonnement.
pub fn est_resume_partie_ajoutee(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::PartieResume(_))
}

/// Vrai si le chunk est un delta de texte de resume de raisonnement.
pub fn est_resume_texte_delta(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::DeltaResume(_))
}

/// Vrai si le chunk est une erreur.
pub fn est_chunk_erreur(chunk: &ChunkReponses) -> bool {
    matches!(chunk, ChunkReponses::Erreur(_))
}

/// Dit si un type brut de chunk figure parmi les types connus.
///
/// Sert a documenter le repli `loose` de la source : tout type inconnu suit
/// le chemin d'erreur du stream au lieu d'etre ignore en silence.
pub fn est_type_chunk_connu(type_brut: &str) -> bool {
    matches!(
        type_brut,
        "response.output_text.delta"
            | "response.completed"
            | "response.incomplete"
            | "response.created"
            | "response.output_item.added"
            | "response.output_item.done"
            | "response.function_call_arguments.delta"
            | "response.image_generation_call.partial_image"
            | "response.code_interpreter_call_code.delta"
            | "response.code_interpreter_call_code.done"
            | "response.output_text.annotation.added"
            | "response.reasoning_summary_part.added"
            | "response.reasoning_summary_text.delta"
            | "error"
    )
}

// ---------------------------------------------------------------------------
// Petites regles de mapping (pieges `?` contre `??` inclus)
// ---------------------------------------------------------------------------

/// Statut effectif d'un appel ordinateur.
///
/// La source ecrit `part.status || "completed"` : c'est un test de veracite,
/// donc une chaine vide devient `"completed"`, contrairement a un coalescent
/// `??` qui l'aurait gardee.
pub fn statut_computer_ou_termine(statut: Option<&str>) -> &str {
    match statut {
        Some(s) if !s.is_empty() => s,
        _ => STATUT_COMPUTER_DEFAUT,
    }
}

/// Titre effectif d'une source document.
///
/// La source ecrit `annotation.quote ?? annotation.filename ?? "Document"` :
/// c'est un test de nullite, donc une chaine vide **survit** et n'est pas
/// remplacee. C'est l'inverse du cas `statut_computer_ou_termine`.
pub fn titre_source_annotation<'a>(quote: Option<&'a str>, filename: Option<&'a str>) -> &'a str {
    quote.or(filename).unwrap_or("Document")
}

/// Nom de fichier effectif d'une source document (`filename ?? file_id`).
pub fn nom_fichier_source<'a>(filename: Option<&'a str>, file_id: &'a str) -> &'a str {
    filename.unwrap_or(file_id)
}

/// Nom d'outil de recherche web avec repli (`webSearchToolName ?? ...`).
///
/// Coalescent : une chaine vide configuree survit.
pub fn nom_outil_recherche(nom_configure: Option<&str>) -> &str {
    nom_configure.unwrap_or(NOM_RECHERCHE_DEFAUT)
}

/// Tokens d'entree hors cache (`total - cached` quand le cache est connu).
///
/// Correspond au calcul `noCache` de `doGenerate` et du `flush` de `doStream`.
pub fn tokens_sans_cache(total: u64, en_cache: Option<u64>) -> Option<u64> {
    en_cache.map(|c| total.saturating_sub(c))
}

/// Tokens totaux (`entree + sortie` quand les deux sont connus).
pub fn tokens_totaux(entree: Option<u64>, sortie: Option<u64>) -> Option<u64> {
    match (entree, sortie) {
        (Some(e), Some(s)) => Some(e.saturating_add(s)),
        _ => None,
    }
}

/// Convertit `created_at` (secondes) en millisecondes epoch.
///
/// La source construit `new Date(response.created_at * 1000)`.
pub fn horodatage_ms(created_at_s: f64) -> i64 {
    (created_at_s * 1000.0) as i64
}

/// Vrai quand un resume de raisonnement est vide et doit etre complete.
///
/// La source pousse `{ type: "summary_text", text: "" }` dans ce cas pour
/// garantir au moins une partie de raisonnement.
pub fn resume_a_completer(longueur_resume: usize) -> bool {
    longueur_resume == 0
}

/// Vrai quand la sortie contient au moins un appel de fonction client.
///
/// Correspond au drapeau `hasFunctionCall` de `doGenerate` / `doStream`,
/// qui pilote la raison de fin via `mapOpenAIResponseFinishReason`.
pub fn a_appel_fonction(elements: &[ElementSortie]) -> bool {
    elements
        .iter()
        .any(|e| matches!(e, ElementSortie::AppelFonction { .. }))
}

/// Construit l'entree JSON d'outil `code_interpreter` (cles camelCase).
///
/// La source fait `JSON.stringify({ code, containerId })`.
pub fn entree_interprete_json(code: Option<&str>, container_id: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "containerId": container_id,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Construit l'entree JSON d'outil `local_shell` (cles camelCase).
///
/// Convertit l'action snake_case de l'API (`timeout_ms`, `working_directory`)
/// vers les cles camelCase du schema d'entree (`timeoutMs`, ...), avec
/// `type: "exec"` constant comme dans la source.
pub fn entree_shell_json(action: &ActionShell) -> serde_json::Value {
    let mut objet = serde_json::Map::new();
    objet.insert("type".to_string(), serde_json::Value::String("exec".to_string()));
    objet.insert(
        "command".to_string(),
        serde_json::Value::Array(
            action
                .command
                .iter()
                .map(|c| serde_json::Value::String(c.clone()))
                .collect(),
        ),
    );
    if let Some(t) = action.timeout_ms {
        objet.insert(
            "timeoutMs".to_string(),
            serde_json::Value::from(t),
        );
    }
    if let Some(u) = &action.user {
        objet.insert("user".to_string(), serde_json::Value::String(u.clone()));
    }
    if let Some(w) = &action.working_directory {
        objet.insert(
            "workingDirectory".to_string(),
            serde_json::Value::String(w.clone()),
        );
    }
    if let Some(env) = &action.env {
        let carte: serde_json::Map<String, serde_json::Value> = env
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        objet.insert("env".to_string(), serde_json::Value::Object(carte));
    }
    serde_json::Value::Object(objet)
}

/// Echappe un delta de code pour l'inserer dans une chaine JSON.
///
/// Portage pur de `JSON.stringify(value.delta).slice(1, -1)` : on serialise
/// puis on retire les guillemets externes.
pub fn delta_code_echappe(delta: &str) -> String {
    let brut = serde_json::to_string(delta).unwrap_or_default();
    if brut.len() >= 2 {
        brut[1..brut.len() - 1].to_string()
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Etats de suivi du stream (donnees pures, orchestration non portee)
// ---------------------------------------------------------------------------

/// Suivi d'un appel d'outil en cours pendant le stream.
///
/// La `TransformStream` qui lit et ecrit ces etats n'est pas portee.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppelOutilEnCours {
    pub nom_outil: String,
    pub id_appel: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "containerId")]
    pub conteneur_interprete: Option<String>,
}

/// Suivi d'un bloc de raisonnement actif pendant le stream.
///
/// Copilot regenere les ids chiffres a chaque evenement, donc le suivi se
/// fait par `output_index` avec un id canonique issu de `output_item.added`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaisonnementActif {
    pub id_canonique: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contenu_chiffre: Option<String>,
    #[serde(default)]
    pub parties_resume: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_gpt5_reste_non_raisonnant_en_system() {
        let cfg = config_modele_reponses("gpt-5-chat-latest");
        assert!(!cfg.est_raisonnement);
        assert_eq!(cfg.mode_systeme, ModeMessageSysteme::Systeme);
    }

    #[test]
    fn o1_mini_est_raisonnant_sans_message_systeme() {
        let cfg = config_modele_reponses("o1-mini-2024-09-12");
        assert!(cfg.est_raisonnement);
        assert_eq!(cfg.mode_systeme, ModeMessageSysteme::Supprime);
    }

    #[test]
    fn gpt4_sans_flex_mais_avec_priorite() {
        let cfg = config_modele_reponses("gpt-4o-2024-08-06");
        assert!(!cfg.est_raisonnement);
        assert!(!prend_en_charge_flex("gpt-4o-2024-08-06"));
        assert!(prend_en_charge_priorite("gpt-4o-2024-08-06"));
        assert_eq!(cfg.mode_systeme, ModeMessageSysteme::Systeme);
    }

    #[test]
    fn logprobs_vrai_demande_le_maximum() {
        assert_eq!(resoudre_top_logprobs(&None), None);
        assert_eq!(
            resoudre_top_logprobs(&Some(OptionLogprobs::Drapeau(true))),
            Some(TOP_LOGPROBS_MAX)
        );
        assert_eq!(
            resoudre_top_logprobs(&Some(OptionLogprobs::Drapeau(false))),
            None
        );
        assert_eq!(
            resoudre_top_logprobs(&Some(OptionLogprobs::Nombre(5))),
            Some(5)
        );
    }

    #[test]
    fn statut_vide_computer_devient_complete() {
        assert_eq!(statut_computer_ou_termine(None), "completed");
        assert_eq!(statut_computer_ou_termine(Some("")), "completed");
        assert_eq!(statut_computer_ou_termine(Some("failed")), "failed");
    }

    #[test]
    fn titre_vide_annotation_survit() {
        assert_eq!(titre_source_annotation(Some(""), Some("f.txt")), "");
        assert_eq!(titre_source_annotation(None, Some("f.txt")), "f.txt");
        assert_eq!(titre_source_annotation(None, None), "Document");
        assert_eq!(nom_fichier_source(None, "file-123"), "file-123");
    }

    #[test]
    fn options_serialise_en_camel_case() {
        let opts = OptionsProviderReponses {
            include: None,
            instructions: None,
            logprobs: None,
            max_tool_calls: Some(4.0),
            metadata: None,
            parallel_tool_calls: None,
            previous_response_id: Some("resp-1".to_string()),
            prompt_cache_key: None,
            reasoning_effort: None,
            reasoning_summary: None,
            safety_identifier: None,
            service_tier: Some(PalierService::Flex),
            store: None,
            strict_json_schema: None,
            text_verbosity: None,
            user: None,
        };
        let json = serde_json::to_value(&opts).unwrap();
        assert_eq!(json["maxToolCalls"], serde_json::json!(4.0));
        assert_eq!(json["previousResponseId"], serde_json::json!("resp-1"));
        assert_eq!(json["serviceTier"], serde_json::json!("flex"));
        assert!(json.get("max_tool_calls").is_none());
        assert!(json.get("previous_response_id").is_none());
    }

    #[test]
    fn cache_soustrait_du_total_entree() {
        assert_eq!(tokens_sans_cache(100, Some(30)), Some(70));
        assert_eq!(tokens_sans_cache(100, None), None);
        assert_eq!(tokens_totaux(Some(100), Some(50)), Some(150));
        assert_eq!(tokens_totaux(None, Some(50)), None);
    }
}
