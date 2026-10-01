# swarm-oc2-git

===DEBUT===
fichier : src/core/git.rs
source : opencode/packages/core/src/git.ts
taille : 18200 octets
tests : 7

```rust
//! Portage Rust de la logique pure de `opencode/packages/core/src/git.ts`.
//!
//! La source fait 987 lignes et melange deux choses :
//! - un modele de donnees (Repository, Worktree, erreurs, entrees des appels) ;
//! - une mecanique d effets (Effect, ChildProcess, verrous, acces disque).
//!
//! Ce fichier ne porte que le modele et les fonctions pures qui se deduisent
//! ligne a ligne de la source. Tout ce qui lance un processus `git`, touche
//! le disque ou utilise un verrou n est pas porte ici. La liste exacte est
//! dans le rapport sous POINT FAIBLE.
//!
//! Conventions suivies :
//! - `Schema.Class` devient un `struct` avec `Serialize, Deserialize` ;
//! - chaque variante d enum porte son `rename` explicite, sans `rename_all` ;
//! - chaque champ `camelCase` porte son `rename` explicite ;
//! - chaque `Option` porte `skip_serializing_if = "Option::is_none"` ;
//! - `readonly Set` devient `BTreeSet`, `readonly Map` devient `BTreeMap` ;
//! - tous les commentaires sont en francais sans accents, ASCII seul.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Profondeur de clone par defaut (`input.depth ?? 100`, git.ts:270).
pub const DEFAULT_CLONE_DEPTH: u32 = 100;

/// Nom de remote par defaut (`name = "origin"`, `remote ?? "origin"`).
pub const DEFAULT_REMOTE: &str = "origin";

/// Contexte de diff par defaut (`input.context ?? 3`, git.ts:601).
pub const DEFAULT_DIFF_CONTEXT: u32 = 3;

/// Preavis utilise par `git diff --no-index` : le code 1 veut dire
/// "des differences ont ete trouvees", pas "echec" (git.ts:771-772).
pub fn is_no_index_success(exit_code: i64) -> bool {
    exit_code == 0 || exit_code == 1
}

// ---------------------------------------------------------------------------
// Types marques (brands)
// ---------------------------------------------------------------------------

/// `ChangeSet` : patch texte capture par `Git.change.capture` (git.ts:20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChangeSet(pub String);

impl ChangeSet {
    pub fn make(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `TreeID` : identifiant d arbre rendu par `write-tree` (git.ts:23).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TreeID(pub String);

impl TreeID {
    pub fn make(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Repository et Worktree
// ---------------------------------------------------------------------------

/// `Repository` (git.ts:14-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub worktree: String,
    #[serde(rename = "gitDirectory")]
    pub git_directory: String,
    #[serde(rename = "commonDirectory")]
    pub common_directory: String,
}

impl Repository {
    pub fn new(worktree: String, git_directory: String, common_directory: String) -> Self {
        Self {
            worktree,
            git_directory,
            common_directory,
        }
    }
}

/// Sorte de worktree : `"main"` pour la premiere ligne de
/// `git worktree list --porcelain`, `"linked"` pour les suivantes
/// (git.ts:914-923).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorktreeKind {
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "linked")]
    Linked,
}

/// `Worktree` (git.ts:44-47).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    pub directory: String,
    pub kind: WorktreeKind,
}

// ---------------------------------------------------------------------------
// Erreurs
// ---------------------------------------------------------------------------

/// Valeurs de `OperationError["operation"]` (git.ts:27-40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    #[serde(rename = "clone")]
    Clone,
    #[serde(rename = "fetch")]
    Fetch,
    #[serde(rename = "checkout")]
    Checkout,
    #[serde(rename = "reset")]
    Reset,
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "refresh")]
    Refresh,
    #[serde(rename = "write_tree")]
    WriteTree,
    #[serde(rename = "list_files")]
    ListFiles,
    #[serde(rename = "diff")]
    Diff,
    #[serde(rename = "restore")]
    Restore,
}

/// `Git.OperationError` (git.ts:26-42). Le tag Effect (`_tag`) n est pas
/// serialise : il est expose par `TAG` et `tag()`, comme dans les autres
/// portages du projet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationError {
    pub operation: Operation,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl OperationError {
    pub const TAG: &'static str = "Git.OperationError";
    pub fn tag(&self) -> &'static str {
        Self::TAG
    }
}

/// Valeurs de `WorktreeError["operation"]` (git.ts:50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorktreeOperation {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "remove")]
    Remove,
    #[serde(rename = "list")]
    List,
}

/// `Git.WorktreeError` (git.ts:49-55).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeError {
    pub operation: WorktreeOperation,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "forceRequired")]
    pub force_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl WorktreeError {
    pub const TAG: &'static str = "Git.WorktreeError";
    pub fn tag(&self) -> &'static str {
        Self::TAG
    }
}

/// Valeurs de `PatchError["operation"]` (git.ts:58).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchOperation {
    #[serde(rename = "capture")]
    Capture,
    #[serde(rename = "apply")]
    Apply,
    #[serde(rename = "reset")]
    Reset,
}

/// `Git.PatchError` (git.ts:57-62). `directory` est obligatoire ici,
/// contrairement aux deux autres erreurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchError {
    pub operation: PatchOperation,
    pub directory: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl PatchError {
    pub const TAG: &'static str = "Git.PatchError";
    pub fn tag(&self) -> &'static str {
        Self::TAG
    }
}

// ---------------------------------------------------------------------------
// Diff minimal (miroir de File.Diff, defini dans file.ts)
// ---------------------------------------------------------------------------

/// Statut d un fichier dans un diff. La source produit des chaines nues
/// `"added" | "deleted" | "modified"` (git.ts:586), donc pas de `tag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "modified")]
    Modified,
}

