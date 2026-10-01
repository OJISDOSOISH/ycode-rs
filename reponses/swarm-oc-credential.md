# swarm-oc-credential

===DEBUT===
fichier : reponses/swarm-oc-credential.md (code a placer sous src/core/credential.rs par l'agent principal)
source  : opencode/packages/core/src/credential.ts (+ schema opencode/packages/schema/src/credential.ts + table credential/sql.ts)
taille  : 12717 octets
tests   : 9

```rust
//! Portage Rust de `opencode/packages/core/src/credential.ts`, du schema
//! `opencode/packages/schema/src/credential.ts` et de la table
//! `opencode/packages/core/src/credential/sql.ts`.
//!
//! Un credential est un secret stocke pour une integration : soit un jeton
//! OAuth (acces + refresh), soit une cle API simple. La table ne garde qu'un
//! seul credential par integration : la creation remplace l'ancien.
//!
//! La persistance SQL (drizzle + Effect) est remplacee par de la logique
//! metier pure sur tranches : l'appelant fournit les lignes deja triees par
//! date de creation croissante, comme le faisait le `orderBy` SQL d'origine.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Prefixe des identifiants de credential (`cred_` + suite croissante).
pub const ID_PREFIX: &str = "cred_";

/// Label applique quand la creation ne fournit pas de label.
///
/// L'original fait `input.label ?? "default"` : seule l'absence declenche le
/// defaut, une chaine vide survit.
pub const DEFAULT_LABEL: &str = "default";

/// Identifiant opaque d'un credential.
pub type CredentialId = String;

/// Identifiant opaque d'une integration.
pub type IntegrationId = String;

/// Compteur global pour generer des identifiants uniques dans le process.
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Fabrique un identifiant a partir d'un compteur fourni (pur et testable).
pub fn new_id(counter: u64) -> CredentialId {
    format!("{ID_PREFIX}{counter}")
}

/// Fabrique un identifiant unique pour le process courant.
pub fn create_credential_id() -> CredentialId {
    let n = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    new_id(n)
}

/// Variante OAuth de `Credential.Value` : jeton d'acces et de refresh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthCredential {
    #[serde(rename = "methodID")]
    pub method_id: String,
    pub refresh: String,
    pub access: String,
    /// Delai d'expiration en secondes, toujours positif ou nul.
    pub expires: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
}

/// Variante cle API de `Credential.Value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyCredential {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
}

/// Union taggee `Credential.Value`, discriminee par le champ `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CredentialValue {
    #[serde(rename = "oauth")]
    OAuth(OAuthCredential),
    #[serde(rename = "key")]
    Key(KeyCredential),
}

/// Fiche `Credential.Info` : un secret stocke pour une integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CredentialInfo {
    pub id: CredentialId,
    #[serde(rename = "integrationID")]
    pub integration_id: IntegrationId,
    pub label: String,
    pub value: CredentialValue,
}

/// Ligne brute de la table `credential`, telle que lue en base.
///
/// `integration_id` est nullable en SQL et `value` arrive en JSON brut : les
/// deux raisons pour lesquelles une ligne peut etre ignoree.
#[derive(Debug, Clone, PartialEq)]
pub struct CredentialRow {
    pub id: CredentialId,
    pub integration_id: Option<IntegrationId>,
    pub label: String,
    pub value: serde_json::Value,
}

/// Convertit une ligne en fiche, ou `None` si la ligne est inutilisable.
///
/// L'original fait `if (!row.integration_id) return` : test de veracite, donc
/// une chaine vide est ignoree comme un `None`. Un JSON qui ne colle pas au
/// schema donne aussi `None` au lieu de lever.
pub fn stored(row: &CredentialRow) -> Option<CredentialInfo> {
    let integration_id = match &row.integration_id {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return None,
    };
    let value: CredentialValue = serde_json::from_value(row.value.clone()).ok()?;
    Some(CredentialInfo {
        id: row.id.clone(),
        integration_id,
        label: row.label.clone(),
        value,
    })
}

