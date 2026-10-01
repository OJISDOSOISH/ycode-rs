# swarm-util_iife

===DEBUT===
fichier : src/swarm/util_iife.rs
source  : packages/core/src/util/iife.ts
taille  : 3435 octets
tests   : 7

CONFIANCE : haute
POINT FAIBLE : le test asynchrone (dernier test) est le seul endroit ou j'ai
ecrit du code qui n'est pas dans `iife.ts` : un mini-waker `std::task::Wake` pour
`Box::pin(...).as_mut().poll(...)`, ecrit sans compilateur local. Si la CI
refuse, c'est ce bloc la qu'il faut supprimer, pas le reste du fichier.
AUTRE POINT A VERIFIER : `pub mod util_iife;` est deja present dans
`src/swarm/mod.rs` ligne 26, donc rien a declarer (je n'y ai pas touche).
A VERIFIER : rien d'autre. La source fait 3 lignes, le portage tient en une
fonction de 3 lignes. Aucun struct, aucun nom de champ, donc aucun
`serde(rename = ...)` a verifier ici. `mod.rs` contient aussi un
`mod.rs.tmp` de 491 octets que je n'ai pas touche et qui n'est pas de moi.
===FIN===