/// Miroir minimal de `File.Diff`, construit dans `treeDiff` (git.ts:608-614).
/// La definition de reference vit dans `file.ts` et n est pas dupliquee ici
/// au-dela de ce miroir de lecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub additions: u64,
    pub deletions: u64,
    pub patch: String,
}

// ---------------------------------------------------------------------------
// Entrees des appels (Interface, git.ts:64-171, sans les effets)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneInput {
    pub remote: String,
    pub directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchRemotesInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchBranchInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutBranchInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshInput {
    pub repository: Repository,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignores: Option<Repository>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "maximumUntrackedFileBytes")]
    pub maximum_untracked_file_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeDiffInput {
    pub repository: Repository,
    pub from: TreeID,
    pub to: TreeID,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

/// `files` est une `ReadonlyMap<RelativePath, TreeID` : on prend une
/// `BTreeMap` deterministe, comme l impose la charte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewInput {
    pub repository: Repository,
    pub current: TreeID,
    pub files: BTreeMap<String, TreeID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreFilesInput {
    pub repository: Repository,
    pub files: BTreeMap<String, TreeID>,
}

/// Mode `index` de `change.discard` (git.ts:819).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardIndex {
    #[serde(rename = "preserve")]
    Preserve,
    #[serde(rename = "reset")]
    Reset,
}

/// Mode `untracked` de `change.discard` (git.ts:820).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardUntracked {
    #[serde(rename = "preserve")]
    Preserve,
    #[serde(rename = "remove")]
    Remove,
}

// ---------------------------------------------------------------------------
// Petites fonctions pures
// ---------------------------------------------------------------------------

/// Supprime les `\r` et `\n` de fin, comme `value.replace(/[\r\n]+$/, "")`
/// (git.ts:982). Seule la fin est touchee, pas le debut.
pub fn trim_trailing_newlines(value: &str) -> &str {
    value.trim_end_matches(['\r', '\n'])
}

