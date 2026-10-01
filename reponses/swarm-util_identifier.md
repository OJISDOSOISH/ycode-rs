# swarm-util_identifier

===DEBUT===
fichier : src/swarm/util_identifier.rs
source  : packages/core/src/util/identifier.ts
taille  : 11099 octets
tests   : 6

CONFIANCE : moyenne
POINT FAIBLE : ma source ne fait qu'une seule ligne de reexport ; j'ai donc porte le fichier reexporte (`packages/schema/src/identifier.ts`, verifie libre de tout autre claim) et j'ai remplace la source cryptographique `crypto.getRandomValues` par un SplitMix64 amorce depuis `RandomState`, car `Cargo.toml` ne declare ni `getrandom` ni `rand` et que je n'ai pas le droit de le modifier.
A VERIFIER : par ordre. (1) Les 6 octets de l'heure : j'utilise `i128` signe et je masque `& 0xff` sur l'entier signe, pas sur un octet, pour reproduire la semantique de `BigInt` sur `~current` negatif ; c'est couvert par le test qui attend exactement `ffffffffeffe` pour timestamp 1 et compteur 1, mais c'est le point le plus subtil du fichier. (2) Le compteur global est porte par un `Mutex<State>` alors que le TypeScript compte sur le mono-thread : le reset sur changement de timestamp et l'incrementation sont fidelement reproduits, mais la concurrence change. (3) Le modulo 62 sur l'octet, volontairement biaise comme l'original. (4) Le parametre s'appelle `descending_order` et non `descending`, parce que le module expose deja une fonction `descending()` ; verifier que les appelants n'attendent pas le nom d'origine.
AUCUN AUTRE FICHIER : `mod.rs` declare deja `pub mod util_identifier;` (ligne 27), rien a signaler de ce cote. Pas de struct ni d'enum serialise, donc aucun `serde(rename)` en jeu. 0 octet non-ASCII verifie.
===FIN===
