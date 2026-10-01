//! Port de `core/src/plugin/host.ts` (openCode).
//!
//! Le fichier TS est de la **logique pure** : `PluginHost.make` construit le
//! contexte (`PluginContext` v2 « effect ») remis aux plugins, en reliant les
//! services de l'application (agents, aisdk, catalogue, commandes, intégration,
//! plugins, références, compétences) à des rappels de transformation/hook.
//! Aucune donnée sérialisable n'y figure, donc aucun contrat serde/JSON :
//! ce module ne définit volontairement aucune structure `Serialize`.
//!
//! Comme pour `plugin_promise.rs`, l'hôte « Effect » du TS est modélisé par des
//! traits asynchrones ; le `mutable()` TS (marquage `DeepMutable` à la
//! compilation) disparaît en Rust où la mutabilité est structurelle.

use std::any::Any;
use std::sync::{Arc, Mutex};

use super::plugin_promise::{BoiteFutur, FuturPossede, RappelEvenement, RappelRedaction};

/// Domaine transformable + rechargeable côté hôte (agent, catalogue, commande,
/// référence, compétence). Miroir des services `*.transform` / `*.reload` du TS.
pub trait Domaine: Send + Sync {
    fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, ()>;
    fn recharge(&self) -> BoiteFutur<'_, ()>;
}

/// Sous-trait `agent` du contexte : recharge + transformation restreinte.
/// Le TS fabrique un `draft` avec `list/get/default/update/remove` sur des
/// identifiants typés ; ici le brouillon reste effacé (`dyn Any`).
pub trait SousDomaineAgent: Send + Sync {
    fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, ()>;
    fn recharge(&self) -> BoiteFutur<'_, ()>;
}

/// Hook `aisdk.sdk` : reçoit un événement (model, package, options, sdk) dont
/// le champ `sdk` est réécrit avec la valeur éventuellement mutée par le rappel.
pub trait HookAisdk: Send + Sync {
    fn sdk<'a>(&'a self, rappel: RappelEvenement) -> BoiteFutur<'a, ()>;
    /// Hook `aisdk.language` : même mécanisme, le champ réécrit est `language`.
    fn language<'a>(&'a self, rappel: RappelEvenement) -> BoiteFutur<'a, ()>;
}

/// Sous-trait `integration.connection` du contexte.
pub trait ConnexionIntegration: Send + Sync {
    fn active<'a>(&'a self, id: &'a str) -> BoiteFutur<'a, ()>;
    /// Résout une connexion : dans le TS, un type `"credential"` voit son `id`
    /// converti en identifiant typé `Credential.ID` ; ici une simple validation.
    fn resout<'a>(&'a self, connexion: &'a (dyn Any + Send + Sync)) -> BoiteFutur<'a, ()>;
}

/// Sous-trait `integration.method.update` du TS, discriminé comme dans
/// l'original : `authorize` (OAuth), `env` (noms de variables), `key` (étiquette).
pub enum MiseAJourMethode {
    /// Branche `"authorize" in input` : méthode OAuth avec rappels autorisation/rafraîchissement.
    Autorize {
        id_integration: String,
        id_methode: String,
        /// Optionnel (spread conditionnel `refresh ?` dans le TS).
        rafraichit: Option<Arc<dyn Fn(&str) -> FuturPossede<()> + Send + Sync>>,
        /// Optionnel (spread conditionnel `input.label ?` dans le TS).
        etiquette: Option<String>,
    },
    /// Branche `input.method.type === "env"`.
    Env {
        id_integration: String,
        noms: Vec<String>,
    },
    /// Branche par défaut : méthode de type `key`.
    Clef {
        id_integration: String,
        etiquette: String,
    },
}

/// Sous-trait `integration.method` du contexte.
pub trait MethodesIntegration: Send + Sync {
    fn liste<'a>(&'a self, id: &'a str) -> BoiteFutur<'a, ()>;
    fn met_a_jour<'a>(&'a self, mise_a_jour: &'a MiseAJourMethode) -> BoiteFutur<'a, ()>;
    fn supprime<'a>(&'a self, id: &'a str, methode: &'a (dyn Any + Send + Sync)) -> BoiteFutur<'a, ()>;
}

/// Sous-trait `plugin` du contexte : ajout/retrait dynamique de plugins.
pub trait GestionPluginsHote: Send + Sync {
    fn ajoute<'a>(&'a self, id: &'a str, effet: Arc<dyn Fn() -> FuturPossede<()> + Send + Sync>)
        -> BoiteFutur<'a, ()>;
    fn supprime<'a>(&'a self, id: &'a str) -> BoiteFutur<'a, ()>;
}

