# swarm-location

===DEBUT===
fichier : src/swarm/location.rs
source  : packages/core/src/location.ts
taille  : 41807 octets (924 lignes)
tests   : 25

CONFIANCE : moyenne
POINT FAIBLE : `bound_node` a une signature que la source n'a pas -- elle recoit
la `Layer` et le `noeud` projet en parametres la ou `boundNode(ref)` les derive.
C'est la seule fois ou j'ai change la forme d'une API pour la rendre traduisible,
et je ne suis pas sur que ce soit le bon arbitrage : un relecteur qui attend
`boundNode` sans parametres peut le lire comme une invention.
A VERIFIER : dans cet ordre.
  1. Que `#[derive(Eq)]` avait bien ete retire de `Interface` et de
     `ProjectResolved` pour la bonne raison. `Vcs` (src/swarm/project_schema.rs
     ligne 158) ne derive que `Debug, Clone, PartialEq, Serialize, Deserialize`,
     et `derive(Eq)` exige `Eq` sur le type de chaque champ. C'etaient deux
     erreurs de compilation, pas des differences de comportement.
  2. Que le prefixe `wrk` de `workspaceID` compte. `packages/schema/src/
     workspace-id.ts` fait `Schema.String.check(Schema.isStartsWith("wrk"))`,
     donc l'etat teste par l'ancien `un_workspace_vide_survit_...` n'existe pas
     cote TypeScript. J'ai renomme ce test en `l_encodage_ne_supprime_jamais_une
     _chaine_vide` et j'ai pose `WORKSPACE_ID_PREFIX` pour rendre la divergence
     explicite. A l'inverse, `AbsolutePath` est une marque sans verification :
     `directory: ""` reste un test valable, lui.
  3. Que `serde` ignore bien `workspaceId` a la lecture. Les deux tests
     `une_cle_snake_case_est_ignoree_a_la_lecture_sur_info` et
     `..._sur_interface` verifient l'absence d'erreur, parce que c'est ce que
     fait `serde` -- et parce que la source fait pareil (`Schema.Struct` ignore
     les proprietes en trop). Une faute de casse ne provoque donc pas une erreur,
     elle provoque une **valeur perdue** silencieusement. Je n'ai **pas** ajoute
     `deny_unknown_fields` : ce serait un ecart de fidelite, et sur `Interface`
     c'est de toute facon impossible (incompatible avec `#[serde(flatten)]`).
  4. La surface ajoutee sur `crate::swarm::effect_app_node_builder` : si ce
     module bouge, `location.rs` casse. Il n'est pas modifie par moi.
===FIN===

## Ce que j'ai change, et pourquoi

Le fichier etait deja sur disque et il etait bon sur le contrat. Je ne l'ai pas
reecrit : j'ai corrige deux erreurs qui refuses de compiler, et j'ai remplace
trois affirmations devenues fausses par de la verifie.

### 1. Deux erreurs de compilation retirees

`Interface` et `ProjectResolved` deriving tous deux `Eq`, avec chacun un champ
`vcs: Option<Vcs>`. Or `Vcs` ne derive pas `Eq` (project_schema.rs ligne 158).
`derive(Eq)` genere un `AssertParamIsEq` sur le type de **chaque** champ, donc
`Option<Vcs>: Eq` echoue. Deux refus de compilation, invisibles a la lecture.

`Eq` ne sert a rien ici : la source n'en parle pas et tous les tests comparent
des valeurs, ce qui ne demande que `PartialEq`. Retire des deux.

### 2. `node` et `boundNode` sont enfin portes

L'en-tete affirmait : "Le graphe de couches `LayerNode` / `Layer` n'existe pas
encore en Rust", et laissait les deux exports reduits a des constantes. C'etait
vrai au moment de l'ecriture, c'est faux aujourd'hui :
`crate::swarm::effect_app_node_builder` porte desormais `AppNode`, `NodeTag` et
`LayerRef`, avec exactement les constructeurs necessaires. J'ai donc ajoute
`node()` et `bound_node(...)` sur ces types, sans rien inventer.

