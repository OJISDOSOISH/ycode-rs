//! Portage Rust de `opencode/packages/core/src/snapshot.ts`.
//!
//! Logique metier pure uniquement, sans IO.
//! Les effets FS et git ne sont pas portes : capture reelle, acces
//! git.tree (files, diff, preview, restore, checkout), repository,
//! layer, locationLayer, node, Config, Location, Global, FSUtil.
//! Voir POINT FAIBLE du rapport pour la liste exacte.
//!
//! Ce qui est porte :
//! - Snapshot.ID -> SnapshotId
//! - Snapshot.Error et operation -> SnapshotError et Operation
//! - CompareInput, DiffInput, RestoreInput, PreviewInput -> structs serde
//! - LegacyFileDiff et status -> struct et enum serde
//! - failure, enabled, scope relatif, plan anti echappement,
//!   filtre des ignores, choix des chemins de diff, operation de checkout,
//!   valeurs par defaut du noop.
//!
//! Conventions :
//! - structs avec Serialize + Deserialize
//! - enum Operation avec rename explicite par variante
//! - champs Option avec skip_serializing_if
//! - la source ne contient aucun champ camelCase, donc aucun rename
//!   de champ n est requis ici.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Limite appliquee a la capture des fichiers non suivis (2 Mio).
pub const MAXIMUM_UNTRACKED_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Identifiant content-addressed d un arbre capture.
///
/// La source en fait une chaine brandee. On garde une chaine neuve
/// pour eviter les confusions avec les autres IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(pub String);

impl SnapshotId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Chemin relatif a la racine du projet, separateur slash.
///
/// La source normalise les anti-slash Windows vers slash et
/// utilise "." pour la racine elle-meme.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelativePath(pub String);

impl RelativePath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Operation a l origine d une erreur snapshot.
///
/// Chaque variante porte son rename explicite pour rester
/// compatible avec les literaux TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operation {
    #[serde(rename = "capture")]
    Capture,
    #[serde(rename = "files")]
    Files,
    #[serde(rename = "diff")]
    Diff,
    #[serde(rename = "preview")]
    Preview,
    #[serde(rename = "restore")]
    Restore,
}

/// Erreur du domaine snapshot.
///
/// La source ajoute une cause defect optionnelle. On la stocke
/// comme chaine de debug, car Rust ne transporte pas de defect nu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotError {
    pub operation: Operation,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// Entrees de comparaison entre deux arbres captures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareInput {
    pub from: SnapshotId,
    pub to: SnapshotId,
}

/// Entrees de diff entre deux arbres captures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffInput {
    pub from: SnapshotId,
    pub to: SnapshotId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<RelativePath>>,
}

/// Entrees de restauration selective.
///
/// La cle est le chemin relatif slash, la valeur l arbre source.
/// BTreeMap pour un ordre deterministe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreInput {
    pub files: BTreeMap<RelativePath, SnapshotId>,
}

/// Entrees de previsualisation de restauration selective.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewInput {
    pub files: BTreeMap<RelativePath, SnapshotId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<i64>,
}

/// Statut d un diff de fichier persiste (forme legacy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegacyDiffStatus {
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "modified")]
    Modified,
}

/// Forme legacy d un diff de fichier de session, gardee pour lecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyFileDiff {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    pub additions: i64,
    pub deletions: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<LegacyDiffStatus>,
}

/// Construit une erreur snapshot avec cause textuelle optionnelle.
pub fn failure(operation: Operation, message: impl Into<String>, cause: Option<String>) -> SnapshotError {
    SnapshotError {
        operation,
        message: message.into(),
        cause,
    }
}

/// Applique la regle de la source : si la cause est deja une erreur
/// snapshot de meme operation, on la garde telle quelle, sinon on
/// l emballe dans une nouvelle erreur de l operation demandee.
pub fn wrap_error(operation: Operation, existing: SnapshotError) -> SnapshotError {
    if existing.operation == operation {
        return existing;
    }
    let message = existing.message.clone();
    failure(operation, message, Some(format!("{:?}", existing)))
}

