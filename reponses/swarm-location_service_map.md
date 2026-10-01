# swarm-location_service_map

===DEBUT===
fichier : src/swarm/location_service_map.rs
source  : packages/core/src/location-service-map.ts
taille  : 22602 octets
tests   : 17

CONFIANCE : haute
POINT FAIBLE : le `LocationRef` importe ne derive que Debug/Clone/PartialEq/
Serialize/Deserialize, pas Ord/Hash ni Eq. J ai donc du abandonner la BTreeMap
et la recherche est lineaire dans les tests. Si un jour le schema gagne Ord, la
fautre de table triee n existe plus, mais aucun test ne le signale.
A VERIFIER :
  1. que `crate::core::session::schema::LocationRef` porte bien
     `#[serde(rename = "workspaceID")]` au jour de la relecture. Mes tests le
     verifient, mais ils ne tourneront qu sur GitHub Actions.
  2. que le nom de cle `"@opencode/example/LocationServiceMap"` est bien la
     source. Le segment `example` est suspect : il est dans `core`, pas dans un
     paquet `example`. Lu tel quel dans location-service-map.ts:10.
  3. que j ai bien retire `invalidate` du trait pour la bonne raison. Je
     l ai retire parce qu il n est pas appele par la source et qu il imposait
     une mutation derriere `&self` ; si la relecture lit `invalidate` comme une
     partie legitime du contrat `LayerMap`, c est un choix a arbitrer avec
     l agent de `location-services.ts`, qui construit la carte.

===FIN===

## Ce que j ai change par rapport a la version de la vague 3

Le fichier existait deja et ses limites etaient bien documentees. Je n ai pas
recommence : j ai corrige trois choses.

**1. `LocationRef` ne declare plus une copie, il importe le contrat.**
C etait le vrai defaut. La version precedente declarait son propre
`struct LocationRef` avec ses propres constructeurs `new` et `with_workspace`.
`src/core/session/schema.rs:176` declare deja le meme type, avec le bon
`#[serde(rename = "workspaceID")]`. Deux definitions d un meme contrat ne
donnent aucune erreur de compilation : elles divergent en silence, ce qui est
exactement ce que la relecture a deja attrape deux fois ailleurs. Le fichier
 suit maintenant le precedent pose par `src/swarm/location.rs:143` :
 `pub use crate::core::session::schema::LocationRef;` plus un reexport `Ref`
 pour coller au nom de la source.

**2. `invalidate` retire du trait.** La source ne l appelle pas. Il venait de
`plugin/layer-map.example.ts:94`, ou c est une utilisation, pas une definition.
Le garder imposait une mutation derriere `&self`, donc un `Rc<RefCell<...>>`
pour y repondre. C etait exactement la mecanique de concurrence que la consigne
interdit. Le trait contient maintenant uniquement `get`, ce que la source
utilise.

**3. Un bug qui aurait casse la compilation.** La signature etait
`fn get(&self, key: &LocationRef) -> Result<S::Services, Self::Error>;`
`S::` a l interieur d une declaration de trait : `S` n existe pas la. C est
maintenant `Self::`.

## Ce que j ai garde tel quel, et pourquoi

La decision de **ne pas simuler le graphe de couches Effect** est la bonne et
je l ai conservee telle quelle. `Context.Service`, `Layer`, `Effect.gen`,
`Layer.unwrap`, `LayerNode` : rien de tout cela n existe en Rust standard, et un
`Mutex` pose autour d une `BTreeMap` aurait l air d une traduction alors que ce
serait une invention. Ce qui est portable l est : le contrat du service (un
`trait` avec deux types associes `Services` et `Error`), la cle de la table
(le `LocationRef` importe), et la forme de la valeur exportee `node`
(`NodeKind::Unbound`, nom = cle de service, dependances vides, tag `global`).

Les quatre morceaux sans traduction sont documentes dans l en-tete du fichier,
avec les lignes exactes de la bibliotheque `effect` qui les bornent. J y ai
ajoute deux precisions de la version precedente :

- l effet observable de `unbound` : `LayerNode.compile`
  (`effect/layer-node.ts:260`) leve `Unbound layer node: <nom>` si le graphe en
  contient un encore ;
- qui produit reellement la carte : `buildLocationServiceMap`
  (`location-services.ts:84-112`), et non ce fichier, qui ne fait que la
  consommer.

## Ce qui n est pas verifie

Je n ai compile rien, conformement a la regle. Concretement, ce qui reste non
verifie : que `espace.map(str::to_string)` type correctement sur
`Option<&str>`, et que la closure de `avec_entrees` avec
`(*cle).clone()` sur un motif `&(&LocationRef, &str)` benefit bien des modes de
liaison par defaut de Rust 2021. Les deux sont des idiomes standards, mais
c est de la relecture, pas de la compilation.

Aucun autre fichier touche. `src/swarm/mod.rs:40` declare deja
`pub mod location_service_map;`, donc rien a signaler de ce cote.