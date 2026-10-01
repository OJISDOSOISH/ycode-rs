# swarm-provider_vercel

===DEBUT===
fichier : src/swarm/provider_vercel.rs
source  : packages/core/src/plugin/provider/vercel.ts
taille  : 28748 octets
tests   : 23

CONFIANCE : moyenne
POINT FAIBLE : j'ai du decider quoi faire de l'import dynamique
  `import("@ai-sdk/vercel")` + `mod.createVercel(evt.options)`, qui n'a pas
  d'equivalent Rust. J'ai porte le crochet jusqu'a sa decision et fait rendre
  `VercelSdkAction::Create { factory: "createVercel", options }` a l'appelant, au
  lieu de fabriquer un objet `sdk`. Si la revue attend autre chose (un trait
  injectable, une fabrique Rust, ou rien du tout), c'est la partie a retraiter.
  Second doute, plus technique : j'ai recree localement les formes
  `ProviderApi` / `ProviderRequest` / une partie de `ProviderV2Info` sous les
  noms `Vercel*`, en reduisant `VercelProvider` a `id` + `api` + `request`.
  Un autre module du swarm porte peut-etre les formes completes ; ces recopies
  devront disparaitre au profit d'un `use` le jour ou elles existent. Un portage
  qui serialize un `VercelCatalogDraft` vers le TS perdrait `name`, `disabled`
  et `integrationID`.
A VERIFIER : par priorite,
  1. `VercelCatalogDraft::update` renvoie `false` sur un identifiant absent, alors
     que le vrai `Catalog.Draft.update` CREE le fournisseur (`ProviderV2.Info.empty`).
     Verifier que c'est bien inobservable : le plugin n'appelle `update` que sur
     des ids venus de `list()`, donc oui a priori, mais c'est la seule
     divergence de comportement volontaire du fichier.
  2. Noms de champs : aucun nom n'est camelCase ici, le seul nom subtil etant le
     discriminant `type` (mot cle Rust), porte par `#[serde(tag = "type")]` sur
     `VercelApi`. Verifier `{"type":"aisdk","package":"..."}` et
     `{"type":"native","settings":{}}` en sortie, et que la variante native
     n'ecrit NI `package` NI `url`.
  3. `headers` est un `BTreeMap` : le TS ecrit `http-referer` puis `x-title`, et
     le JSON produit sera trie par cle. Sans consequence ici (ce sont deux cles
     differentes et jamais comparees), mais le noter.
  4. `provider.request.headers[key] = value` est une affectation inconditionnelle :
     une valeur deja presente sur `http-referer` ou `x-title` est donc ECRASEE.
     J'ai garde ce comportement et un test le verrouille. Verifier qu'on veut
     bien l'ecrasement et non un respect de la valeur utilisateur.
  5. Les en-tetes sont en minuscules (`http-referer`, `x-title`). Le test TS
     verifie explicitement que `HTTP-Referer` et `X-Title` ne sont PAS ecrits ;
     j'ai porte ce test tel quel.
  6. `VercelSdkEvent` omet `model` (non lu par ce plugin) et `sdk?: any`
     (non representable). `VercelSdkAction` ne derive pas de serde : c'est une
     decision locale, sans version JSON.
  7. Non porte, volontairement : l'enregistrement des crochets
     (`ctx.catalog.transform`, `ctx.aisdk.sdk`) et les `Effect.fn`, qui
     appartiennent a l'hote de plugins.
NOTE : `pub mod provider_vercel;` est deja present dans src/swarm/mod.rs
  (ecrit par l'agent principal), rien a signaler ni a modifier de ce cote.
  `coordination/controle.ps1` passe sur ce fichier : aucun caractere accentue,
  des tests presents, aucune collision de type (tous mes types sont prefixes
  `Vercel`).
===FIN===