/// Toutes les fiches valides, dans l'ordre de la tranche fournie.
pub fn all_sorted(rows: &[CredentialRow]) -> Vec<CredentialInfo> {
    rows.iter().filter_map(stored).collect()
}

/// Fiches valides d'une seule integration, dans l'ordre fourni.
pub fn list_for_integration(rows: &[CredentialRow], integration_id: &str) -> Vec<CredentialInfo> {
    rows.iter()
        .filter_map(stored)
        .filter(|c| c.integration_id == integration_id)
        .collect()
}

/// Une fiche par identifiant, ou `None` si absente ou inutilisable.
pub fn get_by_id(rows: &[CredentialRow], id: &str) -> Option<CredentialInfo> {
    rows.iter().find(|r| r.id == id).and_then(stored)
}

/// Entree de creation : le label absent donne "default".
#[derive(Debug, Clone, PartialEq)]
pub struct CreateInput {
    pub integration_id: IntegrationId,
    pub value: CredentialValue,
    pub label: Option<String>,
}

/// Construit la fiche a stocker, avec un identifiant frais.
///
/// `label: None` donne `"default"`, `Some("")` survit tel quel (coalescent
/// `??`, pas test de veracite).
pub fn build_create(input: CreateInput) -> CredentialInfo {
    CredentialInfo {
        id: create_credential_id(),
        integration_id: input.integration_id,
        label: input.label.unwrap_or_else(|| DEFAULT_LABEL.to_string()),
        value: input.value,
    }
}

/// Insere en remplacant tout ancien credential de la meme integration.
///
/// Reproduit la transaction d'origine : suppression des lignes de
/// l'integration puis insertion de la nouvelle fiche.
pub fn create_replace(store: &mut Vec<CredentialInfo>, input: CreateInput) -> CredentialInfo {
    let credential = build_create(input);
    store.retain(|c| c.integration_id != credential.integration_id);
    store.push(credential.clone());
    credential
}

/// Modifications partielles d'un credential (label ou valeur secretes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CredentialUpdates {
    pub label: Option<String>,
    pub value: Option<CredentialValue>,
}

/// La garde d'origine `if (!updates.label && !updates.value) return` teste la
/// veracite : `None` et `Some("")` sont tous deux sans effet.
pub fn has_effective_update(updates: &CredentialUpdates) -> bool {
    let label_hit = updates.label.as_deref().is_some_and(|s| !s.is_empty());
    label_hit || updates.value.is_some()
}

/// Applique les modifications, ou rend `false` si il n'y en avait aucune.
pub fn apply_update(credential: &mut CredentialInfo, updates: CredentialUpdates) -> bool {
    if !has_effective_update(&updates) {
        return false;
    }
    if let Some(label) = updates.label {
        if !label.is_empty() {
            credential.label = label;
        }
    }
    if let Some(value) = updates.value {
        credential.value = value;
    }
    true
}

