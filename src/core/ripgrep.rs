//! Portage Rust de `opencode/packages/core/src/ripgrep.ts`.
//!
//! Petit adaptateur d execution ripgrep possede par le coeur. Il expose
//! volontairement des lignes brutes orientees processus, pas de texte modele
//! ni de comportement de permission. La recherche projette ces lignes en
//! resultats de fichiers, les outils feuilles possedent la presentation et
//! les invites de permission.
//!
//! Ce portage ne garde que la logique metier pure :
//! - constantes de garde,
//! - schemas JSON bruts de ripgrep,
//! - construction des arguments,
//! - normalisation des chemins relatifs,
//! - troncature du texte et des sous correspondances,
//! - classification des codes de sortie.
//!
//! Tout ce qui lance un processus est volontairement absent : `process.spawn`,
//! `Stream.decodeText`, `collectStream`, `waitForAbort`, `Layer.effect`,
//! `Service`, `makeGlobalNode`, `AbortSignal` et le rappel `onEntry`.
//! Voir POINT FAIBLE dans le rapport.
//!
//! Note sur le reexport : la premiere ligne de la source est
//! `export * as Ripgrep from "./ripgrep"`, soit un auto reexport d espace de
//! noms. En Rust c est le module lui meme, rien a ecrire. Fichier minimal sur
//! ce point, rien d invente.

use serde::{Deserialize, Serialize};

/// Octets de stderr conserves pour diagnostiquer un echec.
pub const ERROR_BYTES: usize = 8 * 1024;
/// Taille max d une ligne JSON `--json` avant rejet.
pub const MAX_RECORD_BYTES: usize = 64 * 1024;
/// Sous correspondances conservees par correspondance.
pub const MAX_SUBMATCHES: usize = 100;
/// Longueur max du texte d une correspondance, en unites TS (UTF-16).
pub const MATCH_TEXT_MAX_CHARS: usize = 2_000;
/// Glob d exclusion applique a chaque appel.
pub const GIT_EXCLUDE_GLOB: &str = "!**/.git/**";
/// Valeur de `Entry.type` construite par `find`, `glob` et `grep`.
pub const ENTRY_FILE: &str = "file";

/// Enveloppe `{ text }` partagee par `path`, `lines` et `match`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextWrapper {
    pub text: String,
}

/// Une sous correspondance brute de ripgrep `--json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSubmatch {
    #[serde(rename = "match")]
    pub match_body: TextWrapper,
    pub start: u64,
    pub end: u64,
}

/// Donnees brutes d un enregistrement `match` de ripgrep `--json`.
///
/// `line_number` est un entier strictement positif en TS (`PositiveInt`),
/// `absolute_offset`, `start` et `end` sont positifs ou nuls
/// (`NonNegativeInt`). On les porte en `u64` sans revalider ici : c est
/// ripgrep qui les produit, pas l utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMatchData {
    pub path: TextWrapper,
    pub lines: TextWrapper,
    #[serde(rename = "line_number")]
    pub line_number: u64,
    #[serde(rename = "absolute_offset")]
    pub absolute_offset: u64,
    pub submatches: Vec<RawSubmatch>,
}

/// Enregistrement brut complet, avec son discriminant `type = "match"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMatch {
    #[serde(rename = "type")]
    pub kind: String,
    pub data: RawMatchData,
}

/// Echec generique `Ripgrep.Error` : message plus cause texte optionnelle.
///
/// En TS la cause est un `Schema.Defect` opaque. On la porte en chaine
/// optionnelle pour garder la structure sans inventer de type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RipgrepErrorBody {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// Echec `Ripgrep.InvalidPatternError` : motif refuse plus message brut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidPatternBody {
    pub pattern: String,
    pub message: String,
}

/// Union des deux erreurs taggees de la source.
///
/// Miroir de `Schema.TaggedErrorClass` : tag `_tag` avec un `rename`
/// explicite par variante, comme l exige la regle du swarm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum RipgrepFailure {
    #[serde(rename = "Ripgrep.Error")]
    Error(RipgrepErrorBody),
    #[serde(rename = "Ripgrep.InvalidPatternError")]
    InvalidPattern(InvalidPatternBody),
}

/// Entree construite par `find` et `glob`.
///
/// `path` est un `RelativePath` opaque en TS, une simple chaine ici.
/// `hidden` et `follow` sont optionnels en TS, donc `Option` avec
/// `skip_serializing_if`. Le signal d annulation et le rappel `onEntry`
/// sont des effets, volontairement non portes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindInput {
    pub cwd: String,
    pub pattern: String,
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow: Option<bool>,
}