Ce qui reste hors de portee, et uniquement cela : `Layer.effect` et la closure
`layer(ref)`. Une `Layer` d Effect est une valeur branchee, sans equivalent. Le
graphe est decrit par `bound_node`, le corps de la couche -- l'unique logique du
fichier -- par `bind`. C'est ce qui rend la signature de `bound_node` differente :
elle recoit la `Layer` (par son nom) et `Project.node`, que `project.ts` -- non
porte -- construit.

### 3. Le piege `?` contre `??` etait analyse, mais sur une mauvaise valeur

L'ancien fichier disait, avec raison, que la source ne contient ni ternaire ni
coalescent et que le seul point de decision est le filtre
`value !== undefined` de `optional` : un test de nullite, donc `""` survit. La
conclusion etait juste, la valeur testee ne l'etait pas.

`WorkspaceID` est `Schema.String.check(Schema.isStartsWith("wrk"))`. Une chaine
vide est **refusee** a la decodage, donc un `workspaceID` vide n'existe pas dans
une source bien typee. Le test ne verifiait pas une regle de la source, il
verifiait un etat non representable. Je l'ai renomme et cadre explicitement, et
j'ai pose `WORKSPACE_ID_PREFIX` pour que la divergence soit ecrite quelque part
plutot que repetee dans des commentaires de test.

`AbsolutePath`, lui, est une marque sans verification : `directory: ""` est une
valeur legitime, et ce test-la reste une regle de la source. La distinction
compte, et elle est maintenant ecrite.

### 4. Le piege de lecture, qui n'etait traite nulle part

Le piege d'ecriture est regle par `#[serde(rename = "workspaceID")]`. Le piege
de **lecture** ne l'est pas : `serde` ignore par defaut une cle inconnue, et ce
fichier n'emploie pas `deny_unknown_fields`. Un payload ou la cle s'appelle
`workspaceId` est donc accepte sans bruit, donne `None`, et perd la valeur a la
re-serialisation. C'est exactement le mode de panne que la mission signale, dans
sa forme inversee : pas une erreur franche, une disparition.

Trois tests ajoutes : la forme snake_case sur `Info`, la meme sur `Interface`, et
le contrepoint (aller-retour sans perte avec la vraie casse). Je n'ai **volontairement
pas** ajoute `deny_unknown_fields` -- la source ne l'a pas, l'ajouter serait
inventer un contrat plus strict, et sur `Interface` c'est impossible puisque
l'attribut est incompatible avec `#[serde(flatten)]`, qui est lui indispensable.

### 5. Divers

- `use crate::core::session::schema::LocationRef` retire de l'import prive : le
  nom n'etait utilise que via l'alias `Ref`, c'etait un import mort.
- Un test qui solidarise `TAG_LOCATION` / `TAG_GLOBAL` avec
  `NodeTag::Location.as_str()` / `NodeTag::Global.as_str()`. Ces chaines
  decrivent le meme contrat que l'enum du module voisin ; le test fait echouer
  l'un des deux si l'autre bouge, ce qui est le seul moyen de ne pas avoir deux
  verites.
- La justification du reexport de `LocationRef` est completee par le type genere
  du SDK (`packages/sdk/js/src/v2/gen/types.gen.ts` ligne 3047) :
  `LocationRef = { directory: string, workspaceID?: string }`. Ce n'est donc pas
  une coincidence de forme, c'est le meme contrat.
- Zero octet non-ASCII, zero ligne au-dela de 100 colonnes. Verifie par lecture
  de tous les octets du fichier.
- Aucun compilateur n'a ete invoque. Fichiers touches : celui-la, et celui-ci.

## Ce que je n'ai pas touche

`mod.rs`, `lib.rs`, `Cargo.toml` : rien. `pub mod location;` etait deja en place
dans `src/swarm/mod.rs` ligne 39, donc aucune integration n'est a faire.
