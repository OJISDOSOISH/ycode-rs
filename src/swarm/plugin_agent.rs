//! Portage de `packages/core/src/plugin/agent.ts`.
//!
//! La source fait 202 lignes : un plugin `agent` qui declare sept agents
//! (`default`, `plan`, `general`, `explore`, `compaction`, `title`, `summary`)
//! avec leurs prompts systeme et leurs jeux de permissions.
//!
//! Seule la partie pure est portee ici, sans `Effect` ni filesystem :
//! - l identifiant du plugin et les prompts systeme (litteraux) ;
//! - les jeux de permissions sous forme de donnees ;
//! - les identites des sept agents (id, description, mode, visibilite).
//!
//! Volontairement non porte : `define`, `ctx.agent.transform`,
//! `Location.Service`, `Global.Path`, `path.join`, `path.relative`,
//! `PermissionV2.merge` et tout l `Effect.fn` qui orchestre la mutation du
//! catalogue. Ces parties dependent du runtime et sont signalees comme
//! sautees.

use serde::{Deserialize, Serialize};

/// Identifiant du plugin tel qu enregistre par `define`.
pub const ID: &str = "agent";

/// Systeme du agent par defaut.
pub const SYSTEME_CONSTRUCTION: &str = "You are an AI coding agent. Help the user accomplish software engineering tasks by inspecting the workspace, making targeted changes, and using tools according to the configured permissions.";

pub const PROMPT_EXPLORATION: &str = r#"You are a file search specialist. You excel at thoroughly navigating and exploring codebases.

Your strengths:
- Rapidly finding files using glob patterns
- Searching code and text with powerful regex patterns
- Reading and analyzing file contents

Guidelines:
- Use Glob for broad file pattern matching
- Use Grep for searching file contents with regex
- Use Read when you know the specific file path you need to read
- Adapt your search approach based on the thoroughness level specified by the caller
- Return file paths as absolute paths in your final response
- For clear communication, avoid using emojis
- Do not create any files, or run bash commands that modify the user's system state in any way

Complete the user's search request efficiently and report your findings clearly."#;

pub const PROMPT_COMPACTION: &str = r#"You are a context summarization agent. You are given a conversation between a user and an agent. Your goal is to produce a structured summary matching the format specified so another coding agent can continue the work.

Always follow the exact output structure requested by the user prompt. Keep every section, preserve exact file paths and identifiers when known, and prefer terse bullets over paragraphs.

Do not continue the conversation. Do not respond to any questions in the conversation. Only output the structured summary in the exact format requested by the user prompt. Respond in the same language as the conversation."#;

pub const PROMPT_TITRE: &str = r#"You are a title generator. You output ONLY a thread title. Nothing else."#;

pub const PROMPT_RESUME: &str = r#"Summarize what was done in this conversation. Write like a pull request description."#;

/// Mode d un agent : primaire ou sous-agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModeAgent {
    #[serde(rename = "primary")]
    Principal,
    #[serde(rename = "subagent")]
    SousAgent,
}

/// Une regle de permission : `{ action, resource, effect }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReglePermission {
    pub action: String,
    pub resource: String,
    pub effect: String,
}

impl ReglePermission {
    pub fn nouvelle(
        action: impl Into<String>,
        resource: impl Into<String>,
        effect: impl Into<String>,
    ) -> Self {
        ReglePermission {
            action: action.into(),
            resource: resource.into(),
            effect: effect.into(),
        }
    }
}

/// Fiche d identite d un des sept agents du plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FicheAgent {
    pub id: String,
    pub description: String,
    pub mode: ModeAgent,
    pub hidden: bool,
}

impl FicheAgent {
    pub fn nouvelle(
        id: impl Into<String>,
        description: impl Into<String>,
        mode: ModeAgent,
        hidden: bool,
    ) -> Self {
        FicheAgent {
            id: id.into(),
            description: description.into(),
            mode,
            hidden,
        }
    }
}

/// Les sept agents declares par le plugin, dans l ordre de la source.
pub fn fiches_agents() -> Vec<FicheAgent> {
    vec![
        FicheAgent::nouvelle(
            "default",
            "The default agent. Executes tools based on configured permissions.",
            ModeAgent::Principal,
            false,
        ),
        FicheAgent::nouvelle("plan", "Plan mode. Disallows all edit tools.", ModeAgent::Principal, false),
        FicheAgent::nouvelle(
            "general",
            "General-purpose agent for researching complex questions and executing multi-step tasks. Use this agent to execute multiple units of work in parallel.",
            ModeAgent::SousAgent,
            false,
        ),
        FicheAgent::nouvelle(
            "explore",
            "Fast agent specialized for exploring codebases.",
            ModeAgent::SousAgent,
            false,
        ),
        FicheAgent::nouvelle("compaction", "Context summarization agent.", ModeAgent::Principal, true),
        FicheAgent::nouvelle("title", "Title generator.", ModeAgent::Principal, true),
        FicheAgent::nouvelle("summary", "Conversation summarizer.", ModeAgent::Principal, true),
    ]
}

