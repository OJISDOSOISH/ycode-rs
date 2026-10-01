# swarm-provider_zenmux

===DEBUT===
fichier : src/swarm/provider_zenmux.rs
source  : packages/core/src/plugin/provider/zenmux.ts
taille  : 30546 octets
tests   : 12

CONFIANCE : haute
POINT FAIBLE : j'ai declare localement les formes `Info` / `Api` / `Request` / `IdFournisseur` que le plugin manipule, parce qu'elles viennent de `packages/schema/src/provider.ts` (non reclame par moi, et `oc_provider.claim.json` pointe sur le simple reexport `core/src/provider.ts`). Les 17 autres plugins `provider/*.ts` du meme lot vont redefinir les memes structs : il y aura un doublon, et c'est ma copie qui devrait ceder a l'integration. Second point, plus discret : je n'imite PAS `normalizeApi`, que `catalog.ts:112` appelle dans `provider.update` et qui recopie `request.body.baseURL` dans `api.url` puis supprime la cle du corps. Ce comportement est reellement produit par le plugin TypeScript, il n'est pas dans mon fichier, et il est documente en tete du .rs.
A VERIFIER : (1) `#[serde(rename = "integrationID")]` sur `Info::integration_id` - seul renommage du fichier, et c'est exactement la forme `sessionID`/`callID` que la revue a deja attrape deux fois ; un test verrouille que `integrationId` et `integration_id` ne sortent jamais. (2) Le `#[serde(tag = "type")]` sur l'enum `Api` : j'ai choisi `tag` et non `untagged`, parce que dans la source `type` EST le tag de l'union (`Schema.toTaggedUnion("type")`) et pas un champ des structs, donc pas de double ecriture - contrairement a `integration_connection.rs` ou le tag etait dans le payload. Un test compte les occurrences de `"type"` dans le JSON. (3) `BTreeMap::entry().or_insert()` pour `??=` : une entete presente avec la chaine vide doit SURVIVRE, un test dedie le verifie. (4) La CI : aucun toolchain Rust sur ce poste, rien n'a ete compile.

NOTES POUR L'INTEGRATION :
- `pub mod provider_zenmux;` est deja dans `src/swarm/mod.rs` ligne 46. Je n'y ai pas touche. Aucun commit, aucun push.
- La source n'est PAS un objet de configuration litteral : c'est un comportement, et la consigne de mission ("le portage tient en un struct") ne s'applique donc pas. Le fichier le dit en tete.
- Le vrai piege de la source est `??=`, pas un nom de champ : c'est une affectation coalescente, donc une entete deja presente survit meme vide. Le piege des noms est ici `integrationID`, un champ que le plugin ne touche meme pas.
- Risque de compilation le plus probable de mon fichier : le `&mut [Info]` et les conversions `Vec<Info>` -> `&mut [Info]`, qui passent par deref coercion. Aucun `Vec<&mut Info>` ne subsiste, j'en ai retire un.
===FIN===
