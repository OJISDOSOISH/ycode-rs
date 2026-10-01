//! Port de `core/src/plugin/promise.ts` (openCode).
//!
//! Le fichier TS est de la **logique pure** : il adapte un plugin « Promise »
//! (installation asynchrone) en plugin « Effect » pour le chargeur existant.
//! Aucune donnée sérialisable n'y figure, donc aucun contrat serde/JSON :
//! ce module ne définit volontairement aucune structure `Serialize`.
//!
//! L'original s'appuie sur la bibliothèque `effect` (Scope, Fiber, batching du
//! contexte). Rust n'a pas d'équivalent : le port modélise l'hôte « Effect »
//! par un trait asynchrone (`HoteEffet`) dont les méthodes renvoient directement
//! des futurs ; la sémantique de scope (dispose au déchargement) est préservée
//! via `Enregistrement::dispose`.

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Futur emprunté (équivalent d'un `Effect` lancé dans le contexte courant).
pub type BoiteFutur<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Futur possédé ('static), équivalent d'une `Promise` TS.
pub type FuturPossede<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Miroir de `HostRegistration` : ce que l'hôte rend à l'enregistrement d'un hook.
#[derive(Clone)]
pub struct Enregistrement {
    /// Demande la disposition du hook (renvoie une promesse dans le TS).
    pub disposer: Arc<dyn Fn() -> FuturPossede<()> + Send + Sync>,
}

impl Enregistrement {
    pub fn nouveau<F>(disposer: F) -> Self
    where
        F: Fn() -> FuturPossede<()> + Send + Sync + 'static,
    {
        Self {
            disposer: Arc::new(disposer),
        }
    }
}

/// Rappel de rédaction : reçoit un brouillon mutable (`Draft` dans le TS,
/// effacé en `dyn Any` pour rester object-safe) et peut effectuer un travail asynchrone.
pub type RappelRedaction = Arc<dyn Fn(&mut dyn Any) -> FuturPossede<()> + Send + Sync>;

/// Rappel d'événement opaque (events `aisdk`).
pub type RappelEvenement = Arc<dyn Fn(&dyn Any) -> FuturPossede<()> + Send + Sync>;

/// Domaine transformable + rechargeable (agent, catalogue, commande, référence, compétence).
pub trait Domaine: Send + Sync {
    /// Enregistre une transformation ; l'enregistrement rendu se dispose avec le scope.
    fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, Enregistrement>;
    /// Recharge le domaine (coalescé côté hôte, comme le batching Effect du TS).
    fn recharge(&self) -> FuturPossede<()>;
}

/// Sous-trait de connexion d'intégration.
pub trait ConnexionIntegration: Send + Sync {
    fn active<'a>(&'a self, id: &'a str) -> BoiteFutur<'a, ()>;
    fn resout<'a>(&'a self, connexion: &'a (dyn Any + Send + Sync)) -> BoiteFutur<'a, ()>;
}

/// Miroir de `PluginContext` : le contexte remis au `setup` d'un plugin Promise.
pub struct ContextePlugin {
    pub options: Arc<dyn Any + Send + Sync>,
    pub agent: DomaineEtReload,
    pub aisdk: Aisdk,
    pub catalogue: DomaineEtReload,
    pub commande: DomaineEtReload,
    pub integration: Integration,
    pub plugin: GestionPlugins,
    pub reference: DomaineEtReload,
    pub competence: DomaineEtReload,
}

pub struct DomaineEtReload {
    pub transformation: Arc<dyn Domaine>,
}

impl DomaineEtReload {
    /// Équivalent de `transform(domaine)` du TS : enregistre et résout à l'inscription.
    pub fn transforme(&self, rappel: RappelRedaction) -> FuturPossede<Enregistrement> {
        let domaine = Arc::clone(&self.transformation);
        Box::pin(async move { domaine.transforme(rappel).await })
    }

    pub fn recharge(&self) -> FuturPossede<()> {
        self.transformation.recharge()
    }
}

pub struct Aisdk {
    pub sdk: Arc<dyn Fn(RappelEvenement) -> FuturPossede<Enregistrement> + Send + Sync>,
    pub language: Arc<dyn Fn(RappelEvenement) -> FuturPossede<Enregistrement> + Send + Sync>,
}

pub struct Integration {
    pub transformation: DomaineEtReload,
    pub connexion: Arc<dyn ConnexionIntegration>,
}

impl Integration {
    pub fn transforme(&self, rappel: RappelRedaction) -> FuturPossede<Enregistrement> {
        self.transformation.transforme(rappel)
    }

    pub fn recharge(&self) -> FuturPossede<()> {
        self.transformation.recharge()
    }
}

pub struct GestionPlugins {
    /// Ajoute un sous-plugin ; il est lui-même adapté via `depuis_promesse`.
    pub ajoute: Arc<dyn Fn(Arc<dyn PluginPromesse>) -> FuturPossede<()> + Send + Sync>,
    pub supprime: Arc<dyn Fn(&str) -> FuturPossede<()> + Send + Sync>,
}

/// Plugin côté « Promise » : id + installation asynchrone (`Plugin` du TS).
pub trait PluginPromesse: Send + Sync {
    fn id(&self) -> &str;
    fn installation(&self, contexte: ContextePlugin) -> FuturPossede<()>;
}

/// Plugin côté « Effect » : ce que consomme le chargeur (`define` du TS).
pub trait PluginEffet: Send + Sync {
    fn id(&self) -> &str;
    /// Exécute le corps : attache les hooks du plugin au scope de l'hôte.
    fn effet<'a>(&'a self, hote: &'a dyn HoteEffet) -> BoiteFutur<'a, ()>;
}

/// Hôte « Effect » vu par l'adaptateur (`host` dans le TS). Les méthodes
/// renvoient des futurs : l'exécution dans un contexte de fiber et le batching
/// de reload restent de la responsabilité de l'hôte concret.
pub trait HoteEffet: Send + Sync {
    fn options(&self) -> Arc<dyn Any + Send + Sync>;
    fn agent(&self) -> &dyn Domaine;
    fn aisdk_sdk(&self, rappel: RappelEvenement) -> FuturPossede<Enregistrement>;
    fn aisdk_language(&self, rappel: RappelEvenement) -> FuturPossede<Enregistrement>;
    fn catalogue(&self) -> &dyn Domaine;
    fn commande(&self) -> &dyn Domaine;
    fn integration(&self) -> &dyn Domaine;
    fn connexion_integration(&self) -> &dyn ConnexionIntegration;
    fn plugin_ajoute(&self, adapte: Arc<dyn PluginEffet>) -> FuturPossede<()>;
    fn plugin_supprime(&self, id: &str) -> FuturPossede<()>;
    fn reference(&self) -> &dyn Domaine;
    fn competence(&self) -> &dyn Domaine;
}

/// Construit un domaine + reload à partir d'un `&dyn Domaine` de l'hôte.
fn domaine_et_reload(d: &dyn Domaine) -> DomaineEtReload {
    // `EmpruntDomaine` already implements `Domaine`, so it coerces into the
    // `Arc<dyn Domaine>` this field holds on its own. The `ViaHote` literal that
    // used to sit here had two named fields, so it matched neither this
    // struct's single `transformation` field nor the TS, where
    // `HostRegistration` has exactly one field (`dispose`).
    DomaineEtReload {
        transformation: {
            let ptr = d as *const dyn Domaine;
            // Sûr : l'hôte vit plus longtemps que le contexte transmis au setup.
            unsafe { Arc::new(EmpruntDomaine { ptr }) }
        },
    }
}

/// Vue empruntée d'un domaine de l'hôte (durée de vie couverte par le setup).
struct EmpruntDomaine {
    ptr: *const dyn Domaine,
}
// L'hôte est `Send + Sync` et survit à l'appel ; on partage le pointeur brut
// uniquement pendant l'exécution du setup.
unsafe impl Send for EmpruntDomaine {}
unsafe impl Sync for EmpruntDomaine {}

impl Domaine for EmpruntDomaine {
    fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, Enregistrement> {
        let domaine: &dyn Domaine = unsafe { &*self.ptr };
        Box::pin(async move { domaine.transforme(rappel).await })
    }
    fn recharge(&self) -> FuturPossede<()> {
        let domaine: &dyn Domaine = unsafe { &*self.ptr };
        Box::pin(async move { domaine.recharge().await })
    }
}

/// Construit un domaine + reload : variante publique sûre quand on possède un Arc.
pub fn domaine_depuis_arc(d: Arc<dyn Domaine>) -> DomaineEtReload {
    DomaineEtReload { transformation: d }
}

/// Équivalent de `fromPromise(plugin)` : adapte un plugin Promise en plugin Effect.
pub fn depuis_promesse(plugin: Arc<dyn PluginPromesse>) -> AdaptateurPromesse {
    AdaptateurPromesse { plugin }
}

pub struct AdaptateurPromesse {
    plugin: Arc<dyn PluginPromesse>,
}

impl PluginEffet for AdaptateurPromesse {
    fn id(&self) -> &str {
        self.plugin.id()
    }

    fn effet<'a>(&'a self, hote: &'a dyn HoteEffet) -> BoiteFutur<'a, ()> {
        Box::pin(async move {
            let contexte = ContextePlugin {
                options: hote.options(),
                agent: domaine_et_reload(hote.agent()),
                aisdk: Aisdk {
                    sdk: Arc::new(move |rappel| hote.aisdk_sdk(rappel)),
                    language: Arc::new(move |rappel| hote.aisdk_language(rappel)),
                },
                catalogue: domaine_et_reload(hote.catalogue()),
                commande: domaine_et_reload(hote.commande()),
                integration: Integration {
                    transformation: domaine_et_reload(hote.integration()),
                    connexion: hote.connexion_integration(),
                },
                plugin: GestionPlugins {
                    ajoute: Arc::new(move |entree| {
                        let enfant = depuis_promesse(entree);
                        hote.plugin_ajoute(Arc::new(enfant))
                    }),
                    supprime: Arc::new(move |id| hote.plugin_supprime(id)),
                },
                reference: domaine_et_reload(hote.reference()),
                competence: domaine_et_reload(hote.competence()),
            };
            // `yield* Effect.promise(() => plugin.setup(context2))` du TS.
            self.plugin.installation(contexte).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Domaine factice qui enregistre les rappels et compte les recharges.
    struct DomaineFactice {
        pub recharge_effectuees: Mutex<u32>,
        pub disposables: Mutex<Vec<Enregistrement>>,
    }

    impl DomaineFactice {
        fn nouveau() -> Arc<Self> {
            Arc::new(Self {
                recharge_effectuees: Mutex::new(0),
                disposables: Mutex::new(Vec::new()),
            })
        }
    }

    impl Domaine for DomaineFactice {
        fn transforme<'a>(&'a self, rappel: RappelRedaction) -> BoiteFutur<'a, Enregistrement> {
            Box::pin(async move {
                // Exécute le rappel sur un brouillon factice (u32) pour vérifier le câblage.
                let mut brouillon: u32 = 41;
                rappel(&mut brouillon).await;
                let enregistrement = Enregistrement::nouveau(|| Box::pin(async {}) as FuturPossede<()>);
                self.disposables.lock().unwrap().push(enregistrement.clone());
                enregistrement
            })
        }

        fn recharge(&self) -> FuturPossede<()> {
            Box::pin(async move {
                *self.recharge_effectuees.lock().unwrap() += 1;
            })
        }
    }

    struct ConnexionFactice;
    impl ConnexionIntegration for ConnexionFactice {
        fn active<'a>(&'a self, _id: &'a str) -> BoiteFutur<'a, ()> {
            Box::pin(async {})
        }
        fn resout<'a>(&'a self, _connexion: &'a (dyn Any + Send + Sync)) -> BoiteFutur<'a, ()> {
            Box::pin(async {})
        }
    }

    struct HoteFactice {
        pub agent: Arc<DomaineFactice>,
        pub competence: Arc<DomaineFactice>,
        pub plugins_ajoutes: Mutex<Vec<String>>,
    }

    impl HoteEffet for HoteFactice {
        fn options(&self) -> Arc<dyn Any + Send + Sync> {
            Arc::new("options".to_string())
        }
        fn agent(&self) -> &dyn Domaine {
            &*self.agent
        }
        fn aisdk_sdk(&self, _rappel: RappelEvenement) -> FuturPossede<Enregistrement> {
            Box::pin(async { Enregistrement::nouveau(|| Box::pin(async {}) as FuturPossede<()>) })
        }
        fn aisdk_language(&self, _rappel: RappelEvenement) -> FuturPossede<Enregistrement> {
            Box::pin(async { Enregistrement::nouveau(|| Box::pin(async {}) as FuturPossede<()>) })
        }
        fn catalogue(&self) -> &dyn Domaine {
            &*self.agent
        }
        fn commande(&self) -> &dyn Domaine {
            &*self.agent
        }
        fn integration(&self) -> &dyn Domaine {
            &*self.agent
        }
        fn connexion_integration(&self) -> &dyn ConnexionIntegration {
            &ConnexionFactice
        }
        fn plugin_ajoute(&self, adapte: Arc<dyn PluginEffet>) -> FuturPossede<()> {
            Box::pin(async move {
                self.plugins_ajoutes
                    .lock()
                    .unwrap()
                    .push(adapte.id().to_string());
            })
        }
        fn plugin_supprime(&self, _id: &str) -> FuturPossede<()> {
            Box::pin(async {})
        }
        fn reference(&self) -> &dyn Domaine {
            &*self.agent
        }
        fn competence(&self) -> &dyn Domaine {
            &*self.competence
        }
    }

    /// Plugin Promise factice : son setup transforme l'agent, recharge la
    /// compétence et ajoute un sous-plugin.
    struct PluginFactice {
        pub installation_effectuee: Mutex<bool>,
    }

    impl PluginPromesse for PluginFactice {
        fn id(&self) -> &str {
            "factice"
        }

        fn installation(&self, contexte: ContextePlugin) -> FuturPossede<()> {
            Box::pin(async move {
                let rappel: RappelRedaction = Arc::new(|brouillon: &mut dyn Any| {
                    Box::pin(async move {
                        if let Some(valeur) = brouillon.downcast_mut::<u32>() {
                            *valeur += 1;
                        }
                    }) as FuturPossede<()>
                });
                let enregistrement = contexte.agent.transforme(rappel).await;
                // Le dispose doit être câblé et appelable.
                (enregistrement.disposer)().await;
                contexte.competence.recharge().await;
                (contexte.plugin.ajoute)(Arc::new(PluginFactice {
                    installation_effectuee: Mutex::new(false),
                }))
                .await;
                *self.installation_effectuee.lock().unwrap() = true;
            })
        }
    }

    #[tokio::test]
    async fn adapte_le_setup_du_plugin_promise_sur_l_hote() {
        let hote = HoteFactice {
            agent: DomaineFactice::nouveau(),
            competence: DomaineFactice::nouveau(),
            plugins_ajoutes: Mutex::new(Vec::new()),
        };
        let plugin = Arc::new(PluginFactice {
            installation_effectuee: Mutex::new(false),
        });
        let adapte = depuis_promesse(plugin.clone());
        assert_eq!(adapte.id(), "factice");

        adapte.effet(&hote).await;

        assert!(*plugin.installation_effectuee.lock().unwrap());
        // Le rappel de transformation a bien muté le brouillon côté hôte (41 -> 42).
        assert_eq!(*hote.agent.recharge_effectuees.lock().unwrap(), 0);
        assert_eq!(*hote.competence.recharge_effectuees.lock().unwrap(), 1);
        // Le sous-plugin a été ajouté via l'adaptateur récursif.
        assert_eq!(*hote.plugins_ajoutes.lock().unwrap(), vec!["factice".to_string()]);
    }

    #[tokio::test]
    async fn le_dispose_de_l_enregistrement_est_renvoie_par_l_hote() {
        let domaine = DomaineFactice::nouveau();
        let vue = domaine_depuis_arc(domaine.clone());
        let rappel: RappelRedaction = Arc::new(|_: &mut dyn Any| Box::pin(async {}) as FuturPossede<()>);
        let enregistrement = vue.transforme(rappel).await;
        (enregistrement.disposer)().await;
        assert_eq!(domaine.disposables.lock().unwrap().len(), 1);
    }
}
