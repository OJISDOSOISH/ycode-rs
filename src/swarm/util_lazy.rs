//! Portage Rust de `opencode/packages/core/src/util/lazy.ts`.
//!
//! La source tient en dix lignes : un calcul est differe jusqu au premier appel,
//! puis sa valeur est memorisee et renvoyee telle quelle a tous les appels
//! suivants.
//!
//! # Concurrence : rien a traduire, et c est volontaire
//!
//! La source TypeScript n utilise **ni promesse ni verrou**. Elle capture deux
//! variables locales (`value` et `loaded`) dans une fermeture, et s en sert
//! directement. Il n y a donc aucun mecanisme de concurrence a reproduire, et on
//! n en invente aucun ici : pas de `Mutex`, pas de `RwLock`, pas de `OnceCell`,
//! pas de `RefCell`.
//!
//! Deux proprietes du modele d origine sont en revanchevolontairement
//! abandonnees, parce qu elles n ont pas d equivalent Rust direct :
//!
//! 1. **Pas de partage implicite.** En JavaScript, la fermeture capturee est une
//!    reference mutable, appelable depuis plusieurs endroits a la fois sans
//!    que l appelant ait a le declarer. Ici `get` prend `&mut self` : c est
//!    l emprise exclusive de l appelant qui remplace le partage implicite, et
//!    c est une restriction de surface, pas un ajout de synchronisation. Un
//!    appelant qui a besoin de la meme valeur depuis une methode `&self` devra
//!    lui meme choisir une strategy, exactement comme il doit le faire pour
//!    n importe quel autre champ.
//! 2. **Pas de memoire interieur.** Aucune copie n est demandee a `T` : `get`
//!    renvoie `&T`, donc la valeur reste stockee en un seul endroit et se
//!    comporte comme l objet unique de JavaScript. Une version qui renvoie `T`
//!    par valeur obligerait d ajouter une borne `T: Clone` qui n existe pas dans
//!    la source.
//!
//! # Le cas `undefined`
//!
//! La source distingue `value: T | undefined` (la valeur) de `loaded: boolean`
//! (le temoin de chargement). Ce sont deux informations separees, et le portage
//! les garde separees : `value: Option<T>` et `loaded: bool`. Conflater les
//! deux aurait casse le cas ou le calcul renvoie legitimately `None`.
//!
//! L ordre des operations est egalement respecte : `loaded` passe a `true`
//! **avant** l appel du calcul, comme dans la source.

/// Un calcul differe, execute au premier appel puis memorise.
///
/// Voir [le module](crate::swarm::util_lazy) pour le detail des choix de
/// portage. Le type est volontairement minimal : pas de `Default`, pas de
/// `Clone`, pas de `Send`, car la source n'en demande aucun.
pub struct Lazy<T> {
    /// Le calcul, consomme au premier appel. Il disparait ensuite, ce qui
    /// garantit qu il ne peut pas etre relance meme par erreur.
    init: Option<Box<dyn FnOnce() -> T>>,
    /// La valeur deja calculee. `None` signifie "pas encore calculee", sauf
    /// dans le cas incoherent decrit plus bas.
    value: Option<T>,
    /// Temoin de chargement, separe de `value` pour rester fidele a la source.
    loaded: bool,
}

impl<T> Lazy<T> {
    /// Prepare un calcul differe. Rien n est execute avant le premier appel a
    /// [Lazy::get].
    pub fn new(init: impl FnOnce() -> T + 'static) -> Self {
        Self {
            init: Some(Box::new(init)),
            value: None,
            loaded: false,
        }
    }

    /// Indique si le calcul a deja ete execute. N'execute rien.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Renvoie la valeur, en executant le calcul si c'est la premiere fois.
    ///
    /// Entre sa creation et son premier appel, la valeur peut changer de
    /// contenu sans que l'appelant ne le voie. A partir du premier appel, elle
    /// est figee.
    ///
    /// # Panique
    ///
    /// Entre en panique si le calcul a ete declare charge alors qu'aucune valeur
    /// n'est stockee. Cela n'arrive que si le calcul a lui-meme leve une
    /// panique, ou s'il s'est rappele recursivement sur le meme `Lazy` pendant
    /// son propre execution. La source JavaScript renvoie alors silencieusement
    /// `undefined` : ce resultat n a pas de traduction Rust honnete, puisque `T`
    /// ne peut pas valoir "aucune valeur", donc on leve une panne explicite
    /// plutot que de rendre un `T` construit de toutes pieces.
    pub fn get(&mut self) -> &T {
        if !self.loaded {
            // Comme dans la source, le temoin passe avant l'appel du calcul.
            self.loaded = true;
            let init = self.init.take();
            let value = match init {
                Some(init) => init(),
                None => panic!("Lazy: calcul manquant alors que loaded vaut true"),
            };
            self.value = Some(value);
        }
        match self.value.as_ref() {
            Some(value) => value,
            None => panic!("Lazy: calcule mais valeur absente, le calcul a echoue"),
        }
    }
}

