# review-effect_memo_map

## VERDICT : APPROUVE

Pas de BUG bloquant demontrable ligne a ligne contre la source TS.
La source fait 3 lignes sans logique propre, tout le comportement vient
de la lib `effect` non installee ici. Le portage est une interpretation
documentee et honnete, avec confiance moyenne affichee dans le rapport.
Reserve principale : la semantique exacte de `makeMemoMapUnsafe` reste
a confirmer quand la lib sera lisible. En l etat, rien ne contredit le TS.

## Sources lues en entier

- `opencode/packages/core/src/effect/memo-map.ts` : 3 lignes.
  `import { Layer } from "effect"` + `export const memoMap = Layer.makeMemoMapUnsafe()`.
- `ycode-rs/src/core/session/compaction.rs` : 171 lignes, lues comme charte
  de style (constantes, docs, tests, gestion UTF-8 par `chars`, pas de serde
  inventee).
- `ycode-rs/reponses/swarm-effect_memo_map.md` : 15 lignes, rapport present.
- `ycode-rs/src/swarm/effect_memo_map.rs` : 389 lignes, 11 tests.
- Complement : `opencode/packages/core/src/effect/runtime.ts` : 21 lignes.
  Confirme l usage : `memoMap` passe a `ManagedRuntime.make` en
  `runtime.ts:8-10`, sert a construire chaque couche une seule fois.

## Points verifies OK

1. JSON / renames : sans objet. Source TS n exporte aucune donnee
   serialisable, seulement un objet de cycle de vie. Rust ne derive ni
   `Serialize` ni `Deserialize`, ne met aucun `serde(rename)`. OK.
   Piege ID / Id / majuscules : sans objet ici, aucun champ JSON.
2. Variantes perdues : aucune union, aucun enum, aucun status dans la
   source. Rust n a donc aucune variante a perdre. `LayerKey` / `ScopeKey` /
   `MemoEntry` / `MemoMap` sont des ajouts assumes pour modeliser cle
   double et portee fille, pas des variantes TS oubliees. OK.
3. Operateurs JS pieges :
   - Pas de `?` vs `??`, pas de `||` vs `??`, pas de truthiness dans la
     source (3 lignes, aucun acces optionnel).
   - Pas de `length` UTF-16 a convertir. Rust compte par `BTreeMap::len`
     en nombre d entrees, pas en taille texte. OK.
   - Pas de `Object.keys` / insertion order a preserver cote TS, car le
     TS n expose aucune iteration. OK.
4. Types numeriques : pas de `f64` / `i64` / `u64` dans la source.
   Rust utilise `u64` pour `ScopeKey` et `next_scope`, `usize` pour `len`.
   Pas de conversion flottant vers entier, pas de perte de precision. OK.
5. Semantique `Unsafe` : TS utilise `makeMemoMapUnsafe`, non synchronise,
   possede par un seul runtime. Rust prend `&mut self`, sans `Mutex` /
   `Arc` / finaliseur. Choix coherent, documente en tete de fichier
   lignes 42-45. OK.
6. Coeur porte : double cle (couche, portee courante), construction une
   seule fois si absent, ouverture d une portee fille par entree,
   purge par portee. C est une deduction depuis l usage dans
   `runtime.ts`, annoncee comme telle dans le rapport. Pas de
   sur-interpretation silencieuse. OK.
7. Tests couvrant les pieges applicables :
   - construction unique (`une_couche_absente_est_construite_puis_reutilisee`) ;
   - double cle (`une_meme_couche_dans_deux_portees_est_construite_deux_fois`) ;
   - retour a la portee d origine reutilise l entree ;
   - couche construite memorisee sous le nom de la cle ;
   - portee fille distincte par entree ;
   - `forget_scope` retire sa portee et laisse les autres ;
   - `forget_scope` inconnue ne change rien ;
   - portee `None` par defaut ;
   - ordre deterministe des cles.
   11 tests, conformes a l annonce du rapport. OK.

## Nits non bloquants

1. `keys()` trie par `BTreeMap`, donc ordre alphabetique.
   Une `Map` JS garde l ordre d insertion. Ici `keys()` est une API
   inventee pour tests et debug, sans equivalent TS. Pas de BUG TS vs Rust.
   Suggestion : documenter que l ordre est trie, pas d insertion.
2. `forget_scope` purge sur la portee de la CLE, pas sur la portee FILLE
   de l entree. Choix delibere, signale dans le rapport comme point
   a verifier (1). Tant que la lib `effect` n est pas lisible, impossible
   de trancher. Ne pas changer sans preuve. A recontroler avec
   `node_modules/effect` ou doc officielle.
3. `get_or_else_memoize` modelise `build` comme `FnOnce() -> LayerKey`
   synchrone et infaillible, de meme type que la cle. Le vrai `Layer`
   construit un service de type arbitraire, souvent async et fallible.
   Simplification assumee, mais limite la reutilisation pour un vrai
   runtime. Evolution future : rendre le cache generique sur `V`.
4. Pas de singleton global cote Rust (`MemoMap::new()` a appeler), alors
   que le TS exporte une `const memoMap` partagee. C est normal en Rust
   (pas de global mutable sans sync) et coherent avec `&mut self`.
   Il faudra juste relier l instance au futur proprietaire de runtime.
5. `fork_scope` fait `next_scope += 1` sans `checked_add`. Overflow
   theorique apres 2^64 portees, sans impact pratique. Si la charte
   impose du `saturating` partout, uniformiser.
6. `ScopeKey(999)` en test accede au champ prive via `ScopeKey(999)`.
   OK car test enfant du module, mais un constructeur `from_value`
   serait plus propre si des tests externes arrivent.
7. Taille annoncee 13712 octets dans le rapport : valeur informative,
   non verifiee ici comme critere de qualite. Sans impact.

## Conclusion

APPROUVE en l etat, avec reserves documentees ci-dessus.
Aucun commit, aucun push, aucune modif de `src/` ou `claims/`.
