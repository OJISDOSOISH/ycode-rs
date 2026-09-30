//! Portage de `packages/core/src/effect/runtime.ts`.
//!
//! ## Ce que la source contient
//!
//! Vingt et une lignes, une seule fonction exportee, `makeRuntime`, qui tient
//! dans un descripteur :
//!
//! ```ts
//! export function makeRuntime<I, S, E>(service: Context.Service<I, S>, layer: Layer.Layer<I, E>) {
//!   let rt: ManagedRuntime.ManagedRuntime<I, E> | undefined
//!   const getRuntime = () =>
//!     (rt ??= ManagedRuntime.make(Layer.provideMerge(layer, Observability.layer) as Layer.Layer<I, E>, {
//!       memoMap,
//!     }))
//!
//!   return {
//!     runSync: <A, Err>(fn) => getRuntime().runSync(service.use(fn)),
//!     runPromiseExit: <A, Err>(fn, options?) => getRuntime().runPromiseExit(service.use(fn), options),
//!     runPromise: <A, Err>(fn, options?) => getRuntime().runPromise(service.use(fn), options),
//!     runFork: <A, Err>(fn) => getRuntime().runFork(service.use(fn)),
//!     runCallback: <A, Err>(fn) => getRuntime().runCallback(service.use(fn)),
//!   }
//! }
//! ```
//!
//! Il n y a **aucune donnee** dans ce fichier : ni configuration, ni table, ni
//! calcul. Il y a une **composition**, et c est tout ce qui est reporte.
//!
//! ## Les trois faits que ce portage porte
//!
//! 1. **La construction est differee et unique.** `rt` vaut `undefined` tant
//!    qu aucun point d entree n a ete appele, et `rt ??=` ne le construit qu une
//!    fois. Les cinq appels suivants reutilisent le meme objet.
//! 2. **La couche de l observabilite est toujours injectee.**
//!    `Layer.provideMerge(layer, Observability.layer)` n est jamais conditionnel :
//!    la couche de l application est toujours fusionnee avec celle du noeud
//!    voisin `observability`, dont le nom vient de `observability::NODE_NAME`.
//! 3. **La table de memorisation est partagee par tout le processus.** Le
//!    `memoMap` passe a `ManagedRuntime.make` est celui exporte par
//!    `effect/memo-map.ts`, donc un singleton, et tous les runtimes construits
//!    par `makeRuntime` s en servent.
//!
//! Ces trois faits sont ici des **donnees** ([`Composition`], [`SharedMemoMap`],
//! [`Etat`]) et non des mecanismes, parce que le mecanisme n existe pas en Rust.
//!
//! ## Ce qui n a pas de traduction, et qui n est pas simule
//!
//! Conformement a la consigne du lot, ni le runtime d Effect, ni le graphe de
//! couches, ni la concurrence ne sont reproduits. Concretement :
//!
//! - **`Effect.Effect<A, Err, I>` n a pas de type.** Les deux methodes
//!   generiques `runSync<A, Err>` et `makeRuntime<I, S, E>` ne produisent que
//!   des annotations de type, effacees a l execution. Elles sont conservees
//!   dans [`Run`] sous forme de `PhantomData`, sans quoi la signature serait
//!   encore plus fausse que la source. Les six parametres de type au total
//!   (`I`, `S`, `E`, `A`, `Err`, et le `I` interne de `Effect`) disparaissent
//!   donc en entier : ce qui survit est un [`ServiceKey`] et un nom de couche.
//! - **La fonction `fn` n est pas conservee.** Elle est de type
//!   `(svc: S) => Effect.Effect<A, Err, I>` : sa valeur de retour n existe pas
//!   ici, donc aucune fonction de ce type ne peut etre stockee, ni meme
//!   seulement designee. Ce que les cinq methodes rendent, [`Run`], est le
//!   **plan** de l appel : le point d entree, le service a fournir, les options.
//!   Elles n executent rien. Retirer le parametre `fn` est un retrait
//!   documente, pas un oubli.
//! - **`service.use(fn)` n est pas un simple appel.** Il extrait `S` du contexte
//!   de services du runtime, contexte produit par la construction du graphe.
//!   Ce que le plan retient est donc la CLE du service, pas sa valeur.
//! - **Aucune promesse, aucune fibre, aucun rappel, aucun thread.** `runPromise`
//!   et `runPromiseExit` rendraient une `Promise`, `runFork` une `Fiber`
//!   detachee, `runCallback` appellerait une fonction. Reproduire cela
//!   demanderait des `Future`, des canaux et des ordonnanceurs : c est
//!   explicitement exclu par ce lot, et le serait aussi par la consigne du
//!   projet. Ce qui est reporte est le **type de retour** de chaque point
//!   d entree, dans [`Rend`].
//! - **Le `as Layer.Layer<I, E>` n est pas reporte parce qu il est un mensonge
//!   de la source.** La fusion ajoute a la sortie des services qui ne font pas
//!   partie de `I` (ceux de l observabilite), et le `as` evite au
//!   verificateur de types de le remarquer. Le portage garde le fait, via
//!   [`Composition`], sans reproduire le faux typage.
//!
//! ## Concurrence : rien a traduire, et c est volontaire
//!
//! Le `let rt` de la source **n est pas synchronise** : JavaScript est mono
//! fil, et `??=` n est pas atomique dans un modele multi fil. Aucun `Mutex`,
//! aucune `Arc`, aucun `OnceLock` n est donc ajoute ici, pas plus que dans
//! [`effect_memo_map`](crate::swarm::effect_memo_map) qui a traite le fichier
//! voisin. Les methodes prennent `&mut self` : c est l emprise exclusive de
//! l appelant qui remplace la file d attente implicite du moteur
//! JavaScript, et c est une restriction de surface, pas une synchronisation.
//!
//! Consequence a garder en tete : deux points d entree appeles « en meme temps »
//! sur le meme runtime n ont pas de sens ici, alors qu ils en ont un en
//! JavaScript. Deux runtimes distincts, en revanche, sont normalement
//! independants, comme dans la source.
//!
//! ## Le `??=` a un second effet, et il est facile a rater
//!
//! `rt ??= ManagedRuntime.make(...)` n attribue `rt` que si `make` **reussit**.
//! Si la construction du graphe leve, `rt` reste `undefined`, et l appel
//! SUIVANT reconstruit. Il n y a donc pas d etat « construction en echec »
//! distinct : il y a un compteur de tentatives qui avance, et un compteur de
//! constructions reussies qui non. C est ce couple de compteurs, et non un
//! booleen, que [`Runtime::construire`] manipule, et c est le seul endroit du
//! fichier ou le succes ou l echec de `ManagedRuntime.make` est decide. Le
//! portage ne choisit pas ce succes : l appelant le fournit, parce que la
//! source, elle, ne le demande nulle part.
//!
//! ## Pas de `serde`
//!
//! Contrairement a `observability.rs` et a ses voisins, ce fichier ne derive
//! aucun `Serialize` / `Deserialize` : la source ne construit **aucun objet a
//! cles nommees**. Elle nomme des methodes, des parametres et un type importe
//! (`Effect.RunOptions`), rien d serialisable. Il n y a donc aucun nom de champ
//! a figer, et aucun `#[serde(rename)]` a ecrire. [RunOptions] est cite dans
//! la signature des deux points d entree asynchrones, et rien de plus.
//!
//! ## Ce qui n est pas ici, et pourquoi
//!
//! - Le contenu du graphe de couches : les couches fournies, leur ordre de
//!   fermeture et la construction effective sont dans la bibliotheque
//!   `effect` et dans les autres noeuds de l arborescence.
//! - Le `memoMap` lui-meme, avec sa cle double (couche, portee) : il est deja
//!   porte par [`effect_memo_map`](crate::swarm::effect_memo_map). Ce fichier
//!   ne fait que le REFERENCES, via [`SharedMemoMap`].
//! - Les options du `ManagedRuntime` autres que le `memoMap` : la source n en
//!   passe aucune.
//!
//! Note d integration : ce module doit etre declare dans `src/swarm/mod.rs`.
//! Le fichier ne le fait pas, la regle du lot interdisant de toucher a `mod.rs`.

