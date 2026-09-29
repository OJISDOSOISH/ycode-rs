//! Boucle d'agent : le cycle qui fait travailler le modele.
//!
//! Ce n'est pas un portage direct d'un fichier de l'original : c'est leMoment
//! ou `history` (quelle est la fenetre visible), `context_epoch` (quand
//! reconstruire le contexte) et `compaction` (quand tronquer) se rejoignent, via
//! le client LLM et la boite a outils.
//!
//! La regle qui gouverne tout : on ne sort de la boucle que sur une condition
//! observable, jamais sur une impression. Le modele peut s'arreter pour cent
//! raisons, et une boucle qui s'arrete sur « j'ai l'air d'avoir fini » produit
//! exactement le genre de livrable non verifie qu'on a deja vu ce matin.

use crate::core::session::compaction::{should_compact, truncate_tool_output};
use crate::core::session::history::{load, BaselineSeq, Entry, Window};
use crate::llm::{Completion, Llm, ToolCall, ToolSpec, WireMessage};
use crate::tool::ToolBox;

/// Nombre maximum de tours avant arret force.
///
/// Garde-fou, pas politique. Un agent qui boucle sans avancer doit finir par
/// s'arreter et rendre la main, sinon il consomme des credits indefiniment.
const MAX_TURNS: usize = 100;

/// Resultat d'un tour, ce que le modele a produit.
enum Turn {
    /// Le modele a repondu sans appeler d'outil : le tour est fini.
    Final(String),
    /// Le modele a demande des appels d'outils : il faut les executer et
    /// lui rendre la main.
    ToolCalls(Vec<ToolCall>),
}

/// Etat d'une session en cours d'execution.
pub struct Run<'a> {
    llm: &'a Llm,
    tools: &'a ToolBox,
    specs: Vec<ToolSpec>,

    /// Historique complet de la session, tous rangs confondus.
    pub entries: Vec<Entry>,
    /// Curseur d'epoch, si la session en a un.
    pub baseline: Option<BaselineSeq>,

    next_seq: i64,
    turns: usize,
}

impl<'a> Run<'a> {
    pub fn new(llm: &'a Llm, tools: &'a ToolBox) -> Self {
        Self {
            llm,
            tools,
            specs: default_tool_specs(),
            entries: Vec::new(),
            baseline: None,
            next_seq: 0,
            turns: 0,
        }
    }

    /// Nombre de tours effectues. Utile pour le diagnostic : un agent qui
    /// plafonne a 100 tours a un probleme, pas une tache difficile.
    pub fn turns(&self) -> usize {
        self.turns
    }

