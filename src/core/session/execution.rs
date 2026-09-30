//! Portage Rust de `opencode/packages/core/src/session/execution.ts`.
//!
//! Ce fichier **ne fait aucun travail** : comme son source TypeScript, il se
//! contente de *declarer* un service et d'en fournir une implementation vide.
//! Il n'y a ni execution, ni planification, ni etat ici.
//!
//! Ce qui est traduit :
//! - l'interface `Interface` devient le `trait` [`Service`] ;
//! - le `Context.Service` devient le `trait` lui-meme (voir la note sur le
//!   graphe de couches plus bas) ;
//! - le `Layer.succeed(noopLayer)` devient la structure [`Noop`].
//!
//! Le `ReadonlySet<SessionSchema.ID>` devient un `BTreeSet<String>` : l'ordre
//! d'iteration est alors deterministe, ce que ne garantissait pas un `HashSet`.
//!
//! ## Ce qui n'est pas traduit, et pourquoi
//!
//! `SessionRunner.RunError` (type d'erreur de `resume`) n'existe pas encore en
//! Rust. Plutot que d'inventer un type qui n'a pas de correspondant, le `trait`
//! expose un **type associe** `Error` : chaque implementation choisit le sien.
//! `Noop`, qui ne peut jamais echouer, choisit `std::convert::Infallible`.
//!
//! Le `export const node = LayerNode.unbound(Service, Node.tags.values.global)`
//! n'a pas de traduction : le graphe de couches n'existe pas encore en Rust, et
//! inventer une mecanique de rattachement serait fabriquer du code absent de la
//! source. Le nom du service est neanmoins conserve dans [`SERVICE_TAG`].
//! Le `export * as SessionExecution from "./execution"` de la ligne 1 est un
//! simple reexport, sans traduction necessaire en Rust.

use std::collections::BTreeSet;

/// Identifiant du service dans le graphe Effect d'origine.
pub const SERVICE_TAG: &str = "@opencode/v2/SessionExecution";

/// Service d'execution des sessions.
///
/// Achemine l'execution depuis un identifiant de session vers le runner
/// possede par la Location de cette session.
pub trait Service {
    /// Type d'erreur que `resume` peut renvoyer.
    ///
    /// En TypeScript, c'est `SessionRunner.RunError`, qui n'est pas encore porte
    /// en Rust. Chaque implementation declare donc le sien.
    type Error;

    /// Instantanes des executions actives detenues par ce processus.
    fn active(&self) -> BTreeSet<String>;

    /// Demarre l'execution si le service est inactif, ou rejoint l'execution
    /// deja en cours.
    fn resume(&self, session_id: &str) -> Result<(), Self::Error>;

    /// Enregistre du travail nouvellement enregistre. Plusieurs reveils peuvent
    /// se regrouper en un seul.
    fn wake(&self, session_id: &str);

    /// Interrompt le travail actif deten par ce processus. Interrompre un
    /// service inactif ne fait rien.
    fn interrupt(&self, session_id: &str);
}

/// Implementation vide du service.
///
/// Elle sert aux appelants qui n'ont besoin que de l'enregistrement durable des
/// sessions : aucune execution n'est jamais lancee, l'ensemble des sessions
/// actives reste vide, et aucune operation n'echoue.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Noop;

impl Service for Noop {
    type Error = std::convert::Infallible;

    /// Aucune session n'est jamais active.
    fn active(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }

    /// Ne lance rien et n'echoue jamais.
    fn resume(&self, _session_id: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    /// N'enregistre rien.
    fn wake(&self, _session_id: &str) {}

    /// N'interrompt rien.
    fn interrupt(&self, _session_id: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_service_vide_ne_declare_aucune_session_active() {
        let service = Noop;
        let actives = service.active();
        assert!(actives.is_empty());
        assert_eq!(actives.len(), 0);
    }

    #[test]
    fn le_service_vide_accepte_resume_wake_et_interrupt_sans_erreur() {
        let service = Noop;
        assert!(service.resume("ses_01").is_ok());
        service.wake("ses_01");
        service.interrupt("ses_01");
        // Aucune de ces operations ne doit avoir rendu la session active.
        assert!(service.active().is_empty());
    }

    #[test]
    fn un_identifiant_vide_est_accepte_sans_effet_de_bord() {
        let service = Noop;
        assert!(service.resume("").is_ok());
        service.wake("");
        service.interrupt("");
        assert!(service.active().is_empty());
    }

    #[test]
    fn les_identifiants_actifs_sont_lonnes_par_ordre_croissant() {
        // Le BTreeSet impose un ordre deterministe : l'insertion dans le desordre
        // ne change pas l'ordre de restitution.
        let mut actives = BTreeSet::new();
        actives.insert("ses_03".to_string());
        actives.insert("ses_01".to_string());
        actives.insert("ses_02".to_string());
        let ordre: Vec<&str> = actives.iter().map(|s| s.as_str()).collect();
        assert_eq!(ordre, vec!["ses_01", "ses_02", "ses_03"]);
    }

    #[test]
    fn un_seul_identifiant_actif_est_restitue_tel_quel() {
        let mut actives = BTreeSet::new();
        actives.insert("ses_01".to_string());
        assert_eq!(actives.len(), 1);
        assert!(actives.contains("ses_01"));
    }

    #[test]
    fn le_service_vide_est_egal_a_lui_meme_par_defaut() {
        assert_eq!(Noop, Noop::default());
        assert_eq!(Noop::default().active().len(), 0);
    }
}
