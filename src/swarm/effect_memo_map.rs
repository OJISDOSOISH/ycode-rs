//! Portage de `packages/core/src/effect/memo-map.ts`.
//!
//! La source tient en trois lignes :
//!
//! ```ts
//! import { Layer } from "effect"
//!
//! export const memoMap = Layer.makeMemoMapUnsafe()
//! ```
//!
//! Il ny a donc **aucune logique de cache ecrite a la main** dans le depot
//! TypeScript : tout le comportement vient de `Layer.makeMemoMapUnsafe()`, qui
//! appartient a la bibliotheque `effect` et pas a `opencode`. Ce fichier se
//! contente de fabriquer une valeur unique, partagee par tout le processus.
//!
//! Son usage est visible dans `packages/core/src/effect/runtime.ts` : le
//! `memoMap` est passe a `ManagedRuntime.make`, qui sen sert pour construire
//! chaque couche du graphe **une seule fois** et reutiliser l instance deja
//! construite pour toutes les requetes suivantes.
//!
//! Les proprietes du cache que lon peut retrouver sans la bibliotheque
//! `effect`, et qui sont donc portees ici :
//!
//! 1. la cle de recherche est double : l identite de la couche **et**
//!    l identite de la portee courante ;
//! 2. une entree manquante declenche la construction, puis la table conserve
//!    le resultat ainsi qu une portee fille associee a l entree ;
//! 3. la fermeture d une portee purge toutes les entrees memorisees sous
//!    cette portee, en liberant au passage les portees filles.
//!
//! Ce qui nest **pas** porte, et qui na pas d equivalent dans la
//! bibliotheque standard Rust :
//!
//! - l identite de couche et l identite de portee sont, en JavaScript, des
//!   references d objet. Ici ce sont des cles deterministes (`LayerKey`,
//!   `ScopeKey`) fournies par l appelant. Cest la seule partie du portage
//!   qui demande un truec de la part du code appelant ;
//! - le graphe de couches `Layer` et le `Scope` d Effect, avec leur
//!   fermeture ordonnee et leur compteur de references, nexistent pas en
//!   Rust. **Ils ne sont pas simules ici** : ce module ne fait qu un cache
//!   cle/valeur, sans graphe, sans finaliseur, sans cycle de vie automatique ;
//! - la variante `Unsafe` de la bibliotheque `effect` nest pas synchronisee :
//!   elle appartient a un seul runtime, qui en est le seul proprietaire.
//!   Aucune synchronisation nest donc ajoutee ici non plus. `MemoMap` prend
//!   `&mut self`, ce qui rend l usage exclusif par construction.
//!
//! Aucune conversion `serde` nest fournie : la source nexporte pas de
//! donnee serialisable, seulement un objet de cycle de vie en memoire.

use std::collections::BTreeMap;

/// Identite d une couche du graphe Effect, remplacee par un nom
/// deterministe. En TypeScript cest l identite par reference de l objet
/// `Layer`, qui na pas d equivalent direct et verifiable en Rust.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerKey(String);

impl LayerKey {
    /// Construit une cle de couche a partir de son nom.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Renvoie le nom de la couche.
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Identite d une portee Effect, remplacee par un entier attribue dans
/// l ordre des appels. `None` represente l absence de portee courante, ce
/// qui correspond au `undefined` du `Scope` d Effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeKey(u64);

impl ScopeKey {
    /// Renvoie le numero de portee.
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Entree memorisee : la couche reellement construite, et la portee fille
/// ouverte pour elle. La portee fille est fermee avec la portee qui a
/// provoque la memorisation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoEntry {
    /// La couche construite, qui peut differer de la cle recherchee.
    pub layer: LayerKey,
    /// La portee fille associee a cette entree.
    pub scope: ScopeKey,
}

/// Table de memorisation des couches, equivalente au `memoMap` exporte par
/// la source.
///
/// La cle de recherche est le couple (couche, portee courante). Deux couches
/// differentes donnent deux entrees, et une meme couche cherchee dans deux
/// portees differentes donne aussi deux entrees.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoMap {
    /// Portee courante, `None` tant qu aucune portee na ete ouverte.
    current_scope: Option<ScopeKey>,
    /// Compteur d attribution des numeros de portee fille.
    next_scope: u64,
    /// Les entrees, triees par (couche, portee).
    state: BTreeMap<(LayerKey, Option<ScopeKey>), MemoEntry>,
}

