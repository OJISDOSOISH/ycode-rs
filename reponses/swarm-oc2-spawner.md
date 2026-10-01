# swarm-oc2-spawner

source : opencode/packages/core/src/cross-spawn-spawner.ts
cible : src/core/process/spawner.rs (propose, a declarer par l agent principal, ne pas toucher mod.rs ici)
tests : 7

===DEBUT===
```rust
//! Portage Rust de `opencode/packages/core/src/cross-spawn-spawner.ts`.
//!
//! Contenu porte : logique metier pure uniquement.
//! - correspondance code errno -> tag d erreur systeme
//! - aplatissement d une commande pipe en liste lineaire
//! - affichage d une chaine de commandes pour les erreurs
//! - resolution de l environnement, du stdin, du stdout et des fds
//! - construction des slots stdio et choix pipe from / to
//! - signaux de kill et commande taskkill Windows
//!
//! Contenu volontairement saute (effets) : spawn via cross-spawn,
//! wiring des streams et sinks Effect, Deferred et signaux de sortie,
//! acquireRelease et nettoyage a la fermeture, forkScoped, timeouts
//! Effect, kill reel de groupe ou simple, couches Layer et service global.
//! Voir POINT FAIBLE dans le rapport.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Tag d erreur systeme, miroir minimal du `SystemErrorTag` de Effect.
/// Le module d origine mappe `NodeJS.ErrnoException.code` vers ce tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemErrorTag {
    #[serde(rename = "NotFound")]
    NotFound,
    #[serde(rename = "PermissionDenied")]
    PermissionDenied,
    #[serde(rename = "AlreadyExists")]
    AlreadyExists,
    #[serde(rename = "BadResource")]
    BadResource,
    #[serde(rename = "Busy")]
    Busy,
    #[serde(rename = "Unknown")]
    Unknown,
}

/// Erreur plateforme construite par `toPlatformError`.
/// `cause` d origine (objet Error) est reduit a son message, seule
/// donnee pure transportable sans effet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformError {
    #[serde(rename = "tag")]
    pub tag: SystemErrorTag,
    #[serde(rename = "module")]
    pub module: String,
    #[serde(rename = "method")]
    pub method: String,
    #[serde(rename = "pathOrDescriptor", skip_serializing_if = "Option::is_none")]
    pub path_or_descriptor: Option<String>,
    #[serde(rename = "syscall", default, skip_serializing_if = "Option::is_none")]
    pub syscall: Option<String>,
    #[serde(rename = "cause", default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// Nature d un fd additionnel, discriminant `type` d origine.
/// Les champs `stream` et `sink` d origine portent des Stream et Sink
/// Effect : effets, donc sautes. On ne garde que le discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FdKind {
    #[serde(rename = "input")]
    Input,
    #[serde(rename = "output")]
    Output,
}

/// Config d un fd additionnel, sans les effets stream et sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalFdConfig {
    #[serde(rename = "type")]
    pub kind: FdKind,
}

/// Entree triee par numero de fd, sortie pure de `fds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdEntry {
    pub fd: u32,
    pub config: AdditionalFdConfig,
}

/// Options de commande, sous ensemble pur de `CommandOptions`.
/// Les champs `stdin`, `stdout`, `stderr` d origine portent des
/// Stream et Sink Effect : sautes ici, traites par les fonctions
/// `resolve_stdin`, `resolve_stdio`, `map_input_to_pipe` et
/// `map_output_to_pipe` a partir de donnees simples.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CommandOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "extendEnv", skip_serializing_if = "Option::is_none")]
    pub extend_env: Option<bool>,
    #[serde(default, rename = "additionalFds", skip_serializing_if = "Option::is_none")]
    pub additional_fds: Option<BTreeMap<String, AdditionalFdConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detached: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, rename = "killSignal", skip_serializing_if = "Option::is_none")]
    pub kill_signal: Option<String>,
    #[serde(default, rename = "forceKillAfter", skip_serializing_if = "Option::is_none")]
    pub force_kill_after: Option<u64>,
}

/// Commande standard : programme plus arguments plus options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardCommand {
    #[serde(rename = "command")]
    pub command: String,
    #[serde(rename = "args")]
    pub args: Vec<String>,
    #[serde(rename = "options")]
    pub options: CommandOptions,
}