/// Dit si les snapshots sont actifs.
///
/// Regle d origine : le vcs doit etre git et l option de config
/// snapshots ne doit pas valoir false. Une option absente vaut actif.
pub fn is_enabled(vcs_type: Option<&str>, snapshots_flag: Option<bool>) -> bool {
    if vcs_type != Some("git") {
        return false;
    }
    snapshots_flag != Some(false)
}

/// L operation d erreur utilisee par checkout.
///
/// Detail piege de la source : checkout rapporte ses echecs sous
/// l operation restore, pas sous une operation checkout.
pub fn checkout_error_operation() -> Operation {
    Operation::Restore
}

/// Remplace les anti-slash par des slash.
fn normalize_sep(input: &str) -> String {
    input.replace('\\', "/")
}

/// Enleve les slash finaux sauf pour la racine bare.
fn trim_trailing_slash(input: &str) -> &str {
    let mut out = input;
    while out.len() > 1 && out.ends_with('/') {
        out = &out[..out.len() - 1];
    }
    out
}

/// Detecte un chemin absolu POSIX ou Windows apres normalisation.
fn is_absolute_path(normalized: &str) -> bool {
    if normalized.starts_with('/') {
        return true;
    }
    let bytes = normalized.as_bytes();
    bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'/' && bytes[0].is_ascii_alphabetic()
}

/// Joint un repertoire et un chemin relatif puis normalise les
/// segments point et point-point de facon lexicale, sans toucher au FS.
fn join_and_normalize(base: &str, rel: &str) -> String {
    let base_norm = normalize_sep(base);
    let rel_norm = normalize_sep(rel);
    let base_trim = trim_trailing_slash(base_norm.as_str());
    let combined = if rel_norm.is_empty() || rel_norm == "." {
        base_trim.to_string()
    } else if is_absolute_path(&rel_norm) {
        rel_norm
    } else {
        format!("{}/{}", base_trim, rel_norm)
    };
    let absolute = is_absolute_path(&combined);
    let mut parts: Vec<&str> = Vec::new();
    for seg in combined.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            if !parts.is_empty() {
                parts.pop();
            }
            continue;
        }
        parts.push(seg);
    }
    let mut out = parts.join("/");
    if absolute {
        // Preserve le prefixe racine ou lecteur.
        if combined.len() >= 3 && combined.as_bytes()[1] == b':' {
            let drive = &combined[..2];
            out = format!("{}/{}", drive, out);
        } else {
            out = format!("/{}", out);
        }
    }
    if out.is_empty() {
        out = ".".to_string();
    }
    out
}

/// Dit si un chemin absolu normalise reste dans le worktree.
fn contains_path(worktree: &str, absolute: &str) -> bool {
    let base = trim_trailing_slash(normalize_sep(worktree).as_str()).to_string();
    let target = trim_trailing_slash(normalize_sep(absolute).as_str()).to_string();
    target == base || target.starts_with(&format!("{}/", base))
}

/// Calcule le scope relatif d un repertoire de travail.
///
/// Reproduit scope() : chemin relatif du worktree vers le repertoire
/// courant, slash uniquement, "." pour egalite. Rejete tout ce qui
/// sort du projet par une erreur capture.
pub fn scope_relative(worktree: &str, location_dir: &str) -> Result<RelativePath, SnapshotError> {
    let base = trim_trailing_slash(normalize_sep(worktree).as_str()).to_string();
    let target = trim_trailing_slash(normalize_sep(location_dir).as_str()).to_string();
    if target == base {
        return Ok(RelativePath::new("."));
    }
    let prefix = format!("{}/", base);
    if let Some(rest) = target.strip_prefix(&prefix) {
        if rest.is_empty() {
            return Ok(RelativePath::new("."));
        }
        return Ok(RelativePath::new(rest.to_string()));
    }
    Err(failure(
        Operation::Capture,
        "Location is outside the project",
        None,
    ))
}

/// Valide un plan de restauration selective.
///
/// Reproduit plan() : chaque fichier doit rester dans le worktree
/// apres resolution. Un fichier qui echappe donne une erreur de
/// l operation demandee (preview ou restore).
pub fn plan_restore(
    files: &BTreeMap<RelativePath, SnapshotId>,
    worktree: &str,
    operation: Operation,
) -> Result<BTreeMap<RelativePath, SnapshotId>, SnapshotError> {
    let mut out = BTreeMap::new();
    for (file, snapshot) in files {
        let absolute = join_and_normalize(worktree, file.as_str());
        if !contains_path(worktree, &absolute) {
            return Err(failure(
                operation,
                format!("Path escapes the project: {}", file.as_str()),
                None,
            ));
        }
        out.insert(file.clone(), snapshot.clone());
    }
    Ok(out)
}