impl MemoMap {
    /// Cree une table vide, sans portee courante. Cest l equivalent de
    /// `Layer.makeMemoMapUnsafe()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renvoie la portee courante, ou `None` si aucune portee est ouverte.
    pub fn current_scope(&self) -> Option<ScopeKey> {
        self.current_scope
    }

    /// Change la portee courante. Les entrees deja memorisees restent en
    /// place : elles redeviennent visibles si lon revient a leur portee.
    pub fn set_current_scope(&mut self, scope: Option<ScopeKey>) {
        self.current_scope = scope;
    }

    /// Ouvre une portee fille et renvoie sa cle. Le numero est attribue dans
    /// l ordre des appels, donc de facon deterministe.
    pub fn fork_scope(&mut self) -> ScopeKey {
        let numero = self.next_scope;
        self.next_scope += 1;
        ScopeKey(numero)
    }

    /// Cherche l entree de la couche donnee dans la portee courante, sans
    /// jamais construire quoi que ce soit.
    pub fn get(&self, layer: &LayerKey) -> Option<&MemoEntry> {
        self.state.get(&(layer.clone(), self.current_scope))
    }

    /// Rend la couche correspondant a `layer` dans la portee courante.
    ///
    /// Si une entree existe deja, elle est renvoyee et `build` nest **pas**
    /// appele. Sinon `build` est appele une fois, une portee fille est
    /// ouverte, le resultat est memorise, et cest lui qui est renvoye.
    pub fn get_or_else_memoize(
        &mut self,
        layer: &LayerKey,
        build: impl FnOnce() -> LayerKey,
    ) -> LayerKey {
        if let Some(entry) = self.get(layer) {
            return entry.layer.clone();
        }

        let construite = build();
        let portee_fille = self.fork_scope();
        let cle = (layer.clone(), self.current_scope);
        self.state.insert(
            cle,
            MemoEntry {
                layer: construite.clone(),
                scope: portee_fille,
            },
        );
        construite
    }

    /// Oublie toutes les entrees memorisees sous la portee donnee et
    /// renvoie combien d entrees ont ete retirees. Les entrees des autres
    /// portees ne sont pas touchees.
    pub fn forget_scope(&mut self, scope: &ScopeKey) -> usize {
        let avant = self.state.len();
        self.state
            .retain(|(_, portee), _| portee.as_ref() != Some(scope));
        avant - self.state.len()
    }

    /// Renvoie le nombre d entrees memorisees, toutes portees confondues.
    pub fn len(&self) -> usize {
        self.state.len()
    }

