# swarm-util_array

===DEBUT===
fichier : src/swarm/util_array.rs
source  : packages/core/src/util/array.ts
taille  : 7009 octets
tests   : 12

CONFIANCE : haute
POINT FAIBLE : le predicat est un `FnMut(&T, usize, &'a [T]) -> bool` alors que le TS prend `item` **par valeur** ; un portage fidele `FnMut(T, usize, &[T])` serait plus strict mais interdirait d'appeler `find_last` sur un `Vec` detruyant l'emprunt. J'ai choisi l'emprunt, ce qui change le contrat pour les appelants (voir plus bas), et je n'ai pas verifie que les 8 sites d'appel reels du TS se traduisent sans `clone()`.
A VERIFIER : (1) que les appelants `permission.rs:133` / `policy.rs:154` qui font `.find_last((rule) => Wildcard.match(action, rule.action))` restent compilables chez eux, qui n'ont pas ete touches ; (2) que la resolution de l'inference de type sur `*item % 2 == 1` (litteral entier non type) aboutit bien a `i32` ; (3) le doctest, qui depend de `pub mod swarm` et `pub mod util_array` declares par l'agent principal dans `mod.rs` -- il ne compile que lorsque les 20 fichiers du lot existent.

DETAIL :
- La source fait 10 lignes et n'exporte que `findLast<T>`. Rien d'autre n'a ete
  invente : pas de `findLastIndex`, pas de version `Vec<T>`, pas de trait.
- `T | undefined` devient `Option<&T>`. On renvoie un emprunt sur la liste
  d'origine, pas une copie : un test dedie le verifie.
- Le predicat garde les trois arguments du TS (`item`, `index`, `items`). Aucun
  des 8 sites d'appel du TS n'utilise le troisieme, mais il fait partie du
  contrat de `Array.prototype.findLast` et ne coute rien a conserver.
- Parcours inverse par `(0..len()).rev()`, pas `len() - 1` : sur une liste
  vide, `usize - 1` deborderait et paniquerait. Le TS ne boucle pas non plus,
  mais pour une autre raison (`-1` est un `number` signe). Ce point est
  commente sur place et couvert par `une_liste_vide_ne_donne_rien`.
- Aucune conversion de ce fichier : pas de struct, pas de champ, donc pas de
  `#[serde(rename)]` a verifier. Le piege des noms de champs ne s'applique pas.
- Tests : liste vide, un seul element (trouve / pas trouve), aucune
  correspondance, derniere occurrence gagne, ordre inverse donne un autre
  resultat, arret au premier succes en remontant, ordre de visite des index
  decroissant, troisieme argument = liste complete, valeurs falsy (`0`, `""`,
  `false`), resultat = emprunt sur la liste d'origine.
- Aucun caractere non ASCII dans le fichier (verifie). Aucune ligne de plus de
  100 caracteres. `cargo fmt --check` est `continue-on-error` dans
  `.github/workflows/build.yml`, le format n'est donc pas bloquant.
- Le doctest utilise `use ycode::swarm::util_array::find_last;` : en edition
  2021 un doctest est un crate externe, donc le chemin doit commencer par le
  nom de la bibliotheque, pas par le module.

===FIN===