/// Constructeur libre, nom identique a l export TypeScript, pour que la
/// correspondance entre les deux fichiers se lise d'un coup d'oeil.
pub fn lazy<T>(init: impl FnOnce() -> T + 'static) -> Lazy<T> {
    Lazy::new(init)
}

#[cfg(test)]
mod tests {
    use super::{lazy, Lazy};
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    #[test]
    fn creer_un_lazy_ne_lance_pas_le_calcul() {
        // Le point essentiel du fichier : la construction ne doit rien
        // executer, sinon un lazy place dans une variable globale travaille
        // des le demarrage du programme.
        let compteur = Rc::new(Cell::new(0));
        let l = {
            let compteur = Rc::clone(&compteur);
            lazy(move || {
                compteur.set(compteur.get() + 1);
                42
            })
        };
        assert_eq!(compteur.get(), 0);
        assert!(!l.is_loaded());
    }

    #[test]
    fn le_premier_appel_renvoie_le_resultat_du_calcul() {
        let mut l = lazy(|| 42);
        assert_eq!(*l.get(), 42);
        assert!(l.is_loaded());
    }

    #[test]
    fn les_appels_suivants_ne_relancent_pas_le_calcul() {
        let compteur = Rc::new(Cell::new(0));
        let mut l = {
            let compteur = Rc::clone(&compteur);
            lazy(move || {
                compteur.set(compteur.get() + 1);
                7
            })
        };
        assert_eq!(*l.get(), 7);
        assert_eq!(*l.get(), 7);
        assert_eq!(*l.get(), 7);
        assert_eq!(compteur.get(), 1, "le calcul ne doit tourner qu'une seule fois");
    }

    #[test]
    fn la_valeur_memorisee_reste_la_meme_adresse() {
        // La source renvoie toujours le meme objet. On verifie ici que la
        // valeur memorisee n est pas deplacee ni recopiee entre deux appels.
        let mut l = lazy(|| 7);
        let premier = l.get() as *const i32;
        let second = l.get() as *const i32;
        assert!(std::ptr::eq(premier, second));
    }

    #[test]
    fn un_resultat_absent_du_type_utilisateur_est_bien_memorise() {
        // Cas limite : ici `T` vaut `Option<String>` et le calcul renvoie
        // `None`. Si le portage confondait "pas calcule" et "calcule a None",
        // le calcul serait relance a chaque appel.
        let compteur = Rc::new(Cell::new(0));
        let mut l = {
            let compteur = Rc::clone(&compteur);
            lazy(move || {
                compteur.set(compteur.get() + 1);
                None::<String>
            })
        };
        assert!(l.get().is_none());
        assert!(l.get().is_none());
        assert!(l.is_loaded());
        assert_eq!(compteur.get(), 1);
    }

    #[test]
    fn deux_lazy_independants_ne_partagent_pas_leir_calcul() {
        let mut premier = lazy(|| 1);
        let mut second = lazy(|| 2);
        assert_eq!(*premier.get(), 1);
        assert_eq!(*second.get(), 2);
        assert_eq!(*premier.get(), 1);
        assert_eq!(*second.get(), 2);
    }

    #[test]
    fn un_calcul_qui_panique_ne_permet_pas_de_relancer_la_lecture() {
        // Divergence assumee et documentee : en JavaScript, un calcul qui leve
        // laisse la fermeture dans un etat ou elle renvoie `undefined` a tout
        // coup. Ici la deuxieme lecture entre en panique au lieu de mentir.
        let mut l: Lazy<i32> = lazy(|| -> i32 { panic!("echec du calcul") });
        let premier = catch_unwind(AssertUnwindSafe(|| {
            let _ = l.get();
        }));
        assert!(premier.is_err());
        assert!(l.is_loaded(), "le temoin passe avant le calcul, comme en JavaScript");
        let second = catch_unwind(AssertUnwindSafe(|| {
            let _ = l.get();
        }));
        assert!(second.is_err());
    }
}
