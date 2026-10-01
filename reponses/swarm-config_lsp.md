# swarm-config_lsp

===DEBUT===
fichier : src/swarm/config_lsp.rs
source  : packages/core/src/config/lsp.ts
taille  : 20050 octets (491 lignes)
tests   : 15

CONFIANCE : haute
POINT FAIBLE : `{"disabled": true, "command": ["x"]}` est lu comme `Disabled`
  et perd `command`, parce que `Schema.Union([Disabled, Server])` essaie
  `Disabled` en premier. J'ai conserve cet ordre, mais je n'ai pas pu verifier
  le comportement du parseur d'Effect sur les proprietes en trop (le paquet
  `effect` n'est pas installe sur ce poste). Si Effect refuse les cles en trop
  ou ne fait pas du premier coup gagne, mon portage diverge sur ce cas precis.
  Le deuxieme point, plus surement un defaut : j'ai traduit
  `Schema.Literal(true)` par un type-marqueur `LiteralTrue` avec un
  `Deserialize` ecrit a la main, c'est le seul endroit du fichier ou j'ai du
  ecrire de la mecanique serde a la main et je n'ai jamais pu le compiler.
A VERIFIER : (1) ces deux `impl Serialize` / `impl Deserialize<'de>` ecrits a
  la main, c'est le code a lire en premier ; (2) que `#[serde(untagged)]` est
  bien le bon choix sur `Entry` et sur `Info`, l'union du TS n'a aucun champ
  discriminant, donc `false`, `{"disabled": true}` et
  `{"command": [...]}` sont trois formes nues, et l'ordre des variantes doit
  rester celui du TS ; (3) les cinq noms de champs de `Server`
  (`command`, `extensions`, `disabled`, `env`, `initialization`) : ils sont
  des mots uniques en minuscules, donc aucun camelCase du genre `projectID`,
  et le test `les_noms_de_champs_json_sont_ceux_du_typescript` compare la
  liste exacte des cles ; (4) que `Some(false)` reste present en JSON.
===FIN===

## Notes pour le lot

- La source fait 18 lignes et **zero** code executable : un reexport
  d'espace de noms vers lui-meme, un `Schema.Struct` avec un
  `Schema.Literal(true)`, une `Schema.Class` de cinq champs, et deux
  `Schema.Union` sans tag. Rien d'autre a porter.
- Piege 1 (`?` contre `??`) : **absent de la source**, il n'y a ni ternaire
  ni coalescent. Le piege reapparait sous une autre forme et c'est ce que je
  teste : `false` est *falsy* en JavaScript, donc `disabled: Some(false)` doit
  rester dans le JSON (un portage du style `if (b) { Some(b) } else { None }`
  le ferait disparaitre), et une chaine vide dans `command`, `extensions` ou
  une valeur de `env` doit survivre aussi. Deux tests verrouillent ces deux
  cas.
- Piege 2 (noms de champs) : **aucun risque ici**, les cinq champs sont des
  mots uniques en minuscules. J'ai quand meme pose un `#[serde(rename)]`
  explicite sur chacun, comme verrou anti-renommage, coherent avec
  swarm-config_formatter. Aucun champ ne s'appelle `type`, donc pas de `r#type`.
- `Schema.Unknown` devient `serde_json::Value`, donc
  `initialization: Option<BTreeMap<String, serde_json::Value>>`. C'est la
  convention deja en place dans `src/core/provider.rs` et
  `src/core/credential.rs`.
- `Schema.optional` devient `Option<T>` + `skip_serializing_if`. Consequence
  heritee et assumee : Serde lit `null` comme `None`, la ou `Schema.optional`
  refuse `null`. Meme divergence que swarm-config_formatter, meme arbitrage.
- **INTEGRATION A FAIRE PAR L'AGENT PRINCIPAL** : `pub mod config_lsp;` est
  **absent** de `src/swarm/mod.rs` (le fichier s'arrete a `pub mod
  config_watcher;` ligne 16, et il n'y a pas de `config_lsp`). Je ne l'ai pas
  ajoute, regle 1. Sans cette ligne mon fichier n'est pas compile du tout.
- J'ai ajoute deux constructeurs (`Disabled::new`, `Server::new`) et une
  constante `SCHEMA_IDENTIFIER` qui reprend l'identifiant de schema du TS
  (`ConfigV2.LSP.Server`). Ce ne sont pas des traductions de code existant,
  ce sont des commodites de test et de relecture ; `Default` reste derive
  partout. A supprimer si la relecture prefere une stricte parite.
- `effect` n'est pas installe sur ce poste (`node_modules\effect` absent, deja
  signale par swarm-effect_memo_map). Tout ce que j'affirme sur le
  comportement du parseur d'Effect est donc deduit des usages, jamais lu.
- Le schema est consomme par `packages/core/src/config.ts:75` sous le nom
  `lsp`, en `Schema.optional`. Ce fichier n'est pas le mien, je n'y ai pas
  touche.
- Il existe un fichier jumeau `packages/core/src/v1/config/lsp.ts`, qui contient
  une longue liste de serveurs connus (`ruby-lsp`, `sourcekit-lsp`,
  `ocaml-lsp`...). Il n'est dans le lot de personne pour l'instant. Il n'est
  volontairement pas melange au mien : le v2 et le v1 n'ont pas la meme forme.
- Aucun commit, aucun push. J'ai modifie uniquement
  `src/swarm/config_lsp.rs`. Aucune compilation : ni cargo, ni cargo check, ni
  rustc, ni rustup.
