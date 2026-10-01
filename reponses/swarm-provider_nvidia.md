# swarm-provider_nvidia

===DEBUT===
fichier : src/swarm/provider_nvidia.rs
source  : packages/core/src/plugin/provider/nvidia.ts
taille  : 20718 octets
tests   : 12

CONFIANCE : haute
POINT FAIBLE : la source n'est PAS un objet de configuration litteral, c'est
un comportement (une boucle sur le catalogue avec trois filtres et trois
ecritures d'en-tetes). J'ai donc du modeler `Provider` / `Api` / `Request`
comme des VUES PARTIELLES : seuls `id`, `api.{type,package,url}` et
`request.headers` sont ports. `name`, `integrationID`, `disabled`, `env`,
`models`, `api.settings` et `request.body` existent dans `ProviderV2.Info`
mais ce plugin ne les touche pas, donc je ne les ai pas mis. Consequence
assumee : un aller-retour JSON sur ces structs perd ces champs, et ce fichier
ne doit pas servir de source de verite du catalogue. Si un autre agent du lot
a porte le `Provider` complet, le mien fait doublon et divergera.
A VERIFIER : par priorite,
  1. `X-BILLING-INVOKE-ORIGIN` est un `??=` (ecriture seulement si la cle est
     absente) et PAS un `=`. J'ai utilise `entry().or_insert_with()`. Le test
     `un_entete_de_facturation_vide_survit_a_la_transformation` verifie le
     cas piege : en JS `""` est falsy mais n'est ni null ni undefined, donc
     `??=` ne l'ecrase pas. Les deux autres en-tetes sont des `=` et DOIVENT
     ecraser. Verifier que je n'ai pas inverse les deux comportements.
  2. La casse exacte des trois noms d'en-tetes : `HTTP-Referer` (pas
     `Http-Referer`), `X-Title`, `X-BILLING-INVOKE-ORIGIN`. Ce sont des
     chaines de code, pas des champs serde : une faute de casse ne casse
     aucune compilation, seulement les appels HTTP.
  3. Le tag de l'union : `#[serde(tag = "type")]` avec `rename = "aisdk"` et
     `rename = "native"`, comme `Schema.toTaggedUnion("type")`. Attention,
     `type` est un mot reserve Rust, il n'y a donc pas de champ `type` dans
     la variante : c'est le tag qui le porte. C'est volontaire.
  4. Les trois filtres sont dans l'ordre du TS et aucun n'est un test de
     veracite. `api.url` absent (`None`) et `api.url` vide (`Some("")`) ne
     doivent MATCHER ni l'un ni l'autre.

AUTRE : `mod.rs` declare deja `pub mod provider_nvidia;` (ligne 39), rien a
faire de ce cote. Le fichier n'utilise que `serde` et `serde_json`, presents
dans `Cargo.toml`. Aucune dependance entre fichiers du swarm.
===FIN===