/// Entree de `glob`, meme forme que `find` sans rappel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobInput {
    pub cwd: String,
    pub pattern: String,
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow: Option<bool>,
}

/// Entree de `grep`.
///
/// `file` et `include` sont optionnels en TS, donc `Option` avec
/// `skip_serializing_if`. Le signal d annulation est un effet,
/// volontairement non porte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrepInput {
    pub cwd: String,
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    pub limit: u64,
}

/// Fiche fichier minimale, miroir de `Entry.make` utilise dans la source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// Sous correspondance projetee dans `Match.make`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSubmatch {
    pub text: String,
    pub start: u64,
    pub end: u64,
}

/// Resultat projete par `grep`, miroir de `Match.make`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMatch {
    pub entry: Entry,
    pub line: u64,
    pub offset: u64,
    pub text: String,
    pub submatches: Vec<MatchSubmatch>,
}

/// Verdict pur tire du code de sortie et de la limite.
///
/// Replique exacte de la fin de `run` en TS :
/// - plus de lignes que la limite : tronque, sans lire le code de sortie,
/// - code 2 plus stderr de regex invalide : motif invalide,
/// - code hors 0 / 1 / 2 : echec,
/// - code 1 : aucune correspondance (convention ripgrep),
/// - code 2 restant : succes partiel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunVerdict {
    Truncated,
    InvalidPattern,
    Failed,
    Success { empty: bool, partial: bool },
}

/// Construit une erreur generique, miroir de `failure` en TS.
pub fn failure(message: &str, cause: Option<String>) -> RipgrepFailure {
    RipgrepFailure::Error(RipgrepErrorBody {
        message: message.to_string(),
        cause,
    })
}

/// Vrai si le stderr signale une regex invalide.
///
/// Replique exacte : cherche les deux chaines vues par ripgrep.
pub fn is_invalid_pattern(stderr: &str) -> bool {
    stderr.contains("regex parse error") || stderr.contains("error parsing regex")
}

/// Enleve un seul prefixe `./` ou `.\` en tete, une seule fois.
///
/// Miroir de `.replace(/^\.[\\/]/, "")` utilise sur `match.data.path.text`
/// pendant le decodage `grep`. Sans prefixe, retourne le texte tel quel.
pub fn strip_one_dot_slash_prefix(text: &str) -> &str {
    if text.starts_with("./") || text.starts_with(".\\") {
        &text[2..]
    } else {
        text
    }
}

/// Normalise une ligne `--files` en chemin relatif a barres obliques.
///
/// Replique exacte de la chaine en TS :
/// `.replace(/^(?:\.[\\/])+/u, "").replace(/^[\\/]+/u, "").replaceAll("\\", "/")`
/// soit : enleve tous les `./` de tete, puis tous les `/` ou `\` de tete,
/// puis remplace chaque `\` restant par `/`.
pub fn normalize_relative_path(line: &str) -> String {
    let mut rest = line;
    loop {
        if let Some(next) = rest.strip_prefix("./").or_else(|| rest.strip_prefix(".\\")) {
            rest = next;
        } else {
            break;
        }
    }
    let trimmed = rest.trim_start_matches(|c| c == '/' || c == '\\');
    trimmed.replace('\\', "/")
}

/// Construit les arguments `--files` de `glob`.
pub fn build_glob_args(pattern: &str, hidden: bool, follow: bool) -> Vec<String> {
    let mut args = vec![
        "--no-config".to_string(),
        "--files".to_string(),
    ];
    if hidden {
        args.push("--hidden".to_string());
    }
    if follow {
        args.push("--follow".to_string());
    }
    args.push(format!("--glob={}", pattern));
    args.push(format!("--glob={}", GIT_EXCLUDE_GLOB));
    args.push(".".to_string());
    args
}

/// Construit les arguments `--files` de `find`.
///
/// Cas limite : le motif `"*"` veut dire tout, donc sans `--glob`,
/// comme en TS avec `...(input.pattern === "*" ? [] : [...])`.
pub fn build_find_args(pattern: &str, hidden: bool, follow: bool) -> Vec<String> {
    let mut args = vec![
        "--no-config".to_string(),
        "--files".to_string(),
    ];
    if hidden {
        args.push("--hidden".to_string());
    }
    if follow {
        args.push("--follow".to_string());
    }
    if pattern != "*" {
        args.push(format!("--glob={}", pattern));
    }
    args.push(format!("--glob={}", GIT_EXCLUDE_GLOB));
    args.push(".".to_string());
    args
}

