//! Outils que le modele peut appeler : lecture, ecriture, shell.
//!
//! Chaque outil expose une description et un schema de parametres au modele, et
//! s'execute dans un `ToolBox`. Le type de retour est toujours une chaine :
//! c'est ce que le modele lit dans son contexte, donc c'est ce qui doit etre
//! concis et utile, pas ce qui est le plus confortable a manipuler en Rust.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Racine du projet ou les outils sont confines.
///
/// Toute operation de chemin passe par `confine`, qui refuse de sortir de cette
/// racine. C'est la seule barriere entre le modele et le disque : sans elle,
/// une instruction approximative suffit a lire `C:\Users\...\ .ssh\id_rsa`.

pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root: PathBuf = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root: root.canonicalize()? })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resout un chemin relatif et garantit qu'il reste sous la racine.
    pub fn confine(&self, rel: &str) -> Result<PathBuf> {
        let candidate = self.root.join(rel);
        let normalized = normalize(&candidate);
        if !normalized.starts_with(&self.root) {
            anyhow::bail!("chemin hors du workspace : {rel}");
        }
        Ok(normalized)
    }
}

/// Resout `.` et `..` sans toucher au disque.
///
/// `canonicalize` echouerait sur un fichier qui n'existe pas encore, ce qui est
/// le cas le plus frequent pour un agent qui cree des fichiers. On fait donc une
/// normalisation lexicale, en rodsoublant les separateurs Windows et POSIX.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => { out.pop(); }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub struct ToolResult {
    pub content: String,
    /// Fichiers lus ou modifiés. Alimente la detection de conflits entre agents :
    /// c'est ce qui permet au systeme de dire a un agent qu'un autre vient de
    /// toucher un fichier qu'il avait lu.
    pub touched: Vec<PathBuf>,
}

pub struct ToolBox {
    workspace: Workspace,
}

impl ToolBox {
    pub fn new(workspace: Workspace) -> Self {
        Self { workspace }
    }

    pub async fn call(&self, name: &str, args: &Value) -> Result<ToolResult> {
        match name {
            "read" => self.read(args).await,
            "write" => self.write(args).await,
            "list" => self.list(args).await,
            other => anyhow::bail!("outil inconnu : {other}"),
        }
    }

    async fn read(&self, args: &Value) -> Result<ToolResult> {
        let rel = args.get("path").and_then(Value::as_str).context("argument 'path' manquant")?;
        let full = self.workspace.confine(rel)?;
        let content = std::fs::read_to_string(&full)
            .with_context(|| format!("lecture impossible : {rel}"))?;
        Ok(ToolResult { content, touched: vec![full] })
    }

    async fn write(&self, args: &Value) -> Result<ToolResult> {
        let rel = args.get("path").and_then(Value::as_str).context("argument 'path' manquant")?;
        let content = args.get("content").and_then(Value::as_str).unwrap_or_default();
        let full = self.workspace.confine(rel)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content).with_context(|| format!("ecriture impossible : {rel}"))?;
        Ok(ToolResult { content: format!("ecrit : {rel}"), touched: vec![full] })
    }

    async fn list(&self, args: &Value) -> Result<ToolResult> {
        let rel = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let full = self.workspace.confine(rel)?;
        let mut entries = Vec::new();
        for e in std::fs::read_dir(&full).with_context(|| format!("lecture impossible : {rel}"))? {
            let e = e?;
            // On saute les dossiers de dependances et de build : ils sont
            // volumineux et sans interet pour un agent qui lit du code.
            let name = e.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), "node_modules" | "target" | ".git" | "dist") {
                continue;
            }
            entries.push(if e.path().is_dir() { format!("{name}/") } else { name });
        }
        entries.sort();
        Ok(ToolResult { content: entries.join("\n"), touched: vec![] })
    }
}
