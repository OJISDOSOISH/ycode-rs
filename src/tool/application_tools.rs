//! Port of `opencode/packages/core/src/tool/application-tools.ts`.
//!
//! The source is an Effect service, not a data shape: it wires a `State`
//! store, a `Layer` and a global node, none of which are portable. What is
//! portable is the service's one decision, and it is silent when it matters:
//!
//! - `register` returns EARLY when the record is empty, before any name is
//!   validated. An empty registration is not an error; it is a no-op that
//!   never even looks at a name.
//!
//! - Each name goes through `Tool.validateName`, then the entry is filed as
//!   `{ identity: {}, tool }`. The identity is always the empty object - it
//!   exists so the registry has something stable to compare against, not to
//!   carry data. `Identity` here is that `{}`, and it serializes to exactly
//!   `{}`.
//!
//! - `Map.set` overwrites: registering the same name twice keeps only the
//!   LAST entry. No collision, no error, no stack - the shadowing stack lives
//!   in `registry.rs`, not here.
//!
//! Not ported, by decision: the `Tool.AnyTool` handle inside `Entry` is an
//! opaque Effect value with no serializable shape, so `Entry` carries only
//! the identity; the `State`/`Layer`/`Scope` machinery; the global node.
//! Nothing in this file serializes to JSON except `Identity`.

use std::collections::BTreeMap;

use crate::tool::tool::{invalid_name_message, is_valid_name};

/// The `identity: {}` every entry is filed under. Serializes to `{}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Identity {}

/// `Entry`: an identity beside the tool it belongs to.
///
/// The TS entry also holds the `Tool.AnyTool` itself; that handle is opaque
/// Effect machinery and is not carried here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Entry {
    #[serde(rename = "identity")]
    pub identity: Identity,
}

/// The application tools: a name-to-entry map, filled only by `enregistrer`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplicationTools {
    entries: BTreeMap<String, Entry>,
}

impl ApplicationTools {
    /// An empty set of application tools.
    pub fn new() -> Self {
        Self::default()
    }

    /// `register`: validates every name, then files an entry per name.
    ///
    /// An empty set does nothing at all, which is why the TS returns before
    /// validating any name. Duplicate names are silently overwritten - the
    /// last registration of a name wins, like `Map.set`.
    pub fn enregistrer(&mut self, names: &[&str]) -> Result<(), String> {
        if names.is_empty() {
            return Ok(());
        }
        for name in names {
            if !is_valid_name(name) {
                return Err(invalid_name_message(name));
            }
        }
        for name in names {
            self.entries.insert(name.to_string(), Entry::default());
        }
        Ok(())
    }

    /// `entries`: the filed entries, name in order.
    pub fn entrees(&self) -> &BTreeMap<String, Entry> {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enregistrer_refuse_un_nom_invalide_sans_rien_fichier() {
        let mut tools = ApplicationTools::new();
        let erreur = tools.enregistrer(&["read", "1bad"]).unwrap_err();
        assert_eq!(erreur, "Invalid tool name: 1bad");
        assert!(tools.entrees().is_empty(), "nothing is filed when a name fails");
    }

    #[test]
    fn enregistrer_n_valide_rien_quand_la_liste_est_vide() {
        let mut tools = ApplicationTools::new();
        // The empty short-circuit comes BEFORE validation, so no name is looked at.
        assert!(tools.enregistrer(&[]).is_ok());
        assert!(tools.entrees().is_empty());
    }

    #[test]
    fn enregistrer_fichier_une_entree_par_nom() {
        let mut tools = ApplicationTools::new();
        tools.enregistrer(&["read", "write"]).unwrap();
        assert_eq!(tools.entrees().len(), 2);
        assert!(tools.entrees().contains_key("read"));
        assert!(tools.entrees().contains_key("write"));
    }

    #[test]
    fn la_derniere_enregistrement_d_un_nom_gagne() {
        let mut tools = ApplicationTools::new();
        tools.enregistrer(&["read"]).unwrap();
        tools.enregistrer(&["read", "write"]).unwrap();
        // Map.set overwrites: "read" is still there, once, and "write" joined it.
        assert_eq!(tools.entrees().len(), 2);
        assert!(tools.entrees().contains_key("read"));
    }

    #[test]
    fn identity_serialize_en_objet_vide() {
        assert_eq!(serde_json::to_string(&Identity {}).unwrap(), "{}");
        assert_eq!(serde_json::to_string(&Entry::default()).unwrap(), r#"{"identity":{}}"#);
    }
}
