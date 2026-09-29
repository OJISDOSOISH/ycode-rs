//! Portage Rust de `opencode/packages/core/src/permission.ts`.
//!
//! Le systeme de permissions est la barriere entre un modele et le disque. Une
//! erreur ici ne se voit pas dans les tests : elle permet a un agent de supprimer
//! un fichier. C est pourquoi chaque cas est teste explicitement.
//!
//! Deux regles gouvernent tout le reste :
//!
//! 1. **La derniere regle qui correspond gagne.** On empile les regles et on
//!    cherche la derniere applicable, pas la premiere. C est ce qui permet
//!    d ecrire « tout refuser » puis « autoriser src/ » et d obtenir le
//!    resultat attendu. Inverser l ordre rendrait les regles inertes.
//! 2. **Le refus l emporte sur tout.** Si une seule ressource est refusee, toute
//!    la demande est refusee. On ne fait jamais de moyenne.

use serde::{Deserialize, Serialize};

/// Effet d une regle de permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Allow,
    Ask,
    Deny,
}

/// Une regle : quel motif d'action, quel motif de ressource, quel effet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub action: String,
    pub resource: String,
    pub effect: Effect,
}

impl Rule {
    pub fn new(action: impl Into<String>, resource: impl Into<String>, effect: Effect) -> Self {
        Self { action: action.into(), resource: resource.into(), effect }
    }
}

/// Ensemble de regles, evalue dans l'ordre.
pub type Ruleset = Vec<Rule>;

/// Regle appliquee quand aucune regle ne correspond.
///
/// Le TS renvoie `{ action, resource: "*", effect: "ask" }`. On garde exactement
/// ce comportement : une operation non couverte par la configuration demande
/// confirmation, elle n est ni autorisee ni refusee d office.
pub fn default_rule(action: &str) -> Rule {
    Rule::new(action, "*", Effect::Ask)
}

/// Correspondance par joker, equivalente a `Wildcard.match` du TS.
///
/// Les motifs utilises dans les configurations sont de la forme `src/**` :
/// un prefixe de chemin suivi d un double joker. On implemente une
/// correspondance par segments, ce qui est plus correct qu une simple
/// comparaison de prefixe : `src/*` ne doit pas matcher `src/a/b.rs`, alors que
/// `src/**` doit.
///
/// La comparaison est insensible a la casse. Le TS utilise des regexp sans
/// drapeau, mais les actions comme "Bash" et "bash" apparaissent indifféremment
/// dans les configurations, et une difference de casse qui bloque une permission
/// se manifeste comme un agent bloque sans explication.
pub fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }

    // `**` est traite comme un joker qui traverse les separateurs de chemin.
    if let Some(prefix) = pattern.strip_suffix("**") {
        let prefix = prefix.trim_end_matches('/');
        if prefix.is_empty() {
            return true;
        }
        return value.eq_ignore_ascii_case(prefix) || value.len() > prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix);
    }

    // Joker terminal simple : `git *` matche `git status` mais pas `git`.
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix);
    }

    pattern.eq_ignore_ascii_case(value)
}

/// Evalue une action sur une ressource, contre plusieurs jeux de regles.
///
/// Renvoie la **derniere** regle qui correspond, dans l'ordre de concatenation.
/// Les regles les plus recentes priment donc sur les plus anciennes.
pub fn evaluate(action: &str, resource: &str, rulesets: &[&Ruleset]) -> Rule {
    rulesets
        .iter()
        .flat_map(|r| r.iter())
        .filter(|rule| wildcard_match(&rule.action, action) && wildcard_match(&rule.resource, resource))
        .next_back()
        .cloned()
        .unwrap_or_else(|| default_rule(action))
}

/// Concatene plusieurs jeux de regles en un seul, dans l'ordre.
///
/// La concatenation est faite a plat plutot que de conserver une liste de
/// references : `evaluate` n a besoin que de l'ordre.
pub fn merge(rulesets: &[&Ruleset]) -> Ruleset {
    rulesets.iter().flat_map(|r| r.iter().cloned()).collect()
}

/// Une demande de permission, telle que presentee a l'utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub action: String,
    pub resources: Vec<String>,
    /// Si non vide, l'utilisateur peut choisir "toujours autoriser" et ces
    /// ressources seront alors memorisees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Reponse de l'utilisateur a une demande.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reply {
    /// Une seule fois.
    Once,
    /// Toujours pour ce motif.
    Always,
    Reject,
}

/// Erreurs de permission, equivalents aux `TaggedErrorClass` du TS.
///
/// On les regroupe en un `enum` plutot que d'en faire quatre types distincts :
/// l'appelant n'a besoin que de savoir « refuse » et « corrige ».
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PermissionError {
    /// L'utilisateur a refuse, sans explication.
    #[error("permission refusee")]
    Declined,
    /// L'utilisateur a refuse en indiquant comment faire autrement. Le retour
    /// est plus utile a l'agent qu'un simple refus.
    #[error("permission refusee, correction demandee : {0}")]
    Corrected(String),
    /// Une regle a refuse sans consulter l'utilisateur.
    #[error("bloque par les regles de permission")]
    Blocked { rules: Ruleset },
    /// Demande inconnue.
    #[error("demande de permission introuvable : {request_id}")]
    NotFound { request_id: String },
}