use std::marker::PhantomData;

use crate::swarm::effect_memo_map::LayerKey;
use crate::swarm::observability::NODE_NAME;

/// La table de memorisation `memoMap` de `effect/memo-map.ts`.
///
/// Un singleton de tout le processus, passe a `ManagedRuntime.make` par la
/// source. Ce type n en porte **pas** la table : il n en porte que la
/// reference, c est a dire l information « tous les runtimes utilisent LA MEME
/// table ». La table elle-meme est dans
/// [`effect_memo_map`](crate::swarm::effect_memo_map).
///
/// C est la limite assumee de ce fichier sur la composition : deux runtimes
/// partagent la table dans la source, et ici ils portent une valeur egale, ce
/// qui est la seule forme de partage verifiable sans ni `Arc`, ni `Mutex`, ni
/// `static`, c est a dire sans aucun mecanisme de concurrence. Pour la meme
/// raison, une table mutatee a travers cette reference n est pas observable
/// depuis ce module, et le module ne pretend pas davantage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedMemoMap;

/// La table de memorisation, en tant que constante partagee.
pub const MEMO_MAP: SharedMemoMap = SharedMemoMap;

/// Le nom du service identifie dans un contexte, `Context.Service<I, S>`.
///
/// Dans la source, `I` est le TAG du service et `S` sa forme. Les deux sont
/// des types, donc absents a l execution. Ce qui reste et qui se lit dans le
/// code appelant est le NOM sous lequel le service est enregistre, et c est
/// ce que porte ce type. C est la seule partie du service qui demande un
/// vraic de la part de l appelant.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceKey(String);

impl ServiceKey {
    /// Construit une cle de service a partir de son nom.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Renvoie le nom du service.
    pub fn nom(&self) -> &str {
        &self.0
    }
}

/// L etat de la variable `rt` de la source.
///
/// Deux etats et pas trois : `rt` est soit `undefined`, soit un
/// `ManagedRuntime`. Une construction **en echec** n est pas un etat
/// distinct, parce que `??=` ne modifie pas `rt` quand `make` leve : on
/// reste dans [`Etat::NonConstruit`](Etat::NonConstruit) et l appel suivant
/// retente. Voir la section sur `??=` dans la doc du module.
///
/// L'implementation de `Default` est ecrite a la main, comme dans
/// `observability.rs` : l'attribut `#[default]` sur une variante d'enumeration
/// est plus recent que le reste du depot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Etat {
    /// `rt === undefined` : aucun point d'entree n a encore construit le
    /// runtime. C'est l'etat de sortie de `makeRuntime`.
    NonConstruit,
    /// `rt` contient le `ManagedRuntime` construit au premier appel.
    Construit,
}

