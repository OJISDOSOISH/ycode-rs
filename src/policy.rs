//! Portage Rust de `opencode/packages/core/src/policy.ts`.
//!
//! Le service de politique decide allow / deny pour un couple
//! (action, ressource), a partir d une liste de declarations chargees.
//! C est le pendant minimal du systeme de permissions : ici pas de
//! demande a l utilisateur, juste une decision avec repli.
//!
//! Regles de portage suivies :
//! - `Schema.Literals(["allow", "deny"])` devient un enum serde en minuscules.
//! - `Effect.fn` devient des methodes Rust pures et synchrones.
//! - `findLast` du TS est reproduit par un parcours inverse : la derniere
//!   declaration qui correspond gagne.
//! - `Wildcard.match` est reimplemente sans crate regex pour garder le meme
//!   Cargo.lock : normalisation des backslashes, `*` = toute sequence,
//!   `?` = un caractere, casse insensible sur Windows seulement comme le TS.

use serde::{Deserialize, Serialize};

/// Effet d une declaration de politique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Allow,
    Deny,
}

/// Une declaration : quel motif d action, quel effet, quel motif de ressource.
///
/// Les noms de champs sont en minuscules dans le TS comme dans le JSON,
/// donc aucun `rename` n est requis. On garde l ordre du TS
/// (action, effect, resource) pour la lisibilite des diffs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub action: String,
    pub effect: Effect,
    pub resource: String,
}

impl Info {
    pub fn new(action: impl Into<String>, effect: Effect, resource: impl Into<String>) -> Self {
        Self { action: action.into(), effect, resource: resource.into() }
    }
}

/// Correspondance par joker, equivalente a `Wildcard.match` du TS.
///
/// Etapes reproduites :
/// 1. les `\` deviennent des `/` des deux cotes.
/// 2. `*` matche toute sequence (y compris vide et avec `/`), `?` matche
///    un seul caractere. Les autres caracteres regex sont matche au sens
///    litteral.
/// 3. un motif qui se termine par ` *` (devenu ` .*` apres conversion)
///    matche aussi le prefixe seul : `git *` matche `git` et `git status`.
///    C est le cas `slice(0, -3) + "( .*)?"` du TS.
/// 4. casse insensible sur Windows (`si`), sensible ailleurs (`s`).
///    Le flag `s` (dotAll) ne change rien ici car `.` matche deja `\n`
///    dans notre implementation manuelle.
pub fn wildcard_match(input: &str, pattern: &str) -> bool {
    let normalized: String = input.replace('\\', "/");
    let pat: String = pattern.replace('\\', "/");

    // Cas special du TS : motif se terminant par " *".
    // Apres remplacement de `*` par `.*`, le TS obtient `... .*` puis le
    // reecrit en `...( .*)?`. On reproduit directement : le prefixe seul
    // matche, ou le prefixe suivi d un espace puis de n importe quoi.
    if let Some(prefix) = pat.strip_suffix(" *") {
        if glob_match(&normalized, prefix) {
            return true;
        }
        if let Some(rest) = normalized.strip_prefix(prefix.as_ref() as &str) {
            if rest.starts_with(' ') {
                return true;
            }
        }
        return glob_match(&normalized, &pat);
    }

    glob_match(&normalized, &pat)
}

fn char_eq(a: char, b: char) -> bool {
    if cfg!(windows) {
        a.to_lowercase().next() == b.to_lowercase().next() && {
            // Comparaison insensible a la casse, y compris Unicode simple.
            a.to_lowercase().eq(b.to_lowercase())
        }
    } else {
        a == b
    }
}

fn glob_match(input: &str, pattern: &str) -> bool {
    let input: Vec<char> = input.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    glob_rec(&input, &pattern)
}

fn glob_rec(input: &[char], pattern: &[char]) -> bool {
    if pattern.is_empty() {
        return input.is_empty();
    }
    if pattern[0] == '*' {
        // `*` matche zero ou plusieurs caracteres : on essaie toutes les
        // positions, en commencant par zero pour favoriser le match vide.
        for i in 0..=input.len() {
            if glob_rec(&input[i..], &pattern[1..]) {
                return true;
            }
        }
        return false;
    }
    if input.is_empty() {
        return false;
    }
    if pattern[0] == '?' {
        return glob_rec(&input[1..], &pattern[1..]);
    }
    if char_eq(input[0], pattern[0]) {
        return glob_rec(&input[1..], &pattern[1..]);
    }
    false
}

