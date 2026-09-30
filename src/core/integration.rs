//! Portage Rust de `opencode/packages/core/src/integration.ts`
//! et de son schema `opencode/packages/schema/src/integration.ts`.
//!
//! Une integration regroupe des methodes d'authentification (OAuth, cle API,
//! variables d'environnement) et expose des connexions : les credentials
//! stockes, plus recents d'abord, puis les variables d'environnement
//! detectees. Les tentatives OAuth suivent un cycle
//! pending -> complete | failed | expired, avec retention limitee des
//! tentatives terminees et expiration des tentatives abandonnees.
//!
//! Seule la logique metier pure est portee ici :
//! - les formes du schema (structs + enums tagges) ;
//! - le registre (ajout, retrait, methodes, tri par nom) ;
//! - la projection des connexions (credentials + env) ;
//! - le cycle de vie des tentatives (demarrage, garde de completion,
//!   cloture, nettoyage).
//!
//! Ne sont PAS portes (effets, a reimplementer autour de ce module) :
//! - le graphe Effect (Layer, Service, Scope, SynchronizedRef, Clock) ;
//! - la persistance via Credential.Service et la publication d'evenements ;
//! - les callbacks `authorize` et `refresh`, qui sont des Effect ;
//! - la boucle de nettoyage planifiee (Schedule.spaced) ;
//! - le noeud `node` et ses dependances (Credential.node, EventV2.node).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifiant opaque d'une integration (brand sur String en TS).
///
/// Reexport de `super::credential` : une seule definition pour tout le lot,
/// la passe C le remontera dans `crate::schema`.
pub use super::credential::IntegrationId;

/// Identifiant opaque d'une methode OAuth (brand sur String en TS).
pub type MethodId = String;

/// Identifiant opaque d'une tentative OAuth, prefixe `con_` en TS.
pub type AttemptId = String;

/// Reponses aux invites optionnelles d'une methode (`Record<string, string>`).
pub type Inputs = BTreeMap<String, String>;

/// Variables d'environnement visibles, pour la projection des connexions.
pub type EnvMap = BTreeMap<String, String>;

/// Duree de vie d'une tentative en attente : 10 minutes, en millisecondes.
pub const ATTEMPT_LIFETIME_MS: i64 = 10 * 60 * 1000;

/// Retention d'une tentative terminee avant suppression : 1 minute, en ms.
pub const TERMINAL_RETENTION_MS: i64 = 60 * 1000;

/// Cadence du nettoyage periodique dans l'original : 30 secondes, en ms.
/// Le planificateur lui-meme n'est pas porte, seule la constante est gardee
/// pour documenter le rythme attendu par l'appelant.
pub const SCRUB_INTERVAL_MS: i64 = 30 * 1000;

/// Marge avant expiration qui declenche un refresh OAuth : 5 minutes, en ms.
pub const REFRESH_MARGIN_MS: i64 = 5 * 60 * 1000;

/// Prefixe des identifiants de tentative (`con_` + suite interne).
pub const ATTEMPT_ID_PREFIX: &str = "con_";

/// Message par defaut quand une tentative echoue sans detail.
pub const FAILED_MESSAGE_DEFAULT: &str = "Authorization failed";

/// Condition d'affichage d'une invite (`Integration.When`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct When {
    pub key: String,
    pub op: WhenOp,
    pub value: String,
}

/// Operateur de la condition `When`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhenOp {
    #[serde(rename = "eq")]
    Eq,
    #[serde(rename = "neq")]
    Neq,
}

/// Invite texte simple (`Integration.TextPrompt`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPrompt {
    pub key: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
}

/// Option d'une invite a choix (`SelectPrompt.options[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Invite a choix (`Integration.SelectPrompt`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectPrompt {
    pub key: String,
    pub message: String,
    pub options: Vec<SelectOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
}

/// Union des invites, discriminee par le champ `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Prompt {
    #[serde(rename = "text")]
    Text(TextPrompt),
    #[serde(rename = "select")]
    Select(SelectPrompt),
}

