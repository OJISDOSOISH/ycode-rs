# swarm-util_wildcard

===DEBUT===
fichier : src/swarm/util_wildcard.rs
source  : packages/core/src/util/wildcard.ts
taille  : 12818 octets
tests   : 10

CONFIANCE : haute
POINT FAIBLE : je raisonne en `char` (scalaire Unicode) alors que la regexp JavaScript raisonne en unites UTF-16, donc un caractere hors du plan multilingue de base (un emoji) compte pour un dans mon portage et pour deux dans l'original, et un motif `?` qui matche un emoji chez moi ne matche pas chez l'original.
A VERIFIER : (1) que la syntaxe Rust compile, en particulier le nom brut `r#match` reporte a travers le `use super::*` des tests et la conversion `&Vec<Node>` vers `&[Node]` dans `walk` ; (2) le comportement multi-plateforme, la CI GitHub Actions compilant sur Linux, donc `CASE_INSENSITIVE` y vaut `false` alors que la source sur la meme machine Windows vaudrait `true` ; (3) que `?` est bien remplace par `.` et pas conserve, c'etait mon unique vrai bug, attrape par fuzz.

Notes de portage :

- La source fait 14 lignes et n'exporte qu'une fonction. La ligne 1,
  `export * as Wildcard from "./wildcard"`, est un reexport de l'espace de noms
  du module sur lui-meme, c'est-a-dire du code mort ; en Rust le module joue
  deja ce role, il n'y a rien a porter la-dessus.
- `match` est un mot cle reserve de Rust, la fonction s'appelle donc `r#match`.
  `r#match_with_case(input, pattern, ignore_case)` expose le drapeau `i` pour
  que les tests couvrent les deux plateformes sans dependre de la cible.
- Aucune dependance ajoutee : le crate `regex` n'est pas dans `Cargo.toml` et je
  n'ai pas le droit de le modifier. La transformation de la source ne peut
  produire qu'un langage trivial, litteraux, `.`, `.*`, et au plus un groupe
  optionnel `( .*)?` en fin de motif, donc un petit automate suffit. Il
  memorise l'ensemble des positions atteignables, ce qui supprime le retour
  arriere et son explosion combinatoire.
- Le portage reproduit les quatre remplacements dans l'ordre exact de la source :
  contre-oblique vers barre oblique, echappement des metacaracteres, `*` vers
  `.*`, puis `?` vers `.`. Cet ordre est significatif, l'echappement passe
  avant la conversion des jokers.
- Pieges de sens confirms a la lecture, chacun couvert par un test :
  `?` n'est pas un joker optionnel, il disparait et devient un `.` qui matche
  exactement un caractere ; `.`, `+`, `|`, `(`, `[`, `{`, `^`, `$` du motif sont
  des litteraux ; un motif reduit a ` .*` devient `( .*)?`, donc un motif comme
  `foo *` matche `foo`, `foo ` et `foo bar` mais pas `foobar` ; l'entree comme
  le motif sont normalises, une contre-oblique vaut une barre oblique des deux
  cotes ; le drapeau `s` est toujours actif, `.` traverse donc les retours a la
  ligne.

Validation : l'algorithme a ete transliterate en JavaScript puis compare a la
fonction TypeScript d'origine sur 400 000 couples motifs/entrees aleatoires avec
un alphabet charge en metacaracteres, plus 200 000 de plus en forçant la branche
sensible a la casse, plus les 66 cas fixes de mes tests. Ecart : 0. Cette
validation porte sur la logique, pas sur la syntaxe Rust, qui n'a pas pu etre
compilee (aucune toolchain sur ce poste).

`pub mod util_wildcard;` est deja present ligne 30 de `src/swarm/mod.rs`, je n'y
ai pas touche. Aucun commit, aucun push.
===FIN===
