# swarm-util_slug

===DEBUT===
fichier : src/swarm/util_slug.rs
source  : packages/core/src/util/slug.ts
taille  : 17607 octets (479 lignes)
tests   : 14

CONFIANCE : haute
POINT FAIBLE : la source n'a AUCUNE entree, donc tout ce que la mission demandait (accents, casse, ponctuation, espaces multiples, chaine vide, non ASCII, emoji, longueur max, caracteres reserves) n'existe pas comme cas d'entree : je les ai traites comme des invariants de SORTIE, ce qui est un changement de nature de test et non un simple supplement.
A VERIFIER : (1) que le portage du hasard (SplitMix64 + Mutex au lieu du PRNG V8) est bien accepte, c'est le seul ecart de comportement reel ; (2) que la CI passe les 14 tests, aucune ligne n'a ete compilee.
===FIN===

## La source n'est pas un slugify

C'est le point le plus important du lot, et il change tout.

`packages/core/src/util/slug.ts` fait 74 lignes et expose **une seule**
fonction, `Slug.create()`, qui **ne recoit aucun parametre** :

```ts
export function create() {
  return [
    ADJECTIVES[Math.floor(Math.random() * ADJECTIVES.length)],
    NOUNS[Math.floor(Math.random() * NOUNS.length)],
  ].join("-")
}
```

Elle tire un adjectif au hasard, tire un nom au hasard, et les colle avec un
tiret. Il n'y a rien a normaliser. Les neuf familles de cas limites que la
mission me demandait de couvrir **n'ont pas de cas limites** : pas d'accent a
retirer, pas de casse a plier, pas d'espace a tasser, pas de chaine vide a
trancher, pas d'emoji a echapper, pas de longueur a borner.

Deux consequences, expliquees dans l'en-tete du fichier :

- Je les ai portees comme **invariants de sortie**. Les 60 mots sont en
  minuscules ASCII, sans espace, sans tiret et sans accent, donc le slug est
  toujours exactement `mot-tiret-mot`, de 9 a 16 octets, toujours ASCII, jamais
  vide, avec exactement un tiret. Ce sont des garanties, pas des entrees, et
  les tests les verifient comme telles.
- La fonction `slugify` que la mission avait en tete existe, mais **pas dans ce
  fichier**. Elle est locale a deux autres fichiers, hors de mon lot :
  `packages/opencode/src/worktree/index.ts:91` et
  `packages/opencode/src/server/routes/instance/httpapi/handlers/project-copy.ts:76`.
  Personne dans le swarm ne la porte. Si quelqu'un la cherche dans le portage
  Rust, elle n'y est pas : `Slug.create()` n'est qu'un repli, utilise par
  `project-copy.ts:27`, `:29`, `:59` et `:68`.

## Deux bugs de tests reels, trouves et corriges

Le fichier existait deja sur disque, ecrit par une execution anterieure du meme
claim qui n'avait pas eu le temps d'ecrire le rapport. Je l'ai relu ligne a
ligne contre la source, et j'ai trouve deux assertions qui **echouaient**,
 alors que le code de production etait correct :

1. `assert_eq!(assembler("brave", "moon").len(), 9)` : `brave` fait **cinq**
   lettres, pas quatre. `"brave-moon"` fait 10, le test attendait 9. Le mot le
   plus court est `calm` (4), avec `kind`, `neon` et `tidy`.
2. `ADJECTIVES.iter().min_by_key(|m| m.len())` valait `"brave"` et
   `max_by_key` valait `"playful"`. En realite `min_by_key` rend le **premier**
   minimum, donc `"calm"`, et `max_by_key` rend le **dernier** maximum, donc
   `"stellar"`. Les deux assertions etaient fausses, et pour une raison qui
   n'avait rien a voir avec le code : le deregement d'egalite.

J'ai remplace le test fragile par `mots_de_longueur()`, qui verifie les listes
entieres de mots de longueur 4 et 7, plus une garde `4..=7` et `4..=8` qui
empeche un mot hors bornes de passer inapercu. Verification faite hors
compilateur, en comptant les lettres des 60 mots directement depuis le
TypeScript.

J'ai aussi supprime une assertion tautologique
(`slug.is_ascii_lowercase() || slug.contains('-')`, toujours vraie) et
l'ai remplacee par un controle reel du jeu de caracteres autorises.

## Le piege des noms de champs est sans objet ici

La mission insiste sur `projectID` contre `projectId`. Ce fichier ne declare
**aucun** struct, **aucun** enum, **aucun** champ serialise : la sortie est une
`String`. Il n'y a donc aucun `#[serde(rename)]` a avoir, ni a manquer. Je le
signale explicitement pour que la relecture ne cherche pas un piege inexistant.

## L'ecart assume : la source du hasard

`Math.floor(Math.random() * n)` est **uniforme** sur `[0, n)`. Le modulo naif
`u64 % n` ne l'est pas quand `n` ne divise pas 2 puissance 64, ce qui est le cas
pour 29 et pour 31. Un tirage par rejet est donc utilise, avec le seuil
`2^64 mod n`, calcule en arithmetique qui deborde. Le modulo est ainsi exact.

En revanche `Cargo.toml` ne declare ni `rand` ni `getrandom`, et la regle 1
m'interdit de le modifier. Le germe vient donc de SplitMix64, amorce avec les
cles de `RandomState` de la bibliotheque standard, qui elle-meme vient de l'OS.
Meme choix que `util_identifier.rs` et `pty_schema.rs`, pour que le lot soit
homogene. **La loi de distribution est la meme, la suite de valeurs ne l'est
pas.** Pour un appelant qui ne demande qu'un nom, la divergence est invisible.

Deux proprietes que les tests verrouillent, parce que ce sont les seules
divergences de comportement possibles :

- les deux listes n'ont **pas la meme taille** (29 adjectifs, 31 noms) ; tirer
  l'index du nom avec la longueur de la liste des adjectifs rendrait `wizard`
  et `wolf` inatteignables ;
- les tables sont **disjointes**, donc l'erreur "un seul index tire pour les
  deux moities" se voit : le premier mot serait un nom.

## Integration et regles

- `pub mod util_slug;` est **deja** present a `src/swarm/mod.rs:76`. Je n'y ai
  pas touche.
- Aucune dependance ajoutee : le fichier n'utilise que `std`. Si un jour une
  vraie crate de hasard est ajoutee a `Cargo.toml`, seules `amorcer()` et
  `suivant()` sont a remplacer ; l'appel public `creer()` ne change pas.
- Aucune compilation, aucun `cargo`, aucun `rustc`, aucun `rustup` : conformement
  a la regle 2. La relecture du code se fait a l'oeil, c'est exactement pour
  cela que les deux bugs de tests ci-dessus ont pu survivre a l'ecriture
  initiale.
- 0 octet non ASCII dans le fichier. Aucun commit, aucun push.