/// Jeu `defaults` de la source : tout autorise, sauf `question`,
/// `plan_enter` et `plan_exit`, avec `read` ouvert mais `*.env` sur demande.
///
/// `whitelistedDirs` (`tool-output/*` et `tmp/*`) et `readonlyExternalDirectory`
/// dependent de `Global.Path` et ne sont pas calculables en pur : ils sont
/// representes par les deux entrees parametrees de cette fonction.
pub fn permissions_defaut(sortie_outil_glob: &str, tmp_glob: &str) -> Vec<ReglePermission> {
    vec![
        ReglePermission::nouvelle("*", "*", "allow"),
        ReglePermission::nouvelle("external_directory", "*", "ask"),
        ReglePermission::nouvelle("external_directory", sortie_outil_glob, "allow"),
        ReglePermission::nouvelle("external_directory", tmp_glob, "allow"),
        ReglePermission::nouvelle("question", "*", "deny"),
        ReglePermission::nouvelle("plan_enter", "*", "deny"),
        ReglePermission::nouvelle("plan_exit", "*", "deny"),
        ReglePermission::nouvelle("read", "*", "allow"),
        ReglePermission::nouvelle("read", "*.env", "ask"),
        ReglePermission::nouvelle("read", "*.env.*", "ask"),
        ReglePermission::nouvelle("read", "*.env.example", "allow"),
    ]
}

/// Suffixe du glob de troncature sous un repertoire de donnees :
/// `<data>/tool-output/*`.
pub fn glob_troncature(repertoire_donnees: &str) -> String {
    let base = repertoire_donnees.trim_end_matches(['/', '\\']);
    format!("{}/tool-output/*", base)
}

/// Vrai si une regle refuse tout (`action "*"`, `effect "deny"`).
pub fn est_refus_total(regle: &ReglePermission) -> bool {
    regle.action == "*" && regle.effect == "deny"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_plugin_s_annonce_comme_agent() {
        assert_eq!(ID, "agent");
    }

    #[test]
    fn les_sept_agents_sont_declares_dans_l_ordre_avec_leurs_modes() {
        let fiches = fiches_agents();
        let ids: Vec<&str> = fiches.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["default", "plan", "general", "explore", "compaction", "title", "summary"]);
        assert_eq!(fiches[0].mode, ModeAgent::Principal);
        assert_eq!(fiches[2].mode, ModeAgent::SousAgent);
        assert_eq!(fiches[3].mode, ModeAgent::SousAgent);
        assert!(fiches[4].hidden);
        assert!(fiches[5].hidden);
        assert!(fiches[6].hidden);
        assert!(!fiches[0].hidden);
    }

    #[test]
    fn le_systeme_de_construction_est_celui_de_la_source() {
        assert!(SYSTEME_CONSTRUCTION.starts_with("You are an AI coding agent."));
        assert!(PROMPT_EXPLORATION.contains("file search specialist"));
        assert!(PROMPT_COMPACTION.contains("context summarization agent"));
    }

    #[test]
    fn les_permissions_par_defaut_refusent_question_et_plan() {
        let regles = permissions_defaut("/data/tool-output/*", "/tmp/*");
        assert!(regles.contains(&ReglePermission::nouvelle("question", "*", "deny")));
        assert!(regles.contains(&ReglePermission::nouvelle("plan_enter", "*", "deny")));
        assert!(regles.contains(&ReglePermission::nouvelle("plan_exit", "*", "deny")));
        assert!(regles.contains(&ReglePermission::nouvelle("read", "*.env", "ask")));
        assert!(regles.contains(&ReglePermission::nouvelle("read", "*.env.example", "allow")));
    }

    #[test]
    fn le_glob_de_troncature_joint_tool_output_au_dossier_de_donnees() {
        assert_eq!(glob_troncature("/data"), "/data/tool-output/*");
        assert_eq!(glob_troncature("/data/"), "/data/tool-output/*");
    }

    #[test]
    fn le_refus_total_ne_concerne_que_l_etoile_refusee() {
        assert!(est_refus_total(&ReglePermission::nouvelle("*", "*", "deny")));
        assert!(!est_refus_total(&ReglePermission::nouvelle("*", "*", "allow")));
        assert!(!est_refus_total(&ReglePermission::nouvelle("read", "*", "deny")));
    }

    #[test]
    fn les_modes_se_serialisent_en_minuscules() {
        assert_eq!(serde_json::to_string(&ModeAgent::Principal).unwrap(), "\"primary\"");
        assert_eq!(serde_json::to_string(&ModeAgent::SousAgent).unwrap(), "\"subagent\"");
    }
}