/// Miroir du contexte rendu par `PluginHost.make` (`satisfies Interface`).
/// Chaque champ expose la même surface que l'objet TS.
pub struct HotePlugin {
    /// `options: {}` dans le TS — objet vide, conservé effacé.
    pub options: Arc<dyn Any + Send + Sync>,
    pub agent: Arc<dyn SousDomaineAgent>,
    pub aisdk: Arc<dyn HookAisdk>,
    pub catalogue: Arc<dyn Domaine>,
    pub commande: Arc<dyn Domaine>,
    pub integration: Arc<dyn IntegrationHote>,
    pub plugin: Arc<dyn GestionPluginsHote>,
    pub reference: Arc<dyn Domaine>,
    pub competence: Arc<dyn Domaine>,
}

/// Vue `integration` du contexte (connection + transform + reload).
pub trait IntegrationHote: Send + Sync {
    fn recharge(&self) -> BoiteFutur<'_, ()>;
    fn connexion(&self) -> Arc<dyn ConnexionIntegration>;
    fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, ()>;
    fn methodes(&self) -> Arc<dyn MethodesIntegration>;
}

/// Construit le contexte de l'hôte à partir des services, exactement comme
/// `PluginHost.make` : chaque domaine est relié à son rappel de transformation,
/// les hooks aisdk réécrivent le champ muté, le sous-domaine agent expose la
/// vue restreinte du TS.
pub fn fabrique(
    agents: Arc<dyn SousDomaineAgent>,
    aisdk: Arc<dyn HookAisdk>,
    catalogue: Arc<dyn Domaine>,
    commandes: Arc<dyn Domaine>,
    integration: Arc<dyn IntegrationHote>,
    plugin: Arc<dyn GestionPluginsHote>,
    reference: Arc<dyn Domaine>,
    competence: Arc<dyn Domaine>,
) -> HotePlugin {
    HotePlugin {
        options: Arc::new(()),
        agent: agents,
        aisdk,
        catalogue,
        commande: commandes,
        integration,
        plugin,
        reference,
        competence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Domaine factice enregistrant les rappels et les recharges.
    #[derive(Default)]
    struct DomaineFactice {
        rappels: Mutex<Vec<usize>>,
        /// `Arc` parce que `FuturPossede` est `'static` : un futur ne peut pas
        /// retenir `&self.recharges`. On capture donc un clone de l'`Arc`, ce qui
        /// laisse la recharge dans le futur au lieu de la faire d avance.
        recharges: Arc<Mutex<Vec<()>>>,
    }

    impl Domaine for DomaineFactice {
        fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, ()> {
            Box::pin(async move {
                // Exécute immédiatement comme le ferait le draft côté hôte.
                rappel(&mut 41u32).await;
                self.rappels.lock().unwrap().push(1);
            })
        }
        fn recharge(&self) -> BoiteFutur<'_, ()> {
            let etat = Arc::clone(&self.recharges);
            Box::pin(async move {
                etat.lock().unwrap().push(());
            })
        }
    }

    fn domaine() -> Arc<DomaineFactice> {
        Arc::new(DomaineFactice::default())
    }

    /// Sous-domaine agent : même comportement, type distinct pour la preuve.
    struct AgentFactice(Arc<DomaineFactice>);

    impl SousDomaineAgent for AgentFactice {
        fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, ()> {
            self.0.transforme(rappel)
        }
        fn recharge(&self) -> BoiteFutur<'_, ()> {
            self.0.recharge()
        }
    }

    /// Hooks aisdk : mémorise les rappels et simule la réécriture du champ.
    struct AisdkFactice {
        sdk: Mutex<Vec<usize>>,
        language: Mutex<Vec<usize>>,
    }

    impl HookAisdk for AisdkFactice {
        fn sdk<'a>(&'a self, rappel: RappelEvenement) -> BoiteFutur<'a, ()> {
            Box::pin(async move {
                rappel(&() as &dyn Any).await;
                self.sdk.lock().unwrap().push(1);
            })
        }
        fn language<'a>(&'a self, rappel: RappelEvenement) -> BoiteFutur<'a, ()> {
            Box::pin(async move {
                rappel(&() as &dyn Any).await;
                self.language.lock().unwrap().push(1);
            })
        }
    }

    /// Intégration factice journalisant les méthodes mises à jour.
    #[derive(Default)]
    struct IntegrationFactice {
        maj: Mutex<Vec<&'static str>>,
    }

    impl MethodesIntegration for IntegrationFactice {
        fn liste<'a>(&'a self, _: &'a str) -> BoiteFutur<'a, ()> {
            Box::pin(async {})
        }
        fn met_a_jour<'a>(&'a self, maj: &'a MiseAJourMethode) -> BoiteFutur<'a, ()> {
            let branche = match maj {
                MiseAJourMethode::Autorize { .. } => "authorize",
                MiseAJourMethode::Env { .. } => "env",
                MiseAJourMethode::Clef { .. } => "key",
            };
            Box::pin(async move {
                self.maj.lock().unwrap().push(branche);
            })
        }
        fn supprime<'a>(&'a self, _: &'a str, _: &'a (dyn Any + Send + Sync)) -> BoiteFutur<'a, ()> {
            Box::pin(async {})
        }
    }

    impl IntegrationHote for IntegrationFactice {
        fn recharge(&self) -> BoiteFutur<'_, ()> {
            Box::pin(async {})
        }
        fn connexion(&self) -> Arc<dyn ConnexionIntegration> {
            unimplemented!("non exercé par ces tests")
        }
        fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, ()> {
            Box::pin(async move {
                rappel(&mut ()).await;
            })
        }
        fn methodes(&self) -> Arc<dyn MethodesIntegration> {
            Arc::new(IntegrationFactice::default())
        }
    }

    #[tokio::test]
    async fn relie_les_domaines_au_contexte_comme_make() {
        let agents = domaine();
        let competence = domaine();
        let catalogue = domaine();
        let hote = fabrique(
            Arc::new(AgentFactice(agents.clone())),
            Arc::new(AisdkFactice {
                sdk: Mutex::new(Vec::new()),
                language: Mutex::new(Vec::new()),
            }),
            catalogue.clone(),
            domaine(),
            Arc::new(IntegrationFactice::default()),
            Arc::new(GestionFactice::default()),
            domaine(),
            competence.clone(),
        );

        // Transformation de l'agent : le brouillon numérique est passé au rappel.
        let miroir = Arc::new(Mutex::new(0u32));
        let capteur = miroir.clone();
        let rappel: RappelRedaction = Arc::new(move |brouillon: &mut dyn Any| {
            let capteur = capteur.clone();
            // `&mut dyn Any` n'est pas `Send`, et le futur le retenait, d'où
            // « future cannot be sent between threads safely ». Le rappel fait une
            // seule écriture sans suspension : elle a lieu AVANT la construction
            // du futur, qui reste donc vide et `Send`. L appelant attend aussitot,
            // donc le comportement observable ne change pas.
            if let Some(valeur) = brouillon.downcast_mut::<u32>() {
                *capteur.lock().unwrap() = *valeur;
            }
            Box::pin(async {}) as FuturPossede<()>
        });
        hote.agent.transforme(rappel).await;

        assert_eq!(*miroir.lock().unwrap(), 41);
        assert_eq!(agents.rappels.lock().unwrap().len(), 1);
        hote.competence.recharge().await;
        assert_eq!(competence.recharges.lock().unwrap().len(), 1);
        hote.catalogue.recharge().await;
        assert_eq!(catalogue.recharges.lock().unwrap().len(), 1);
    }

    /// Gestion de plugins factice journalisant les identifiants.
    #[derive(Default)]
    struct GestionFactice {
        ajoutes: Mutex<Vec<String>>,
        supprimes: Mutex<Vec<String>>,
    }

    impl GestionPluginsHote for GestionFactice {
        fn ajoute<'a>(
            &'a self,
            id: &'a str,
            effet: Arc<dyn Fn() -> FuturPossede<()> + Send + Sync>,
        ) -> BoiteFutur<'a, ()> {
            Box::pin(async move {
                effet().await;
                self.ajoutes.lock().unwrap().push(id.to_string());
            })
        }
        fn supprime<'a>(&'a self, id: &'a str) -> BoiteFutur<'a, ()> {
            let id = id.to_string();
            Box::pin(async move {
                self.supprimes.lock().unwrap().push(id);
            })
        }
    }

    #[tokio::test]
    async fn ajoute_et_supprime_un_sous_plugin() {
        let gestion = Arc::new(GestionFactice::default());
        let hote = fabrique(
            Arc::new(AgentFactice(domaine())),
            Arc::new(AisdkFactice {
                sdk: Mutex::new(Vec::new()),
                language: Mutex::new(Vec::new()),
            }),
            domaine(),
            domaine(),
            Arc::new(IntegrationFactice::default()),
            gestion.clone(),
            domaine(),
            domaine(),
        );

        hote.plugin
            .ajoute(
                "secondaire",
                Arc::new(|| Box::pin(async {}) as FuturPossede<()>),
            )
            .await;
        hote.plugin.supprime("secondaire").await;

        assert_eq!(*gestion.ajoutes.lock().unwrap(), vec!["secondaire".to_string()]);
        assert_eq!(*gestion.supprimes.lock().unwrap(), vec!["secondaire".to_string()]);
    }

    #[tokio::test]
    async fn dispatche_les_mises_a_jour_de_methode_comme_le_ts() {
        let methodes = Arc::new(IntegrationFactice::default());
        let maj = |branche: MiseAJourMethode| {
            let methodes = methodes.clone();
            async move { methodes.met_a_jour(&branche).await }
        };

        maj(MiseAJourMethode::Autorize {
            id_integration: "i1".into(),
            id_methode: "oauth".into(),
            rafraichit: None,
            etiquette: Some("Pro".into()),
        })
        .await;
        maj(MiseAJourMethode::Env {
            id_integration: "i1".into(),
            noms: vec!["CLEF".into()],
        })
        .await;
        maj(MiseAJourMethode::Clef {
            id_integration: "i1".into(),
            etiquette: "api".into(),
        })
        .await;

        assert_eq!(
            *methodes.maj.lock().unwrap(),
            vec!["authorize", "env", "key"]
        );
    }
}