impl Default for Etat {
    fn default() -> Self {
        Etat::NonConstruit
    }
}

impl Etat {
    /// `true` si le runtime existe.
    pub fn est_construit(self) -> bool {
        matches!(self, Etat::Construit)
    }
}

/// Les cinq points d entree, dans l ordre de declaration de l objet renvoye
/// par `makeRuntime`.
///
/// L'ordre n est pas neutre : c'est celui dans lequel le descripteur declare
/// `runSync`, `runPromiseExit`, `runPromise`, `runFork` puis `runCallback`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entry {
    /// `runSync` : execute l'effet et rend `A`, ou leve.
    RunSync,
    /// `runPromiseExit` : execute et rend une `Promise<Exit<A, Err>>`.
    RunPromiseExit,
    /// `runPromise` : execute et rend une `Promise<A>`, rejetee si l'effet
    /// echoue.
    RunPromise,
    /// `runFork` : execute et rend une `Fiber<A, Err>`, detachee.
    RunFork,
    /// `runCallback` : execute et rend `void`, l'appelant est prevenu par un
    /// rappel qui recoit l'`Exit`.
    RunCallback,
}

impl Entry {
    /// Les cinq points d entree, dans l'ordre de la source.
    pub const ALL: [Entry; 5] = [
        Entry::RunSync,
        Entry::RunPromiseExit,
        Entry::RunPromise,
        Entry::RunFork,
        Entry::RunCallback,
    ];

    /// Le nom de la methode dans la source, `runSync`, `runFork`, ...
    pub fn nom(self) -> &'static str {
        match self {
            Entry::RunSync => "runSync",
            Entry::RunPromiseExit => "runPromiseExit",
            Entry::RunPromise => "runPromise",
            Entry::RunFork => "runFork",
            Entry::RunCallback => "runCallback",
        }
    }

    /// Retrouve un point d entree a partir de son nom de methode.
    pub fn depuis_nom(nom: &str) -> Option<Entry> {
        Entry::ALL.into_iter().find(|entry| entry.nom() == nom)
    }

    /// `true` si la methode de la source accepte le second parametre
    /// `options`.
    ///
    /// Seules les deux methodes asynchrones le declarent :
    /// `runPromiseExit(fn, options?)` et `runPromise(fn, options?)`. Les trois
    /// autres ne l'ont pas, et n'en ont donc pas besoin.
    pub fn accepte_options(self) -> bool {
        matches!(self, Entry::RunPromiseExit | Entry::RunPromise)
    }

    /// Ce que la methode rend a l'appelant, d'apres la signature de
    /// `ManagedRuntime`.
    ///
    /// C'est la seule partie des cinq methodes qui soit reporte : le
    /// mecanisme qui produit ce rendu n'existe pas en Rust et n'est pas
    /// simule, donc ce type ne dit rien de ce qui se passe reellement, il
    /// dit ce que l'appelant recoit.
    pub fn rend(self) -> Rend {
        match self {
            Entry::RunSync => Rend::Valeur,
            Entry::RunPromiseExit => Rend::PromiseExit,
            Entry::RunPromise => Rend::Promise,
            Entry::RunFork => Rend::Fibre,
            Entry::RunCallback => Rend::Rappel,
        }
    }
}

/// Le type de retour d'un point d'entree, transcrit de la signature de
/// `ManagedRuntime`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rend {
    /// `A`, ou une levee : `runSync` ne rend pas d'issue, il echoue.
    Valeur,
    /// `Promise<Exit<A, Err>>` : l'issue est toujours dans la promesse, donc
    /// une erreur de la source n'est pas une promesse rejetee.
    PromiseExit,
    /// `Promise<A>` : rejetee si l'effet echoue.
    Promise,
    /// `Fiber<A, Err>` : l'effet est detache, l'appelant decide s'il attend.
    Fibre,
    /// `void` : le resultat passe par un rappel.
    Rappel,
}

impl Rend {
    /// Le type de retour ecrit comme dans la bibliotheque `effect`, avec les
    /// parametres de type generiques de la source.
    pub fn type_rend(self) -> &'static str {
        match self {
            Rend::Valeur => "A",
            Rend::PromiseExit => "Promise<Exit<A, Err>>",
            Rend::Promise => "Promise<A>",
            Rend::Fibre => "Fiber<A, Err>",
            Rend::Rappel => "void",
        }
    }

    /// `true` si l'appelant recoit une promesse, donc si l'appel depend d'un
    /// ordonnanceur qui n'est pas simule ici.
    pub fn est_promesse(self) -> bool {
        matches!(self, Rend::PromiseExit | Rend::Promise)
    }
}