/// Enleve les fichiers ignores d une liste decouverte.
pub fn filter_ignored(all: &[RelativePath], ignored: &BTreeSet<RelativePath>) -> Vec<RelativePath> {
    all.iter()
        .filter(|file| !ignored.contains(*file))
        .cloned()
        .collect()
}

/// Choisit les chemins d un diff.
///
/// Reproduit la regle : chemins demandes s ils existent, sinon liste
/// decouverte, puis retrait des ignores dans les deux cas.
pub fn resolve_diff_paths(
    requested: Option<&[RelativePath]>,
    discovered: &[RelativePath],
    ignored: &BTreeSet<RelativePath>,
) -> Vec<RelativePath> {
    let base: &[RelativePath] = match requested {
        Some(paths) => paths,
        None => discovered,
    };
    filter_ignored(base, ignored)
}

/// Valeur capture du noop : aucune capture, donc rien a comparer.
pub fn noop_capture() -> Option<SnapshotId> {
    None
}

/// Liste vide du noop pour files, diff et preview.
pub fn noop_file_list() -> Vec<RelativePath> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(name: &str) -> RelativePath {
        RelativePath::new(name.to_string())
    }

    #[test]
    fn scope_identique_donne_point() {
        let got = scope_relative("/repo/proj", "/repo/proj").unwrap();
        assert_eq!(got, rel("."));
    }

    #[test]
    fn scope_hors_projet_est_rejete() {
        let err = scope_relative("/repo/proj", "/repo/autre").unwrap_err();
        assert_eq!(err.operation, Operation::Capture);
        assert!(scope_relative("/repo/proj", "/repo/proj/../autre").is_err());
    }

    #[test]
    fn plan_qui_echappe_est_rejete() {
        let mut files = BTreeMap::new();
        files.insert(rel("../secret.txt"), SnapshotId::new("abc"));
        let err = plan_restore(&files, "/repo/proj", Operation::Restore).unwrap_err();
        assert_eq!(err.operation, Operation::Restore);
        assert!(err.message.contains("../secret.txt"));
    }

    #[test]
    fn ignores_sont_filtres_y_compris_vide() {
        let empty: Vec<RelativePath> = Vec::new();
        let none = BTreeSet::new();
        assert!(filter_ignored(&empty, &none).is_empty());
        let all = vec![rel("a.txt"), rel("b.txt")];
        let mut ignored = BTreeSet::new();
        ignored.insert(rel("b.txt"));
        assert_eq!(filter_ignored(&all, &ignored), vec![rel("a.txt")]);
    }

    #[test]
    fn diff_utilise_demandes_puis_decouverte() {
        let discovered = vec![rel("a.txt"), rel("b.txt")];
        let requested = vec![rel("b.txt")];
        let none = BTreeSet::new();
        assert_eq!(
            resolve_diff_paths(Some(&requested), &discovered, &none),
            vec![rel("b.txt")]
        );
        assert_eq!(
            resolve_diff_paths(None, &discovered, &none),
            vec![rel("a.txt"), rel("b.txt")]
        );
    }

    #[test]
    fn activation_exige_git_et_flag() {
        assert!(!is_enabled(None, None));
        assert!(!is_enabled(Some("svn"), None));
        assert!(!is_enabled(Some("git"), Some(false)));
        assert!(is_enabled(Some("git"), None));
        assert!(is_enabled(Some("git"), Some(true)));
    }

    #[test]
    fn echec_garde_meme_operation_et_checkout_pointe_restore() {
        let err = failure(Operation::Diff, "boom", None);
        assert_eq!(wrap_error(Operation::Diff, err.clone()), err);
        let wrapped = wrap_error(Operation::Files, err);
        assert_eq!(wrapped.operation, Operation::Files);
        assert_eq!(checkout_error_operation(), Operation::Restore);
        let json = serde_json::to_string(&Operation::Capture).unwrap();
        assert_eq!(json, "\"capture\"");
    }
}
