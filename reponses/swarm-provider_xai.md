# swarm-provider_xai

===DEBUT===
fichier : src/swarm/provider_xai.rs
source  : packages/core/src/plugin/provider/xai.ts
taille  : 29677 octets
tests   : 18

CONFIANCE : haute
POINT FAIBLE : les structs `Sdk` et `Language` sont une invention de ma part, la
source n'a rien de tel. En TS, `evt.sdk` recoit un objet `any` (le retour de
`mod.createXai`) et `evt.language` un `LanguageModelV3` : deux types opaques que
Rust ne peut pas produire. J'ai enregistre la *description de l'appel* (paquet,
fabrique, arguments) plutot que d'inventer une abstraction. Le voisin
provider_mistral a fait un autre choix pour le meme probleme : une fabrique
injectee `FnOnce(&Value) -> Value`. Si la famille `provider_*.rs` doit avoir une
seule forme, c'est mon fichier qui doit ceder, et c'est le point que la relecture
doit trancher en premier.
A VERIFIER :
1. Les noms de champs JSON, un par un. Un seul exige un `rename` :
   `Model.provider_id` porte `#[serde(rename = "providerID")]`, D majuscule. Les
   autres (`model`, `api`, `id`, `package`, `options`, `sdk`, `language`) sont
   des mots uniques en minuscules, aucun `rename` ne doit manquer. Un test
   refuse `providerId` a la lecture, donc si le `rename` saute, la CI le voit.
   Attention : `package` n'est PAS un mot reserve de Rust, il est legal tel quel.
2. `Sdk` et `Language` doivent porter `Serialize, Deserialize`, sinon
   `SdkEvent` qui contient `Option<Sdk>` ne derive pas. C'est le seul endroit du
   fichier ou une erreur de derive se propagerait en cascade.
3. `Model` est type et volontairement reduit a deux champs, alors que
   provider_google et provider_togetherai modelisent `model` par un
   `serde_json::Value` opaque. Divergence de famille a arbitrer, pas une erreur.
4. Le fichier n'a jamais ete compile : aucun toolchain Rust sur ce poste. Le seul
   endroit non banal est la boucle `for (json, nom) in [(&sdk_json, "sdk"), ...]`
   du test des noms de champs, qui indexe un `&Value` (`json["model"]["api"]`).
===FIN===

REMARQUES DE LECTURE (hors gabarit) :

- La source fait 22 lignes : `define({ id: "xai", effect })` ou l'effet enregistre
  deux crochets sur `ctx.aisdk` (`sdk` puis `language`). Il n'y a **aucune**
  donnee de configuration, **aucune** liste, aucun schema. Le port tient donc en
  un struct `Plugin { id }`, deux evenements, et deux fonctions pures
  `apply_sdk` / `apply_language` qui reproduisent le `return` immediat et
  l'affectation de la source.
- Non portable, et dit comme tel dans le fichier : `import("@ai-sdk/xai")` et les
  appels `createXai(...)` / `.responses(...)` visent des objets JavaScript.
- Piege `?` contre `??` : absent ici. La source compare avec `!==`, donc une
  chaine vide est une chaine vide. Deux tests le disent (`""`, `"XAI"`, `"xai "`,
  `" xai"` ne declenchent rien), parce que la comparaison stricte ne doit pas
  devenir un test de veracite.
- Fait utile pour toute la famille : `AISDK.SDKEvent` a `sdk?` facultatif, et
  `AISDK.LanguageEvent` a `sdk` **obligatoire**. La difference se voit, un test la
  verifie. Et `LanguageEvent` porte un champ `package` que le TS ne declare pas :
  j'ai suivi le TS, donc l'evenement de langage n'a pas de cle `package`.
- Integration : `pub mod provider_xai;` est deja present dans `src/swarm/mod.rs`
  ligne 45. Je n'ai rien modifie hors de mon fichier. `serde` et `serde_json` sont
  deja dans `Cargo.toml`. Aucun commit, aucun push.
- 0 caractere non ASCII dans le fichier, verifie.