/// Retire une fiche par identifiant, rend `true` si une fiche est partie.
pub fn remove_by_id(store: &mut Vec<CredentialInfo>, id: &str) -> bool {
    let before = store.len();
    store.retain(|c| c.id != id);
    store.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cle_row(id: &str, integration: Option<&str>) -> CredentialRow {
        CredentialRow {
            id: id.to_string(),
            integration_id: integration.map(|s| s.to_string()),
            label: "default".to_string(),
            value: json!({ "type": "key", "key": "secret" }),
        }
    }

    #[test]
    fn un_magasin_vide_donne_une_liste_vide() {
        let rows: Vec<CredentialRow> = vec![];
        assert!(all_sorted(&rows).is_empty());
        assert!(get_by_id(&rows, "cred_1").is_none());
    }

    #[test]
    fn une_ligne_sans_integration_est_ignoree() {
        let rows = vec![cle_row("cred_1", None)];
        assert!(all_sorted(&rows).is_empty());
        assert!(get_by_id(&rows, "cred_1").is_none());
    }

    #[test]
    fn une_integration_vide_est_ignoree_comme_absente() {
        // Piege `?` contre `??` : `if (!row.integration_id)` est un test de
        // veracite, donc `Some("")` est ignore comme `None`.
        let rows = vec![cle_row("cred_1", Some(""))];
        assert!(all_sorted(&rows).is_empty());
    }

    #[test]
    fn un_singleton_valide_passe_et_filtre_par_integration() {
        let rows = vec![cle_row("cred_1", Some("int_1"))];
        let all = all_sorted(&rows);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "cred_1");
        assert_eq!(list_for_integration(&rows, "int_1").len(), 1);
        assert!(list_for_integration(&rows, "int_2").is_empty());
    }

    #[test]
    fn l_ordre_inverse_de_la_tranche_est_conserve() {
        let rows = vec![cle_row("cred_2", Some("int_1")), cle_row("cred_1", Some("int_1"))];
        let all = all_sorted(&rows);
        assert_eq!(all[0].id, "cred_2");
        assert_eq!(all[1].id, "cred_1");
    }

    #[test]
    fn un_label_absent_donne_default_mais_vide_survit() {
        // `?? "default"` : seule l'absence declenche le defaut.
        let key = || CredentialValue::Key(KeyCredential { key: "k".to_string(), metadata: None });
        let sans = build_create(CreateInput {
            integration_id: "int_1".to_string(),
            value: key(),
            label: None,
        });
        assert_eq!(sans.label, "default");
        let vide = build_create(CreateInput {
            integration_id: "int_1".to_string(),
            value: key(),
            label: Some("".to_string()),
        });
        assert_eq!(vide.label, "");
    }

    #[test]
    fn creer_remplace_l_ancien_de_la_meme_integration() {
        let key = || CredentialValue::Key(KeyCredential { key: "k".to_string(), metadata: None });
        let mut store = vec![build_create(CreateInput {
            integration_id: "int_1".to_string(),
            value: key(),
            label: None,
        })];
        create_replace(
            &mut store,
            CreateInput { integration_id: "int_1".to_string(), value: key(), label: Some("nouv".to_string()) },
        );
        assert_eq!(store.len(), 1);
        assert_eq!(store[0].label, "nouv");
    }

    #[test]
    fn une_mise_a_jour_vide_ne_fait_rien() {
        let key = || CredentialValue::Key(KeyCredential { key: "k".to_string(), metadata: None });
        let mut info = build_create(CreateInput {
            integration_id: "int_1".to_string(),
            value: key(),
            label: Some("vieux".to_string()),
        });
        assert!(!apply_update(&mut info, CredentialUpdates::default()));
        assert!(!apply_update(
            &mut info,
            CredentialUpdates { label: Some("".to_string()), value: None }
        ));
        assert_eq!(info.label, "vieux");
        assert!(apply_update(
            &mut info,
            CredentialUpdates { label: Some("neuf".to_string()), value: None }
        ));
        assert_eq!(info.label, "neuf");
    }

    #[test]
    fn la_serialisation_garde_le_camel_case_et_le_tag() {
        let info = CredentialInfo {
            id: "cred_1".to_string(),
            integration_id: "int_1".to_string(),
            label: "default".to_string(),
            value: CredentialValue::OAuth(OAuthCredential {
                method_id: "m_1".to_string(),
                refresh: "r".to_string(),
                access: "a".to_string(),
                expires: 60,
                metadata: None,
            }),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"integrationID\":\"int_1\""));
        assert!(json.contains("\"methodID\":\"m_1\""));
        assert!(json.contains("\"type\":\"oauth\""));
        let back: CredentialInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
    }
}
```
===FIN===

CONFIANCE : moyenne
POINT FAIBLE : le format exact de Credential.ID.create (prefixe cred_ et fonction ascending) n est pas visible depuis le fichier source, le compteur local peut diverger du TypeScript ; les colonnes SQL connector_id, method_id et active sont ignorees car la logique metier ne les lit jamais.
A VERIFIER : le renommage methodID (champ OAuth du schema) et integrationID, et le comportement falsy de stored sur chaine vide.
