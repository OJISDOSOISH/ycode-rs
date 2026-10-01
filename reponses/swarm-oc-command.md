# swarm-oc-command

===DEBUT===
fichier : src/core/command.rs
source  : opencode/packages/core/src/command.ts
taille  : 4761 octets
tests   : 6

```rust
//! Portage Rust de `opencode/packages/core/src/command.ts`.
//!
//! La source TypeScript est un registre de commandes personnalisees adosse
//! a un etat transformable (Effect + State) :
//!
//! - `Data` : une `Map<string, Info>` mutable des commandes connues.
//! - `Draft` : une vue mutable avec `list`, `get`, `update`, `remove`.
//! - `Interface` : le service expose (`get`, `list`, plus reload/transform).
//!
//! En Rust on ne porte que la logique metier pure, sur une tranche de
//! donnees deterministe (`BTreeMap`). Pas d'Effect, pas d'async, pas de
//! couche : juste le comportement observable du registre.
//!
//! Regle pointee par la source : `update` cree une entree par defaut
//! `{ name, template: "" }` si la cle est absente, applique la mise a jour,
//! puis force `current.name = name` pour que la cle et le champ restent
//! coherents.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Reference vers un modele, portage de `Model.Ref`.
///
/// `providerID` garde sa casse exacte via `rename` : ce n'est pas
/// `providerId`. Voir le piege 2 de la charte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Description d'une commande, portage de `Command.Info`.
///
/// Champs obligatoires : `name`, `template`. Tout le reste est optionnel
/// et absent du JSON quand il vaut `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask: Option<bool>,
}

impl CommandInfo {
    /// Construit une commande minimale, comme le defaut `{ name, template }`
    /// de la source quand `update` vise une cle absente.
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template: template.into(),
            description: None,
            agent: None,
            model: None,
            subtask: None,
        }
    }
}

/// Registre des commandes, portage de `Data.commands`.
///
/// `BTreeMap` pour un ordre deterministe (voir table de conversion :
/// `Map` -> `BTreeMap`), donc `list` est trie par nom.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandStore {
    pub commands: BTreeMap<String, CommandInfo>,
}

impl CommandStore {
    /// Cree un registre vide, comme `initial: () => ({ commands: new Map() })`.
    pub fn new() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }

    /// Liste toutes les commandes, triees par nom.
    ///
    /// Portage de `list: () => Array.from(draft.commands.values())`.
    pub fn list(&self) -> Vec<CommandInfo> {
        self.commands.values().cloned().collect()
    }

    /// Recupere une commande par son nom.
    ///
    /// Portage de `get: (name) => draft.commands.get(name)`.
    pub fn get(&self, name: &str) -> Option<&CommandInfo> {
        self.commands.get(name)
    }

    /// Insere ou met a jour une commande.
    ///
    /// Portage de `update` : si la cle est absente on part du defaut
    /// `{ name, template: "" }`, on applique `update`, puis on force
    /// `info.name = name` pour garder cle et champ coherents.
    pub fn update(&mut self, name: &str, update: impl FnOnce(&mut CommandInfo)) {
        let entry = self
            .commands
            .entry(name.to_string())
            .or_insert_with(|| CommandInfo::new(name, ""));
        update(entry);
        entry.name = name.to_string();
    }

    /// Supprime une commande. Sans effet si la cle est absente.
    ///
    /// Portage de `remove: (name) => { draft.commands.delete(name) }`.
    pub fn remove(&mut self, name: &str) {
        self.commands.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_registre_vide_liste_rien() {
        let store = CommandStore::new();
        assert!(store.list().is_empty());
        assert!(store.get("nimporte-quoi").is_none());
    }

    #[test]
    fn un_singleton_est_visible_par_get_et_list() {
        let mut store = CommandStore::new();
        store.update("deploy", |c| c.template = "deploy.sh".to_string());
        let got = store.get("deploy").expect("commande absente");
        assert_eq!(got.name, "deploy");
        assert_eq!(got.template, "deploy.sh");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn update_cree_le_defaut_puis_force_le_nom() {
        // `update` sur une cle absente part de `{ name, template: "" }`.
        let mut store = CommandStore::new();
        store.update("lint", |_| {});
        let got = store.get("lint").expect("commande absente");
        assert_eq!(got.template, "");
        assert_eq!(got.name, "lint");
        // Meme si la mise a jour tente de changer le nom, la cle gagne.
        store.update("lint", |c| c.name = "pirate".to_string());
        let got = store.get("lint").expect("commande absente");
        assert_eq!(got.name, "lint");
        assert!(store.get("pirate").is_none());
    }

    #[test]
    fn la_liste_est_triee_par_nom_meme_en_ordre_inverse() {
        let mut store = CommandStore::new();
        for name in ["zebra", "mike", "alpha"] {
            store.update(name, |c| c.template = "x".to_string());
        }
        let names: Vec<String> = store.list().iter().map(|c| c.name.clone()).collect();
        assert_eq!(names, vec!["alpha", "mike", "zebra"]);
    }

    #[test]
    fn remove_supprime_et_reste_silencieux_sur_absent() {
        let mut store = CommandStore::new();
        store.update("a", |c| c.template = "x".to_string());
        store.remove("absent-sans-effet");
        assert!(store.get("a").is_some());
        store.remove("a");
        assert!(store.get("a").is_none());
        assert!(store.list().is_empty());
    }

    #[test]
    fn la_serialisation_garde_les_noms_de_champs_attendus() {
        // Les optionnels absents ne sont pas serialises, et `providerID`
        // garde sa casse exacte (piege 2 de la charte).
        let mut store = CommandStore::new();
        store.update("run", |c| {
            c.template = "run.sh".to_string();
            c.model = Some(ModelRef {
                id: "m1".to_string(),
                provider_id: "p1".to_string(),
                variant: None,
            });
        });
        let info = store.get("run").expect("commande absente");
        let json = serde_json::to_value(info).expect("serialisation");
        assert_eq!(json["name"], "run");
        assert_eq!(json["template"], "run.sh");
        assert!(json.get("description").is_none());
        assert!(json.get("agent").is_none());
        assert!(json.get("subtask").is_none());
        assert_eq!(json["model"]["providerID"], "p1");
        assert!(json["model"].get("providerId").is_none());
        assert!(json["model"].get("variant").is_none());
    }
}
```

===FIN===

CONFIANCE : haute
POINT FAIBLE : Le shim `export * as CommandV2 from "./command"` en tete du fichier source est un auto re-export probablement genere par erreur ; je l'ai ignore et porte uniquement le registre, mais s'il designait un autre module le portage serait incomplet.
A VERIFIER : confirmer que `Model.Ref` (id, providerID, variant?) est bien la forme attendue cote Rust et que `list` trie par nom convient alors que l'original suivait l'ordre d'insertion.