/// Equivalent pur de `FSUtil.windowsPath` (fs-util.ts:257-264) : sur win32 la
/// source reecrit `/c:`, `/c/`, `/cygdrive/c`, `/mnt/c` en `C:/`. Ici on
/// applique la meme reecriture de facon deterministe, sans tester la
/// plate-forme, pour que le resultat ne depende pas de la machine.
pub fn windows_path(p: &str) -> String {
    let after_cygdrive = p
        .strip_prefix("/cygdrive/")
        .or_else(|| p.strip_prefix("/mnt/"))
        .unwrap_or(p);
    let s = after_cygdrive;
    if s.len() >= 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[1].is_ascii_alphabetic() {
        let drive = s.as_bytes()[1].to_ascii_uppercase() as char;
        let rest = &s[2..];
        if rest.is_empty() || rest == ":" || rest.starts_with(':') || rest.starts_with('/') {
            let tail = if rest.starts_with(':') { &rest[1..] } else { rest };
            let tail = tail.strip_prefix('/').unwrap_or(tail);
            if tail.is_empty() {
                return format!("{drive}:/");
            }
            return format!("{drive}:/{tail}");
        }
    }
    s.to_string()
}

/// Dit si un chemin est absolu, au sens large (POSIX ou lecteur Windows).
fn is_absolute_path(p: &str) -> bool {
    if p.starts_with('/') || p.starts_with('\\') {
        return true;
    }
    let b = p.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\') {
        return true;
    }
    if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return true;
    }
    false
}

/// Normalisation simplifiee : coupe sur `/` et `\`, resout `.` et `..`.
fn normalize_parts(raw: &str) -> String {
    let is_abs = is_absolute_path(raw);
    let mut parts: Vec<&str> = Vec::new();
    for seg in raw.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        } else if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    let joined = parts.join("/");
    if is_abs {
        if joined.is_empty() {
            "/".to_string()
        } else if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
            joined
        } else {
            format!("/{joined}")
        }
    } else {
        joined
    }
}

/// `resolvePath` (git.ts:981-987) : sans les appels Node, en pur.
/// Chaine vide apres rognage donne `cwd`. Sinon on normalise puis on
/// renvoie le chemin absolu ou `cwd` joint au relatif.
pub fn resolve_path(cwd: &str, value: &str) -> String {
    let trimmed = trim_trailing_newlines(value);
    if trimmed.is_empty() {
        return cwd.to_string();
    }
    let normalized = windows_path(trimmed);
    if is_absolute_path(&normalized) {
        return normalize_parts(&normalized);
    }
    let joined = format!("{}/{}", cwd.trim_end_matches(['/', '\\']), normalized);
    normalize_parts(&joined)
}

/// `result.text.trim() || undefined` (git.ts:208, 224, 230) : une chaine
/// vide ou blanche donne `None`. C est le test de veracite (`||`), pas le
/// coalescent (`??`) : `Some("")` ne survit pas ici.
pub fn non_empty_trimmed(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// `roots` (git.ts:211-219) : decoupe sur `\n`, rogne, jette les vides,
/// trie (`toSorted`).
pub fn parse_root_commits(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split('\n')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    out.sort();
    out
}

/// Partie pure de `remoteHead` (git.ts:233-240) : rogne puis retire le
/// prefixe `refs/remotes/<remote>/`. Chaine vide finale donne `None`.
pub fn default_remote_branch(text: &str, remote: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let prefix = format!("refs/remotes/{remote}/");
    let stripped = t.strip_prefix(&prefix).unwrap_or(t);
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

/// `effective_depth` : `input.depth ?? 100` (git.ts:270). Le coalescent
/// garde `0` : `Some(0)` donne `0`, seul `None` donne `100`.
pub fn effective_depth(depth: Option<u32>) -> u32 {
    depth.unwrap_or(DEFAULT_CLONE_DEPTH)
}

/// `input.remote ?? "origin"` (git.ts:296, 305).
pub fn effective_remote(remote: Option<&str>) -> String {
    remote.unwrap_or(DEFAULT_REMOTE).to_string()
}

/// `input.context ?? 3` (git.ts:601). Comme pour `depth`, `Some(0)`
/// donne `0`.
pub fn effective_context(context: Option<u32>) -> u32 {
    context.unwrap_or(DEFAULT_DIFF_CONTEXT)
}

/// Classification de `treeDiff` (git.ts:586) :
/// ligne qui commence par `A` -> ajoute, par `D` -> supprime, sinon modifie.
pub fn classify_file_status(status_text: &str) -> FileStatus {
    if status_text.starts_with('A') {
        FileStatus::Added
    } else if status_text.starts_with('D') {
        FileStatus::Deleted
    } else {
        FileStatus::Modified
    }
}

/// Resultat de `--numstat` : `stats[0] === "-" || stats[1] === "-"`
/// veut dire binaire, et dans ce cas ajouts et suppressions valent 0
/// (git.ts:595-612). `Number(stats[0] ?? 0)` donne 0 si absent ou invalide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Numstat {
    pub additions: u64,
    pub deletions: u64,
    pub binary: bool,
}

pub fn parse_numstat(text: &str) -> Numstat {
    let mut cols = text.split('\t');
    let a = cols.next().unwrap_or("").trim();
    let b = cols.next().unwrap_or("").trim();
    let binary = a == "-" || b == "-";
    if binary {
        return Numstat {
            additions: 0,
            deletions: 0,
            binary: true,
        };
    }
    Numstat {
        additions: a.parse::<u64>().unwrap_or(0),
        deletions: b.parse::<u64>().unwrap_or(0),
        binary: false,
    }
}

/// Entree lue par `ls-tree -z` (git.ts:619-636).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub object: String,
}

