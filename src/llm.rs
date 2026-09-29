//! Client vers les modeles heberes gratuits (NVIDIA NIM).
//!
//! Un seul point de sortie pour tous les appels. L'orchestrateur multi-agents
//! passe par ici, ce qui rend le routage de modeles trivial : on change une
//! chaine de caracteres.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::schema::session_message::ModelRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Message envoye au modele, dans le format OpenAI-compatible.
///
/// Distingué de `schema::session_message::Message` : celui-la est le modele de
/// donnees riche d'OpenCode (avec snapshots, compaction, tokens), celui-ci est
/// ce qu'on poste sur le reseau. La conversion se fait dans `to_wire`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl WireMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into(), name: None, tool_call_id: None }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into(), name: None, tool_call_id: None }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into(), name: None, tool_call_id: None }
    }
    pub fn tool(content: impl Into<String>, name: impl Into<String>, call_id: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            name: Some(name.into()),
            tool_call_id: Some(call_id.into()),
        }
    }
}

/// Description d'un outil, au format `tools` de l'API OpenAI.
///
/// Le `type: "function"` est obligatoire cote requete : l'API OpenAI-compatible
/// attend `{"type":"function","function":{...}}` et refuse 400 sans lui.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSpec {
    fn to_wire(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        })
    }
}

/// Appel d'outil demande par le modele.
///
/// `kind` est `function` en pratique, mais l'API l'exige explicitement : sans
/// lui, les backends OpenAI-compatibles repondent 400. C'est un champ que rien
/// dans le code ne lit, ce qui le rend facile a oublier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "function_kind")]
    pub kind: String,
    pub function: ToolCallFunction,
}

fn function_kind() -> String {
    "function".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    /// JSON serialise, tel qu'envoye par le modele.
    ///
    /// On garde la chaine brute plutot qu'une `Value` : les modeles produisent
    /// regulierement du JSON malforme, et perdre la sortie originale empeche
    /// ensuite de rapporter une erreur utile.
    pub arguments: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
    pub usage: Usage,
}

impl Completion {
    pub fn wants_tool(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [WireMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

pub struct Llm {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: ModelRef,
}

impl Llm {
    pub fn new(model: ModelRef) -> Result<Self> {
        let api_key = std::env::var("NVIDIA_API_KEY")
            .context("NVIDIA_API_KEY absent : les modeles heberes gratuits en ont besoin")?;
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()?,
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key,
            model,
        })
    }

    pub fn model(&self) -> &ModelRef {
        &self.model
    }

    /// Appel non-streaming.
    ///
    /// Pas de streaming ici : la boucle d'agent a besoin de la completion entiere
    /// avant d'agir, et le streaming n'apporterait que de la complexite.
    pub async fn complete(
        &self,
        messages: &[WireMessage],
        tools: Option<&[ToolSpec]>,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<Completion> {
        let full_model = format!("{}/{}", self.model.provider, self.model.model);

        let req = ChatRequest {
            model: &full_model,
            messages,
            tools: tools.map(|t| t.iter().map(ToolSpec::to_wire).collect()),
            max_tokens,
            temperature,
            stream: false,
        };

        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .context("envoi de la requete au modele")?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            anyhow::bail!("le modele a refuse ({status}) : {}", truncate(&body, 300));
        }

        let parsed: ChatResponse = serde_json::from_str(&body).context("reponse illisible")?;
        let choice = parsed.choices.into_iter().next().context("zero choix renvoye")?;

        Ok(Completion {
            text: choice.message.content.unwrap_or_default(),
            tool_calls: choice.message.tool_calls,
            finish_reason: choice.finish_reason,
            usage: parsed.usage.unwrap_or_default(),
        })
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}