/// Options entre deux maillons d un pipe : `from` et `to`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PipeOptions {
    #[serde(default, rename = "from", skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, rename = "to", skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// Maillon pipe : gauche, droite et options de liaison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipedCommand {
    #[serde(rename = "left")]
    pub left: Box<Command>,
    #[serde(rename = "right")]
    pub right: Box<Command>,
    #[serde(rename = "options")]
    pub options: PipeOptions,
}

/// Union de commandes, miroir du `_tag` d origine.
/// Chaque variante porte son rename explicite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum Command {
    #[serde(rename = "StandardCommand")]
    StandardCommand(StandardCommand),
    #[serde(rename = "PipedCommand")]
    PipedCommand(PipedCommand),
}

/// Resultat pur de `flatten` : commandes lineaires et options de liaison.
#[derive(Debug, Clone, PartialEq)]
pub struct Flattened {
    pub commands: Vec<StandardCommand>,
    pub opts: Vec<PipeOptions>,
}

/// Config stdin resolue, version pure de `StdinConfig`.
/// Quand l entree d origine est un Stream Effect, on ne garde que le
/// marqueur `"pipe"`, le contenu du stream etant un effet saute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StdinConfig {
    #[serde(rename = "stream")]
    pub stream: String,
    #[serde(rename = "encoding")]
    pub encoding: String,
    #[serde(rename = "endOnDone")]
    pub end_on_done: bool,
}

/// Config stdout et stderr resolue, version pure de `StdoutConfig`.
/// Quand l entree d origine est un Sink Effect, on ne garde que le
/// marqueur `"pipe"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StdoutConfig {
    #[serde(rename = "stream")]
    pub stream: String,
}

/// Entree stdin avant resolution, pour reproduire les quatre branches
/// de la fonction `stdin` sans importer de Stream Effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinKind {
    Missing,
    Literal(String),
    Stream,
    Detailed {
        stream_literal: Option<String>,
        stream_is_stream: bool,
        encoding: Option<String>,
        end_on_done: Option<bool>,
    },
}

/// Entree stdout et stderr avant resolution, pour reproduire les quatre
/// branches de la fonction `stdio` sans importer de Sink Effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdioKind {
    Missing,
    Literal(String),
    Sink,
    Detailed(String),
}

/// Origine d un flux pour un pipe, miroir de `source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeSource {
    Stdout,
    Stderr,
    All,
    Fd(u32),
    FallbackStdout,
}

/// Destination d un flux pour un pipe, miroir de la boucle `PipedCommand`
/// dans `spawnCommand` : `stdin` par defaut, sinon `fdN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeTarget {
    Stdin,
    Fd(u32),
    FallbackStdin,
}

/// Valeur par defaut du stdin d origine.
pub const DEFAULT_STDIN_ENCODING: &str = "utf-8";

/// Mappe un code errno vers un tag, miroir de `toTag`.
/// `None` correspond a un code absent : tag inconnu.
pub fn errno_to_tag(code: Option<&str>) -> SystemErrorTag {
    match code {
        Some("ENOENT") => SystemErrorTag::NotFound,
        Some("EACCES") => SystemErrorTag::PermissionDenied,
        Some("EEXIST") => SystemErrorTag::AlreadyExists,
        Some("EISDIR") => SystemErrorTag::BadResource,
        Some("ENOTDIR") => SystemErrorTag::BadResource,
        Some("EBUSY") => SystemErrorTag::Busy,
        Some("ELOOP") => SystemErrorTag::BadResource,
        _ => SystemErrorTag::Unknown,
    }
}

/// Normalise une erreur inconnue en message, equivalent pur de `toError`.
/// La version TypeScript deballe `instanceof Error`, sinon convertit en
/// chaine. Ici l appelant fournit deja le message optionnel et la valeur
/// brute affichee.
pub fn normalize_error_message(message: Option<&str>, fallback: &str) -> String {
    match message {
        Some(m) => m.to_string(),
        None => fallback.to_string(),
    }
}

/// Affiche une commande standard, brique de `toPlatformError`.
pub fn format_standard_command(cmd: &StandardCommand) -> String {
    if cmd.args.is_empty() {
        return cmd.command.clone();
    }
    format!("{} {}", cmd.command, cmd.args.join(" "))
}

