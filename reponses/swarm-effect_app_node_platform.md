# swarm-effect_app_node_platform

===DEBUT===
fichier : src/swarm/effect_app_node_platform.rs
source  : packages/core/src/effect/app-node-platform.ts
taille  : 24332 octets
tests   : 24

CONFIANCE : moyenne
POINT FAIBLE : les trois cles de service "FileSystem", "Path" et "HttpClient" sont
  deduites, pas verifiees : le paquet `effect` n est pas installee dans le depot
  (`node_modules/effect` absent), donc je n ai pas pu lire la declaration des
  tags. Elles viennent de la regle usuelle de `effect`, ou une cle non
  explicitement fournie prend le nom du symbole declare. Les deux cles du
  paquet `llm` sont verifiees dans la source et sont exactes.
A VERIFIER : par priorite, (1) les trois cles ci-dessus, (2) que `build_order`
  reste un ordre declaratif et n est pas pris pour une compilation de couches,
  (3) la contrainte d etiquette `global` de `app-node.ts`, notee en commentaire
  mais non codee, car c est un controle de type TypeScript.

CE QUE J AI FAIT
Ce fichier etait deja sur le disque, porte par un agent de la vague 3. Je ne
l ai pas reecrit, je l ai relu contre les sources et corrige. Corrections :

1. BUG REEL, un test ne pouvait pas passer. `build_order_from` choisissait le
   prochain service dans l ordre de l entree recue, alors que sa documentation
   et le test `l_ordre_ne_depend_pas_de_l_ordre_de_l_entree` promettaient un tri
   par ordre de declaration. Concretement, sur la table de la source,
   l ordre direct valait FileSystem, Path, HttpClient, RequestExecutor, LlmClient
   et l ordre inverse valait HttpClient, RequestExecutor, LlmClient, Path,
   FileSystem : le test comparait deux listes differentes et aurait echoue.
   Les candidats sont maintenant parcourus dans l ordre de declaration, donc le
   resultat ne depend plus de l ordre de l entree. Test verifie a la main sur
   les deux entrees.
2. Test ajoute, `l_ordre_de_construction_suit_l_ordre_de_declaration_pour_les_etapes_libres`,
   qui fige l ordre exact plutot que de seulement comparer des positions.
3. Deux tests renommes, faute de frappe dans le francais : `ddepend` -> `depend`.
4. Suppression de `of_service`, simple alias de `PlatformNode::of`, sans
   aucun appelant dans le depot. Rien d autre n a ete retire.
5. Documentation : le `memoMap` est attribue a tort a `LayerNode.compile`, il
   est en realite fourni a la construction du runtime dans
   `effect/runtime.ts`. Correction faite. Ajout d un paragraphe sur la
   contrainte d etiquette `global` (`tags = { location: ["global"], global: [] }`
   dans `app-node.ts`).

CE QUI EST PORTE ET CE QUI NE L EST PAS
La source fait dix-huit lignes et ne contient aucun calcul : cinq appels a
`makeGlobalNode`, plus un auto reexport. C est une table de cablage, rien
d autre. Est porte : les cinq services dans l ordre de la source, le nom de la
couche qui realise chacun, les dependances declarees, l etiquette `global`, et
les operations structurelles que le code appelant peut faire sur ces noms
(recherche par cle, dependants, racines, feuilles, ordre topologique
declaratif). Ne l est pas, et c est dit dans l entete du fichier : `Layer`, les
`Context.Tag`, le `Scope`, le `memoMap`, et la compilation du graphe qui vit
dans `layer-node.ts`. Aucun `Mutex`, aucun `Arc`, aucune mecanique de
concurrence : la source n en demande pas, et un tableau statique de cinq
entrees n en a pas besoin. Le graphe de couches n est pas simule.

SIGNALEMENT HORS PERIMETRE
`src/swarm/mod.rs` contient deja `pub mod effect_app_node_platform;`. Je n y ai
touche rien. Aucun autre fichier du depot ne reference les items de ce module,
donc la suppression de `of_service` ne casse aucun appelant.

AUCUN COMPILATEUR UTILISE. Ni cargo, ni rustc, ni rustup. Verification faite
par relecture : 0 octet non ASCII, accolades, parentheses et crochets
equilibres, 24 tests.
===FIN===