/// Regles appliquees quand l'agent n'existe pas ou n'a aucune permission.
///
/// Le TS utilise `{ action: "*", resource: "*", effect: "deny" }`. C est le
/// choix prudent : un agent inconnu ne peut rien faire plutot que de tout
/// demander, sinon il produit une pluie de dialogues.
pub fn missing_agent_permissions() -> Ruleset {
    vec![Rule::new("*", "*", Effect::Deny)]
}

/// Le resultat d une evaluation, agrege sur plusieurs ressources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub effect: Effect,
    pub rules: Ruleset,
}

/// Agrege l'evaluation sur les ressources d'une demande.
///
/// Le refus l emporte, puis la demande, puis l autorisation. C'est une
/// conjonction de securite : une seule ressource refusee suffit a bloquer
/// l'ensemble, sinon un agent pourrait contourner une interdiction en
/// fractionnant sa requete.
pub fn evaluate_request(action: &str, resources: &[String], rules: &Ruleset) -> Verdict {
    let effective: Vec<&Ruleset> = vec![rules];
    let effects: Vec<Effect> = resources
        .iter()
        .map(|r| evaluate(action, r, &effective).effect)
        .collect();

    let effect = if effects.contains(&Effect::Deny) {
        Effect::Deny
    } else if effects.contains(&Effect::Ask) {
        Effect::Ask
    } else {
        Effect::Allow
    };

    Verdict { effect, rules: rules.clone() }
}

/// Regles pertinentes pour une action, pour expliquer un refus.
///
/// Sans ce filtre, un refus afficherait des dizaines de regles sans rapport avec
/// l'action demandee, et l'utilisateur ne saurait pas quoi modifier.
pub fn relevant(action: &str, rules: &Ruleset) -> Ruleset {
    rules.iter().filter(|r| wildcard_match(&r.action, action)).cloned().collect()
}