/// Methode OAuth (`Integration.OAuthMethod`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthMethod {
    pub id: MethodId,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<Prompt>>,
}

/// Methode par cle API (`Integration.KeyMethod`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMethod {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Methode par variables d'environnement (`Integration.EnvMethod`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvMethod {
    pub names: Vec<String>,
}

/// Union des methodes, discriminee par le champ `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Method {
    #[serde(rename = "oauth")]
    OAuth(OAuthMethod),
    #[serde(rename = "key")]
    Key(KeyMethod),
    #[serde(rename = "env")]
    Env(EnvMethod),
}

/// Reference courte d'une integration (`Integration.Ref`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ref {
    pub id: IntegrationId,
    pub name: String,
}

/// Connexion par credential stocke (`Connection.CredentialInfo`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialConnection {
    pub id: String,
    pub label: String,
}

/// Connexion par variable d'environnement (`Connection.EnvInfo`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvConnection {
    pub name: String,
}

/// Union des connexions, discriminee par le champ `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConnectionInfo {
    #[serde(rename = "credential")]
    Credential(CredentialConnection),
    #[serde(rename = "env")]
    Env(EnvConnection),
}

/// Vue complete d'une integration (`Integration.Info`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub id: IntegrationId,
    pub name: String,
    pub methods: Vec<Method>,
    pub connections: Vec<ConnectionInfo>,
}

/// Horodatage d'une tentative, en millisecondes depuis l'epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptTime {
    pub created: i64,
    pub expires: i64,
}

/// Mode d'autorisation OAuth : `auto` se termine seul, `code` attend un code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "code")]
    Code,
}

/// Tentative OAuth exposee a l'appelant (`Integration.Attempt`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    #[serde(rename = "attemptID")]
    pub attempt_id: AttemptId,
    pub url: String,
    pub instructions: String,
    pub mode: OAuthMode,
    pub time: AttemptTime,
}

/// Etat observable d'une tentative (`Integration.AttemptStatus`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum AttemptStatus {
    #[serde(rename = "pending")]
    Pending { time: AttemptTime },
    #[serde(rename = "complete")]
    Complete { time: AttemptTime },
    #[serde(rename = "failed")]
    Failed { message: String, time: AttemptTime },
    #[serde(rename = "expired")]
    Expired { time: AttemptTime },
}

/// Erreurs du domaine, tags repris tels quels du TS.
/// `cause` (un Defect inconnu en TS) est reduit a son message texte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum IntegrationError {
    #[serde(rename = "Integration.CodeRequired")]
    CodeRequired {
        #[serde(rename = "attemptID")]
        attempt_id: AttemptId,
    },
    #[serde(rename = "Integration.Authorization")]
    Authorization { cause: String },
}

/// Entree interne du registre : reference + methodes declarees.
/// La table des implementations OAuth (des callbacks Effect en TS) n'est
/// pas portee : la recherche se fait par balayage de `methods`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub reference: Ref,
    pub methods: Vec<Method>,
}

/// Registre des integrations, deterministe pour des tests stables.
pub type Registry = BTreeMap<IntegrationId, Entry>;

/// Credential stocke vu par la projection des connexions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedCredential {
    pub id: String,
    pub integration_id: IntegrationId,
    pub label: String,
}

/// Tentative en attente : garde l'autorisation et l'etat de completion.
/// Les callbacks Effect (`authorize`) ne sont pas portes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAttempt {
    pub completing: bool,
    pub mode: OAuthMode,
    pub url: String,
    pub instructions: String,
    pub integration_id: IntegrationId,
    pub method_id: MethodId,
    pub label: Option<String>,
    pub time: AttemptTime,
}

/// Issue possible d'une tentative terminee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOutcome {
    Complete,
    Failed,
    Expired,
}

/// Tentative terminee, conservee jusqu'a `remove_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAttempt {
    pub outcome: TerminalOutcome,
    pub message: Option<String>,
    pub remove_at: i64,
    pub time: AttemptTime,
}

