//! Portage de `packages/core/src/plugin/provider.ts`.
//!
//! La source fait 71 lignes : elle importe les 32 plugins fournisseurs puis
//! les range dans `ProviderPlugins`, dans un ordre fige. Aucune logique, un
//! seul export de fond.
//!
//! Ce module porte la partie pure : la liste des identifiants dans l ordre de
//! la source, avec le predicat d appartenance. Les objets plugins eux-memes
//! (`AlibabaPlugin`, `define`, `Effect`, `Scope`) dependent du runtime et ne
//! sont pas portes.

/// Identifiants des plugins fournisseurs, dans l ordre de `ProviderPlugins`.
pub const PLUGINS_FOURNISSEURS: [&str; 34] = [
    "alibaba",
    "amazon-bedrock",
    "anthropic",
    "azure-cognitive-services",
    "azure",
    "cerebras",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "cohere",
    "deepinfra",
    "gateway",
    "github-copilot",
    "gitlab",
    "google",
    "google-vertex-anthropic",
    "google-vertex",
    "groq",
    "kilo",
    "llmgateway",
    "mistral",
    "nvidia",
    "opencode",
    "snowflake-cortex",
    "openai-compatible",
    "openai",
    "openrouter",
    "perplexity",
    "sap-ai-core",
    "togetherai",
    "vercel",
    "venice",
    "xai",
    "zenmux",
    "dynamic",
];

/// Nombre de plugins dans le registre.
pub const NOMBRE_PLUGINS: usize = PLUGINS_FOURNISSEURS.len();

/// Vrai si l identifiant figure au registre.
pub fn est_plugin_fournisseur(id: &str) -> bool {
    PLUGINS_FOURNISSEURS.contains(&id)
}

/// Position d un identifiant dans le registre, ou `None` s il est absent.
pub fn position_plugin(id: &str) -> Option<usize> {
    PLUGINS_FOURNISSEURS.iter().position(|candidat| *candidat == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn le_registre_contient_trente_quatre_entrees() {
        assert_eq!(NOMBRE_PLUGINS, 34);
        assert_eq!(PLUGINS_FOURNISSEURS.len(), 34);
    }

    #[test]
    fn le_premier_et_le_dernier_sont_ceux_de_la_source() {
        assert_eq!(PLUGINS_FOURNISSEURS.first(), Some(&"alibaba"));
        assert_eq!(PLUGINS_FOURNISSEURS.last(), Some(&"dynamic"));
    }

    #[test]
    fn le_plugin_dynamique_est_bien_le_dernier() {
        assert_eq!(position_plugin("dynamic"), Some(33));
        assert_eq!(position_plugin("alibaba"), Some(0));
    }

    #[test]
    fn chaque_fournisseur_connu_est_reconnu() {
        for id in ["openai", "anthropic", "google", "github-copilot", "llmgateway", "openai-compatible"] {
            assert!(est_plugin_fournisseur(id), "plugin oublie : {}", id);
        }
    }

    #[test]
    fn un_inconnu_n_est_pas_un_plugin() {
        assert!(!est_plugin_fournisseur("inconnu"));
        assert!(!est_plugin_fournisseur(""));
        assert_eq!(position_plugin("inconnu"), None);
    }

    #[test]
    fn les_identifiants_sont_uniques() {
        let uniques: BTreeSet<&&str> = PLUGINS_FOURNISSEURS.iter().collect();
        assert_eq!(uniques.len(), PLUGINS_FOURNISSEURS.len());
    }
}
