# swarm-config_formatter

===DEBUT===
fichier : src/swarm/config_formatter.rs
source  : packages/core/src/config/formatter.ts
taille  : 12846 octets (303 lignes)
tests   : 12

CONFIANCE : haute
POINT FAIBLE : le seul point ou je ne suis pas sur, c'est la `null` sur les
  champs optionnels. Serde lit `{"disabled": null}` comme `None`, alors que
  `Schema.optional` du TS n'accepte que `undefined` et refuserait ce `null`.
  Je l'ai assume et documente dans le fichier, parce qu'aucun JSON ecrit par le
  TS ne contient `null` ici, mais si la relecture veut une parite stricte, il
  faut un deserializeur custom.
A VERIFIER : (1) que `#[serde(untagged)]` sur `Info` est bien le bon choix et
  pas `#[serde(tag = ...)]` -- l'union du TS n'a aucun champ discriminant, donc
  `false` et `{ "prettier": {...} }` sont deux formes nues, et l'ordre
  `Toggle` d'abord doit rester. (2) les quatre `#[serde(rename = "...")]` sont
  redondants (les noms du TS sont deja des mots bascules) : je les ai poses
  comme verrou anti-renommage, mais un relecteur peut vouloir les supprimer.
  (3) que `command` reste une liste de chaines non decoupee, comme
  `Schema.String.pipe(Schema.Array)`.
===FIN===

## Notes pour le lot

- Le schema est consomme par `packages/core/src/config.ts:72` sous le nom
  `formatter`, en `Schema.optional`. Le champ du parent n'est pas mon fichier.
- Il existe un fichier jumeau, `packages/core/src/v1/config/formatter.ts`,
  avec la meme forme mais en `Schema.Struct` au lieu de `Schema.Class` et
  avec un `export type Info` en plus. Il n'est pas dans mon lot, je ne l'ai
  pas touche. Si quelqu'un le porte, la structure Rust sera quasi identique.
- Aucun nom de champ en camelCase dans cette source, donc le piege 2 n'a pas
  mordu ici. Le piege 1 (`?` contre `??`) n'apparait pas non plus : le fichier
  ne contient aucun ternaire ni coalescent.
- Le `export * as ConfigFormatter from "./formatter"` de la ligne 1 est un
  reexport du module sur lui-meme : rien a porter, il est absent du fichier.
- `mod.rs` declare deja `pub mod config_formatter;`. Rien a signaler de ce
  cote.
- J'ai modifie uniquement `src/swarm/config_formatter.rs`. Ni commit, ni push.