/// Construit les arguments `--json` de `grep`.
///
/// `file ?? "."` en TS devient `unwrap_or(".")` ici. Le piege `?` contre
/// `??` ne s applique pas : `file` est une `Option`, pas une chaine vide,
/// donc `Some("")` survit comme en TS avec `??`.
pub fn build_grep_args(pattern: &str, file: Option<&str>, include: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--no-config".to_string(),
        "--json".to_string(),
        "--hidden".to_string(),
        "--no-messages".to_string(),
    ];
    if let Some(glob) = include {
        args.push(format!("--glob={}", glob));
    }
    args.push(format!("--glob={}", GIT_EXCLUDE_GLOB));
    args.push("--".to_string());
    args.push(pattern.to_string());
    args.push(file.unwrap_or(".").to_string());
    args
}

/// Vrai si une ligne `--json` depasse la taille max autorisee.
///
/// `Buffer.byteLength(line, "utf8")` en TS vaut exactement le nombre
/// d octets UTF-8, soit `as_bytes().len()` en Rust.
pub fn record_exceeds_limit(line: &str) -> bool {
    line.as_bytes().len() > MAX_RECORD_BYTES
}

/// Vrai si une valeur JSON est un enregistrement `match` a garder.
///
/// En TS, tout ce qui n est pas un objet avec `type === "match"` donne
/// `Effect.succeed(undefined)` puis est filtre. Cette fonction porte ce
/// filtre sans l effet.
pub fn is_match_record(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        == Some("match")
}

/// Tronque le texte d une correspondance a 2 000 unites plus `"..."`.
///
/// Replique de `match.lines.text.length > 2_000 ? slice(0, 2_000)... : text`.
/// La longueur TS compte en unites UTF-16 (`String.length`), donc on compte
/// en `encode_utf16` ici, pas en `chars`, pour rester fidele sur les
/// caracteres hors plan de base. La seconde etape TS enleve un eventuel
/// surrogate haut isole en fin de coupe (`/[\uD800-\uDBFF]$/`) : en Rust les
/// `char` ne produisent jamais de surrogate, la coupe reste donc toujours
/// sur une frontiere valide et cette etape est un no-op documente.
pub fn truncate_match_text(text: &str) -> String {
    if text.encode_utf16().count() <= MATCH_TEXT_MAX_CHARS {
        return text.to_string();
    }
    let mut units = 0usize;
    let mut end = 0usize;
    for (idx, c) in text.char_indices() {
        let w = c.len_utf16();
        if units + w > MATCH_TEXT_MAX_CHARS {
            break;
        }
        units += w;
        end = idx + c.len_utf8();
    }
    let mut cut = text[..end].to_string();
    cut.push_str("...");
    cut
}

/// Garde au plus `MAX_SUBMATCHES` sous correspondances, dans l ordre.
///
/// Miroir de `submatches.slice(0, MAX_SUBMATCHES)` en TS.
pub fn truncate_submatches<T>(items: Vec<T>) -> Vec<T> {
    items.into_iter().take(MAX_SUBMATCHES).collect()
}

/// Decide du sort d un appel `run` sans toucher au processus.
///
/// `row_count` est le nombre de lignes collecte, avant troncature.
/// `limit` est la limite demandee. `code` est le code de sortie ripgrep.
/// `stderr` est le stderr complet. `has_pattern` vaut vrai pour `grep`
/// seul, car `find` et `glob` n envoient pas de motif a tester.
pub fn decide_run(
    row_count: usize,
    limit: usize,
    code: i64,
    stderr: &str,
    has_pattern: bool,
) -> RunVerdict {
    if row_count > limit {
        return RunVerdict::Truncated;
    }
    if has_pattern && code == 2 && is_invalid_pattern(stderr) {
        return RunVerdict::InvalidPattern;
    }
    if code != 0 && code != 1 && code != 2 {
        return RunVerdict::Failed;
    }
    if code == 1 {
        return RunVerdict::Success {
            empty: true,
            partial: false,
        };
    }
    RunVerdict::Success {
        empty: false,
        partial: code == 2,
    }
}

