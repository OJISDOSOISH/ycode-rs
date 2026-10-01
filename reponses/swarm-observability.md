# swarm-observability

===DEBUT===
fichier : src/swarm/observability.rs
source  : packages/core/src/observability.ts
taille  : 30623 octets
tests   : 32

CONFIANCE : moyenne

POINT FAIBLE : la source ne contient AUCUNE donnee, seulement du cablage de
couches Effect, donc j ai du decider quoi porter, et le risque n est pas une
erreur de nom de champ mais un excedent ou un manque. Concretement, mes champs
`loggers`, `options`, `services`, `minimumLogLevel`, `tracingLayer` ne viennent
PAS de l interface `LayerNode.Node` : j ai verifie cette interface, elle ne
declare que `kind`, `name`, `service?`, `implementation?`, `dependencies`,
`tag?`, et j ai donc corrige la documentation qui laissait croire le contraire.
Seuls `name` et `dependencies` sont de vraies donnees portees ; `kind` est la
constante `layer` (exportee a part, `NODE_KIND`), `service` et `tag` valent
`undefined` pour cet appel, `implementation` est le graphe de couches non
simule. Les cinq autres champs decrivent l objet opaque `Layer.Any` de la
source. Si la relecture conclut qu un descripteur de ce genre est une invention,
c est le fichier qu il faut reduire, pas les noms.

A VERIFIER, par priorite :
1. La divergence VOLONNAIRE sur la table des niveaux. `value in levels` remonte
   la chaine de prototypes en JavaScript : `OPENCODE_LOG_LEVEL=toString`
   installerait `Object.prototype.toString`, une FONCTION, comme niveau minimum.
   Mon portage garde une table ferme et rend `Info`. Documente en tete de
   fichier, teste (`un_nom_du_prototype_javascript_donne_info_et_non_une_fonction`).
   A valider comme choix, ou a trancher autrement.
2. Le piege des noms de champs n a presque rien a verifier : la source ne
   declare aucun champ, et son seul vrai nom camelCase est
   `mergeWithExisting`, qui porte un `#[serde(rename)]` explicite et teste. Mes
   autres noms de JSON (`orDie`, `minimumLogLevel`, `tracingLayer`) sont
   choisis d apres la syntaxe de la source (`Layer.orDie`,
   `Logging.minimumLogLevel()`, `Otlp.tracingLayer`) : ils ne sont PAS
   contractuels, il ne faut pas les lire comme une correspondance.
3. `un_noeud_serialise_comme_un_environnement_vide` compare une chaine JSON
   complete, cles ET ordre compris. C est le test le plus fragille du fichier :
   le moindre reordonnancement le casse. Voulu, mais bruyant.
4. L ordre de `SERVICES` est l ordre du `pipe` de la source, donc du plus proche
   au plus exterieur. Non verifie : le paquet `effect` n est pas installe sur ce
   poste, comme swarm-effect_memo_map l a signale. C est une transcription de
   l ecrit, pas une deduction.
5. La table des niveaux est un DOUBLON ATTENDU. Quand `logging.ts` sera porte,
   c est mon `minimum_log_level` qui doit ceder, pas l autre.

AJOUTS DE CETTE PASSE (le fichier existait deja, 28 tests, portage juge bon) :
1. Une section de doc "Les trois nuances qu il faut savoir avant de relire" :
   `Layer.unwrap` / `Effect.gen` / `Effect.promise` non simules (seul reste le
   booleen `tracingLayer`) ; la divergence `in levels` decrite plus bas ; et le
   fait que `otlp.ts` FIGE le point de collecte a l import
   (`const endpoint = Flag.OTEL...` au chargement du module) alors que
   `Logging.loggers()` et `minimumLogLevel()` relisent `process.env` a chaque
   appel, alors que mon `Options::from_env` lit les trois au meme instant.
2. `layer(&Options) -> Node` en plus de `node() -> Node`, pour que les DEUX
   exports de la source existent. Meme calcul, options explicites d un cote,
   environnement du processus de l autre.
3. `NODE_KIND: &str = "layer"`, la valeur que `LayerNode.make` ecrit dans tout
   noeud, plus une doc de `Node` qui dit enfin quels champs viennent vraiment de
   l interface `LayerNode.Node` et lesquels sont ajoutes par ce portage.
4. Quatre tests : nom de prototype (`toString`), point de collecte contenant
   deja `/v1/logs` (verifie que `strip_suffix` est ancre a la fin), les deux
   exports donnent le meme noeud, et le `kind` produit par `make`.

Detail pour la suite : la ligne 1 de la source,
`export * as Observability from "./observability"`, est un reexport de l espace
de noms du module sur LUI-MEME. C est du code mort, comme `config/command.ts` et
`util/wildcard.ts`, et ce n est PAS un import : il n existe pas de fichier
`Observability.ts` voisin a chercher.

Les deux pieges du contexte, traites et testes :
- `?` contre `??` : `Otlp.loggers()` fait `if (!endpoint) return []`, un test de
  VERACITE, donc un `OTEL_EXPORTER_OTLP_ENDPOINT` VIDE desactive l OTLP. Un
  simple `.map()` sur un `Option` l aurait laisse survivre et aurait produit
  l URL "/v1/logs". Test dedie. A l inverse `Logging.loggers()` fait `=== "1"`,
  une egalite stricte : "true", "0" et " 1" n ajoutent rien, test dedie. Et un
  point de collecte de deux espaces reste ACTIF comme en JavaScript, test dedie.
- Noms de champs : sans objet sur la source, traite ci-dessus en point 2.

Le graphe de couches d Effect (`Scope`, ordre de fermeture, `References`) n est
PAS simule, comme dans `effect_memo_map.rs`. Ni `Layer.unwrap`, ni
`Effect.gen`, ni `Effect.promise` ne le sont. Ce qui est porte, c est la
composition vue comme donnee : `Node` + `Options`, plus `layer()` et `node()`
comme deux points d entree vers le meme calcul, parce que la source exporte
`layer` et `node`.

INTEGRATION : `pub mod observability;` est deja present a la ligne 41 de
`src/swarm/mod.rs`. Je n y ai pas touche. `observability_shared.rs` n est ni
modifie, ni duplique : mon fichier n appelle pas `run_id()` et ne redefinit
aucun identifiant d execution.

FINDING POUR LA RELECTURE, hors de mon perimetre :
`observability_shared.rs` (lignes 38-45) affirme que `Cargo.toml` n active pas
la feature `v4` de `uuid`. C est faux : `Cargo.toml` ligne 14 declare
`uuid = { version = "1", features = ["v4", "v7", "serde"] }`. Le commentaire est
perime, le code de ce fichier n est pas affecte (il n utilise que `std`). Je ne
le touche pas, ce n est pas mon fichier.

Aucun commit, aucun push. Aucun compilateur Rust n a ete lance : le fichier n a
ete ni compile, ni type, ni teste. Verification faite a la lecture : 0 octet non
ASCII, 32 `#[test]`, accolades equilibrees.
===FIN===