/// Contenu du magasin des tentatives (ex-SynchronizedRef en TS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptEntry {
    Pending(PendingAttempt),
    Terminal(TerminalAttempt),
}

/// Magasin des tentatives indexe par identifiant.
pub type AttemptStore = BTreeMap<AttemptId, AttemptEntry>;

/// Decision de la garde de completion, dans l'ordre des tests du TS :
/// code manquant d'abord, completion deja en cours ensuite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteDecision {
    NotFound,
    Settled,
    AlreadyCompleting,
    CodeRequired,
    Ready,
}

/// Deux methodes se recouvrent si meme `type`, et meme `id` pour OAuth.
/// Sert a la fois a l'ajout (remplacement) et au retrait.
pub fn method_matches(candidate: &Method, target: &Method) -> bool {
    match (candidate, target) {
        (Method::OAuth(a), Method::OAuth(b)) => a.id == b.id,
        (Method::Key(_), Method::Key(_)) => true,
        (Method::Env(_), Method::Env(_)) => true,
        _ => false,
    }
}

/// Cree l'entree si absente (`{ id, name: id }`), applique la mutation,
/// puis force `id` : l'editeur ne peut pas renommer l'identifiant.
pub fn registry_update(registry: &mut Registry, id: &str, update: impl FnOnce(&mut Ref)) {
    let entry = registry.entry(id.to_string()).or_insert_with(|| Entry {
        reference: Ref {
            id: id.to_string(),
            name: id.to_string(),
        },
        methods: Vec::new(),
    });
    update(&mut entry.reference);
    entry.reference.id = id.to_string();
}

/// Retire une integration du registre, sans effet si absente.
pub fn registry_remove(registry: &mut Registry, id: &str) {
    registry.remove(id);
}

/// Une integration par identifiant, ou `None` si absente.
pub fn registry_get(registry: &Registry, id: &str) -> Option<Ref> {
    registry.get(id).map(|entry| entry.reference.clone())
}

/// Toutes les references, dans l'ordre deterministe du registre.
pub fn registry_list(registry: &Registry) -> Vec<Ref> {
    registry.values().map(|entry| entry.reference.clone()).collect()
}

/// Methodes declarees d'une integration, vide si absente.
pub fn method_list(registry: &Registry, integration_id: &str) -> Vec<Method> {
    registry
        .get(integration_id)
        .map(|entry| entry.methods.clone())
        .unwrap_or_default()
}

/// Ajoute ou remplace la methode qui recouvre `method` (voir
/// `method_matches`). Cree l'entree si absente, comme le TS.
pub fn method_update(registry: &mut Registry, integration_id: &str, method: Method) {
    let entry = registry.entry(integration_id.to_string()).or_insert_with(|| Entry {
        reference: Ref {
            id: integration_id.to_string(),
            name: integration_id.to_string(),
        },
        methods: Vec::new(),
    });
    match entry.methods.iter_mut().find(|m| method_matches(m, &method)) {
        Some(slot) => *slot = method,
        None => entry.methods.push(method),
    }
}

/// Retire la methode qui recouvre `method`, sans effet si absente.
pub fn method_remove(registry: &mut Registry, integration_id: &str, method: &Method) {
    if let Some(entry) = registry.get_mut(integration_id) {
        entry.methods.retain(|m| !method_matches(m, method));
    }
}

/// Vrai si une methode par cle est declaree (requis pour connecter par cle).
pub fn has_key_method(methods: &[Method]) -> bool {
    methods.iter().any(|m| matches!(m, Method::Key(_)))
}

/// Methode OAuth par identifiant, ou `None` si inconnue.
pub fn find_oauth_method(methods: &[Method], method_id: &str) -> Option<OAuthMethod> {
    methods.iter().find_map(|m| match m {
        Method::OAuth(oauth) if oauth.id == method_id => Some(oauth.clone()),
        _ => None,
    })
}

/// Credentials stockes d'une seule integration, ordre d'arrivee conserve.
pub fn saved_for(all: &[SavedCredential], integration_id: &str) -> Vec<SavedCredential> {
    all.iter()
        .filter(|c| c.integration_id == integration_id)
        .cloned()
        .collect()
}

