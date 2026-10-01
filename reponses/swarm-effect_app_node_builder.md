# swarm-effect_app_node_builder

===DEBUT===
fichier : src/swarm/effect_app_node_builder.rs
source  : packages/core/src/effect/app-node-builder.ts
taille  : 36764 octets
tests   : 19

CONFIANCE : haute

POINT FAIBLE : `LayerNode.hasUnbound` compare des **identites d objet** en
TypeScript (`node === source`) et je compare un **nom plus un genre**. C est la
seule divergence de comportement du fichier. Concretement, deux noeuds non lies
distincts de meme cle restent indiscernables pour moi alors que le TypeScript les
distingue. Un noeud de couche ou un groupe qui porte le nom du service ne
compte plus chez moi, ce qui est un gain de fidelite par rapport a la version
precedente. A verifier en priorite : que l approximation du genre ne casse
aucun des appelants reels. `crate::swarm::location` n utilise pas cette
fonction, donc l impact est nul aujourd hui.

A VERIFIER :
1. **La signature de `build` a change.** Elle prend maintenant 4 parametres et
   renvoie un `LayerRef` au lieu de renvoyer un `BuildPlan`. Raison : la source
   fait `return LayerNode.compile(root, allReplacements)`, donc `build` doit
   rendre le resultat de la compilation, pas la liste a compiler. La
   compilation est donc injectee comme la construction de la carte. Aucun
   appelant dans le depot : j ai verifie, `location.rs` n importe que
   `AppNode`, `LayerRef` et `NodeTag`, dont les signatures sont inchangees. La
   decision seule reste testable via `plan_replacements`.
2. **Doublon de cle de service.** `crate::swarm::location_service_map::SERVICE_KEY`
   et mon `LOCATION_SERVICE_MAP_SERVICE_KEY` portent la meme chaine
   `"@opencode/example/LocationServiceMap"`. J ai garde ma constante locale
   pour ne pas coupler deux fichiers du lot, donc la deduplication est pour
   l agent principal. J ai verifie caractere par caractere que la mienne est
   exacte (les deux "@" et le point).
3. **Rien n a ete compile.** Aucune toolchain, conformement a la regle. J ai
   verifie a la main : equilibre des accolades, 19 `#[test]`, 0 caractere non
   ASCII, et l API publique Importee par `location.rs` est intacte.

CE QUI A CHANGE PAR RAPPORT A LA VERSION DE LA VAGUE 3 :
- l en-tete affirmait que le graphe `LayerNode` n existait pas en Rust. C etait
  faux, le fichier le portait deja (`AppNode`, `NodeTag`, `LayerRef`). L excuse
  est supprimee, l en-tete dit desormais ce qui est reellement porte ;
- `has_unbound_named` verifie desormais le genre `unbound` en plus du nom ;
- `build` rend le resultat de la compilation au lieu de s arreter au plan ;
- 4 nouveaux tests : genre du noeud, double demande, liste d origine intacte,
  et les deux tests de bout en bout sur `build`.

PORTEE : 23 lignes de source, dont 6 de code effectif. Porte : `NodeKind`,
`NodeTag`, `AppNode` (avec `group`, `unbound`, `make`), `LayerRef`,
`Replacement`/`ReplacementValue`, `has_replacement`, la cle du service, les deux
noeuds de la carte, la decision et l ajout en fin de liste. Non porte et
signale : la valeur `Layer` d Effect (elle n a pas d equivalent, elle est donc
injectee par l appelant sous forme d un nom), le corps de `LayerNode.compile`,
`LayerNode.hoist`, `walk` et `replacementNode` qui sont d un autre fichier
TypeScript, et la table de balises qui n est qu une verification de type jamais
executee. Aucun `Mutex`, aucun `Arc`, aucune concurrence fabricatee : la source
n en contient aucune.

===FIN===