/// Joint une chaine de commandes avec `" | "`, miroir exact du `join`
/// dans `toPlatformError`.
pub fn format_command_chain(commands: &[StandardCommand]) -> String {
    commands
        .iter()
        .map(format_standard_command)
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Aplati un arbre de pipes en liste lineaire, miroir de `flatten`.
/// Erreur si aucun maillon standard, comme le `throw` d origine.
pub fn flatten_command(cmd: &Command) -> Result<Flattened, String> {
    let mut commands: Vec<StandardCommand> = Vec::new();
    let mut opts: Vec<PipeOptions> = Vec::new();

    fn walk(cmd: &Command, commands: &mut Vec<StandardCommand>, opts: &mut Vec<PipeOptions>) {
        match cmd {
            Command::StandardCommand(s) => commands.push(s.clone()),
            Command::PipedCommand(p) => {
                walk(&p.left, commands, opts);
                opts.push(p.options.clone());
                walk(&p.right, commands, opts);
            }
        }
    }

    walk(cmd, &mut commands, &mut opts);
    if commands.is_empty() {
        return Err("flatten produced empty commands array".to_string());
    }
    Ok(Flattened { commands, opts })
}

/// Construit l erreur plateforme, partie pure de `toPlatformError`.
/// L acces disque et le spawn sont des effets et restent hors portage.
pub fn build_platform_error(
    method: &str,
    code: Option<&str>,
    syscall: Option<&str>,
    cause: Option<&str>,
    chain: &[StandardCommand],
) -> PlatformError {
    PlatformError {
        tag: errno_to_tag(code),
        module: "ChildProcess".to_string(),
        method: method.to_string(),
        path_or_descriptor: Some(format_command_chain(chain)),
        syscall: syscall.map(|s| s.to_string()),
        cause: cause.map(|s| s.to_string()),
    }
}

/// Resout l environnement, miroir de `env`.
/// Si `extend_env` est vrai, fusionne base et surcharge, la surcharge
/// gagnant. Sinon garde uniquement la surcharge, y compris `None`.
pub fn resolve_env(
    extend_env: bool,
    base: &BTreeMap<String, String>,
    overrides: Option<&BTreeMap<String, String>>,
) -> Option<BTreeMap<String, String>> {
    if extend_env {
        let mut merged = base.clone();
        if let Some(o) = overrides {
            for (k, v) in o {
                merged.insert(k.clone(), v.clone());
            }
        }
        Some(merged)
    } else {
        overrides.cloned()
    }
}

/// Mappe une entree stdin vers un slot, miroir de `input`.
/// Un Stream Effect donne `"pipe"`, sinon on garde le literal tel quel.
pub fn map_input_to_pipe(is_stream: bool, literal: Option<&str>) -> Option<String> {
    if is_stream {
        return Some("pipe".to_string());
    }
    literal.map(|s| s.to_string())
}

/// Mappe une sortie vers un slot, miroir de `output`.
/// Un Sink Effect donne `"pipe"`, sinon on garde le literal tel quel.
pub fn map_output_to_pipe(is_sink: bool, literal: Option<&str>) -> Option<String> {
    if is_sink {
        return Some("pipe".to_string());
    }
    literal.map(|s| s.to_string())
}

/// Resout la config stdin, miroir de `stdin`.
pub fn resolve_stdin(kind: &StdinKind) -> StdinConfig {
    let defaults = StdinConfig {
        stream: "pipe".to_string(),
        encoding: DEFAULT_STDIN_ENCODING.to_string(),
        end_on_done: true,
    };
    match kind {
        StdinKind::Missing => defaults,
        StdinKind::Literal(s) => StdinConfig {
            stream: s.clone(),
            ..defaults
        },
        StdinKind::Stream => defaults,
        StdinKind::Detailed {
            stream_literal,
            stream_is_stream,
            encoding,
            end_on_done,
        } => {
            let stream = if *stream_is_stream {
                "pipe".to_string()
            } else if let Some(s) = stream_literal {
                s.clone()
            } else {
                defaults.stream.clone()
            };
            StdinConfig {
                stream,
                encoding: encoding
                    .clone()
                    .unwrap_or_else(|| defaults.encoding.clone()),
                end_on_done: end_on_done.unwrap_or(defaults.end_on_done),
            }
        }
    }
}

/// Resout une config stdout ou stderr, miroir de `stdio`.
pub fn resolve_stdio(kind: &StdioKind) -> StdoutConfig {
    match kind {
        StdioKind::Missing => StdoutConfig {
            stream: "pipe".to_string(),
        },
        StdioKind::Literal(s) => StdoutConfig { stream: s.clone() },
        StdioKind::Sink => StdoutConfig {
            stream: "pipe".to_string(),
        },
        StdioKind::Detailed(s) => StdoutConfig { stream: s.clone() },
    }
}

/// Parse un nom `fdN` vers son numero, miroir de `parseFdName`.
/// Accepte `fd3` a `fd9` et au dela, refuse vide, `fd` seul et tout
/// suffixe non numerique. Retourne `None` pour entree invalide.
pub fn parse_fd_name(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("fd")?;
    if rest.is_empty() {
        return None;
    }
    if !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.parse::<u32>().ok()
}

/// Construit un nom `fdN`, miroir de `fdName`.
pub fn fd_name(fd: u32) -> String {
    format!("fd{fd}")
}

/// Collecte et trie les fds additionnels, miroir de `fds`.
/// Les noms invalides sont ignores, le tri est croissant.
pub fn collect_extra_fds(map: Option<&BTreeMap<String, AdditionalFdConfig>>) -> Vec<FdEntry> {
    let Some(m) = map else {
        return Vec::new();
    };
    let mut out: Vec<FdEntry> = m
        .iter()
        .filter_map(|(name, config)| {
            parse_fd_name(name).map(|fd| FdEntry { fd, config: *config })
        })
        .collect();
    out.sort_by_key(|e| e.fd);
    out
}

/// Applique la regle Windows `pipe` -> `overlapped`, miroir du helper
/// `pipe` local a `stdios`.
pub fn win32_pipe_slot(slot: Option<&str>, platform: &str) -> Option<String> {
    match slot {
        Some("pipe") if platform == "win32" => Some("overlapped".to_string()),
        Some(s) => Some(s.to_string()),
        None => None,
    }
}

/// Construit le tableau stdio, miroir de `stdios`.
/// Les trois premiers slots sont stdin, stdout et stderr.
/// Les fds supplementaires remplissent les trous avec `"ignore"` et
/// forcent `"pipe"` sur chaque fd demande, sans passer par `overlapped`.
pub fn build_stdio_slots(
    stdin: Option<&str>,
    stdout: Option<&str>,
    stderr: Option<&str>,
    extra: &[FdEntry],
    platform: &str,
) -> Vec<Option<String>> {
    let mut slots: Vec<Option<String>> = vec![
        win32_pipe_slot(stdin, platform),
        win32_pipe_slot(stdout, platform),
        win32_pipe_slot(stderr, platform),
    ];
    if extra.is_empty() {
        return slots;
    }
    let max = extra.iter().map(|e| e.fd).max().unwrap_or(2);
    if max >= 3 {
        for _ in 3..=max {
            slots.push(Some("ignore".to_string()));
        }
    }
    for e in extra {
        let idx = e.fd as usize;
        while slots.len() <= idx {
            slots.push(Some("ignore".to_string()));
        }
        slots[idx] = Some("pipe".to_string());
    }
    slots
}

/// Resout le `cwd` demande, partie pure de `cwd`.
/// La verification d acces disque est un effet et reste hors portage :
/// on ne fait que propager la valeur, `None` si absente.
pub fn resolve_cwd(requested: Option<&str>) -> Option<String> {
    requested.map(|s| s.to_string())
}

/// Choisit le flux source d un pipe, miroir de `source`.
/// Defaut `stdout`, `fdN` valide donne le fd, `fd` invalide retombe
/// sur `stdout` comme l original.
pub fn resolve_pipe_from(raw: Option<&str>) -> PipeSource {
    match raw.unwrap_or("stdout") {
        "stdout" => PipeSource::Stdout,
        "stderr" => PipeSource::Stderr,
        "all" => PipeSource::All,
        other => match parse_fd_name(other) {
            Some(fd) => PipeSource::Fd(fd),
            None => PipeSource::FallbackStdout,
        },
    }
}

/// Choisit la destination d un pipe, miroir de la boucle `PipedCommand`.
/// `None` ou `"stdin"` donne stdin, `fdN` valide donne le fd, le reste
/// retombe sur stdin comme l original.
pub fn resolve_pipe_target(raw: Option<&str>) -> PipeTarget {
    match raw {
        None => PipeTarget::Stdin,
        Some("stdin") => PipeTarget::Stdin,
        Some(other) => match parse_fd_name(other) {
            Some(fd) => PipeTarget::Fd(fd),
            None => PipeTarget::FallbackStdin,
        },
    }
}

/// Signaux et plateforme, partie pure de `timeout`, `kill` et `killGroup`.

/// Signal par defaut `SIGTERM` quand aucune option n est fournie.
pub fn default_kill_signal(raw: Option<&str>) -> String {
    raw.unwrap_or("SIGTERM").to_string()
}

/// Vrai si une duree `forceKillAfter` impose une escalade vers SIGKILL.
pub fn needs_force_kill(force_kill_after: Option<u64>) -> bool {
    force_kill_after.is_some()
}

/// Construit la commande `taskkill` Windows, miroir de la branche
/// `win32` de `killGroup`. L appel `exec` lui meme est un effet saute.
pub fn build_taskkill_command(pid: u32) -> String {
    format!("taskkill /pid {pid} /T /F")
}

/// Predicat de plateforme pour la branche Windows.
pub fn is_windows_platform(platform: &str) -> bool {
    platform == "win32"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std_cmd(name: &str, args: &[&str]) -> StandardCommand {
        StandardCommand {
            command: name.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            options: CommandOptions::default(),
        }
    }

    #[test]
    fn les_codes_errno_donnent_le_bon_tag() {
        assert_eq!(errno_to_tag(Some("ENOENT")), SystemErrorTag::NotFound);
        assert_eq!(errno_to_tag(Some("EACCES")), SystemErrorTag::PermissionDenied);
        assert_eq!(errno_to_tag(Some("EBUSY")), SystemErrorTag::Busy);
        assert_eq!(errno_to_tag(Some("ELOOP")), SystemErrorTag::BadResource);
        assert_eq!(errno_to_tag(Some("CODEX")), SystemErrorTag::Unknown);
        assert_eq!(errno_to_tag(None), SystemErrorTag::Unknown);
    }

    #[test]
    fn un_pipe_imbrique_s_aplatit_dans_l_ordre() {
        let left = Command::StandardCommand(std_cmd("a", &[]));
        let mid = Command::StandardCommand(std_cmd("b", &["x"]));
        let inner = Command::PipedCommand(PipedCommand {
            left: Box::new(left),
            right: Box::new(mid),
            options: PipeOptions {
                from: Some("stdout".to_string()),
                to: Some("stdin".to_string()),
            },
        });
        let right = Command::StandardCommand(std_cmd("c", &[]));
        let root = Command::PipedCommand(PipedCommand {
            left: Box::new(inner),
            right: Box::new(right),
            options: PipeOptions {
                from: None,
                to: Some("fd3".to_string()),
            },
        });
        let flat = flatten_command(&root).expect("aplatissement attendu");
        let names: Vec<&str> = flat.commands.iter().map(|c| c.command.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        assert_eq!(flat.opts.len(), 2);
        assert_eq!(flat.opts[1].to.as_deref(), Some("fd3"));
    }

    #[test]
    fn une_chaine_de_commandes_se_joint_avec_un_pipe() {
        let cmds = vec![std_cmd("ls", &["-la"]), std_cmd("grep", &["x"])];
        assert_eq!(format_command_chain(&cmds), "ls -la | grep x");
        let single = vec![std_cmd("ls", &[])];
        assert_eq!(format_command_chain(&single), "ls");
    }

    #[test]
    fn l_environnement_fusionne_ou_remplace_selon_extend_env() {
        let base: BTreeMap<String, String> =
            [("A".to_string(), "1".to_string())].into_iter().collect();
        let over: BTreeMap<String, String> = [
            ("A".to_string(), "2".to_string()),
            ("B".to_string(), "3".to_string()),
        ]
        .into_iter()
        .collect();
        let merged = resolve_env(true, &base, Some(&over)).expect("fusion attendue");
        assert_eq!(merged.get("A").map(String::as_str), Some("2"));
        assert_eq!(merged.get("B").map(String::as_str), Some("3"));
        let replaced = resolve_env(false, &base, Some(&over)).expect("remplacement attendu");
        assert_eq!(replaced.len(), 2);
        assert!(resolve_env(false, &base, None).is_none());
    }

    #[test]
    fn le_stdin_garde_ses_defauts_et_applique_les_surcharges() {
        let missing = resolve_stdin(&StdinKind::Missing);
        assert_eq!(missing.stream, "pipe");
        assert_eq!(missing.encoding, "utf-8");
        assert!(missing.end_on_done);
        let lit = resolve_stdin(&StdinKind::Literal("ignore".to_string()));
        assert_eq!(lit.stream, "ignore");
        let det = resolve_stdin(&StdinKind::Detailed {
            stream_literal: None,
            stream_is_stream: true,
            encoding: Some("ascii".to_string()),
            end_on_done: Some(false),
        });
        assert_eq!(det.stream, "pipe");
        assert_eq!(det.encoding, "ascii");
        assert!(!det.end_on_done);
        assert_eq!(map_input_to_pipe(true, None).as_deref(), Some("pipe"));
        assert_eq!(map_output_to_pipe(false, Some("inherit")).as_deref(), Some("inherit"));
    }

    #[test]
    fn les_slots_stdio_remplissent_ignore_et_overlapped_sur_windows() {
        let extra = vec![FdEntry {
            fd: 4,
            config: AdditionalFdConfig { kind: FdKind::Input },
        }];
        let slots = build_stdio_slots(Some("pipe"), Some("pipe"), Some("pipe"), &extra, "win32");
        assert_eq!(slots[0].as_deref(), Some("overlapped"));
        assert_eq!(slots[3].as_deref(), Some("ignore"));
        assert_eq!(slots[4].as_deref(), Some("pipe"));
        let plain = build_stdio_slots(Some("pipe"), None, Some("ignore"), &[], "linux");
        assert_eq!(plain.len(), 3);
        assert_eq!(plain[0].as_deref(), Some("pipe"));
        assert!(parse_fd_name("fd3").is_some());
        assert!(parse_fd_name("fd").is_none());
        assert!(parse_fd_name("stdout").is_none());
        assert_eq!(fd_name(3), "fd3");
    }

    #[test]
    fn les_routes_de_pipe_et_le_kill_choisissent_les_bonnes_cibles() {
        assert_eq!(resolve_pipe_from(None), PipeSource::Stdout);
        assert_eq!(resolve_pipe_from(Some("all")), PipeSource::All);
        assert_eq!(resolve_pipe_from(Some("fd5")), PipeSource::Fd(5));
        assert_eq!(resolve_pipe_from(Some("nope")), PipeSource::FallbackStdout);
        assert_eq!(resolve_pipe_target(None), PipeTarget::Stdin);
        assert_eq!(resolve_pipe_target(Some("fd4")), PipeTarget::Fd(4));
        assert_eq!(resolve_pipe_target(Some("nope")), PipeTarget::FallbackStdin);
        assert_eq!(default_kill_signal(None), "SIGTERM");
        assert!(needs_force_kill(Some(1000)));
        assert!(!needs_force_kill(None));
        assert_eq!(build_taskkill_command(42), "taskkill /pid 42 /T /F");
        assert_eq!(collect_extra_fds(None).len(), 0);
    }
}
```
===FIN===

CONFIANCE : moyenne
POINT FAIBLE : parties sautees : spawn cross-spawn, wiring stdin stdout stderr et fds supplementaires via NodeSink et NodeStream, Deferred et signaux exit et close, acquireRelease et nettoyage killGroup et killOne au relachement, forkScoped des flux, timeouts Effect et escalade SIGKILL executee, couches Layer et noeud global ; zone a risque : AdditionalFdConfig reduit au discriminant car stream et sink sont des effets, et parse_fd_name suppose prefixe fd minuscule comme l original sans verifier les bornes hautes.
A VERIFIER : ordre des opts dans flatten, separateur " | " dans l erreur, fusion extendEnv avec priorite aux surcharges, pipe vers overlapped uniquement sur win32, retombees pipe from et to vers stdout et stdin pour fd invalide, noms serde _tag StandardCommand PipedCommand extendEnv additionalFds killSignal forceKillAfter endOnDone pathOrDescriptor type input output.
