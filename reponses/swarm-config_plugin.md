# swarm-config_plugin

===DEBUT===
fichier : src/swarm/config_plugin.rs
source  : packages/core/src/config/plugin.ts
taille  : 12752 octets
tests   : 12

CONFIANCE : haute
POINT FAIBLE : `Options` est un `BTreeMap<String, Value>` alors que le TS utilise
  un objet JavaScript dont l'ordre d'insertion des cles est garanti. J'ai suivi la
  conversion du tableau de CONTEXTE-SWARM (Map -> BTreeMap deterministe), mais
  si une config reelle ecrit des options dont l'ordre compte, le JSON produit
  par Rust sera trie par cle, different de celui du TS.
A VERIFIER : par priorite,
  1. `#[serde(untagged)]` sur `Plugin` : c'est la seule partie du fichier ou j'ai
     du choisir, la source ne dit rien de l'ordre de tentative des variantes.
     verifier que `Name` puis `Entry` donne bien le meme resultat que
     `Schema.Union([Schema.String, Entry])` pour `42`, `null`, `{}`,
     `{"autre":1}` (doit echouer) et pour un objet valide (doit donner Entry).
  2. `options` a `#[serde(default, skip_serializing_if = "Option::is_none")]` :
     verifier qu'une entree sans options serialise en `{"package":"..."}` sans
     cle `options`, et non `options: null`.
  3. Noms de champs : `package` et `options` sont identiques en TS et en Rust,
     donc aucun `rename`. Le piege `sessionID` / `callID` ne s'applique pas ici,
     mais le test `les_noms_de_champs_json_sont_ceux_du_typescript` verifie quand
     meme l'absence de variante en majuscules.
  4. `Plugin` a deux accesseurs non demandes par la source, `package_name()` et
     `options()`. Ce sont des Sucrees de lecture, pas de la logique : les
     supprimer ne change rien au comportement. A supprimer si la revue prefere
     un portage strictement minimal.
  5. Non portee, volontairement : l'identifiant de schema
     `"ConfigV2.Plugin.Entry"` et l'export `export * as ConfigPlugin`, qui n'a
     pas d'equivalent hors de Rust.
NOTE : `pub mod config_plugin;` est deja present dans src/swarm/mod.rs (ecrit
  par l'agent principal), rien a signaler ni a modifier de ce cote.
===FIN===
