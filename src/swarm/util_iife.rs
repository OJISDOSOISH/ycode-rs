//! Portage de `packages/core/src/util/iife.ts`.
//!
//! La source tient en trois lignes, et il n'y a rien d'autre a porter :
//!
//! ```ts
//! export function iife<T>(fn: () => T) {
//!   return fn()
//! }
//! ```
//!
//! IIFE veut dire "Immediately Invoked Function Expression". En JavaScript on
//! ecrit `(function () { ... })()` pour creer une portee locale au module ; le
//! reste du code opencode s'en sert partout pour evaluer un bloc qui a besoin
//! de variables declarees juste au-dessus, et la valeur de retour devient le
//! contenu de ce bloc.
//!
//! En Rust la portee de bloc existe deja : `{ let x = 1; ... }` fait exactement
//! le meme travail. La seule chose qui reste de la source est l'appel d'une
//! closure, ecrit partout dans le projet sous la forme d'un simple appel.
//!
//! On fournit donc la fonction generique equivalente, et rien de plus. Aucun
//! struct, aucun enum, aucun nom de champ : il n'y a donc aucun
//! `serde(rename = ...)` a poser ici. Comme `T` est generique, le cas
//! `iife(async () => ...)` du test d'origine tombe tout seul : la future est
//! simplement la valeur de retour.
//!
//! Seule deviation par rapport a la source : le parametre s'appelle `f` et non
//! `fn`, car `fn` est un mot cle reserve en Rust.

/// Appelle immediatement la closure passee en argument et renvoie son resultat.
///
/// Equivaut a `(function () { ... })()` en JavaScript.
pub fn iife<T, F>(f: F) -> T
where
    F: FnOnce() -> T,
{
    f()
}

#[cfg(test)]
mod tests {
    use super::iife;

    #[test]
    fn une_closure_qui_renvoie_un_entier_donne_cet_entier() {
        assert_eq!(iife(|| 42), 42);
    }

    #[test]
    fn une_closure_est_executee_une_seule_fois_au_moment_de_l_appel() {
        let compteur = std::cell::Cell::new(0);
        let f = || {
            compteur.set(compteur.get() + 1);
            compteur.get()
        };
        assert_eq!(compteur.get(), 0, "rien ne doit tourner avant l'appel");
        assert_eq!(iife(f), 1);
        assert_eq!(compteur.get(), 1);
    }

    #[test]
    fn une_closure_sans_retour_explicite_donne_le_vide() {
        let resultat: () = iife(|| {});
        assert_eq!(resultat, ());
    }

    #[test]
    fn une_closure_qui_renvoie_une_liste_vide_donne_une_liste_vide() {
        let liste: Vec<i32> = iife(|| Vec::new());
        assert!(liste.is_empty());
    }

    #[test]
    fn une_closure_peut_porter_une_valeur_par_deplacement() {
        let texte = String::from("opencode");
        let recupere = iife(move || texte);
        assert_eq!(recupere, "opencode");
    }

    #[test]
    fn une_erreur_produite_par_la_closure_est_renvoyee_telle_quelle() {
        assert!(iife(|| Err::<i32, &str>("boom")).is_err());
    }

    #[test]
    fn une_closure_asynchrone_donne_une_futur_qui_livre_son_resultat() {
        use std::future::Future;
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};

        struct WakerSansEffet;
        impl Wake for WakerSansEffet {
            fn wake(self: Arc<Self>) {}
        }

        let mut futur = Box::pin(iife(|| async { 7i32 }));
        let waker = Waker::from(Arc::new(WakerSansEffet));
        let mut contexte = Context::from_waker(&waker);

        match futur.as_mut().poll(&mut contexte) {
            Poll::Ready(valeur) => assert_eq!(valeur, 7),
            Poll::Pending => panic!("la future aurait du etre prete du premier coup"),
        }
    }
}
