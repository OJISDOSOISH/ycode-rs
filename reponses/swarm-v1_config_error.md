# swarm-v1_config_error

===DEBUT===
fichier : src/swarm/v1_config_error.rs
source  : packages/core/src/v1/config/error.ts
taille  : 22836 octets (538 lignes)
tests   : 11

CONFIANCE : moyenne

POINT FAIBLE : j'ai remplace le choix de la vague 3 (`#[serde(tag = "name")]`
+ variantes STRUCT `{ data: X }`) par `#[serde(tag = "name", content = "data")]` +
variantes NEWTYPE. Les deux produisent le meme JSON, mais je n'ai pas compile,
donc je ne peux pas affirmer que le test exact `{"name":...,"data":...}` passe
chez vous. Si la CI casse, c'est ici en premier qu'il faut regarder. Le point
restant non verifie en machine est le `#[serde(flatten)]` de `Issue`, demande
par `Schema.StructWithRest`.

A VERIFIER (par ordre de priorite) :

1. LA FORME JSON, c'est le coeur du fichier. J'ai verifie dans la source TS,
   pas suppose : `util/error.ts:26-29` fait
   `Schema.Struct({ name: Schema.Literal(name), data })` et `toObject()`
   (`error.ts:55-60`) renvoie `{ name, data }`. Donc DEUX NIVEAUX, confirme.
   La deduction qui m'a fait changer de technique : avec `tag` seul (tag
   interne), une variante NEWTYPE est aplatie a cote du tag. C'est confirme
   dans le code source de `serde_derive` lui-meme, `de/enum_internally.rs` :
   `Style::Newtype => deserialize_newtype_variant(...)`. Donc
   `Json(JsonErrorData)` aurait donne `{"name":...,"path":...}`. La vague 3
   avait donc raison sur le symptome, et c'est bien pour ca qu'elle avait pris
   des variantes struct. `tag` + `content` (representation adjacente) donne la
   forme exacte SANS detour. Verifiez que `content = "data"` est bien accepte
   par serde 1.0.229 et que le JSON produit est bien en 2 cles, dans cet
   ordre : `{"name":...,"data":{...}}`.

2. LE FLATTEN. Il n'en reste qu'UN, sur le champ `rest` de `Issue`, et il
   n'est pas dans une variante d'enum : c'est un champ de struct, la forme la
   plus ordinaire. Il est necessaire : sans lui, serde ignore silencieusement
   les cles inconnues a la deserialization, alors que `StructWithRest` les
   conserve. Si vous voulez le supprimer, il faut un `Deserialize` manuel.
   C'est le point que je n'ai pas pu confirmer a l'execution.
   Rappel de la vague 3 : le `flatten` imbrique dans une VARIANTE D'ENUM est
   bien un vrai piege serde (issue #1358, les interactions
   `FlatMapDeserializer` / tag interne). Je l'ai elimine en passant en tag
   adjacent, mais le champ map reste, lui, imbrique dans la variante
   `Invalid` via `data.issues`. C'est la seule situation qui me reste en
   tete.

3. LES TAGS. Ce ne sont PAS les noms des exports TS. Le TS exporte
   `JsonError` mais enregistre `"ConfigJsonError"`. Les cinq chaines ont ete
   recopiees une par une, sans `rename_all`, et un test compare `name()` a
   la valeur du champ `name` du JSON pour les cinq.

4. `toObject()` du TS = `{ name, data }` = exactement
   `serde_json::to_value(erreur)`. Il n'y a rien d'autre a porter de ce
   cote-la. Aucune classe d'erreur Rust n'est creee : ce sont des types, pas
   des objets `Error`. Si vous voulez une interface commune avec le reste du
   lot, c'est un chantier a faire dans un autre fichier.

NOTE : rien n'a ete compile (ni cargo, ni rustc). `pub mod v1_config_error;`
est deja present dans `src/swarm/mod.rs` ligne 79, je n'y ai pas touche. Aucun
autre fichier modifie, aucun commit, aucun push.

===FIN===