/// `Effect.RunOptions`, cite par la source et jamais construit par elle.
///
/// Seules `runPromiseExit` et `runPromise` le recoivent, et uniquement pour le
/// transmettre tel quel a `ManagedRuntime`. La source ne lit jamais un champ
/// de ce type, n'en construit aucun et n'en controle aucun : son contenu n'est
/// donc **pas** reporte, et ce type est un marqueur opaque. Deux jeux
/// d'options differents sont indiscernables ici, ce qui est une limite du
/// portage et non une propriete de la source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunOptions;

/// Le graphe de couches de la source, `Layer.provideMerge(layer,
/// Observability.layer)`, porte comme DONNEE.
///
/// La source passe le resultat a `ManagedRuntime.make` avec un `as
/// Layer.Layer<I, E>` qui masque le fait que la fusion fournit plus que `I` :
/// les services de l'observabilite sortent de la couche fusionnee sans
/// etre demandes. Ce portage ne reproduit pas le faux typage, il garde le
/// fait.
///
/// Ce qui n'est pas reporte : la construction effective des couches, leur
/// ordre de fermeture, la portee, et le graphe lui-meme. Il n'y a pas de
/// `Layer` en Rust, donc [`Composition`] est une liste de noms, pas un
/// programme.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Composition {
    /// La couche de l'application, celle que `makeRuntime` recoit en
    /// parametre.
    applicative: LayerKey,
    /// Toujours la couche du noeud `observability` : l'injection n'est pas
    /// optionnelle dans la source.
    observabilite: LayerKey,
    /// `provideMerge` : la sortie de la couche fournie est unie a celle de
    /// `observability`, et non remplacee par elle.
    fusionnee: bool,
}

impl Composition {
    /// La composition de la couche donnee.
    pub fn new(applicative: LayerKey) -> Self {
        Self {
            applicative,
            observabilite: LayerKey::new(NODE_NAME),
            fusionnee: true,
        }
    }

    /// La couche fournie par l'appelant.
    pub fn applicative(&self) -> &LayerKey {
        &self.applicative
    }

    /// La couche d'observabilite, toujours presente.
    pub fn observabilite(&self) -> &LayerKey {
        &self.observabilite
    }

    /// Les deux couches, dans l'ordre des arguments de `provideMerge` : la
    /// couche de l'application d'abord, celle qui recoit ensuite.
    pub fn couches(&self) -> Vec<LayerKey> {
        vec![self.applicative.clone(), self.observabilite.clone()]
    }

    /// `true` si la sortie de la couche fournie est unie a celle de
    /// l'observabilite.
    pub fn est_fusionnee(&self) -> bool {
        self.fusionnee
    }

    /// `true` si la composition installe la couche du noeud `observability`.
    ///
    /// Vrai pour toute composition construite par [`Composition::new`] : c
    /// est le point que la source ne rend pas configurable et qu'un test
    /// verifie ici.
    pub fn installe_observabilite(&self) -> bool {
        self.observabilite.name() == NODE_NAME
    }
}

/// Le descripteur renvoye par `makeRuntime`.
///
/// Le runtime lui-meme n'existe pas ici : ce struct en porte la COMPOSITION
/// (quelle couche, quel service, quelle table) et son ETAT de construction,
/// qui sont les deux seules choses que `rt ??=` laisse observer de
/// l'exterieur. Les cinq methodes ajoutent le journal des points d'entree
/// appeles, que la source ne tient pas mais qui rend la construction differee
/// verifiable sans executor quoi que ce soit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    /// Le tag du service fourni a `makeRuntime`, celui que `service.use(fn)`
    /// extrait du contexte.
    service: ServiceKey,
    /// `Layer.provideMerge(layer, Observability.layer)`.
    composition: Composition,
    /// Le `memoMap` partage, reference et non copie.
    memo_map: SharedMemoMap,
    /// L'etat de `rt`.
    etat: Etat,
    /// Nombre d'appels a `ManagedRuntime.make`, qu'ils reussissent ou non.
    tentatives: usize,
    /// Nombre de constructions reussies, donc nombre de fois ou `rt` a recu
    /// une valeur. Il ne peut depasser [`Runtime::tentatives`].
    constructions: usize,
    /// Les points d'entree appeles, dans l'ordre.
    appels: Vec<Entry>,
}

impl Runtime {
    /// `makeRuntime(service, layer)` : prepare un descripteur sans rien
    /// construire. `rt` vaut `undefined` a la sortie.
    pub fn new(service: ServiceKey, layer: LayerKey) -> Self {
        Self {
            service,
            composition: Composition::new(layer),
            memo_map: MEMO_MAP,
            etat: Etat::NonConstruit,
            tentatives: 0,
            constructions: 0,
            appels: Vec::new(),
        }
    }

    /// Le tag du service fourni.
    pub fn service(&self) -> &ServiceKey {
        &self.service
    }

    /// Le graphe de couches, porte comme donnee.
    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    /// La table de memorisation referencee, qui est la table partagee du
    /// processus.
    pub fn memo_map(&self) -> SharedMemoMap {
        self.memo_map
    }

    /// L'etat de `rt`.
    pub fn etat(&self) -> Etat {
        self.etat
    }

    /// Le nombre de tentatives de construction, reussies ou non.
    pub fn tentatives(&self) -> usize {
        self.tentatives
    }

    /// Le nombre de constructions reussies. Au plus une par `makeRuntime`.
    pub fn constructions(&self) -> usize {
        self.constructions
    }