/// Projette les connexions : credentials stockes, plus recents d'abord
/// (le TS fait `.toReversed()`), puis variables d'environnement presentes.
///
/// Le test de presence env reprend la veracite JS (`if (process.env[n])`) :
/// une variable absente OU vide est ignoree, pas seulement absente.
pub fn resolve_connections(
    saved: &[SavedCredential],
    methods: &[Method],
    env: &EnvMap,
) -> Vec<ConnectionInfo> {
    let mut out: Vec<ConnectionInfo> = saved
        .iter()
        .rev()
        .map(|c| {
            ConnectionInfo::Credential(CredentialConnection {
                id: c.id.clone(),
                label: c.label.clone(),
            })
        })
        .collect();
    for method in methods {
        if let Method::Env(env_method) = method {
            for name in &env_method.names {
                let present = env.get(name).is_some_and(|v| !v.is_empty());
                if present {
                    out.push(ConnectionInfo::Env(EnvConnection { name: name.clone() }));
                }
            }
        }
    }
    out
}

/// Connexion active : la premiere projetee, ou `None` si aucune.
pub fn active_connection(
    saved: &[SavedCredential],
    methods: &[Method],
    env: &EnvMap,
) -> Option<ConnectionInfo> {
    resolve_connections(saved, methods, env).into_iter().next()
}

/// Assemble la vue complete d'une entree et de ses connexions.
pub fn project_info(entry: &Entry, connections: Vec<ConnectionInfo>) -> Info {
    Info {
        id: entry.reference.id.clone(),
        name: entry.reference.name.clone(),
        methods: entry.methods.clone(),
        connections,
    }
}

/// Une integration et ses connexions, ou `None` si inconnue.
pub fn get_info(
    registry: &Registry,
    id: &str,
    all_saved: &[SavedCredential],
    env: &EnvMap,
) -> Option<Info> {
    registry.get(id).map(|entry| {
        project_info(
            entry,
            resolve_connections(&saved_for(all_saved, id), &entry.methods, env),
        )
    })
}

/// Trie les vues par nom croissant.
///
/// Ecart assume : le TS trie par `localeCompare`, ici par ordre des
/// caracteres. Le rang des noms sans accents courants est identique.
pub fn sort_infos_by_name(infos: &mut [Info]) {
    infos.sort_by(|a, b| a.name.cmp(&b.name));
}

/// Toutes les vues, triees par nom croissant.
pub fn list_infos(registry: &Registry, all_saved: &[SavedCredential], env: &EnvMap) -> Vec<Info> {
    let mut infos: Vec<Info> = registry
        .values()
        .map(|entry| {
            let saved = saved_for(all_saved, &entry.reference.id);
            project_info(entry, resolve_connections(&saved, &entry.methods, env))
        })
        .collect();
    sort_infos_by_name(&mut infos);
    infos
}

/// Horodatage d'une tentative creee a `created`.
pub fn make_attempt_time(created: i64) -> AttemptTime {
    AttemptTime {
        created,
        expires: created + ATTEMPT_LIFETIME_MS,
    }
}

/// Forme un identifiant de tentative a partir d'un suffixe fourni.
/// Le suffixe reel (12 hex de temps + 14 aleatoires base62, voir
/// `ascending` de `identifier.ts`) est a la charge de l'appelant.
pub fn format_attempt_id(suffix: &str) -> AttemptId {
    format!("{ATTEMPT_ID_PREFIX}{suffix}")
}

/// Demarre une tentative : `completing` vaut vrai d'entree en mode `auto`,
/// comme le TS qui execute le callback en tache de fond aussitot.
pub fn start_attempt(
    attempt_id: &str,
    integration_id: &str,
    method_id: &str,
    mode: OAuthMode,
    url: &str,
    instructions: &str,
    label: Option<String>,
    created: i64,
) -> (PendingAttempt, Attempt) {
    let time = make_attempt_time(created);
    let pending = PendingAttempt {
        completing: matches!(mode, OAuthMode::Auto),
        mode,
        url: url.to_string(),
        instructions: instructions.to_string(),
        integration_id: integration_id.to_string(),
        method_id: method_id.to_string(),
        label,
        time,
    };
    let attempt = Attempt {
        attempt_id: attempt_id.to_string(),
        url: url.to_string(),
        instructions: instructions.to_string(),
        mode,
        time,
    };
    (pending, attempt)
}