/// Partie pure de `entry` : retire le `\0` final, rend `None` si vide,
/// sinon extrait `(mode, object)` selon `/^(\d+)\s+\w+\s+([0-9a-f]+)\t/`.
/// Retourne `Err` avec le message `Invalid tree entry for <file>` si le
/// format ne colle pas. Sans la crate `regex`, l analyse est manuelle.
pub fn parse_tree_entry(text: &str, file: &str) -> Result<Option<TreeEntry>, String> {
    let body = text.strip_suffix('\0').unwrap_or(text);
    if body.is_empty() {
        return Ok(None);
    }
    let tab = body.find('\t').ok_or_else(|| format!("Invalid tree entry for {file}"))?;
    let head = &body[..tab];
    let mut words = head.split_whitespace();
    let mode = words.next().ok_or_else(|| format!("Invalid tree entry for {file}"))?;
    let kind = words.next().ok_or_else(|| format!("Invalid tree entry for {file}"))?;
    let object = words.next().ok_or_else(|| format!("Invalid tree entry for {file}"))?;
    if mode.is_empty() || !mode.bytes().all(|c| c.is_ascii_digit()) {
        return Err(format!("Invalid tree entry for {file}"));
    }
    if kind.is_empty() || !kind.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err(format!("Invalid tree entry for {file}"));
    }
    if object.is_empty()
        || !object.bytes().all(|c| c.is_ascii_hexdigit() && (c.is_ascii_digit() || c.is_ascii_lowercase()))
    {
        return Err(format!("Invalid tree entry for {file}"));
    }
    Ok(Some(TreeEntry {
        mode: mode.to_string(),
        object: object.to_string(),
    }))
}

/// `worktreeList` (git.ts:912-923) : garde les lignes qui commencent par
/// `"worktree "`, resout le chemin contre `base`, la premiere ligne
/// gardee est `"main"`, les suivantes `"linked"`.
pub fn parse_worktree_list(output: &str, base: &str) -> Vec<Worktree> {
    output
        .split('\n')
        .filter(|line| line.starts_with("worktree "))
        .map(|line| line["worktree ".len()..].trim())
        .enumerate()
        .map(|(index, raw)| Worktree {
            directory: resolve_path(base, raw),
            kind: if index == 0 {
                WorktreeKind::Main
            } else {
                WorktreeKind::Linked
            },
        })
        .collect()
}

/// `scope` de `capture` et `discard` (git.ts:730, 822) :
/// relatif a `worktree`, antislashs vers `/`, vide donne `"."`.
pub fn scope_for_path(worktree: &str, abs_path: &str) -> String {
    let rel = relative_path(worktree, abs_path).replace('\\', "/");
    if rel.is_empty() {
        ".".to_string()
    } else {
        rel
    }
}

/// Relatif simplifie, sans acces disque : retire le prefixe `base + "/"`.
/// Si `path == base`, rend `""` pour que l appelant mette `"."`.
fn relative_path(base: &str, path: &str) -> String {
    let norm_base = base.trim_end_matches(['/', '\\']).replace('\\', "/");
    let norm_path = path.replace('\\', "/");
    if norm_path == norm_base {
        return String::new();
    }
    if let Some(rest) = norm_path.strip_prefix(&format!("{norm_base}/")) {
        return rest.to_string();
    }
    norm_path
}