/// Projette un `RawMatchData` en `FileMatch`, miroir du `map` final de `grep`.
///
/// Etapes : nettoie le chemin (`strip` puis `normalize`), tronque le texte
/// a 2 000 unites, limite les sous correspondances a 100, recopie
/// `line_number` vers `line` et `absolute_offset` vers `offset`.
pub fn map_raw_match_to_file_match(raw: &RawMatchData) -> FileMatch {
    let stripped = strip_one_dot_slash_prefix(&raw.path.text);
    let relative = normalize_relative_path(stripped);
    FileMatch {
        entry: Entry {
            path: relative,
            kind: ENTRY_FILE.to_string(),
        },
        line: raw.line_number,
        offset: raw.absolute_offset,
        text: truncate_match_text(&raw.lines.text),
        submatches: truncate_submatches(raw.submatches.clone())
            .into_iter()
            .map(|s| MatchSubmatch {
                text: s.match_body.text,
                start: s.start,
                end: s.end,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_normalisation_nettoie_prefixes_et_antislash() {
        assert_eq!(normalize_relative_path("./foo/bar"), "foo/bar");
        assert_eq!(normalize_relative_path("././foo"), "foo");
        assert_eq!(normalize_relative_path("/foo/bar"), "foo/bar");
        assert_eq!(normalize_relative_path(".\\foo\\bar"), "foo/bar");
        assert_eq!(normalize_relative_path("foo\\bar"), "foo/bar");
        assert_eq!(normalize_relative_path(""), "");
    }

    #[test]
    fn le_strip_simple_ne_retire_qu_un_prefixe() {
        assert_eq!(strip_one_dot_slash_prefix("./foo"), "foo");
        assert_eq!(strip_one_dot_slash_prefix("././foo"), "./foo");
        assert_eq!(strip_one_dot_slash_prefix("foo"), "foo");
    }

    #[test]
    fn la_detection_de_motif_invalide_couvre_les_deux_messages() {
        assert!(is_invalid_pattern("regex parse error: toto"));
        assert!(is_invalid_pattern("error parsing regex: toto"));
        assert!(!is_invalid_pattern(""));
        assert!(!is_invalid_pattern("no matches"));
    }

    #[test]
    fn le_find_etoile_n_envoie_pas_de_glob() {
        let args = build_find_args("*", false, false);
        assert!(!args.iter().any(|a| a.starts_with("--glob=") && !a.contains(".git")));
        let args2 = build_find_args("*.ts", false, false);
        assert!(args2.contains(&"--glob=*.ts".to_string()));
    }

    #[test]
    fn le_grep_choisit_include_et_fichier_par_defaut() {
        let args = build_grep_args("hello", None, Some("*.ts"));
        assert!(args.contains(&"--glob=*.ts".to_string()));
        assert_eq!(args.last().map(|s| s.as_str()), Some("."));
        let args2 = build_grep_args("hello", Some("src"), None);
        assert_eq!(args2.last().map(|s| s.as_str()), Some("src"));
    }

    #[test]
    fn un_texte_court_survit_et_un_long_est_coupe() {
        assert_eq!(truncate_match_text("court"), "court");
        let long = "x".repeat(MATCH_TEXT_MAX_CHARS + 10);
        let out = truncate_match_text(&long);
        assert!(out.ends_with("..."));
        assert_eq!(out.len(), MATCH_TEXT_MAX_CHARS + 3);
    }

    #[test]
    fn la_decision_couvre_troncature_invalide_echec_et_succes() {
        assert_eq!(
            decide_run(11, 10, 0, "", true),
            RunVerdict::Truncated
        );
        assert_eq!(
            decide_run(2, 10, 2, "regex parse error", true),
            RunVerdict::InvalidPattern
        );
        assert_eq!(decide_run(2, 10, 3, "", true), RunVerdict::Failed);
        assert_eq!(
            decide_run(0, 10, 1, "", true),
            RunVerdict::Success { empty: true, partial: false }
        );
        assert_eq!(
            decide_run(2, 10, 2, "boom", true),
            RunVerdict::Success { empty: false, partial: true }
        );
    }

    #[test]
    fn la_serialisation_garde_les_noms_camel_case() {
        let raw = RawMatch {
            kind: "match".to_string(),
            data: RawMatchData {
                path: TextWrapper { text: "./a".to_string() },
                lines: TextWrapper { text: "hi".to_string() },
                line_number: 3,
                absolute_offset: 42,
                submatches: vec![],
            },
        };
        let json = serde_json::to_value(&raw).expect("serialisation JSON");
        assert_eq!(json["type"], serde_json::json!("match"));
        assert_eq!(json["data"]["line_number"], serde_json::json!(3));
        assert_eq!(json["data"]["absolute_offset"], serde_json::json!(42));
        assert!(json["data"].get("lineNumber").is_none());
        let fail = RipgrepFailure::InvalidPattern(InvalidPatternBody {
            pattern: "(".to_string(),
            message: "regex parse error".to_string(),
        });
        let v = serde_json::to_value(&fail).expect("serialisation erreur");
        assert_eq!(v["_tag"], serde_json::json!("Ripgrep.InvalidPatternError"));
    }
}