/// Etat observable d'une entree du magasin. Une defaillance sans message
/// rend le message par defaut, comme le TS (`?? "Authorization failed"`).
pub fn status_of(entry: &AttemptEntry) -> AttemptStatus {
    match entry {
        AttemptEntry::Pending(pending) => AttemptStatus::Pending { time: pending.time },
        AttemptEntry::Terminal(terminal) => match terminal.outcome {
            TerminalOutcome::Complete => AttemptStatus::Complete { time: terminal.time },
            TerminalOutcome::Failed => AttemptStatus::Failed {
                message: terminal
                    .message
                    .clone()
                    .unwrap_or_else(|| FAILED_MESSAGE_DEFAULT.to_string()),
                time: terminal.time,
            },
            TerminalOutcome::Expired => AttemptStatus::Expired { time: terminal.time },
        },
    }
}

/// Garde de completion, sans mutation : code requis en mode `code`,
/// refus si deja en cours, silence si deja reglee.
pub fn check_complete(store: &AttemptStore, attempt_id: &str, code: Option<&str>) -> CompleteDecision {
    match store.get(attempt_id) {
        None => CompleteDecision::NotFound,
        Some(AttemptEntry::Terminal(_)) => CompleteDecision::Settled,
        Some(AttemptEntry::Pending(pending)) => {
            if pending.mode == OAuthMode::Code && code.is_none() {
                CompleteDecision::CodeRequired
            } else if pending.completing {
                CompleteDecision::AlreadyCompleting
            } else {
                CompleteDecision::Ready
            }
        }
    }
}

/// Reserve une tentative pour completion (passe `completing` a vrai).
/// Rend la decision ; seul `Ready` mute le magasin.
pub fn claim_complete(store: &mut AttemptStore, attempt_id: &str, code: Option<&str>) -> CompleteDecision {
    let decision = check_complete(store, attempt_id, code);
    if decision == CompleteDecision::Ready {
        if let Some(AttemptEntry::Pending(pending)) = store.get_mut(attempt_id) {
            pending.completing = true;
        }
    }
    decision
}

/// Cloture une tentative en succes, avec retention d'une minute.
pub fn settle_success(pending: &PendingAttempt, now: i64) -> TerminalAttempt {
    TerminalAttempt {
        outcome: TerminalOutcome::Complete,
        message: None,
        remove_at: now + TERMINAL_RETENTION_MS,
        time: pending.time,
    }
}

/// Cloture une tentative en echec, avec retention d'une minute.
pub fn settle_failure(pending: &PendingAttempt, now: i64, message: &str) -> TerminalAttempt {
    TerminalAttempt {
        outcome: TerminalOutcome::Failed,
        message: Some(message.to_string()),
        remove_at: now + TERMINAL_RETENTION_MS,
        time: pending.time,
    }
}

/// Nettoyage : les tentatives en attente expirees passent a `expired`
/// (retention d'une minute), les terminees perimees sont supprimees.
/// Rend le nombre de tentatives expirees par ce passage.
pub fn scrub_entries(store: &mut AttemptStore, now: i64) -> usize {
    let mut closed = 0usize;
    let mut drop_ids: Vec<AttemptId> = Vec::new();
    for (id, entry) in store.iter_mut() {
        match entry {
            AttemptEntry::Pending(pending) if pending.time.expires <= now => {
                let time = pending.time;
                *entry = AttemptEntry::Terminal(TerminalAttempt {
                    outcome: TerminalOutcome::Expired,
                    message: None,
                    remove_at: now + TERMINAL_RETENTION_MS,
                    time,
                });
                closed += 1;
            }
            AttemptEntry::Terminal(terminal) if terminal.remove_at <= now => {
                drop_ids.push(id.clone());
            }
            _ => {}
        }
    }
    for id in drop_ids {
        store.remove(&id);
    }
    closed
}