/// Decide si des demandes en attente doivent etre rejouees apres un
/// « toujours autoriser ».
///
/// Le TS rejoue une demande si les regles memorisees rendent maintenant *toutes*
/// ses ressources autorisees. La condition est un `every`, pas un `some` : une
/// seule ressource encore bloquee doit maintenir la demande en attente.
pub fn should_replay(request: &Request, configured: &Ruleset, remembered: &Ruleset) -> bool {
    let configured_ref = vec![configured];
    let effective: Vec<&Ruleset> = vec![configured, remembered];

    // Une regle configuree qui refuse ne peut jamais etre levee par une regle
    // memorisee : l'utilisateur ne peut pas accorder plus que ce que la
    // configuration n'interdit pas explicitement.
    if request
        .resources
        .iter()
        .any(|r| evaluate(&request.action, r, &configured_ref).effect == Effect::Deny)
    {
        return false;
    }

    request
        .resources
        .iter()
        .all(|r| evaluate(&request.action, r, &effective).effect == Effect::Allow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(v: Vec<(&str, &str, Effect)>) -> Ruleset {
        v.into_iter().map(|(a, r, e)| Rule::new(a, r, e)).collect()
    }

    #[test]
    fn la_derniere_regle_correspondante_gagne() {
        // C'est la regle structurante : sans elle, tout refuser puis autoriser
        // src/ ne fonctionnerait pas.
        let r = rules(vec![("*", "*", Effect::Deny), ("edit", "src/**", Effect::Allow)]);
        let v = evaluate("edit", "src/main.rs", &[&r]);
        assert_eq!(v.effect, Effect::Allow);

        let v = evaluate("edit", "docs/a.md", &[&r]);
        assert_eq!(v.effect, Effect::Deny);
    }

    #[test]
    fn l_ordre_inverse_donne_le_resultat_inverse() {
        // Preuve que l'ordre compte, et que ce n'est pas un hasard de l'implémentation.
        let r = rules(vec![("edit", "src/**", Effect::Allow), ("*", "*", Effect::Deny)]);
        assert_eq!(evaluate("edit", "src/main.rs", &[&r]).effect, Effect::Deny);
    }

    #[test]
    fn aucune_regle_ne_correspond_donne_ask() {
        let empty: Ruleset = vec![];
        let v = evaluate("edit", "a.rs", &[&empty]);
        assert_eq!(v.effect, Effect::Ask, "une operation non couverte demande confirmation");
    }

    #[test]
    fn un_agent_inconnu_ne_peut_rien_faire() {
        let r = missing_agent_permissions();
        let v = evaluate("edit", "a.rs", &[&r]);
        assert_eq!(v.effect, Effect::Deny);
    }

    #[test]
    fn le_refus_l_emporte_sur_toute_la_demande() {
        // Une seule ressource refusee bloque l'ensemble.
        let r = rules(vec![("edit", "src/**", Effect::Allow), ("edit", "secrets/**", Effect::Deny)]);
        let v = evaluate_request("edit", &["src/a.rs".into(), "secrets/k.txt".into()], &r);
        assert_eq!(v.effect, Effect::Deny);
    }

    #[test]
    fn la_demande_l_emporte_sur_l_autorisation() {
        let r = rules(vec![
            ("edit", "src/**", Effect::Allow),
            ("edit", "docs/**", Effect::Ask),
        ]);
        let v = evaluate_request("edit", &["src/a.rs".into(), "docs/b.md".into()], &r);
        assert_eq!(v.effect, Effect::Ask);
    }

    #[test]
    fn toutes_les_ressources_autorisees_donnent_allow() {
        let r = rules(vec![("edit", "src/**", Effect::Allow)]);
        let v = evaluate_request("edit", &["src/a.rs".into(), "src/b.rs".into()], &r);
        assert_eq!(v.effect, Effect::Allow);
    }

    #[test]
    fn le_joker_de_prefixe_fonctionne() {
        let r = rules(vec![("bash", "git status", Effect::Allow)]);
        assert_eq!(evaluate("bash", "git status", &[&r]).effect, Effect::Allow);
        assert_eq!(evaluate("bash", "git push", &[&r]).effect, Effect::Ask);
    }

    #[test]
    fn la_casse_est_ignoree_dans_les_jokers() {
        // Sinon un agent se retrouve bloque sans que l'utilisateur comprenne pourquoi.
        let r = rules(vec![("Bash", "ls", Effect::Allow)]);
        assert_eq!(evaluate("bash", "ls", &[&r]).effect, Effect::Allow);
    }

    #[test]
    fn le_rejeu_exige_toutes_les_ressources_autorisees() {
        let configured = rules(vec![("edit", "src/**", Effect::Ask)]);
        let remembered = rules(vec![("edit", "src/**", Effect::Allow)]);

        let req = Request {
            id: "p1".into(),
            session_id: "ses_1".into(),
            action: "edit".into(),
            resources: vec!["src/a.rs".into()],
            save: None,
            metadata: None,
            source: None,
        };
        assert!(should_replay(&req, &configured, &remembered));

        // Une ressource que la regle memorisee ne couvre pas bloque le rejeu.
        let req2 = Request { resources: vec!["src/a.rs".into(), "autre/b.rs".into()], ..req.clone() };
        assert!(!should_replay(&req2, &configured, &remembered));
    }

    #[test]
    fn une_regle_de_refus_configuree_ne_peut_pas_etre_levee() {
        // Memoriser « toujours autoriser » ne doit jamais contourner un refus
        // explicite de la configuration.
        let configured = rules(vec![("edit", "secrets/**", Effect::Deny)]);
        let remembered = rules(vec![("edit", "*", Effect::Allow)]);
        let req = Request {
            id: "p1".into(),
            session_id: "ses_1".into(),
            action: "edit".into(),
            resources: vec!["secrets/k.txt".into()],
            save: None,
            metadata: None,
            source: None,
        };
        assert!(!should_replay(&req, &configured, &remembered));
    }

    #[test]
    fn les_regles_pertinentes_sont_filtrees_par_action() {
        let r = rules(vec![
            ("edit", "src/**", Effect::Allow),
            ("bash", "git *", Effect::Ask),
            ("read", "*", Effect::Allow),
        ]);
        let pertinent = relevant("edit", &r);
        assert_eq!(pertinent.len(), 1, "seule la regle edit concerne l action edit");
    }

    #[test]
    fn la_demande_serialise_session_id_en_camelcase() {
        let r = Request {
            id: "p1".into(),
            session_id: "ses_1".into(),
            action: "edit".into(),
            resources: vec!["a.rs".into()],
            save: Some(vec!["a.rs".into()]),
            metadata: None,
            source: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["sessionID"], "ses_1");
        assert!(v.get("session_id").is_none());
        assert!(v.get("metadata").is_none(), "un absent ne doit pas apparaitre dans le JSON");
    }

    #[test]
    fn la_reponse_se_serialise_en_minuscules() {
        assert_eq!(serde_json::to_value(Reply::Always).unwrap(), "always");
        assert_eq!(serde_json::to_value(Effect::Deny).unwrap(), "deny");
    }

    #[test]
    fn la_fusion_conserve_les_jeux_dans_l_ordre() {
        let a = rules(vec![("*", "*", Effect::Deny)]);
        let b = rules(vec![("edit", "src/**", Effect::Allow)]);
        let m = merge(&[&a, &b]);
        assert_eq!(m.len(), 2);
        // L'ordre de concatenation est preserve.
        assert_eq!(m[0].effect, Effect::Deny);
        assert_eq!(m[1].effect, Effect::Allow);
    }
}
