# swarm-util_binary

===DEBUT===
fichier : src/swarm/util_binary.rs
source  : packages/core/src/util/binary.ts
taille  : 20925 octets
tests   : 19 (plus 2 doctests)

AVERTISSEMENT DE PROCEDURE, A LIRE EN PREMIER : JE N AI PAS ECRIT CE FICHIER.

Quand j'ai recu la mission, `src/swarm/util_binary.rs` existait deja, entierement
porte (501 lignes, 19 tests, 2 doctests), et `reponses/swarm-util_binary.md`
existait deja aussi. Le claim `claims/util_binary.claim.json` porte
`"owner":"kilo-swarm3"`, c est a dire un tour de swarm anterieur. Le tableau de
board montre une session "W3: util_binary" a l etat "cancelled" : elle a ecrit
le fichier et le rapport, puis a ete annulee apres coup.

La regle du contexte est explicite : "Un fichier deja porte n est jamais
reecrit." Je n ai donc PAS touche a `util_binary.rs`. J ai relu la source
TypeScript puis le fichier Rust ligne par ligne, et j ai rejoue les deux
algorithmes a la main sur chacun des 19 tests. Ma contribution est donc une
REVIFICATION, pas un portage. Si l agent principal attendait un second jet
independant, il faut le demander explicitement, car il remplacerait un fichier
que je juge correct.

CONFIANCE : haute. Les deux boucles ont ete rejouees a la main, test par test,
et chaque resultat affirme par un test a ete confirme par un calcul. Aucun
des 19 tests n affirme quoi que ce soit que je n ai pas retrace. La confiance
sur la SYNTAXE reste non verifiable, aucune compilation n etant autorisee sur ce
poste et je n en ai lance aucune.
POINT FAIBLE : la signature `S: AsRef<str>` du comparateur, reprise telle
quelle du jet anterieur. Elle accepte un comparateur qui renvoie une chaine
POSEE, ce qui est le cas des 19 tests, mais un comparateur qui renvoie un
emprunt de son argument (`|e: &Entree| &e.cle`) impose au compilateur
d unifier le type de sortie avec la duree de vie de l argument, et je n ai pas
pu verifier que cette unification passe. C est un risque de compilation qui ne
touche que les APPELANTS, pas ce fichier. Lever la reserve : garder `String`
en sortie, ou ecrire un seul test avec un comparateur a emprunt.
A VERIFIER : (1) `pub mod util_binary;` est bien present a la ligne 70 de
`src/swarm/mod.rs` -- le point d integration signale par le rapport anterieur
est RESOLU, je n y ai rien touche ; (2) `SearchResult` porte a la fois un champ
`found` et une fonction associee `SearchResult::found()` : c est legal en Rust
(espaces de noms distincts) mais c est le point de syntaxe le moins courant du
fichier ; (3) la resolution d inference sur `let mid_key: &str =
mid_id.as_ref()`, ou le type a ete force precisement parce que `S` peut
implanter plusieurs `AsRef` a la fois.

REVERIFICATION DETAILLEE :

- Integrite du fichier : 20925 octets, ce qui correspond exactement a la taille
  annoncee par le rapport anterieur. 19 marqueurs `#[test]`, 2 blocs de
  doctest. 0 octet non ASCII, verifie octet par octet : ni accent, ni guillemet
  typographique, ni CJK. Aucun octet binaire, aucun fichier de test contenant
  du binaire.

- ALERTE DE NOM, elle vaut pour toute la vague : ce fichier ne manipule AUCUN
  fichier binaire. Le nom est trompeur, c est une recherche BINAIRE sur un
  tableau trie. Aucun octet, aucune lecture disque. La consigne de mission sur
  les fichiers binaires ne s'applique pas ici.

- La source fait 41 lignes : un `namespace Binary` avec `search` et `insert`.
  Aucun schema, aucun ternaire `?`, aucun `??`, aucune donnee serialisee.

- Piege des noms de champs : il n y en a pas. Les deux seules cles exposees
  sont `found` et `index`, en minuscules simples, donc aucun `#[serde(rename)]`
  n est necessaire. La parade est la troisieme partie du fichier : derive
  `Serialize, Deserialize` sur `SearchResult` plus trois tests qui verrouillent
  les noms, dont un qui verifie qu un `{"isFound":...}` est REFUSE a la
  deserialisation. Ce refus fonctionne parce que `found` n a pas de valeur par
  defaut : un mauvais nom produit une erreur au lieu de passer en silence.

- `search` : borne droite `len - 1` avec `left <= right`, borne gauche en
  `isize` et non en `usize`. C est indispensable : sur une liste vide la
  source demarre a `-1`, et `0usize - 1` deborderait et paniquerait. J ai
  retrace la division : dans la boucle `left >= 0` et `right >= left`, donc
  `left + right >= 0`, et la division entiere de Rust qui tronque vers zero
  coincide avec le `Math.floor` de la source. Sortie de boucle, `left` ne peut
  jamais etre negatif, donc `left as usize` est sur. Indexation : `mid` est
  dans `[left, right]`, donc dans `[0, len - 1]`, toujours valide.

- `insert` : borne droite `len` avec `left < right`, donc `mid <= right - 1 <
  right <= len`, et l indexation reste valide meme sur une liste vide. C est
  la seule difference de borne entre les deux fonctions, et elle est
  necessaire, pas cosmetique. Position finale `left` dans `[0, len]`, ce que
  `Vec::insert` accepte.

