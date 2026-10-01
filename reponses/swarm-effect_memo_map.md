# swarm-effect_memo_map

===DEBUT===
fichier : src/swarm/effect_memo_map.rs
source  : packages/core/src/effect/memo-map.ts
taille  : 13712 octets
tests   : 11

CONFIANCE : moyenne
POINT FAIBLE : la source fait 3 lignes (`export const memoMap = Layer.makeMemoMapUnsafe()`) et **tout** le comportement vient de la bibliotheque `effect`, qui n est pas installee sur cette machine (`node_modules\effect` absent) : je n ai donc pas pu lire l implementation de `makeMemoMapUnsafe`, je l ai deduite de son usage dans `packages/core/src/effect/runtime.ts`. Ce que j ai porte, c est un cache cle/valeur deterministe a double cle (couche, portee courante) avec portee fille et purge a la fermeture de portee. Ce qui manque et que je n ai **pas** invente : le graphe de couches `Layer`, le `Scope` d Effect, sa fermeture ordonnee et son compteur de references. Aucun `Mutex` / `Arc` / finaliseur n a ete ajoute, conformement a la consigne : la variante `Unsafe` du TS est justement non synchronisee et possedee par un seul runtime, et mon `MemoMap` prend `&mut self`.
A VERIFIER : (1) que la semantique double cle (couche, portee) est bien celle de `makeMemoMapUnsafe` et que `forget_scope` doit purger sur la portee de la CLE et non sur la portee FILLE de l entree, choix que j ai fait deliberement ; (2) que `LayerKey` et `ScopeKey` ne doivent pas porter de `Serialize` — je ne les ai pas derives, la source n exportant aucune donnee serialisable, mais un autre agent pourrait etre persuade du contraire ; (3) qu aucun `#[serde(rename)]` ne manque (il n y a aucun struct JSON ici, le piege des noms de champs est sans objet sur ce fichier) ; (4) que `mod.rs` ligne 19 declare bien `pub mod effect_memo_map;` — c est deja le cas, je n y ai pas touche.

Detail d usage trouve en lisant la source : le `memoMap` est passe a `ManagedRuntime.make` dans `runtime.ts:8-10`, ce qui confirme qu il sert a construire chaque couche une seule fois par runtime. L API de mon module est donc pensee pour etre.appelee par un futur proprietaire de runtime, pas par les appelants de `runtime.ts`.
Aucun commit, aucun push.
===FIN===