/// `text.split("\0").filter(Boolean)` (git.ts:437, 521, 763).
pub fn split_nul_list(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// `items.join("\0") + "\0"` utilise comme `stdin` (git.ts:451, 479, 486).
pub fn join_nul_stdin(items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = items.join("\0");
    out.push('\0');
    out
}

/// Arguments de `clone` (git.ts:267-275).
pub fn build_clone_args(remote: &str, directory: &str, branch: Option<&str>, depth: Option<u32>) -> Vec<String> {
    let mut args = vec![
        "clone".to_string(),
        "--depth".to_string(),
        effective_depth(depth).to_string(),
    ];
    if let Some(b) = branch {
        args.push("--branch".to_string());
        args.push(b.to_string());
    }
    args.push("--".to_string());
    args.push(remote.to_string());
    args.push(directory.to_string());
    args
}

/// Arguments de `fetch --all` (git.ts:289). `prune` vaut vrai par defaut :
/// seul `Some(false)` retire `--prune` (`input.prune === false`).
pub fn build_fetch_remotes_args(prune: Option<bool>) -> Vec<String> {
    let mut args = vec!["fetch".to_string(), "--all".to_string()];
    if prune != Some(false) {
        args.push("--prune".to_string());
    }
    args
}

/// Arguments de `fetchBranch` (git.ts:292-299). `force` vaut vrai par
/// defaut : seul `Some(false)` retire le `+` (`input.force === false`).
pub fn build_fetch_branch_args(remote: Option<&str>, branch: &str, force: Option<bool>) -> Vec<String> {
    let remote_name = effective_remote(remote);
    let spec = format!("refs/heads/{branch}:refs/remotes/{remote_name}/{branch}");
    let refspec = if force == Some(false) { spec } else { format!("+{spec}") };
    vec!["fetch".to_string(), remote_name, refspec]
}

/// Arguments de `checkoutRemoteBranch` (git.ts:301-310). `reset` vaut vrai
/// par defaut : seul `Some(false)` donne le mode simple.
pub fn build_checkout_args(remote: Option<&str>, branch: &str, reset: Option<bool>) -> Vec<String> {
    let mut args = vec!["checkout".to_string()];
    if reset == Some(false) {
        args.push(branch.to_string());
    } else {
        let remote_name = effective_remote(remote);
        args.push("-B".to_string());
        args.push(branch.to_string());
        args.push(format!("{remote_name}/{branch}"));
    }
    args
}

/// Arguments de `resetHard` (git.ts:312-314).
pub fn build_reset_args(revision: &str) -> Vec<String> {
    vec!["reset".to_string(), "--hard".to_string(), revision.to_string()]
}

/// Arguments de restauration dans `discard` (git.ts:823-826) :
/// `reset` donne `checkout HEAD -- <scope>`, sinon `checkout -- <scope>`.
pub fn build_discard_restore_args(scope: &str, index: DiscardIndex) -> Vec<String> {
    match index {
        DiscardIndex::Reset => vec![
            "checkout".to_string(),
            "HEAD".to_string(),
            "--".to_string(),
            scope.to_string(),
        ],
        DiscardIndex::Preserve => vec!["checkout".to_string(), "--".to_string(), scope.to_string()],
    }
}

/// Detection de `forceRequired` dans `worktreeRun` (git.ts:875) :
/// seulement pour `remove`, insensible a la casse, sur l un des deux
/// motifs `contains modified or untracked files` ou `is dirty`.
pub fn is_force_required(operation: WorktreeOperation, message: &str) -> bool {
    if operation != WorktreeOperation::Remove {
        return false;
    }
    let lower = message.to_ascii_lowercase();
    lower.contains("contains modified or untracked files") || lower.contains("is dirty")
}

/// `ChangeSet.make([tracked.text, ...created].filter(Boolean).join("\n"))`
/// (git.ts:786).
pub fn combine_changeset(tracked: &str, created: &[String]) -> ChangeSet {
    let mut parts: Vec<&str> = Vec::new();
    if !tracked.is_empty() {
        parts.push(tracked);
    }
    for c in created {
        if !c.is_empty() {
            parts.push(c);
        }
    }
    ChangeSet::make(parts.join("\n"))
}

/// Message d erreur quand git sort en echec (git.ts:257, 354-355, 517-519) :
/// `stderr` rogne, sinon `text` rogne, sinon `Git <operation> failed`.
pub fn error_message_fallback(stderr: &str, text: &str, operation: &str) -> String {
    let s = stderr.trim();
    if !s.is_empty() {
        return s.to_string();
    }
    let t = text.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    format!("Git {operation} failed")
}

/// Decoupe pure de `refresh` (git.ts:472-473) : `stage` garde les candidats
/// autorises qui ne sont pas dans `skipped`, `remove` regroupe `ignored`
/// et `skipped`. La mesure des tailles sur disque n est pas ici.
pub fn partition_refresh_stage(
    allowed: &[String],
    ignored: &BTreeSet<String>,
    skipped: &[String],
) -> (Vec<String>, Vec<String>) {
    let skipped_set: BTreeSet<&String> = skipped.iter().collect();
    let stage: Vec<String> = allowed
        .iter()
        .filter(|item| !skipped_set.contains(item))
        .cloned()
        .collect();
    let mut remove: Vec<String> = ignored.iter().cloned().collect();
    remove.extend(skipped.iter().cloned());
    (stage, remove)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_serialisation_garde_les_noms_git_directory_et_common_directory() {
        let repo = Repository::new("/w".to_string(), "/g".to_string(), "/c".to_string());
        let v = serde_json::to_value(&repo).expect("serialisation");
        assert_eq!(v.get("worktree").and_then(|x| x.as_str()), Some("/w"));
        assert_eq!(v.get("gitDirectory").and_then(|x| x.as_str()), Some("/g"));
        assert_eq!(v.get("commonDirectory").and_then(|x| x.as_str()), Some("/c"));
        assert!(v.get("git_directory").is_none());
        let err = OperationError {
            operation: Operation::WriteTree,
            message: "x".to_string(),
            directory: None,
            cause: None,
        };
        let j = serde_json::to_value(&err).expect("serialisation");
        assert_eq!(j.get("operation").and_then(|x| x.as_str()), Some("write_tree"));
        assert!(j.get("directory").is_none());
        assert!(j.get("cause").is_none());
    }

    #[test]
    fn zero_survit_au_coalescent_mais_chaine_vide_devient_none() {
        assert_eq!(effective_depth(Some(0)), 0);
        assert_eq!(effective_depth(None), 100);
        assert_eq!(effective_context(Some(0)), 0);
        assert_eq!(effective_context(None), 3);
        assert_eq!(non_empty_trimmed(""), None);
        assert_eq!(non_empty_trimmed("   "), None);
        assert_eq!(non_empty_trimmed("  abc  "), Some("abc".to_string()));
        assert_eq!(effective_remote(None), "origin");
        assert_eq!(effective_remote(Some("upstream")), "upstream");
    }

    #[test]
    fn les_commits_racines_sont_filtres_rognes_et_tries() {
        let got = parse_root_commits("ccc\n\n  aaa \nbbb\n");
        assert_eq!(got, vec!["aaa".to_string(), "bbb".to_string(), "ccc".to_string()]);
        assert!(parse_root_commits("  \n ").is_empty());
        assert_eq!(
            default_remote_branch("refs/remotes/origin/main", "origin"),
            Some("main".to_string())
        );
        assert_eq!(default_remote_branch("refs/remotes/origin/", "origin"), None);
        assert_eq!(default_remote_branch("  ", "origin"), None);
    }

    #[test]
    fn le_statut_et_le_numstat_suivent_la_source() {
        assert_eq!(classify_file_status("A\tf"), FileStatus::Added);
        assert_eq!(classify_file_status("D\tf"), FileStatus::Deleted);
        assert_eq!(classify_file_status("M\tf"), FileStatus::Modified);
        assert_eq!(classify_file_status(""), FileStatus::Modified);
        let n = parse_numstat("10\t4\t");
        assert_eq!(n.additions, 10);
        assert_eq!(n.deletions, 4);
        assert!(!n.binary);
        let b = parse_numstat("-\t-\tbinaire");
        assert!(b.binary);
        assert_eq!(b.additions, 0);
        assert_eq!(b.deletions, 0);
    }

    #[test]
    fn l_entree_d_arbre_donne_mode_et_objet_ou_erreur() {
        let ok = parse_tree_entry("100644 blob abc123def456\tfile", "file").expect("valide");
        assert_eq!(
            ok,
            Some(TreeEntry {
                mode: "100644".to_string(),
                object: "abc123def456".to_string()
            })
        );
        assert_eq!(parse_tree_entry("", "file").expect("vide"), None);
        assert!(parse_tree_entry("n importe quoi", "file").is_err());
        assert!(parse_tree_entry("100644 blob ABCDEF\tfile", "file").is_err());
    }

    #[test]
    fn les_arguments_par_defaut_suivent_les_ternaires_source() {
        assert_eq!(
            build_clone_args("r", "d", None, None)[2],
            "100".to_string()
        );
        assert_eq!(build_clone_args("r", "d", None, Some(0))[2], "0".to_string());
        assert!(build_fetch_remotes_args(None).contains(&"--prune".to_string()));
        assert!(!build_fetch_remotes_args(Some(false)).contains(&"--prune".to_string()));
        assert_eq!(
            build_checkout_args(Some("origin"), "main", Some(false)),
            vec!["checkout".to_string(), "main".to_string()]
        );
        assert_eq!(
            build_fetch_branch_args(None, "main", None)[2],
            "+refs/heads/main:refs/remotes/origin/main".to_string()
        );
        assert_eq!(
            build_discard_restore_args(".", DiscardIndex::Reset)[1],
            "HEAD".to_string()
        );
    }

    #[test]
    fn worktree_scope_et_force_suivent_la_source() {
        let list = parse_worktree_list("worktree /a\nworktree /a/b\n", "/a");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].kind, WorktreeKind::Main);
        assert_eq!(list[1].kind, WorktreeKind::Linked);
        assert_eq!(scope_for_path("/a", "/a"), ".");
        assert_eq!(scope_for_path("/a", "/a/x/y"), "x/y");
        assert!(is_force_required(
            WorktreeOperation::Remove,
            "fatal: '/x' IS DIRTY"
        ));
        assert!(!is_force_required(WorktreeOperation::List, "is dirty"));
        assert_eq!(split_nul_list("a\0\0b\0"), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(join_nul_stdin(&["a".to_string()]), "a\0");
        let cs = combine_changeset("t", &["".to_string(), "u".to_string()]);
        assert_eq!(cs.as_str(), "t\nu");
    }
}
```

===FIN===

CONFIANCE : moyenne
POINT FAIBLE : tout ce qui lance un processus ou touche le disque n est pas porte : discover, run, execute, operation, repositoryOperation, clone, fetchRemotes, fetchBranch, checkoutRemoteBranch, resetHard, create avec ses configs et alternates, refresh avec rm et add, ignored avec check-ignore, writeTree, captureTree sous verrou, treeFiles, treeDiff avec ses trois appels par fichier, entry hors parsing pur, preview avec index temporaire et GIT_INDEX_FILE, restore, checkoutTree, capture avec diff binaire et no-index, apply avec git apply, discard avec checkout et clean, worktreeRun, worktreeCreate, worktreeRemove, worktreeList hors parsing pur, Service, Layer, node, KeyedMutex et verrou par gitDirectory ; le reexport `export * as Git from "./git"` est du code mort volontairement non porte ; incertitudes restantes : resolve_path approxime path.normalize et path.resolve de Node, windowsPath est applique sans test de plate-forme, parse_tree_entry refuse les hash en majuscules comme la regex source mais sans regex, et cause de type Defect est mappe en Option<String>.
A VERIFIER : les rename gitDirectory, commonDirectory, forceRequired, maximumUntrackedFileBytes, write_tree, list_files et les trois tags Git.OperationError, Git.WorktreeError, Git.PatchError ; la semantique Some(0) garde face a chaine vide qui devient None ; le premier worktree liste qui est main et les suivants linked.