/// Service de politique en memoire.
///
/// Le TS utilise un Layer Effect avec `statements` mutable en closure.
/// On garde la meme semantique avec un struct simple : `load` remplace
/// tout, `evaluate` cherche la derniere correspondance.
#[derive(Debug, Default, Clone)]
pub struct PolicyService {
    statements: Vec<Info>,
}

impl PolicyService {
    pub fn new() -> Self {
        Self { statements: Vec::new() }
    }

    /// Charge (remplace) les declarations.
    pub fn load(&mut self, statements: Vec<Info>) {
        self.statements = statements;
    }

    /// Vrai si au moins une declaration est chargee.
    pub fn has_statements(&self) -> bool {
        !self.statements.is_empty()
    }

    /// Evalue un couple (action, ressource).
    ///
    /// Renvoie l effet de la derniere declaration qui matche les deux
    /// motifs, ou `fallback` si aucune ne matche.
    pub fn evaluate(&self, action: &str, resource: &str, fallback: Effect) -> Effect {
        for statement in self.statements.iter().rev() {
            if wildcard_match(action, &statement.action)
                && wildcard_match(resource, &statement.resource)
            {
                return statement.effect;
            }
        }
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sans_declaration_on_renvoie_le_repli() {
        let svc = PolicyService::new();
        assert!(!svc.has_statements());
        assert_eq!(svc.evaluate("read", "src/a.ts", Effect::Deny), Effect::Deny);
        assert_eq!(svc.evaluate("read", "src/a.ts", Effect::Allow), Effect::Allow);
    }

    #[test]
    fn la_derniere_declaration_qui_matche_gagne() {
        // Reproduit le `findLast` du TS : l ordre compte.
        let mut svc = PolicyService::new();
        svc.load(vec![
            Info::new("*", Effect::Deny, "*"),
            Info::new("read", Effect::Allow, "src/*"),
        ]);
        assert!(svc.has_statements());
        assert_eq!(svc.evaluate("read", "src/a.ts", Effect::Deny), Effect::Allow);
        assert_eq!(svc.evaluate("read", "autre/b.ts", Effect::Allow), Effect::Deny);
    }

    #[test]
    fn l_ordre_inverse_donne_le_resultat_inverse() {
        let mut svc = PolicyService::new();
        svc.load(vec![
            Info::new("read", Effect::Allow, "src/*"),
            Info::new("*", Effect::Deny, "*"),
        ]);
        assert_eq!(svc.evaluate("read", "src/a.ts", Effect::Allow), Effect::Deny);
    }

    #[test]
    fn le_joker_etoile_matche_les_chemins() {
        assert!(wildcard_match("src/a/b.ts", "src/*"));
        assert!(wildcard_match("n importe quoi", "*"));
        assert!(!wildcard_match("src/a.ts", "docs/*"));
    }

    #[test]
    fn le_motif_git_etoile_matche_git_seul() {
        // Cas special du TS : `git *` matche `git` tout seul.
        assert!(wildcard_match("git", "git *"));
        assert!(wildcard_match("git status", "git *"));
        assert!(!wildcard_match("gitx", "git *"));
    }

    #[test]
    fn le_point_d_interrogation_matche_un_caractere() {
        assert!(wildcard_match("src/ab.ts", "src/??.ts"));
        assert!(!wildcard_match("src/a.ts", "src/??.ts"));
    }

    #[test]
    fn les_backslashes_sont_normalises() {
        assert!(wildcard_match("src\\a.ts", "src/a.ts"));
        assert!(wildcard_match("src/a.ts", "src\\a.ts"));
    }

    #[test]
    fn l_enum_et_info_se_serialisent_comme_le_ts() {
        assert_eq!(serde_json::to_value(Effect::Allow).unwrap(), json!("allow"));
        assert_eq!(serde_json::to_value(Effect::Deny).unwrap(), json!("deny"));
        let info = Info::new("read", Effect::Allow, "src/*");
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["action"], "read");
        assert_eq!(v["effect"], "allow");
        assert_eq!(v["resource"], "src/*");
    }

    #[test]
    fn load_remplace_tout_l_historique() {
        let mut svc = PolicyService::new();
        svc.load(vec![Info::new("*", Effect::Deny, "*")]);
        assert!(svc.has_statements());
        svc.load(vec![]);
        assert!(!svc.has_statements());
        assert_eq!(svc.evaluate("read", "x", Effect::Allow), Effect::Allow);
    }
}