    /// Les points d'entree appeles, dans l'ordre.
    pub fn appels(&self) -> &[Entry] {
        &self.appels
    }

    /// `rt ??= ManagedRuntime.make(...)`.
    ///
    /// Ne fait rien si le runtime existe deja. Sinon compte une tentative, et
    /// ne passe a [`Etat::Construit`] que si `reussi` est vrai.
    ///
    /// L'argument `reussi` tient lieu de resultat de `ManagedRuntime.make`.
    /// La source ne le demande nulle part : elle laisse `make` lever, et
    /// c'est alors `rt` qui reste `undefined`. Ici, ce choix appartient a
    /// l'appelant, parce que ce module n'a aucun moyen de savoir si la
    /// construction d'un graphe a reussi. Les cinq points d'entree passent
    /// `true`, qui correspond au cas normal ; un appelant qui modele un
    /// echec appelle cette methode directement.
    pub fn construire(&mut self, reussi: bool) -> Etat {
        if self.etat.est_construit() {
            return self.etat;
        }
        self.tentatives += 1;
        if reussi {
            self.etat = Etat::Construit;
            self.constructions += 1;
        }
        self.etat
    }

    /// Construit au besoin, journalise le point d'entree, et rend le plan de
    /// l'appel. Point de passage unique des cinq methodes.
    fn entrer<A, Err>(&mut self, entry: Entry, options: Option<RunOptions>) -> Run<A, Err> {
        // `getRuntime()` est evalue en premier, avant l'argument
        // `service.use(fn)` : la construction a donc lieu meme si l'appelant
        // ne fournit pas de fonction exploitable. L'ordre est le meme ici.
        let etat = self.construire(true);
        self.appels.push(entry);
        Run {
            entry,
            service: self.service.clone(),
            options,
            etat,
            _effet: PhantomData,
        }
    }

    /// `runSync(fn)`.
    pub fn run_sync<A, Err>(&mut self) -> Run<A, Err> {
        self.entrer(Entry::RunSync, None)
    }

    /// `runPromiseExit(fn, options?)`.
    pub fn run_promise_exit<A, Err>(&mut self, options: Option<RunOptions>) -> Run<A, Err> {
        self.entrer(Entry::RunPromiseExit, options)
    }

    /// `runPromise(fn, options?)`.
    pub fn run_promise<A, Err>(&mut self, options: Option<RunOptions>) -> Run<A, Err> {
        self.entrer(Entry::RunPromise, options)
    }

    /// `runFork(fn)`.
    pub fn run_fork<A, Err>(&mut self) -> Run<A, Err> {
        self.entrer(Entry::RunFork, None)
    }

    /// `runCallback(fn)`.
    pub fn run_callback<A, Err>(&mut self) -> Run<A, Err> {
        self.entrer(Entry::RunCallback, None)
    }
}

/// `export function makeRuntime(service, layer)`.
pub fn make_runtime(service: ServiceKey, layer: LayerKey) -> Runtime {
    Runtime::new(service, layer)
}

/// Le plan d'un appel a un point d'entree.
///
/// Ce n'est pas une execution : la source execute, ici on decrit. Voir la
/// section « Ce qui n a pas de traduction » dans la doc du module, qui
/// explique pourquoi la fonction `fn` et le type `Effect` ont disparu.
/// `Copy` n'est pas derivable ici : le plan porte un `ServiceKey`, donc une
/// `String`, et la source renvoie un objet neuf a chaque appel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run<A, Err> {
    /// Le point d'entre appele.
    pub entry: Entry,
    /// Le tag du service que `service.use(fn)` aurait extrait du contexte.
    pub service: ServiceKey,
    /// Les options transmises telles quelles, `None` si la methode de la
    /// source ne les declare pas ou si l'appelant ne les fournit pas.
    pub options: Option<RunOptions>,
    /// L'etat du runtime au moment ou le plan a ete rendu.
    pub etat: Etat,
    /// Les types `A` et `Err` de `Effect.Effect<A, Err, I>`, effaces a
    /// l'execution. Les conserver dans la signature evite que le portage
    /// presume d'une reponse que la source ne formule pas.
    _effet: PhantomData<(A, Err)>,
}

impl<A, Err> Run<A, Err> {
    /// Le point d'entre.
    pub fn entry(&self) -> Entry {
        self.entry
    }

    /// Le tag du service a fournir.
    pub fn service(&self) -> &ServiceKey {
        &self.service
    }

    /// Les options transmises.
    pub fn options(&self) -> Option<RunOptions> {
        self.options
    }

    /// Ce que la methode rend a l'appelant.
    pub fn rend(&self) -> Rend {
        self.entry.rend()
    }

    /// `true` si l'effet est detache de l'appelant, donc si personne
    /// n'attend son resultat.
    pub fn est_detache(&self) -> bool {
        matches!(self.entry, Entry::RunFork)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        make_runtime, Composition, Entry, Etat, Rend, RunOptions, Runtime, ServiceKey,
        SharedMemoMap, MEMO_MAP, NODE_NAME,
    };
    use crate::swarm::effect_memo_map::LayerKey;
    use std::collections::BTreeSet;

    fn runtime() -> Runtime {
        make_runtime(ServiceKey::new("Session"), LayerKey::new("SessionLayer"))
    }

    // --- la construction differee -----------------------------------------