/// Vrai si un credential OAuth doit etre rafraichi : le TS garde la valeur
/// seulement quand `expires > now + 5 minutes`, sinon il rafraichit.
pub fn needs_oauth_refresh(expires: i64, now: i64) -> bool {
    expires <= now + REFRESH_MARGIN_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn oauth_method(id: &str, label: &str) -> Method {
        Method::OAuth(OAuthMethod {
            id: id.to_string(),
            label: label.to_string(),
            prompts: None,
        })
    }

    fn saved(id: &str, integration: &str, label: &str) -> SavedCredential {
        SavedCredential {
            id: id.to_string(),
            integration_id: integration.to_string(),
            label: label.to_string(),
        }
    }

    fn pending_code(id: &str) -> (AttemptId, AttemptEntry) {
        let (pending, _) = start_attempt(
            id,
            "openai",
            "chatgpt",
            OAuthMode::Code,
            "https://exemple.test/autoriser",
            "Coller le code",
            None,
            1_000,
        );
        (id.to_string(), AttemptEntry::Pending(pending))
    }

    #[test]
    fn un_registre_vide_donne_des_listes_vides() {
        let registry: Registry = BTreeMap::new();
        let env: EnvMap = BTreeMap::new();
        assert!(registry_list(&registry).is_empty());
        assert!(method_list(&registry, "openai").is_empty());
        assert!(get_info(&registry, "openai", &[], &env).is_none());
        assert!(list_infos(&registry, &[], &env).is_empty());
        assert!(resolve_connections(&[], &[], &env).is_empty());
        assert!(active_connection(&[], &[], &env).is_none());
    }

    #[test]
    fn method_matches_separe_les_types_et_les_ids_oauth() {
        let key_a = Method::Key(KeyMethod { label: None });
        let key_b = Method::Key(KeyMethod {
            label: Some("Cle".to_string()),
        });
        let env_m = Method::Env(EnvMethod { names: vec![] });
        // Deux methodes par cle se recouvrent, quel que soit le label.
        assert!(method_matches(&key_a, &key_b));
        assert!(!method_matches(&key_a, &env_m));
        assert!(!method_matches(&oauth_method("m1", "A"), &key_a));
        // OAuth : meme id recouvre, id different ne recouvre pas.
        assert!(method_matches(&oauth_method("m1", "A"), &oauth_method("m1", "B")));
        assert!(!method_matches(&oauth_method("m1", "A"), &oauth_method("m2", "A")));
    }

    #[test]
    fn method_update_remplace_sans_doublon_et_remove_nettoie() {
        let mut registry: Registry = BTreeMap::new();
        registry_update(&mut registry, "openai", |r| r.name = "OpenAI".to_string());
        assert_eq!(registry_get(&registry, "openai").unwrap().name, "OpenAI");
        // L'identifiant force reste inchangeable par l'editeur.
        registry_update(&mut registry, "openai", |r| r.id = "pirate".to_string());
        assert_eq!(registry_get(&registry, "openai").unwrap().id, "openai");

        method_update(&mut registry, "openai", oauth_method("m1", "ChatGPT"));
        method_update(&mut registry, "openai", oauth_method("m1", "ChatGPT Bis"));
        assert_eq!(method_list(&registry, "openai").len(), 1);
        method_update(&mut registry, "openai", oauth_method("m2", "Autre"));
        assert_eq!(method_list(&registry, "openai").len(), 2);
        assert!(find_oauth_method(&method_list(&registry, "openai"), "m1").is_some());
        assert!(find_oauth_method(&method_list(&registry, "openai"), "zzz").is_none());
        assert!(!has_key_method(&method_list(&registry, "openai")));
        method_update(
            &mut registry,
            "openai",
            Method::Key(KeyMethod { label: Some("Cle".to_string()) }),
        );
        assert!(has_key_method(&method_list(&registry, "openai")));

        method_remove(&mut registry, "openai", &oauth_method("m1", "Peu importe"));
        let rest = method_list(&registry, "openai");
        assert_eq!(rest.len(), 2);
        assert!(find_oauth_method(&rest, "m1").is_none());
        registry_remove(&mut registry, "openai");
        assert!(registry_get(&registry, "openai").is_none());
    }

    #[test]
    fn resolve_connections_met_les_recents_d_abord_puis_l_env_present() {
        let all = vec![saved("vieux", "acme", "Vieux"), saved("recent", "acme", "Recent")];
        let kept = saved_for(&all, "acme");
        let methods = vec![Method::Env(EnvMethod {
            names: vec![
                "CLE_PRESENTE".to_string(),
                "CLE_VIDE".to_string(),
                "CLE_ABSENTE".to_string(),
            ],
        })];
        let env: EnvMap = BTreeMap::from([
            ("CLE_PRESENTE".to_string(), "secret".to_string()),
            ("CLE_VIDE".to_string(), "".to_string()),
        ]);
        let out = resolve_connections(&kept, &methods, &env);
        // Plus recent d'abord, puis l'env present ; vide et absent ignores.
        assert_eq!(out.len(), 3);
        assert!(matches!(&out[0], ConnectionInfo::Credential(c) if c.id == "recent"));
        assert!(matches!(&out[1], ConnectionInfo::Credential(c) if c.id == "vieux"));
        assert!(matches!(&out[2], ConnectionInfo::Env(e) if e.name == "CLE_PRESENTE"));
        assert!(matches!(
            active_connection(&kept, &methods, &env).unwrap(),
            ConnectionInfo::Credential(c) if c.id == "recent"
        ));
    }

    #[test]
    fn la_liste_est_triee_par_nom_croissant() {
        let mut registry: Registry = BTreeMap::new();
        registry_update(&mut registry, "zeta", |r| r.name = "Zeta".to_string());
        registry_update(&mut registry, "alpha", |r| r.name = "Alpha".to_string());
        let env: EnvMap = BTreeMap::new();
        let infos = list_infos(&registry, &[], &env);
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].name, "Alpha");
        assert_eq!(infos[1].name, "Zeta");
        let one = get_info(&registry, "zeta", &[], &env).unwrap();
        assert_eq!(one.id, "zeta");
        assert!(one.connections.is_empty());
    }

    #[test]
    fn complete_exige_un_code_en_mode_code() {
        let mut store: AttemptStore = BTreeMap::new();
        let (id_code, entry_code) = pending_code("con_1");
        store.insert(id_code.clone(), entry_code);
        let (auto_pending, _) = start_attempt(
            "con_2",
            "openai",
            "navigateur",
            OAuthMode::Auto,
            "https://exemple.test/autoriser",
            "Se connecter",
            None,
            1_000,
        );
        assert!(auto_pending.completing);
        store.insert("con_2".to_string(), AttemptEntry::Pending(auto_pending));

        assert_eq!(check_complete(&store, "con_1", None), CompleteDecision::CodeRequired);
        assert_eq!(
            check_complete(&store, "con_1", Some("1234")),
            CompleteDecision::Ready
        );
        assert_eq!(check_complete(&store, "con_2", None), CompleteDecision::AlreadyCompleting);
        assert_eq!(check_complete(&store, "zzz", None), CompleteDecision::NotFound);

        assert_eq!(claim_complete(&mut store, "con_1", Some("1234")), CompleteDecision::Ready);
        assert_eq!(
            check_complete(&store, "con_1", Some("1234")),
            CompleteDecision::AlreadyCompleting
        );
        // Une tentative terminee ne fait plus rien, sans erreur.
        let pending = match store.get(&id_code).unwrap() {
            AttemptEntry::Pending(p) => p.clone(),
            _ => panic!("attente d une tentative en attente"),
        };
        store.insert(id_code.clone(), AttemptEntry::Terminal(settle_success(&pending, 2_000)));
        assert_eq!(check_complete(&store, &id_code, Some("1234")), CompleteDecision::Settled);
    }

    #[test]
    fn settle_puis_scrub_font_avancer_les_tentatives() {
        let (pending, attempt) = start_attempt(
            "con_9",
            "openai",
            "chatgpt",
            OAuthMode::Code,
            "https://exemple.test/autoriser",
            "Coller le code",
            Some("Perso".to_string()),
            1_000,
        );
        // La tentative exposee garde les memes temps que l'attente interne.
        assert_eq!(attempt.time, pending.time);
        assert_eq!(pending.time.expires - pending.time.created, ATTEMPT_LIFETIME_MS);

        let done = settle_success(&pending, 2_000);
        assert_eq!(done.remove_at, 2_000 + TERMINAL_RETENTION_MS);
        assert!(matches!(
            status_of(&AttemptEntry::Terminal(done)),
            AttemptStatus::Complete { .. }
        ));
        let failed = settle_failure(&pending, 2_000, "refus");
        assert!(matches!(
            status_of(&AttemptEntry::Terminal(failed)),
            AttemptStatus::Failed { .. }
        ));
        // Sans message, le statut rend le message par defaut.
        let bare = TerminalAttempt {
            outcome: TerminalOutcome::Failed,
            message: None,
            remove_at: 9_999,
            time: pending.time,
        };
        match status_of(&AttemptEntry::Terminal(bare)) {
            AttemptStatus::Failed { message, .. } => assert_eq!(message, FAILED_MESSAGE_DEFAULT),
            _ => panic!("attente d un statut failed"),
        }

        let mut store: AttemptStore = BTreeMap::new();
        store.insert("con_vieux".to_string(), AttemptEntry::Pending(pending.clone()));
        store.insert("con_fini".to_string(), AttemptEntry::Terminal(settle_success(&pending, 500)));
        // Apres expiration : l'attente devient expired, la terminee perimee part.
        let closed = scrub_entries(&mut store, pending.time.expires + 1);
        assert_eq!(closed, 1);
        assert!(matches!(
            status_of(&store["con_vieux"]),
            AttemptStatus::Expired { .. }
        ));
        assert!(!store.contains_key("con_fini"));
        // Second passage apres retention : l'expiree est supprimee aussi.
        let closed2 = scrub_entries(&mut store, pending.time.expires + 1 + TERMINAL_RETENTION_MS + 1);
        assert_eq!(closed2, 0);
        assert!(!store.contains_key("con_vieux"));
    }

    #[test]
    fn le_refresh_est_anticipe_de_cinq_minutes() {
        let now = 100_000_i64;
        assert!(!needs_oauth_refresh(now + REFRESH_MARGIN_MS + 1, now));
        assert!(needs_oauth_refresh(now + REFRESH_MARGIN_MS, now));
        assert!(needs_oauth_refresh(now + 1_000, now));
        assert!(needs_oauth_refresh(now - 1_000, now));
    }

    #[test]
    fn la_serialisation_garde_les_noms_camel_et_les_tags() {
        let attempt = Attempt {
            attempt_id: format_attempt_id("1"),
            url: "https://exemple.test/autoriser".to_string(),
            instructions: "Coller le code".to_string(),
            mode: OAuthMode::Code,
            time: make_attempt_time(1_000),
        };
        let json = serde_json::to_string(&attempt).unwrap();
        assert!(json.contains("\"attemptID\":\"con_1\""));
        assert!(json.contains("\"mode\":\"code\""));
        let back: Attempt = serde_json::from_str(&json).unwrap();
        assert_eq!(back, attempt);

        let info = Info {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            methods: vec![oauth_method("chatgpt", "ChatGPT")],
            connections: vec![ConnectionInfo::Env(EnvConnection {
                name: "CLE_API".to_string(),
            })],
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"type\":\"oauth\""));
        assert!(json.contains("\"type\":\"env\""));
        let back: Info = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);

        let err = IntegrationError::CodeRequired {
            attempt_id: "con_1".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("Integration.CodeRequired"));
        assert!(json.contains("\"attemptID\":\"con_1\""));
    }
}