    /// Renvoie `true` si rien nest memorise.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Renvoie toutes les cles (couche, portee), dans l ordre de tri de la
    /// table, donc de facon deterministe.
    pub fn keys(&self) -> Vec<(LayerKey, Option<ScopeKey>)> {
        self.state.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn une_table_neuve_ne_contient_rien() {
        let map = MemoMap::new();

        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert!(map.keys().is_empty());
        assert_eq!(map.current_scope(), None);
        assert!(map.get(&LayerKey::new("Observability")).is_none());
    }

    #[test]
    fn une_couche_absente_est_construite_puis_reutilisee() {
        let mut map = MemoMap::new();
        let cle = LayerKey::new("Observability.layer");
        let constructions = Cell::new(0);

        let premier = map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });
        let second = map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });

        assert_eq!(constructions.get(), 1);
        assert_eq!(premier, second);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn deux_couches_differentes_donnent_deux_entrees() {
        let mut map = MemoMap::new();

        map.get_or_else_memoize(&LayerKey::new("a"), || LayerKey::new("a"));
        map.get_or_else_memoize(&LayerKey::new("b"), || LayerKey::new("b"));

        assert_eq!(map.len(), 2);
        assert!(map.get(&LayerKey::new("a")).is_some());
        assert!(map.get(&LayerKey::new("b")).is_some());
        assert!(map.get(&LayerKey::new("c")).is_none());
    }

    #[test]
    fn une_meme_couche_dans_deux_portees_est_construite_deux_fois() {
        let mut map = MemoMap::new();
        let cle = LayerKey::new("cache");
        let constructions = Cell::new(0);

        let premiere_portee = map.fork_scope();
        map.set_current_scope(Some(premiere_portee));
        map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });

        let seconde_portee = map.fork_scope();
        map.set_current_scope(Some(seconde_portee));
        map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });

        assert_eq!(constructions.get(), 2);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn revenir_a_la_premiere_portee_reutilise_son_entree() {
        let mut map = MemoMap::new();
        let cle = LayerKey::new("cache");
        let constructions = Cell::new(0);
        let premiere_portee = map.fork_scope();

        map.set_current_scope(Some(premiere_portee));
        map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });

        let seconde_portee = map.fork_scope();
        map.set_current_scope(Some(seconde_portee));
        map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });

        map.set_current_scope(Some(premiere_portee));
        map.get_or_else_memoize(&cle, || {
            constructions.set(constructions.get() + 1);
            cle.clone()
        });

        assert_eq!(constructions.get(), 2);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn la_couche_construite_est_memorisee_sous_le_nom_de_la_cle() {
        let mut map = MemoMap::new();
        let cle = LayerKey::new("cle");
        let construite = LayerKey::new("couche-reelle");

        let rendue = map.get_or_else_memoize(&cle, || construite.clone());

        assert_eq!(rendue, construite);
        assert_eq!(map.get(&cle).map(|e| e.layer.clone()), Some(construite));
    }

    #[test]
    fn chaque_entree_recoit_une_portee_fille_distincte() {
        let mut map = MemoMap::new();

        map.get_or_else_memoize(&LayerKey::new("a"), || LayerKey::new("a"));
        map.get_or_else_memoize(&LayerKey::new("b"), || LayerKey::new("b"));

        let portee_a = map.get(&LayerKey::new("a")).map(|e| e.scope);
        let portee_b = map.get(&LayerKey::new("b")).map(|e| e.scope);

        assert!(portee_a.is_some());
        assert!(portee_b.is_some());
        assert_ne!(portee_a, portee_b);
    }

    #[test]
    fn oublier_une_portee_retire_ses_entrees_et_laisse_les_autres() {
        let mut map = MemoMap::new();
        let portee_a = map.fork_scope();
        let portee_b = map.fork_scope();

        map.set_current_scope(Some(portee_a));
        map.get_or_else_memoize(&LayerKey::new("a"), || LayerKey::new("a"));

        map.set_current_scope(Some(portee_b));
        map.get_or_else_memoize(&LayerKey::new("b"), || LayerKey::new("b"));

        let retires = map.forget_scope(&portee_a);

        assert_eq!(retires, 1);
        assert_eq!(map.len(), 1);
        assert!(map.get(&LayerKey::new("b")).is_some());
    }

    #[test]
    fn oublier_une_portee_inconnue_ne_change_rien() {
        let mut map = MemoMap::new();
        let portee = map.fork_scope();
        map.set_current_scope(Some(portee));
        map.get_or_else_memoize(&LayerKey::new("a"), || LayerKey::new("a"));

        let retires = map.forget_scope(&ScopeKey(999));

        assert_eq!(retires, 0);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn sans_portee_ouverte_les_cle_partagent_la_portee_par_defaut() {
        let mut map = MemoMap::new();
        let cle = LayerKey::new("a");

        map.get_or_else_memoize(&cle, || cle.clone());
        let attendu = vec![(cle.clone(), None)];

        assert_eq!(map.keys(), attendu);
        assert!(map.get(&cle).is_some());
    }

    #[test]
    fn les_cles_sont_rendues_dans_un_ordre_deterministe() {
        let mut map = MemoMap::new();
        let portee = map.fork_scope();
        map.set_current_scope(Some(portee));

        map.get_or_else_memoize(&LayerKey::new("c"), || LayerKey::new("c"));
        map.get_or_else_memoize(&LayerKey::new("a"), || LayerKey::new("a"));
        map.get_or_else_memoize(&LayerKey::new("b"), || LayerKey::new("b"));

        let noms: Vec<String> = map
            .keys()
            .into_iter()
            .map(|(couche, _)| couche.name().to_string())
            .collect();
        let attendu = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        assert_eq!(noms, attendu);
    }
}