    #[test]
    fn un_runtime_neuf_n_est_pas_construit() {
        // `let rt: ManagedRuntime | undefined` : rien n'est construit tant
        // qu'aucun point d'entree n'a ete appele.
        let rt = runtime();

        assert_eq!(rt.etat(), Etat::NonConstruit);
        assert!(!rt.etat().est_construit());
        assert_eq!(rt.tentatives(), 0);
        assert_eq!(rt.constructions(), 0);
        assert!(rt.appels().is_empty());
    }

    #[test]
    fn le_premier_point_d_entree_construit_le_runtime() {
        let mut rt = runtime();

        let plan = rt.run_sync::<(), ()>();

        assert_eq!(plan.etat, Etat::Construit);
        assert_eq!(rt.etat(), Etat::Construit);
        assert_eq!(rt.tentatives(), 1);
        assert_eq!(rt.constructions(), 1);
    }

    #[test]
    fn les_quatre_autres_points_d_entree_reutilisent_le_meme_runtime() {
        // `rt ??=` : le second et les suivants ne reconstruisent rien. C'est
        // le point central du fichier.
        let mut rt = runtime();

        rt.run_sync::<(), ()>();
        rt.run_promise_exit::<(), ()>(None);
        rt.run_promise::<(), ()>(None);
        rt.run_fork::<(), ()>();
        rt.run_callback::<(), ()>();

        assert_eq!(rt.constructions(), 1, "le runtime ne doit etre construit qu'une fois");
        assert_eq!(rt.tentatives(), 1, "les appels suivants ne doivent pas retenter");
    }

    #[test]
    fn un_appel_apres_construction_ne_change_plus_l_etat() {
        let mut rt = runtime();
        rt.construire(true);

        let etat = rt.construire(true);

        assert_eq!(etat, Etat::Construit);
        assert_eq!(rt.tentatives(), 1);
        assert_eq!(rt.constructions(), 1);
    }

    #[test]
    fn un_echec_de_construction_laisse_rt_indefini_et_le_suivant_retente() {
        // `rt ??= make(...)` : si `make` leve, l'affectation n'a pas lieu,
        // `rt` reste `undefined` et l'appel suivant reconstruit. Il n'existe
        // donc pas d'etat « en echec » : c'est le compteur de tentatives qui
        // distingue la situation, pas l'etat.
        let mut rt = runtime();

        let premier = rt.construire(false);
        assert_eq!(premier, Etat::NonConstruit);
        assert_eq!(rt.tentatives(), 1);
        assert_eq!(rt.constructions(), 0);

        let second = rt.construire(true);
        assert_eq!(second, Etat::Construit);
        assert_eq!(rt.tentatives(), 2, "le second appel doit retenter");
        assert_eq!(rt.constructions(), 1, "mais une seule construction reussie");
    }

    #[test]
    fn les_constructions_reussies_ne_peuvent_pas_depasser_les_tentatives() {
        let mut rt = runtime();

        rt.construire(false);
        rt.construire(false);
        rt.construire(true);
        rt.construire(true);

        assert_eq!(rt.tentatives(), 3);
        assert_eq!(rt.constructions(), 1);
    }

    #[test]
    fn deux_runtimes_ne_partagent_pas_leur_construction() {
        // `rt` est une variable locale a chaque appel de `makeRuntime` : deux
        // descripteurs sont deux objets distincts.
        let mut premier = runtime();
        let mut second = make_runtime(
            ServiceKey::new("Autre"),
            LayerKey::new("AutreLayer"),
        );

        premier.run_sync::<(), ()>();
        second.run_promise::<(), ()>(None);

        assert_eq!(premier.constructions(), 1);
        assert_eq!(second.constructions(), 1);
        assert_eq!(premier.appels(), &[Entry::RunSync]);
        assert_eq!(second.appels(), &[Entry::RunPromise]);
    }

    // --- les cinq points d'entree -----------------------------------------

    #[test]
    fn la_source_expose_cinq_points_d_entree_dans_ceux_declare() {
        // L'ordre du tableau est celui des cles de l'objet renvoye par
        // `makeRuntime` : runSync, runPromiseExit, runPromise, runFork,
        // runCallback.
        let noms: Vec<&str> = Entry::ALL.iter().map(|e| e.nom()).collect();
        assert_eq!(
            noms,
            vec![
                "runSync",
                "runPromiseExit",
                "runPromise",
                "runFork",
                "runCallback"
            ]
        );
        assert_eq!(Entry::ALL.len(), 5);
    }

    #[test]
    fn un_nom_de_methode_retrouve_son_point_d_entree() {
        for entry in Entry::ALL {
            assert_eq!(Entry::depuis_nom(entry.nom()), Some(entry));
        }
        assert_eq!(Entry::depuis_nom("runLater"), None);
        assert_eq!(Entry::depuis_nom("runsync"), None, "les noms sont camelCase");
    }

    #[test]
    fn seuls_les_deux_points_promesse_acceptent_des_options() {
        // `runPromiseExit(fn, options?)` et `runPromise(fn, options?)` sont les
        // deux seules methodes qui declarent le second parametre.
        let attendus: BTreeSet<Entry> = [Entry::RunPromiseExit, Entry::RunPromise]
            .into_iter()
            .collect();
        let vus: BTreeSet<Entry> = Entry::ALL
            .into_iter()
            .filter(|e| e.accepte_options())
            .collect();

        assert_eq!(vus, attendus);
        assert!(!Entry::RunSync.accepte_options());
        assert!(!Entry::RunFork.accepte_options());
        assert!(!Entry::RunCallback.accepte_options());
    }