- Divergence de sens assumee et documentee : `insert` place l element AVANT
  une cle egale, alors que `Vec::insert_sorted` de la bibliotheque standard
  place APRES. Sur une liste a cles en double l ordre des elements change.
  J ai retrace le test dedie : sur `[a, b]`, `left=0 right=2 mid=1` donne `"b"`
  qui n est pas inferieur a `"b"`, donc `right=1`, puis `mid=0` donne `"a"`
  inferieur, donc `left=1`, et l insertion se fait en position 1, donc devant
  l ancien. Le test affirme `liste[1].valeur == 99` puis `liste[2].valeur == 2`.
  C est correct.

- Contrat non ameliore, volontairement : ni `search` ni `insert` ne verifient
  le tri. J ai retrace le test de liste decroissante `[c, b, a]` avec `"a"` :
  `mid=1` donne `"b"` qui est superieur, donc `right=0`, puis `mid=0` donne
  `"c"` superieur, donc `right=-1`, sortie, et le resultat est bien
  `not_found(0)` alors que `"a"` est present a la position 2. C est le
  comportement de la source, le trier serait un changement silencieux.

- Complexite logarithmique : j ai retrace les deux tests a compteur. Sur 8
  elements, `search("h")` fait 4 comparaisons (bornes `4 <= 4`, juste) et
  `insert("h")` en fait 4 plus celle de l element insere, donc 5 (juste). Un
  parcours lineaire en ferait 8 et 9, les tests le detecteraient.

- Convention partagee : j ai retrace l invariant sur les 4 cles testees
  (`"A"`, `"b"`, `"e"`, `"h"`) et sur chacune `search` renvoie bien l indice
  ou `insert` place l element. C est ce qui permet aux appelants d ecrire
  `list.splice(result.index, 0, item)`.

- Piege `?` contre `??` : SANS OBJET sur ce fichier, la source ne contient ni
  l un ni l autre. En revanche le test
  `une_cle_vide_est_traitee_comme_une_cle_et_non_comme_une_absence` verifie un
  point voisin et pertinent : la source teste l egalite stricte et la
  comparaison, jamais la veracite, donc la chaine vide est une CLE comme une
  autre et ne disparait pas. J ai retrace : sur `["", "b"]`, `mid=0` donne la
  chaine vide, egale, donc `found(0)`. Correct.

- Cle composite : `la_cle_dune_liste_peut_etre_composee` imite `messageKey`
  qui vaut `time.created + id`. J ai retrace le format `{:020}` : la liste
  `[bruno(date 1), alfa(date 2)]` produit les cles `...01bruno` et `...02alfa`,
  donc la liste est bien triee malgre l ordre lexicographique inverse des
  identifiants. Recherche, puis insertion de `zulu(date 3)` a la fin et de
  `omega(date 0)` au debut donne bien `[omega, bruno, alfa, zulu]`. Correct.

- Verification de code mort, refaite par moi : sur l arbre complet du depot
  opencode, 40 appels a `Binary.search` et ZERO appel a `Binary.insert`. Le
  rapport anterieur annoncait le meme chiffre, je le confirme. `insert` est
  donc exportee mais jamais exercee par le TypeScript.

- Verification des comparateurs reels : j ai relu les 40 sites d appel. La
  quasi totalite passe `(item) => item.id`, mais DEUX fichiers passent autre
  chose : `packages/app/src/context/sync.tsx:71` et
  `packages/app/src/context/server-session.ts:113` passent `messageKey`, qui
  vaut `time.created + id`. C est ce qui justifie le comparateur generique et
  le test de cle composite. Un portage qui aurait impose `id` aurait casse ces
  deux sites, silencieusement.

- Hazard signale pour le portage des appelants, il est reel et non theorique :
  quand la cle est absente, `index` peut valoir `array.len()`. En JavaScript
  `array[array.length]` vaut `undefined` sans erreur, en Rust `liste[liste.len()]`
  PANIQUE. Apres un `found: false`, il faut passer par `Vec::insert` ou
  `Vec::remove`, jamais par un indexage direct. Le fichier le documente et le
  teste.

- Comparaison de chaines : le TS compare en unites UTF-16, Rust en UTF-8
  octets. Les deux concordent exactement sur de l ASCII, ce qui couvre tous les
  identifiants du projet (hexadecimal et base62). Ils divergent des que la cle
  contient un caractere hors ASCII, par exemple certains emoji dont l ordre
  UTF-16 n est pas l ordre de points de code. Commente dans l entete du
  module, non corrige, car corriger exigerait une comparaison UTF-16 qui
  n existe pas dans la bibliotheque standard.

- Comparaison des deux API : `insert` renvoie `()` alors que le TS renvoie le
  tableau qu il vient de modifier. Le tableau est deja emprunte en ecriture en
  Rust, donc rien ne se perd. C est le seul ecart d API du fichier, il est
  signale dans la documentation de la fonction.

- Doctests : j ai verifie leurs types a la main, ce sont eux le risque de
  compilation le plus reel du fichier. `search(&cles, ...)` fait passer
  `&Vec<String>` pour un `&[T]`, ce qui passe par la deref coercion.
  `assert_eq!(cles, vec!["a", "b", "c"])` compare un `Vec<String>` a un
  `Vec<&str>`, ce qui exige `impl PartialEq<&str> for String` dans la
  bibliotheque standard : il existe. Le nom de crate `ycode` employe dans les
  doctests correspond bien au `name = "ycode"` de `Cargo.toml`.

- `serde` et `serde_json` sont deja dans `Cargo.toml` (lignes 7 et 8), rien a
  ajouter. Les derives `Serialize` et `Deserialize` avec la fonctionnalite
  `derive` sont actives.

- Aucun commit, aucun push. Je n ai modifie aucun fichier du projet, seulement
  ce rapport, qui est mon canal de sortie.

===FIN===
