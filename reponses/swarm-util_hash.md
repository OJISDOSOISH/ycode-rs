# swarm-util_hash

===DEBUT===
fichier : src/swarm/util_hash.rs
source  : packages/core/src/util/hash.ts
taille  : 11461 octets
tests   : 9

API publique portee : `fast(input) -> String` et `sha256(input) -> String`,
qui correspondent a `Hash.fast` et `Hash.sha256`. Le namespace TS `Hash`
devient le module Rust lui-meme, donc pas de module imbrique.

POINT CHOISI : la source est minuscules (11 lignes). Le vrai sujet du
portage n'est pas la traduction, c'est la dependance manquante. `Cargo.toml`
ne declare aucun crate de hachage (`sha1`, `sha2`, `hex`, `blake3` : aucun),
et un sous-agent n'a pas le droit d'y toucher. Utiliser un crate non declare
casserait la compilation de TOUT le lot a la CI. J'ai donc implemente SHA-1
et SHA-256 en pur Rust dans le fichier, sans aucune dependance, et
l'hexadecimal est produit par une table. Si le lot echoue a la CI sur un
`unresolved import`, c'est ici qu'il faut regarder en premier.

Detail de fidelite : `Hash.fast` est du **SHA-1**, malgre son nom. Le nom est
garde tel quel : `flock.ts:321` s'en sert pour composer un nom de fichier
`.lock` deja present sur le disque, donc le renommer casserait la
compatibilite. `digest("hex")` rend de l'hexadecimal **minuscule**,
reproduit. `string | Buffer` devient `input: impl AsRef<[u8]>`, ce qui
accepte `&str` / `String` (UTF-8, comme Node) et `&[u8]`.

CONFIANCE : haute
POINT FAIBLE : je n'ai pas de compilateur Rust ici, donc le code n'a JAMAIS
ete compile ni teste. J'ai verifie la logique algorithmique en transliterant
le fichier a l'identique en JavaScript (semantique u32 : `Math.imul`-free,
additions `>>> 0`, memes constantes, meme remplissage) puis compare a
`crypto` de Node sur 40 cas : longueur 0, 1, 2, 3, 54, 55, 56, 57, 63, 64,
65, 119, 120, 127, 128, 129, 1000, 4096 octets, deux motifs de remplissage
differents, plus "opencode", le message de 56 octets et un octet nul.
40/40 conformes, 0 echec. Ce qui reste non verifie, c'est **uniquement la
syntaxe et le typage Rust** : le plus probable en cas d'echec CI est une
erreur de borrow ou de coercion que je n'ai pas pu attraper a l'oeil.

A VERIFIER (par priorite) :
1. La compilation. C'est le risque numero un, tout le reste est secondaire.
2. `mod.rs` : `pub mod util_hash;` est **deja present** (ligne 25). Je n'y ai
   rien touche, mais il faut confirmer que la declaration n'a pas ete
   supprimee entre-temps.
3. Les 6 constantes `K` de SHA-256 et les 5 etats initiaux de SHA-1 :
   verifies par le test vectoriel, mais relire les 64 valeurs ligne a ligne
   reste le controle le plus rentable si un seul test echoue en CI.
4. Aucun nom de champ concerne : ce fichier ne serialize rien et ne
   deserialize rien. Le piege `camelCase` est sans objet ici.
5. Le `Cargo.toml` : si le responsable ajoute `sha1`/`sha2`, seules
   `sha1_digest` et `sha256_digest` sont a supprimer. L'API publique ne
   bouge pas.
===FIN===