    #[test]
    fn les_appels_sont_journalises_dans_l_ordre() {
        let mut rt = runtime();

        rt.run_fork::<(), ()>();
        rt.run_sync::<(), ()>();
        rt.run_fork::<(), ()>();

        assert_eq!(rt.appels(), &[Entry::RunFork, Entry::RunSync, Entry::RunFork]);
    }

    // --- ce que chaque point d'entree rend --------------------------------

    #[test]
    fn chaque_point_d_entree_rend_un_type_different() {
        // Les cinq methodes ont cinq contrats de retour distincts : les
        // confondre ferait perdre la difference entre une erreur et une
        // issue, qui est le point de `runPromiseExit`.
        let rendeurs: BTreeSet<Rend> = Entry::ALL.iter().map(|e| e.rend()).collect();
        assert_eq!(rendeurs.len(), 5, "les cinq rendements doivent rester distincts");
        assert_eq!(Entry::RunSync.rend(), Rend::Valeur);
        assert_eq!(Entry::RunPromiseExit.rend(), Rend::PromiseExit);
        assert_eq!(Entry::RunPromise.rend(), Rend::Promise);
        assert_eq!(Entry::RunFork.rend(), Rend::Fibre);
        assert_eq!(Entry::RunCallback.rend(), Rend::Rappel);
    }

    #[test]
    fn le_type_rendu_est_annotable_comme_dans_la_bibliotheque_effect() {
        assert_eq!(Rend::Valeur.type_rend(), "A");
        assert_eq!(Rend::PromiseExit.type_rend(), "Promise<Exit<A, Err>>");
        assert_eq!(Rend::Promise.type_rend(), "Promise<A>");
        assert_eq!(Rend::Fibre.type_rend(), "Fiber<A, Err>");
        assert_eq!(Rend::Rappel.type_rend(), "void");
    }

    #[test]
    fn seuls_les_deux_points_promesse_donnent_une_promesse() {
        for entry in Entry::ALL {
            assert_eq!(
                entry.rend().est_promesse(),
                entry.accepte_options(),
                "{} : accepter des options et rendre une promesse vont ensemble",
                entry.nom()
            );
        }
    }

    #[test]
    fn seul_run_fork_detache_l_effet() {
        // `runFork` rend une `Fiber` que l'appelant peut ignorer. Les quatre
        // autres gardent l'appelant responsable du resultat.
        for entry in Entry::ALL {
            assert_eq!(
                entry == Entry::RunFork,
                entry.rend() == Rend::Fibre,
                "{} : le detachement ne concerne que runFork",
                entry.nom()
            );
        }
    }

    // --- le plan rendu par les methodes -----------------------------------

    #[test]
    fn un_plan_porte_le_point_d_entree_et_le_service_donne() {
        let mut rt = runtime();

        let plan = rt.run_sync::<(), ()>();

        assert_eq!(plan.entry(), Entry::RunSync);
        assert_eq!(plan.service().nom(), "Session");
        assert_eq!(rt.service().nom(), "Session");
    }

    #[test]
    fn les_cinq_points_d_entree_envoient_tous_le_service() {
        // Les cinq methodes de la source sont ecrites `service.use(fn)` :
        // aucune ne se dispense du service. Un plan sans service serait un
        // portage faux.
        let mut rt = runtime();

        let plans = [
            rt.run_sync::<(), ()>(),
            rt.run_promise_exit::<(), ()>(None),
            rt.run_promise::<(), ()>(None),
            rt.run_fork::<(), ()>(),
            rt.run_callback::<(), ()>(),
        ];

        for plan in plans {
            assert_eq!(
                plan.service().nom(),
                "Session",
                "{} doit aussi passer par service.use",
                plan.entry().nom()
            );
        }
    }

    #[test]
    fn un_plan_est_construit_contre_un_runtime_deja_construit() {
        // `getRuntime()` est evalue avant l'argument : le plan voit toujours
        // un runtime construit, jamais `undefined`.
        let mut rt = runtime();
        assert_eq!(rt.etat(), Etat::NonConstruit);

        let premier = rt.run_sync::<(), ()>();
        let second = rt.run_promise::<(), ()>(None);

        assert_eq!(premier.etat, Etat::Construit);
        assert_eq!(second.etat, Etat::Construit);
    }

    #[test]
    fn les_options_absentes_vautent_none() {
        // `options?` : l appelant peut s abstenir, et la source transmet alors
        // `undefined` a `runPromise`.
        let mut rt = runtime();

        let plan = rt.run_promise::<(), ()>(None);

        assert_eq!(plan.options(), None);
    }

    #[test]
    fn les_options_fournies_sont_transmises_telles_quoelles() {
        // La source ne lit aucun champ de `Effect.RunOptions` : elle le
        // transmet. Ce portage fait pareil, et n'en dit pas plus.
        let mut rt = runtime();

        let plan = rt.run_promise_exit::<(), ()>(Some(RunOptions));

        assert_eq!(plan.options(), Some(RunOptions));
        assert_eq!(plan.entry(), Entry::RunPromiseExit);
    }

