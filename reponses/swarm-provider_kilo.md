# swarm-provider_kilo

===DEBUT===
fichier : src/swarm/provider_kilo.rs
source  : packages/core/src/plugin/provider/kilo.ts
taille  : 26206 octets
tests   : 22

CONFIANCE : haute
POINT FAIBLE : j'ai declare LOCALEMENT les types `Api` / `ProviderInfo` / `Request`
tires de `packages/schema/src/provider.ts`, alors que le lot ne porte pas ce
fichier-schema. Si un autre agent porte le vrai schema fournisseur, il y aura un
doublon. C'est mon fichier qui doit ceder, pas le sien.
A VERIFIER :
1. `#[serde(tag = "type")]` avec des variantes **newtype** enveloppant un struct
   (`Aisdk(AisdkApi)`). C'est la forme la moins banale du fichier et je n'ai
   aucun moyen de la compiler ici. Si la CI refuse le newtype avec tag interne,
   il faut passer en variantes a champs nommes, ou en `untagged` avec le champ
   `type` reinjecte dans chaque struct (cf. precedent swarm-integration_connection).
2. Le champ `api.type` est bien un `toTaggedUnion("type")` : le tag ne doit pas
   etre ecrit deux fois. Un test compte les cles de `json["api"]` (3), mais il
   faut confirmer a la lecture que `Schema.toTaggedUnion` retire bien `type` des
   structs internes, et ne le garde pas comme champ ordinaire.
3. `integrationID` avec deux majuscules finales, en `#[serde(rename)]` explicite
   (ligne 211). C'est la seule faute de nom de champ du fichier, mais c'est
   exactement la faute que la relecture a deja attrape deux fois sur d'autres
   fichiers.
4. La casse des cles d'entete : `HTTP-Referer` et `X-Title` chez kilo, contre
   `http-referer` et `x-title` chez `vercel.ts` qui fait le meme travail. Un
   test dedie verifie les deux formes.
===FIN===