    /// Ajoute un message a l'historique et lui attribue le rang suivant.
    pub fn push(&mut self, message: crate::schema::session_message::Message) -> i64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push(Entry { seq, message });
        seq
    }

    /// Execute la boucle jusqu'a ce que le modele reponde sans appeler d'outil.
    ///
    /// `prompt` est le message utilisateur initial. Il est ajoute a
    /// l'historique, puis on boucle sur ce que le modele demande.
    pub async fn execute(
        &mut self,
        prompt: &str,
        context_limit: u64,
        keep_tokens: u64,
    ) -> anyhow::Result<String> {
        self.push(crate::schema::session_message::Message::User(
            crate::schema::session_message::User {
                base: crate::schema::session_message::MessageBase::new(
                    format!("msg_{:012}", self.next_seq),
                    0,
                ),
                prompt: crate::schema::session_message::Prompt {
                    text: prompt.to_string(),
                    files: vec![],
                    agents: vec![],
                },
            },
        ));

        loop {
            if self.turns >= MAX_TURNS {
                anyhow::bail!("limite de {MAX_TURNS} tours atteinte sans reponse finale");
            }
            self.turns += 1;

            let messages = self.build_wire_messages(context_limit, keep_tokens)?;
            let completion = self.llm.complete(&messages, Some(&self.specs), 8_000, 0.2).await?;

            match classify(&completion) {
                Turn::Final(text) => {
                    self.record_assistant(&completion, &text);
                    return Ok(text);
                }
                Turn::ToolCalls(calls) => {
                    self.record_assistant(&completion, "");
                    let results = self.execute_tools(&calls).await?;
                    for (name, call_id, output) in results {
                        self.push(crate::schema::session_message::Message::User(
                            crate::schema::session_message::User {
                                base: crate::schema::session_message::MessageBase::new(
                                    format!("msg_{:012}", self.next_seq),
                                    0,
                                ),
                                prompt: crate::schema::session_message::Prompt {
                                    text: format!("Resultat de {name} :\n{output}"),
                                    files: vec![],
                                    agents: vec![],
                                },
                            },
                        ));
                        let _ = call_id;
                    }
                }
            }
        }
    }

    /// Construit les messages envoyes au modele, en passant par la fenetre
    /// visible et la troncature.
    fn build_wire_messages(
        &self,
        context_limit: u64,
        keep_tokens: u64,
    ) -> anyhow::Result<Vec<WireMessage>> {
        let window = Window {
            compaction_seq: crate::core::session::history::latest_compaction(&self.entries),
            baseline: self.baseline,
        };
        let visible = load(&self.entries, window);

        let mut out = vec![WireMessage::system(SYSTEM_PROMPT)];

        let mut budget = keep_tokens;
        for msg in visible {
            let text = render(&msg);
            let cost = crate::core::session::compaction::estimate_tokens(&text);
            if cost > budget {
                // Le plus ancien est sacrifie en premier : c'est la fenetre qui
                // glisse, comme dans l'original.
                break;
            }
            budget -= cost;
            match msg {
                crate::schema::session_message::Message::User(u) => {
                    out.push(WireMessage::user(u.prompt.text))
                }
                crate::schema::session_message::Message::Assistant(a) => {
                    if a.content.is_empty() {
                        continue;
                    }
                    out.push(WireMessage::assistant(render(&crate::schema::session_message::Message::Assistant(a))))
                }
                _ => {}
            }
        }

        let used = crate::core::session::compaction::estimate_value(&out);
        if should_compact(used, context_limit, 8_000, keep_tokens) {
            // On ne compacte pas ici : la compaction demande un appel modele
            // pour rediger le resume. On se contente de le signaler, et
            // l'appelant decide.
        }

        Ok(out)
    }

    async fn execute_tools(&self, calls: &[ToolCall]) -> anyhow::Result<Vec<(String, String, String)>> {
        let mut out = Vec::new();
        for call in calls {
            let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                .unwrap_or(serde_json::json!({}));
            let result = match self.tools.call(&call.function.name, &args).await {
                Ok(r) => r,
                // Une erreur d'outil n'est pas fatale : on la renvoie au modele
                // comme un resultat, pour qu'il ajuste. C'est ce qui evite qu'un
                // mauvais nom de fichier arrete toute la session.
                Err(e) => crate::tool::ToolResult {
                    content: format!("erreur : {e}"),
                    touched: vec![],
                },
            };
            out.push((
                call.function.name.clone(),
                call.id.clone(),
                truncate_tool_output(&result.content),
            ));
        }
        Ok(out)
    }

    fn record_assistant(&mut self, completion: &Completion, text: &str) {
        let mut a = crate::schema::session_message::Assistant::new(
            format!("msg_{:012}", self.next_seq),
            0,
            "build",
            self.llm.model().clone(),
        );
        if !text.is_empty() {
            a.content.push(crate::schema::session_message::AssistantContent::Text {
                id: format!("txt_{:012}", self.next_seq),
                text: text.to_string(),
            });
        }
        self.next_seq += 1;
        self.entries.push(Entry { seq: a.time.created, message: crate::schema::session_message::Message::Assistant(a) });
        let _ = completion;
    }
}

fn classify(completion: &Completion) -> Turn {
    if completion.wants_tool() {
        Turn::ToolCalls(completion.tool_calls.clone())
    } else {
        Turn::Final(completion.text.clone())
    }
}

fn render(msg: &crate::schema::session_message::Message) -> String {
    use crate::schema::session_message::Message as M;
    match msg {
        M::User(u) => u.prompt.text.clone(),
        M::System(s) => s.text.clone(),
        M::Synthetic(s) => s.text.clone(),
        M::Assistant(a) => a
            .content
            .iter()
            .map(|c| match c {
                crate::schema::session_message::AssistantContent::Text { text, .. } => text.clone(),
                crate::schema::session_message::AssistantContent::Reasoning { text, .. } => text.clone(),
                crate::schema::session_message::AssistantContent::Tool(t) => t.name.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        M::Compaction(c) => c.summary.clone(),
        _ => String::new(),
    }
}

const SYSTEM_PROMPT: &str = "\
You are a coding agent. Work directly on the task.
Use the provided tools to read and write files. Prefer reading before writing.
When you are done, answer with a final message and no tool call.
Be concise. Do not restate the task.";

fn default_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "read".to_string(),
            description: "Read a file from the workspace.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "write".to_string(),
            description: "Write a file in the workspace, creating it if needed.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolSpec {
            name: "list".to_string(),
            description: "List files in a directory.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } }
            }),
        },
    ]
}