    #[test]
    fn un_plan_ne_confond_pas_options_et_options_absentes() {
        let mut rt = runtime();

        let avec = rt.run_promise::<(), ()>(Some(RunOptions));
        let sans = rt.run_promise::<(), ()>(None);

        assert_ne!(avec.options(), sans.options());
    }

    #[test]
    fn les_types_de_l_effet_sont_effaces_a_l_execution() {
        // `Effect.Effect<A, Err, I>` ne se transporte pas dans le plan : deux
        // plans construits avec des types de succes et d'erreur differents
        // sont identiques s'ils visent le meme point d'entree. C'est la
        // limite assumee du portage, et elle est visible ici.
        let mut rt = runtime();
        let plan_chaine = rt.run_sync::<String, String>();
        let plan_unitaire = rt.run_sync::<(), ()>();

        assert_eq!(plan_chaine, plan_unitaire);
    }

    #[test]
    fn deux_appels_identiques_donnent_deux_plans_identiques() {
        let mut rt = runtime();

        assert_eq!(rt.run_fork::<(), ()>(), rt.run_fork::<(), ()>());
    }

    // --- la composition de couches ----------------------------------------

    #[test]
    fn la_observabilite_est_toujours_injectee() {
        // `Layer.provideMerge(layer, Observability.layer)` n'a aucune variante
        // conditionnelle : toute composition installe le noeud voisin.
        let composition = Composition::new(LayerKey::new("SessionLayer"));

        assert!(composition.installe_observabilite());
        assert_eq!(composition.observabilite().name(), NODE_NAME);
        assert_eq!(NODE_NAME, "observability", "le nom vient du portage du noeud");
    }

    #[test]
    fn la_couche_de_l_observabilite_est_toujours_fusionnee() {
        // `provideMerge` et non `provide` : la sortie de la couche fournie
        // est unie a celle de l'observabilite, pas remplacee par elle.
        let composition = Composition::new(LayerKey::new("SessionLayer"));

        assert!(composition.est_fusionnee());
        assert_eq!(composition.couches().len(), 2);
    }

    #[test]
    fn les_couches_sont_dans_l_ordre_des_arguments_de_provide_merge() {
        // `provideMerge(self, that)` : d'abord la couche de l'application,
        // ensuite celle qui recoit.
        let composition = Composition::new(LayerKey::new("SessionLayer"));
        let noms: Vec<&str> = composition
            .couches()
            .iter()
            .map(|couche| couche.name())
            .collect();

        assert_eq!(noms, vec!["SessionLayer", NODE_NAME]);
        assert_eq!(composition.applicative().name(), "SessionLayer");
    }

    #[test]
    fn deux_couches_differentes_donnent_deux_compositions_differentes() {
        let premiere = Composition::new(LayerKey::new("A"));
        let seconde = Composition::new(LayerKey::new("B"));

        assert_ne!(premiere, seconde);
        assert_eq!(premiere.observabilite(), seconde.observabilite());
    }

    #[test]
    fn un_runtime_expose_la_composition_de_sa_couche() {
        let rt = runtime();

        assert_eq!(rt.composition().applicative().name(), "SessionLayer");
        assert!(rt.composition().installe_observabilite());
    }

    #[test]
    fn la_couche_de_l_observabilite_ne_depend_pas_du_service_demande() {
        // Le `as Layer.Layer<I, E>` masque le fait que la fusion fournit des
        // services absents de `I`. Le portage garde le fait : la couche
        // d'observabilite est la meme quel que soit le service demande.
        let session = runtime();
        let autre = make_runtime(ServiceKey::new("Autre"), LayerKey::new("AutreLayer"));

        assert_eq!(session.service().nom(), "Session");
        assert_eq!(
            session.composition().observabilite(),
            autre.composition().observabilite(),
            "la couche d'observabilite ne depend pas du service demande"
        );
        assert_ne!(session.service(), autre.service());
    }

    // --- la table de memorisation partagee ---------------------------------

    #[test]
    fn tout_runtime_reference_la_table_partagee() {
        let rt = runtime();

        assert_eq!(rt.memo_map(), MEMO_MAP);
        assert_eq!(rt.memo_map(), SharedMemoMap);
    }

    #[test]
    fn la_table_partagee_est_la_meme_pour_tous_les_runtimes() {
        // `memoMap` est le singleton de `effect/memo-map.ts`, partage par tous
        // les runtimes. C'est la seule forme de partage verifiable ici : pas
        // d'Arc, pas de Mutex, pas de static.
        let premier = runtime();
        let second = make_runtime(
            ServiceKey::new("Autre"),
            LayerKey::new("AutreLayer"),
        );

        assert_eq!(premier.memo_map(), second.memo_map());
    }

    #[test]
    fn la_cle_du_service_est_reutilisable_et_comparable() {
        // `Context.Service<I, S>` n'est qu'un tag : il se compare par son nom.
        let cle = ServiceKey::new("Session");

        assert_eq!(cle.nom(), "Session");
        assert_eq!(cle, ServiceKey::new("Session"));
        assert_ne!(cle, ServiceKey::new("Session "));
    }
}
